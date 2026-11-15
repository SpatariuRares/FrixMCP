//! Writing a recording: unredacted copies and redacted copies with the layout of the
//! fixtures in `crates/frix-core/tests/fixtures/` (docs/PROJECT.md §6.1):
//!
//! - `scpd/<file>`: descriptions (`tr64desc.xml`, SCPD), plain `GET`;
//! - `responses/<Service>-<instance>/<Action>[@<Arg>=<value>&…].xml`: SOAP responses;
//! - `lists/<file>`: `*ListPath` lists, authenticated `GET` (without the query with the SID).
//!
//! Unredacted copies go to `<dir>/unredacted/`, redacted ones to `<dir>/redacted/`.
//! With the default dir (`tests/fixtures/raw/`, in `.gitignore`) neither of them
//! enters the repo: the redacted copies are reviewed and moved by hand among the fixtures.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use frix_core::tr064::soap_request_arguments;
use frix_core::transport::{Channel, HttpRequest, Method};

use crate::record::Exchange;
use crate::redact::{Error, redact};

/// What was written.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Saved {
    /// Relative paths of the fixtures written (the same in `unredacted/` and `redacted/`).
    pub files: Vec<String>,
    /// Requests not saved, with the reason (non-200 response, unrecognised path).
    pub skipped: Vec<String>,
}

/// Writes the exchanges to `dir/unredacted/` and, redacted all together, to `dir/redacted/`.
/// If redaction fails it writes no redacted copy.
pub fn save(exchanges: &[Exchange], dir: &Path) -> Result<Saved, Error> {
    let mut fixtures: BTreeMap<String, String> = BTreeMap::new();
    let mut saved = Saved::default();
    for exchange in exchanges {
        let Some(relative) = fixture_path(&exchange.request) else {
            saved
                .skipped
                .push(format!("unrecognised path: {}", exchange.request.path));
            continue;
        };
        if exchange.response.status != 200 {
            saved
                .skipped
                .push(format!("{relative}: HTTP {}", exchange.response.status));
            continue;
        }
        // the same read repeated by several tools: the last response wins
        fixtures.insert(relative, exchange.response.body.clone());
    }

    for (relative, body) in &fixtures {
        write_file(&dir.join("unredacted").join(relative), body)?;
    }

    // file names go through redaction as text (an argument can be a MAC)
    let documents: Vec<String> = fixtures
        .keys()
        .cloned()
        .chain(fixtures.values().cloned())
        .collect();
    let redacted = redact(&documents)?;
    let (names, bodies) = redacted.split_at(fixtures.len());
    for (relative, body) in names.iter().zip(bodies) {
        write_file(&dir.join("redacted").join(relative), body)?;
    }
    saved.files = fixtures.into_keys().collect();
    Ok(saved)
}

fn write_file(path: &Path, contents: &str) -> Result<(), Error> {
    let error = |e: std::io::Error| Error(format!("{}: {e}", path.display()));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(error)?;
    }
    std::fs::write(path, contents).map_err(error)
}

/// Relative fixture path for a request, or `None` if it has no known shape.
fn fixture_path(request: &HttpRequest) -> Option<String> {
    match (request.method, request.channel) {
        (Method::Get, Channel::Plain) => Some(format!("scpd/{}", file_name(&request.path)?)),
        (Method::Get, Channel::Secure) => {
            let without_query = request.path.split('?').next()?;
            Some(format!("lists/{}", file_name(without_query)?))
        }
        (Method::Post, _) => {
            let (service, action) = request.soap_service_action()?;
            let (kind, instance) = service.split_once(':')?;
            if ![kind, instance, action].iter().all(|s| is_safe(s)) {
                return None;
            }
            let args = soap_arguments(&request.body, action)?;
            let suffix = if args.is_empty() {
                String::new()
            } else {
                let pairs: Vec<String> = args
                    .iter()
                    .map(|(name, value)| format!("{name}={value}"))
                    .collect();
                format!("@{}", pairs.join("&"))
            };
            Some(format!("responses/{kind}-{instance}/{action}{suffix}.xml"))
        }
    }
}

/// Last segment of the path, if it is a safe file name.
fn file_name(path: &str) -> Option<&str> {
    let name = path.rsplit('/').next()?;
    (is_safe(name) && name != "." && name != "..").then_some(name)
}

/// Characters allowed in a fixture file name.
fn is_safe(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ':'))
}

/// Input arguments of the SOAP envelope sent, if their names and values are safe.
fn soap_arguments(envelope: &str, action: &str) -> Option<Vec<(String, String)>> {
    let args = soap_request_arguments(envelope, action).ok()?;
    args.iter()
        .all(|(name, value)| is_safe(name) && (value.is_empty() || is_safe(value)))
        .then_some(args)
}

/// Default dir: `crates/frix-core/tests/fixtures/raw/`, ignored by git.
#[must_use]
pub fn default_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../crates/frix-core/tests/fixtures/raw")
}

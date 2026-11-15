//! The fixtures in the repo are redacted: no real name or IPv6 address of the router
//! or of the Hosts (spec #1, "Testing Decisions").
//!
//! No real value is written here, not even as a hash (a short name is easy to guess
//! back from its hash): the tests check that only redaction substitutes are left.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::net::Ipv6Addr;
use std::path::{Path, PathBuf};

/// XML elements and JSON keys that hold names (hostnames, Hosts, SSIDs, accounts).
const NAME_FIELDS: &[&str] = &[
    "HostName",
    "NewHostName",
    "X_AVM-DE_FriendlyName",
    "NewX_AVM-DE_FriendlyName",
    "NewDeviceName",
    "NewUserName",
    "NewPortMappingDescription",
    "NewSSID",
    "device_name",
];

fn fixture_files(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("fixture directory") {
        let path = entry.expect("entry").path();
        // `raw/` is the local output of `cargo xtask record`, excluded from git: it
        // deliberately holds the unredacted responses.
        if path.is_dir() && path.file_name().is_some_and(|name| name == "raw") {
            continue;
        }
        if path.is_dir() {
            fixture_files(&path, found);
        } else {
            found.push(path);
        }
    }
}

fn fixtures_in(subdir: &str) -> Vec<(PathBuf, String)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(subdir);
    let mut files = Vec::new();
    fixture_files(&dir, &mut files);
    assert!(!files.is_empty(), "no fixtures in {}", dir.display());
    files
        .into_iter()
        .map(|path| {
            let content = std::fs::read_to_string(&path).expect("text fixture");
            (path, content)
        })
        .collect()
}

/// Values of `field` in a text: `<field>value</field>` in XML, `"field": "value"` in JSON.
fn field_values<'a>(content: &'a str, field: &str) -> Vec<&'a str> {
    let xml_open = format!("<{field}>");
    let xml_close = format!("</{field}>");
    let json_key = format!("\"{field}\":");
    let mut values = Vec::new();
    for (i, _) in content.match_indices(&xml_open) {
        let rest = content.get(i + xml_open.len()..).unwrap_or_default();
        if let Some(end) = rest.find(&xml_close) {
            values.push(rest.get(..end).unwrap_or_default());
        }
    }
    for (i, _) in content.match_indices(&json_key) {
        let rest = content
            .get(i + json_key.len()..)
            .unwrap_or_default()
            .trim_start();
        if let Some(rest) = rest.strip_prefix('"')
            && let Some(end) = rest.find('"')
        {
            values.push(rest.get(..end).unwrap_or_default());
        }
    }
    values
}

/// A redaction substitute (`nome-001`, `WLAN-01`) or a value that names no one.
fn is_substitute(value: &str) -> bool {
    let numbered = |prefix: &str| {
        value
            .strip_prefix(prefix)
            .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
    };
    value.is_empty() || value == "none" || numbered("nome-") || numbered("WLAN-")
}

/// IPv6 addresses of a text: runs of hex digits, `:` and `.` that parse.
fn ipv6_addresses(text: &str) -> impl Iterator<Item = Ipv6Addr> + '_ {
    text.split(|c: char| !(c.is_ascii_hexdigit() || c == ':' || c == '.'))
        .map(|token| token.trim_end_matches('.'))
        .filter(|token| token.matches(':').count() >= 2)
        .filter_map(|token| token.parse().ok())
}

#[test]
fn real_fixtures_hold_only_substitutes_in_name_fields() {
    let mut checked = 0;
    for (path, content) in fixtures_in("real") {
        for field in NAME_FIELDS {
            for value in field_values(&content, field) {
                checked += 1;
                assert!(
                    is_substitute(value),
                    "{}: {field} is not a redaction substitute",
                    path.display()
                );
            }
        }
    }
    assert!(
        checked > 100,
        "only {checked} name fields found: wrong field list?"
    );
}

#[test]
fn no_fixture_contains_a_real_ipv6_address() {
    for (path, content) in fixtures_in("") {
        for address in ipv6_addresses(&content) {
            let [first, second, .., sixth, seventh, _] = address.segments();
            let documentation = first == 0x2001 && second == 0x0db8;
            let global = (first & 0xe000) == 0x2000;
            // EUI-64 interface ID: the device's MAC with `ff:fe` in the middle
            let eui64 = (sixth & 0x00ff) == 0x00ff && (seventh & 0xff00) == 0xfe00;
            let real_global = global && !documentation;
            assert!(
                !real_global && !eui64,
                "{} contains the IPv6 address {address}",
                path.display()
            );
        }
    }
}

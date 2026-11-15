//! `cargo xtask record`: records the real router's responses and produces redacted copies
//! (docs/PROJECT.md §6.6). It only reads: the §8.1 tools through the Policy, plus a
//! recorder that itself rejects every action that is not Level Read.
//! All output goes to stderr.

use std::path::PathBuf;
use std::process::ExitCode;

use xtask::record::record_from_router;
use xtask::save::{default_dir, save};

const USAGE: &str = "usage: cargo xtask record [--out <dir>] [frix-mcp flags: --config, --host, --username, --tls-mode, --tls-pin, --tls-ca-file, --http-port]

Runs the reads of the §8.1 tools on the real router (password from the Keychain) and writes:
  <dir>/unredacted/  responses as received (personal data: do not publish them)
  <dir>/redacted/    redacted copies, with the layout of crates/frix-core/tests/fixtures/
Default dir: crates/frix-core/tests/fixtures/raw/ (ignored by git).";

type BoxError = Box<dyn std::error::Error + Send + Sync>;

// why: stderr is the command's only channel.
#[allow(clippy::print_stderr)]
#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(code) => code,
        Err(error) => {
            eprintln!("xtask: {error}");
            ExitCode::FAILURE
        }
    }
}

#[allow(clippy::print_stderr)]
async fn run() -> Result<ExitCode, BoxError> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("record") => {}
        Some("--help" | "-h") | None => {
            eprintln!("{USAGE}");
            return Ok(ExitCode::SUCCESS);
        }
        Some(other) => return Err(format!("unknown command: {other}\n{USAGE}").into()),
    }
    let mut dir = default_dir();
    let mut frix_flags = Vec::new();
    while let Some(arg) = args.next() {
        if arg == "--help" || arg == "-h" {
            eprintln!("{USAGE}");
            return Ok(ExitCode::SUCCESS);
        } else if arg == "--out" {
            dir = PathBuf::from(args.next().ok_or("--out requires a directory")?);
        } else if let Some(value) = arg.strip_prefix("--out=") {
            dir = PathBuf::from(value);
        } else {
            frix_flags.push(arg);
        }
    }

    let recording = record_from_router(frix_flags).await?;
    let mut failed = 0;
    for outcome in &recording.outcomes {
        match &outcome.error {
            None => eprintln!("ok      {}", outcome.tool),
            Some(error) => {
                failed += 1;
                eprintln!("error   {}: {error}", outcome.tool);
            }
        }
    }
    let saved = save(&recording.exchanges, &dir)?;
    for skipped in &saved.skipped {
        eprintln!("skipped {skipped}");
    }
    eprintln!(
        "{} fixtures in {}/unredacted and {}/redacted. Review the redacted copies before moving them to tests/fixtures/.",
        saved.files.len(),
        dir.display(),
        dir.display()
    );
    Ok(if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

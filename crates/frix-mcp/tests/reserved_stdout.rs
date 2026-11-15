//! In `frix-mcp` stdout is the MCP channel: the binary must not write anything
//! there that is not protocol (docs/PROJECT.md §6.3). Help and errors go to stderr.
//!
//! Every startup here fails before the keyring and the network: `--config` points to a
//! nonexistent file and `HOME` to an empty directory, so the real config is never read.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::process::{Command, Output, Stdio};

fn run(args: &[&str]) -> Output {
    let home = std::env::temp_dir().join(format!("frix-mcp-home-{}", std::process::id()));
    Command::new(env!("CARGO_BIN_EXE_frix-mcp"))
        .args(args)
        .env("HOME", &home)
        .stdin(Stdio::null())
        .output()
        .expect("frix-mcp must start")
}

fn assert_empty_stdout(output: &Output) {
    assert!(
        output.stdout.is_empty(),
        "non-empty stdout: {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn missing_config_error_only_on_stderr() {
    let output = run(&["--config", "/nonexistent/path/frixmcp.toml"]);

    assert!(!output.status.success());
    assert_empty_stdout(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("nonexistent/path/frixmcp.toml"), "{stderr}");
}

#[test]
fn help_on_stderr() {
    let output = run(&["--help"]);

    assert!(output.status.success());
    assert_empty_stdout(&output);
    assert!(String::from_utf8_lossy(&output.stderr).contains("--config"));
}

#[test]
fn insecure_flag_rejected() {
    let output = run(&["--insecure", "--config", "/nonexistent/path.toml"]);

    assert!(!output.status.success());
    assert_empty_stdout(&output);
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown flag: --insecure"));
}

#[test]
fn errors_and_help_in_the_config_language() {
    let dir = std::env::temp_dir().join(format!("frix-mcp-lang-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let config = dir.join("config.toml");
    // No `host`: startup stops at validation, before the keyring and the network.
    std::fs::write(&config, "language = \"it\"\nusername = \"frixmcp\"\n").unwrap();
    let config = config.to_str().unwrap();

    let error = run(&["--config", config]);
    let help = run(&["--config", config, "--help"]);
    let flag = run(&["--config", config, "--insecure"]);

    assert!(!error.status.success());
    assert_empty_stdout(&error);
    let stderr = String::from_utf8_lossy(&error.stderr);
    assert!(stderr.contains("configurazione: manca `host`"), "{stderr}");
    assert_empty_stdout(&help);
    assert!(String::from_utf8_lossy(&help.stderr).contains("uso: frix-mcp"));
    assert_empty_stdout(&flag);
    let stderr = String::from_utf8_lossy(&flag.stderr);
    assert!(stderr.contains("flag sconosciuto: --insecure"), "{stderr}");
    assert!(stderr.contains("uso: frix-mcp"), "{stderr}");
    std::fs::remove_dir_all(&dir).unwrap();
}

//! In `frix-mcp` stdout è il canale MCP: il binario non deve scriverci
//! nulla che non sia protocollo (docs/PROJECT.md §6.3). Aiuto ed errori vanno su stderr.
//!
//! Ogni avvio qui fallisce prima del Keychain e della rete: `--config` punta a un file
//! inesistente e `HOME` a una cartella vuota, così non si legge mai il config reale.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::process::{Command, Output, Stdio};

fn esegui(args: &[&str]) -> Output {
    let home = std::env::temp_dir().join(format!("frix-mcp-home-{}", std::process::id()));
    Command::new(env!("CARGO_BIN_EXE_frix-mcp"))
        .args(args)
        .env("HOME", &home)
        .stdin(Stdio::null())
        .output()
        .expect("frix-mcp deve avviarsi")
}

fn assert_stdout_vuoto(uscita: &Output) {
    assert!(
        uscita.stdout.is_empty(),
        "stdout non vuoto: {:?}",
        String::from_utf8_lossy(&uscita.stdout)
    );
}

#[test]
fn config_mancante_errore_solo_su_stderr() {
    let uscita = esegui(&["--config", "/percorso/inesistente/frixmcp.toml"]);

    assert!(!uscita.status.success());
    assert_stdout_vuoto(&uscita);
    let stderr = String::from_utf8_lossy(&uscita.stderr);
    assert!(stderr.contains("inesistente/frixmcp.toml"), "{stderr}");
}

#[test]
fn aiuto_su_stderr() {
    let uscita = esegui(&["--help"]);

    assert!(uscita.status.success());
    assert_stdout_vuoto(&uscita);
    assert!(String::from_utf8_lossy(&uscita.stderr).contains("--config"));
}

#[test]
fn flag_insicuro_rifiutato() {
    let uscita = esegui(&["--insecure", "--config", "/percorso/inesistente.toml"]);

    assert!(!uscita.status.success());
    assert_stdout_vuoto(&uscita);
    assert!(String::from_utf8_lossy(&uscita.stderr).contains("flag sconosciuto: --insecure"));
}

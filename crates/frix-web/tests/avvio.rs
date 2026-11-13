//! `frix-web` per ora è vuoto (Fase 3): fissa solo il layout del workspace.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::process::{Command, Stdio};

#[test]
fn il_binario_vuoto_si_avvia_e_termina() {
    let stato = Command::new(env!("CARGO_BIN_EXE_frix-web"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("frix-web deve avviarsi");

    assert!(stato.success(), "stato di uscita: {stato:?}");
}

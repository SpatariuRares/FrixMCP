//! `frix-web` is empty for now (Phase 3): it only fixes the workspace layout.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::process::{Command, Stdio};

#[test]
fn empty_binary_starts_and_exits() {
    let status = Command::new(env!("CARGO_BIN_EXE_frix-web"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("frix-web must start");

    assert!(status.success(), "exit status: {status:?}");
}

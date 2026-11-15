//! Live test against the real router (docs/PROJECT.md §6.6, ticket #8).
//!
//! Compiled only with the `live` feature and `#[ignore]` anyway: it does not run in CI or
//! with `cargo test`. It reads the config (`~/.config/frixmcp/config.toml`, or the flags in
//! `FRIXMCP_LIVE_ARGS`) and the password from the Keychain, and only reads: the
//! [`xtask::reads::READS`] through the Policy and a recorder that rejects
//! every action that is not Level Read. It writes no file.
//!
//! ```text
//! cargo test -p xtask --features live --test live -- --ignored --nocapture
//! ```
#![cfg(feature = "live")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use frix_core::policy::{Classification, Level, classification};
use frix_core::transport::Method;
use xtask::record::record_from_router;

#[tokio::test]
#[ignore = "real router: cargo test -p xtask --features live --test live -- --ignored"]
async fn tool_reads_work_on_the_real_router() {
    let args: Vec<String> = std::env::var("FRIXMCP_LIVE_ARGS")
        .unwrap_or_default()
        .split_whitespace()
        .map(str::to_owned)
        .collect();

    let recording = record_from_router(args)
        .await
        .unwrap_or_else(|e| panic!("connecting to the router: {e}"));

    let failed: Vec<String> = recording
        .outcomes
        .iter()
        .filter_map(|o| o.error.as_ref().map(|m| format!("{}: {m}", o.tool)))
        .collect();
    assert!(failed.is_empty(), "failed reads:\n{}", failed.join("\n"));
    for exchange in &recording.exchanges {
        if exchange.request.method != Method::Post {
            continue;
        }
        let (service, action) = exchange.request.soap_service_action().expect("SOAP action");
        assert_eq!(
            classification(service, action),
            Some(Classification::Exposed(Level::Read)),
            "{service}#{action}"
        );
    }
}

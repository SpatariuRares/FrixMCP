//! Test live contro il router reale (docs/PROJECT.md §6.6, ticket #8).
//!
//! Compilato solo con la feature `live` e comunque `#[ignore]`: non gira in CI né con
//! `cargo test`. Legge config (`~/.config/frixmcp/config.toml`, o i flag in
//! `FRIXMCP_LIVE_ARGS`) e password dal Keychain, ed esegue solo letture: le
//! [`xtask::letture::LETTURE`] attraverso la Policy e un registratore che rifiuta
//! ogni azione non di Level Read. Non scrive nessun file.
//!
//! ```text
//! cargo test -p xtask --features live --test live -- --ignored --nocapture
//! ```
#![cfg(feature = "live")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use frix_core::policy::{Classification, Level, classification};
use frix_core::transport::Method;
use xtask::registra::registra_dal_router;

#[tokio::test]
#[ignore = "router reale: cargo test -p xtask --features live --test live -- --ignored"]
async fn le_letture_dei_tool_funzionano_sul_router_reale() {
    let args: Vec<String> = std::env::var("FRIXMCP_LIVE_ARGS")
        .unwrap_or_default()
        .split_whitespace()
        .map(str::to_owned)
        .collect();

    let registrazione = registra_dal_router(args)
        .await
        .unwrap_or_else(|e| panic!("collegamento al router: {e}"));

    let fallite: Vec<String> = registrazione
        .esiti
        .iter()
        .filter_map(|e| e.errore.as_ref().map(|m| format!("{}: {m}", e.tool)))
        .collect();
    assert!(
        fallite.is_empty(),
        "letture fallite:\n{}",
        fallite.join("\n")
    );
    for scambio in &registrazione.scambi {
        if scambio.richiesta.method != Method::Post {
            continue;
        }
        let (service, action) = scambio
            .richiesta
            .soap_service_action()
            .expect("azione SOAP");
        assert_eq!(
            classification(service, action),
            Some(Classification::Exposed(Level::Read)),
            "{service}#{action}"
        );
    }
}

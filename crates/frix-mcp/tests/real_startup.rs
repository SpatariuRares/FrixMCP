//! `frix-mcp` startup on the real transport against a fake router on 127.0.0.1:
//! discovery and `GetSecurityPort` in plaintext, the rest over pinned HTTPS with Digest,
//! Router Account and rights in the stderr summary. No request to the real router.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

#[path = "../../frix-core/tests/support/mod.rs"]
mod support;

use std::path::PathBuf;
use std::sync::Arc;

use frix_core::language::Language;
use frix_core::transport::{Credentials, RouterSettings, TlsTrust};
use frix_mcp::startup;
use rmcp::ServiceExt;
use rmcp::model::CallToolRequestParams;
use secrecy::SecretString;
use support::{FakeRouter, ReceivedRequest, cert};

const USERNAME: &str = "frixmcp";
const PASSWORD: &str = "pw-for-startup-test";

fn fixture(relative: &str) -> Option<String> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../frix-core/tests/fixtures");
    std::fs::read_to_string(dir.join(relative)).ok()
}

/// Fake router with the 7590 fixtures: GET and `GetSecurityPort` in plaintext, the rest over HTTPS.
fn router_7590() -> FakeRouter {
    let https_port = Arc::new(std::sync::OnceLock::<u16>::new());
    let port = Arc::clone(&https_port);
    let router = FakeRouter::start(Arc::new(move |r: &ReceivedRequest| {
        let action = r
            .header("soapaction")
            .unwrap_or_default()
            .trim_matches('"')
            .to_owned();
        match (r.tls, r.method.as_str()) {
            (false, "GET") => fixture(&format!("scpd{}", r.path))
                .map_or((404, vec![], String::new()), |f| (200, vec![], f)),
            (false, "POST") if action.ends_with("DeviceInfo:1#GetSecurityPort") => (
                200,
                vec![],
                format!(
                    r#"<?xml version="1.0"?><s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Body><u:GetSecurityPortResponse xmlns:u="urn:dslforum-org:service:DeviceInfo:1"><NewSecurityPort>{}</NewSecurityPort></u:GetSecurityPortResponse></s:Body></s:Envelope>"#,
                    port.get().unwrap()
                ),
            ),
            (true, "POST") if support::valid_digest(r, USERNAME, PASSWORD) => {
                let (service, name) = action
                    .trim_start_matches("urn:dslforum-org:service:")
                    .split_once('#')
                    .unwrap();
                let file = format!("responses/{}/{name}.xml", service.replace(':', "-"));
                fixture(&file).map_or((500, vec![], String::new()), |f| (200, vec![], f))
            }
            (true, _) => (401, vec![support::challenge()], String::new()),
            _ => (400, vec![], String::new()),
        }
    }));
    https_port.set(router.https_port).unwrap();
    router
}

fn settings(router: &FakeRouter) -> RouterSettings {
    RouterSettings {
        host: "127.0.0.1".to_owned(),
        http_port: router.http_port,
        tls: TlsTrust::Pin(cert::CERT_SHA256.parse().unwrap()),
    }
}

fn credentials() -> Credentials {
    Credentials {
        username: USERNAME.to_owned(),
        password: SecretString::from(PASSWORD),
    }
}

#[tokio::test]
async fn startup_discovers_the_https_port_and_reports_router_account_and_rights() {
    let router = router_7590();

    let started = startup::connect(settings(&router), credentials())
        .await
        .unwrap();

    let summary = startup::banner(&started.user, Language::En);
    assert!(
        summary.contains("Router Account: frixmcp; rights:"),
        "{summary}"
    );
    assert!(summary.contains("BoxAdmin=readwrite"), "{summary}");
    assert!(summary.contains("NAS=none"), "{summary}");
    let italian = startup::banner(&started.user, Language::It);
    assert!(
        italian.contains("Router Account: frixmcp; diritti:"),
        "{italian}"
    );
    assert!(italian.contains("NAS=none"), "{italian}");

    let received = router.received();
    let plaintext: Vec<_> = received.iter().filter(|r| !r.tls).collect();
    assert!(
        plaintext
            .iter()
            .all(|r| r.header("authorization").is_none())
    );
    let plaintext_posts: Vec<_> = plaintext.iter().filter(|r| r.method == "POST").collect();
    assert_eq!(
        plaintext_posts.len(),
        1,
        "only GetSecurityPort in plaintext"
    );
    assert!(
        plaintext_posts[0]
            .header("soapaction")
            .unwrap()
            .contains("#GetSecurityPort")
    );
    assert!(received.iter().any(|r| {
        r.tls
            && r.header("soapaction")
                .unwrap_or_default()
                .contains("GetCurrentUser")
    }));
}

#[tokio::test]
async fn tools_work_on_the_real_transport() {
    let router = router_7590();
    let started = startup::connect(settings(&router), credentials())
        .await
        .unwrap();

    let (server_side, client_side) = tokio::io::duplex(64 * 1024);
    tokio::spawn(async move {
        let server = started.server.serve(server_side).await?;
        server.waiting().await?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    });
    let mcp = ().serve(client_side).await.unwrap();
    let outcome = mcp
        .call_tool(CallToolRequestParams::new("get_system_info"))
        .await
        .unwrap();

    assert_ne!(outcome.is_error, Some(true), "{outcome:?}");
    let text = &outcome.content[0].as_text().unwrap().text;
    assert!(text.contains("FRITZ!Box 7590"), "{text}");
}

#[tokio::test]
async fn wrong_fingerprint_stops_startup_with_expected_and_found() {
    let router = router_7590();
    let mut settings = settings(&router);
    settings.tls = TlsTrust::Pin("11".repeat(32).parse().unwrap());

    let error = startup::connect(settings, credentials()).await.unwrap_err();

    let message = error.to_string();
    assert_eq!(error.kind(), "tls_pin_mismatch");
    assert!(message.contains(&["11"; 32].join(":")), "{message}");
    assert!(message.contains(cert::CERT_SHA256), "{message}");
    assert!(!message.contains(PASSWORD));
}

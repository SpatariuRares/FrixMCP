//! Avvio di `frix-mcp` sul trasporto reale contro un router finto su 127.0.0.1:
//! discovery e `GetSecurityPort` in chiaro, il resto su HTTPS pinnato con Digest,
//! Router Account e diritti nel riepilogo per stderr. Nessuna richiesta al router vero.
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

use frix_core::transport::{Credentials, RouterSettings, TlsTrust};
use frix_mcp::startup;
use rmcp::ServiceExt;
use rmcp::model::CallToolRequestParams;
use secrecy::SecretString;
use support::{Ricevuta, RouterFinto, cert};

const UTENTE: &str = "frixmcp";
const PASSWORD: &str = "pw-di-test-avvio";

fn fixture(relativo: &str) -> Option<String> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../frix-core/tests/fixtures");
    std::fs::read_to_string(dir.join(relativo)).ok()
}

/// Router finto con le fixture del 7590: GET e `GetSecurityPort` in chiaro, il resto su HTTPS.
fn router_7590() -> RouterFinto {
    let https_port = Arc::new(std::sync::OnceLock::<u16>::new());
    let porta = Arc::clone(&https_port);
    let router = RouterFinto::avvia(Arc::new(move |r: &Ricevuta| {
        let azione = r
            .header("soapaction")
            .unwrap_or_default()
            .trim_matches('"')
            .to_owned();
        match (r.tls, r.method.as_str()) {
            (false, "GET") => fixture(&format!("scpd{}", r.path))
                .map_or((404, vec![], String::new()), |f| (200, vec![], f)),
            (false, "POST") if azione.ends_with("DeviceInfo:1#GetSecurityPort") => (
                200,
                vec![],
                format!(
                    r#"<?xml version="1.0"?><s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Body><u:GetSecurityPortResponse xmlns:u="urn:dslforum-org:service:DeviceInfo:1"><NewSecurityPort>{}</NewSecurityPort></u:GetSecurityPortResponse></s:Body></s:Envelope>"#,
                    porta.get().unwrap()
                ),
            ),
            (true, "POST") if support::digest_valido(r, UTENTE, PASSWORD) => {
                let (servizio, nome) = azione
                    .trim_start_matches("urn:dslforum-org:service:")
                    .split_once('#')
                    .unwrap();
                let file = format!("responses/{}/{nome}.xml", servizio.replace(':', "-"));
                fixture(&file).map_or((500, vec![], String::new()), |f| (200, vec![], f))
            }
            (true, _) => (401, vec![support::sfida()], String::new()),
            _ => (400, vec![], String::new()),
        }
    }));
    https_port.set(router.https_port).unwrap();
    router
}

fn impostazioni(router: &RouterFinto) -> RouterSettings {
    RouterSettings {
        host: "127.0.0.1".to_owned(),
        http_port: router.http_port,
        tls: TlsTrust::Pin(cert::CERT_SHA256.parse().unwrap()),
    }
}

fn credenziali() -> Credentials {
    Credentials {
        username: UTENTE.to_owned(),
        password: SecretString::from(PASSWORD),
    }
}

#[tokio::test]
async fn avvio_scopre_la_porta_https_e_riporta_router_account_e_diritti() {
    let router = router_7590();

    let avviato = startup::connect(impostazioni(&router), credenziali())
        .await
        .unwrap();

    let riepilogo = startup::banner(&avviato.user);
    assert!(riepilogo.contains("Router Account: frixmcp"), "{riepilogo}");
    assert!(riepilogo.contains("BoxAdmin=readwrite"), "{riepilogo}");
    assert!(riepilogo.contains("NAS=none"), "{riepilogo}");

    let ricevute = router.ricevute();
    let in_chiaro: Vec<_> = ricevute.iter().filter(|r| !r.tls).collect();
    assert!(
        in_chiaro
            .iter()
            .all(|r| r.header("authorization").is_none())
    );
    let post_in_chiaro: Vec<_> = in_chiaro.iter().filter(|r| r.method == "POST").collect();
    assert_eq!(post_in_chiaro.len(), 1, "solo GetSecurityPort in chiaro");
    assert!(
        post_in_chiaro[0]
            .header("soapaction")
            .unwrap()
            .contains("#GetSecurityPort")
    );
    assert!(ricevute.iter().any(|r| {
        r.tls
            && r.header("soapaction")
                .unwrap_or_default()
                .contains("GetCurrentUser")
    }));
}

#[tokio::test]
async fn i_tool_funzionano_sul_trasporto_reale() {
    let router = router_7590();
    let avviato = startup::connect(impostazioni(&router), credenziali())
        .await
        .unwrap();

    let (lato_server, lato_client) = tokio::io::duplex(64 * 1024);
    tokio::spawn(async move {
        let server = avviato.server.serve(lato_server).await?;
        server.waiting().await?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    });
    let mcp = ().serve(lato_client).await.unwrap();
    let esito = mcp
        .call_tool(CallToolRequestParams::new("get_system_info"))
        .await
        .unwrap();

    assert_ne!(esito.is_error, Some(true), "{esito:?}");
    let testo = &esito.content[0].as_text().unwrap().text;
    assert!(testo.contains("FRITZ!Box 7590"), "{testo}");
}

#[tokio::test]
async fn impronta_sbagliata_ferma_l_avvio_con_attesa_e_trovata() {
    let router = router_7590();
    let mut settings = impostazioni(&router);
    settings.tls = TlsTrust::Pin("11".repeat(32).parse().unwrap());

    let errore = startup::connect(settings, credenziali()).await.unwrap_err();

    let messaggio = errore.to_string();
    assert_eq!(errore.kind(), "tls_pin_mismatch");
    assert!(messaggio.contains(&["11"; 32].join(":")), "{messaggio}");
    assert!(messaggio.contains(cert::CERT_SHA256), "{messaggio}");
    assert!(!messaggio.contains(PASSWORD));
}

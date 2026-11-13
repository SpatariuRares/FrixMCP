//! Il trasporto reale (`RouterTransport`) contro un router finto su 127.0.0.1:
//! TLS pinnato o con CA, HTTP Digest, canale in chiaro solo dove previsto.
//! Nessuna richiesta al router vero.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod support;

use std::sync::Arc;

use frix_core::FrixError;
use frix_core::transport::{
    Channel, Credentials, Fingerprint, HttpRequest, HttpTransport, Method, RouterSettings,
    RouterTransport, TlsTrust,
};
use secrecy::SecretString;
use support::{Ricevuta, RouterFinto, cert};

const UTENTE: &str = "frixmcp";
const PASSWORD: &str = "pw-di-test-9f8e7d";

fn credenziali(password: &str) -> Credentials {
    Credentials {
        username: UTENTE.to_owned(),
        password: SecretString::from(password),
    }
}

/// Router finto che chiede Digest su HTTPS e risponde `ok` alle richieste autenticate.
fn router_con_digest() -> RouterFinto {
    RouterFinto::avvia(Arc::new(|r: &Ricevuta| {
        if !r.tls {
            return (200, vec![], format!("chiaro {}", r.path));
        }
        if support::digest_valido(r, UTENTE, PASSWORD) {
            (200, vec![], "ok".to_owned())
        } else {
            (401, vec![support::sfida()], String::new())
        }
    }))
}

fn trasporto(router: &RouterFinto, trust: TlsTrust, password: &str) -> RouterTransport {
    let t = RouterTransport::new(
        RouterSettings {
            host: "127.0.0.1".to_owned(),
            http_port: router.http_port,
            tls: trust,
        },
        credenziali(password),
    )
    .unwrap();
    t.set_secure_port(router.https_port);
    t
}

fn pin_giusto() -> TlsTrust {
    TlsTrust::Pin(cert::CERT_SHA256.parse().unwrap())
}

fn soap(path: &str) -> HttpRequest {
    HttpRequest {
        method: Method::Post,
        channel: Channel::Secure,
        path: path.to_owned(),
        soap_action: Some("urn:dslforum-org:service:DeviceInfo:1#GetInfo".to_owned()),
        body: "<s:Envelope/>".to_owned(),
    }
}

#[tokio::test]
async fn https_con_impronta_giusta_e_digest_arriva_al_router() {
    let router = router_con_digest();
    let t = trasporto(&router, pin_giusto(), PASSWORD);

    let risposta = t.send(soap("/upnp/control/deviceinfo")).await.unwrap();

    assert_eq!((risposta.status, risposta.body.as_str()), (200, "ok"));
    let ricevute = router.ricevute();
    assert_eq!(ricevute.len(), 2, "sfida e poi richiesta autenticata");
    assert!(ricevute.iter().all(|r| r.tls && r.method == "POST"));
    assert_eq!(
        ricevute[1].header("soapaction"),
        Some(r#""urn:dslforum-org:service:DeviceInfo:1#GetInfo""#)
    );
    for r in &ricevute {
        assert!(
            !format!("{r:?}").contains(PASSWORD),
            "password in chiaro: {r:?}"
        );
    }
}

#[tokio::test]
async fn il_nonce_si_riusa_senza_una_nuova_sfida() {
    let router = router_con_digest();
    let t = trasporto(&router, pin_giusto(), PASSWORD);

    t.send(soap("/upnp/control/deviceinfo")).await.unwrap();
    let seconda = t.send(soap("/upnp/control/hosts")).await.unwrap();

    assert_eq!(seconda.status, 200);
    let ricevute = router.ricevute();
    assert_eq!(ricevute.len(), 3);
    assert!(
        ricevute[2]
            .header("authorization")
            .unwrap()
            .contains("nc=00000002")
    );
}

#[tokio::test]
async fn impronta_sbagliata_rifiutata_prima_di_qualunque_richiesta() {
    let router = router_con_digest();
    let atteso: Fingerprint = "AA".repeat(32).parse().unwrap();
    let t = trasporto(&router, TlsTrust::Pin(atteso), PASSWORD);

    let errore = t.send(soap("/upnp/control/deviceinfo")).await.unwrap_err();

    assert_eq!(
        errore,
        FrixError::TlsPinMismatch {
            expected: atteso.to_string(),
            found: cert::CERT_SHA256.to_owned(),
        }
    );
    assert!(router.ricevute().is_empty());
}

#[tokio::test]
async fn certificato_del_router_come_unica_ca() {
    let router = router_con_digest();
    let t = trasporto(&router, TlsTrust::Ca(support::cert_der()), PASSWORD);

    let risposta = t.send(soap("/upnp/control/deviceinfo")).await.unwrap();

    assert_eq!(risposta.status, 200);
}

#[tokio::test]
async fn credenziali_rifiutate_non_si_riprova() {
    let router = router_con_digest();
    let t = trasporto(&router, pin_giusto(), "password-sbagliata");

    let prima = t.send(soap("/upnp/control/deviceinfo")).await.unwrap();
    assert_eq!(prima.status, 401);
    assert_eq!(router.ricevute().len(), 2, "una sfida, un solo tentativo");

    let seconda = t.send(soap("/upnp/control/deviceinfo")).await.unwrap_err();
    assert_eq!(seconda, FrixError::Unauthenticated);
    assert_eq!(
        router.ricevute().len(),
        2,
        "nessun nuovo tentativo al router"
    );
}

#[tokio::test]
async fn descrizioni_in_chiaro_senza_credenziali() {
    let router = router_con_digest();
    let t = trasporto(&router, pin_giusto(), PASSWORD);

    let risposta = t
        .send(HttpRequest {
            method: Method::Get,
            channel: Channel::Plain,
            path: "/tr64desc.xml".to_owned(),
            soap_action: None,
            body: String::new(),
        })
        .await
        .unwrap();

    assert_eq!(risposta.body, "chiaro /tr64desc.xml");
    let ricevute = router.ricevute();
    assert_eq!(ricevute.len(), 1);
    assert!(!ricevute[0].tls);
    assert_eq!(ricevute[0].header("authorization"), None);
}

#[tokio::test]
async fn senza_porta_https_nota_nessuna_richiesta_autenticata() {
    let router = router_con_digest();
    let t = RouterTransport::new(
        RouterSettings {
            host: "127.0.0.1".to_owned(),
            http_port: router.http_port,
            tls: pin_giusto(),
        },
        credenziali(PASSWORD),
    )
    .unwrap();

    let errore = t.send(soap("/upnp/control/deviceinfo")).await.unwrap_err();

    assert_eq!(errore.kind(), "transport");
    assert!(router.ricevute().is_empty());
}

#[test]
fn nessun_segreto_in_debug() {
    let t = RouterTransport::new(
        RouterSettings {
            host: "127.0.0.1".to_owned(),
            http_port: 1,
            tls: pin_giusto(),
        },
        credenziali(PASSWORD),
    )
    .unwrap();

    assert!(!format!("{t:?}").contains(PASSWORD));
    assert!(!format!("{:?}", credenziali(PASSWORD)).contains(PASSWORD));
}

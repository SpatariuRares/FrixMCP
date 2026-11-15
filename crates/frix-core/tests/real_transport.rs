//! The real transport (`RouterTransport`) against a fake router on 127.0.0.1:
//! pinned or CA TLS, HTTP Digest, plain channel only where intended.
//! No request to the real router.
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
use support::{FakeRouter, ReceivedRequest, cert};

const USER: &str = "frixmcp";
const PASSWORD: &str = "test-pw-9f8e7d";

fn credentials(password: &str) -> Credentials {
    Credentials {
        username: USER.to_owned(),
        password: SecretString::from(password),
    }
}

/// Fake router that asks for Digest over HTTPS and answers `ok` to authenticated requests.
fn router_with_digest() -> FakeRouter {
    FakeRouter::start(Arc::new(|r: &ReceivedRequest| {
        if !r.tls {
            return (200, vec![], format!("plain {}", r.path));
        }
        if support::valid_digest(r, USER, PASSWORD) {
            (200, vec![], "ok".to_owned())
        } else {
            (401, vec![support::challenge()], String::new())
        }
    }))
}

fn transport(router: &FakeRouter, trust: TlsTrust, password: &str) -> RouterTransport {
    let t = RouterTransport::new(
        RouterSettings {
            host: "127.0.0.1".to_owned(),
            http_port: router.http_port,
            tls: trust,
        },
        credentials(password),
    )
    .unwrap();
    t.set_secure_port(router.https_port);
    t
}

fn right_pin() -> TlsTrust {
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
async fn https_with_right_fingerprint_and_digest_reaches_the_router() {
    let router = router_with_digest();
    let t = transport(&router, right_pin(), PASSWORD);

    let response = t.send(soap("/upnp/control/deviceinfo")).await.unwrap();

    assert_eq!((response.status, response.body.as_str()), (200, "ok"));
    let received = router.received();
    assert_eq!(received.len(), 2, "challenge, then authenticated request");
    assert!(received.iter().all(|r| r.tls && r.method == "POST"));
    assert_eq!(
        received[1].header("soapaction"),
        Some(r#""urn:dslforum-org:service:DeviceInfo:1#GetInfo""#)
    );
    for r in &received {
        assert!(
            !format!("{r:?}").contains(PASSWORD),
            "password in plain text: {r:?}"
        );
    }
}

#[tokio::test]
async fn nonce_is_reused_without_a_new_challenge() {
    let router = router_with_digest();
    let t = transport(&router, right_pin(), PASSWORD);

    t.send(soap("/upnp/control/deviceinfo")).await.unwrap();
    let second = t.send(soap("/upnp/control/hosts")).await.unwrap();

    assert_eq!(second.status, 200);
    let received = router.received();
    assert_eq!(received.len(), 3);
    assert!(
        received[2]
            .header("authorization")
            .unwrap()
            .contains("nc=00000002")
    );
}

#[tokio::test]
async fn wrong_fingerprint_rejected_before_any_request() {
    let router = router_with_digest();
    let expected: Fingerprint = "AA".repeat(32).parse().unwrap();
    let t = transport(&router, TlsTrust::Pin(expected), PASSWORD);

    let error = t.send(soap("/upnp/control/deviceinfo")).await.unwrap_err();

    assert_eq!(
        error,
        FrixError::TlsPinMismatch {
            expected: expected.to_string(),
            found: Some(cert::CERT_SHA256.to_owned()),
        }
    );
    assert!(router.received().is_empty());
}

#[tokio::test]
async fn router_certificate_as_only_ca() {
    let router = router_with_digest();
    let t = transport(&router, TlsTrust::Ca(support::cert_der()), PASSWORD);

    let response = t.send(soap("/upnp/control/deviceinfo")).await.unwrap();

    assert_eq!(response.status, 200);
}

#[tokio::test]
async fn rejected_credentials_are_not_retried() {
    let router = router_with_digest();
    let t = transport(&router, right_pin(), "wrong-password");

    let first = t.send(soap("/upnp/control/deviceinfo")).await.unwrap();
    assert_eq!(first.status, 401);
    assert_eq!(
        router.received().len(),
        2,
        "one challenge, a single attempt"
    );

    let second = t.send(soap("/upnp/control/deviceinfo")).await.unwrap_err();
    assert_eq!(second, FrixError::Unauthenticated);
    assert_eq!(router.received().len(), 2, "no new attempt to the router");
}

#[tokio::test]
async fn plain_descriptions_without_credentials() {
    let router = router_with_digest();
    let t = transport(&router, right_pin(), PASSWORD);

    let response = t
        .send(HttpRequest {
            method: Method::Get,
            channel: Channel::Plain,
            path: "/tr64desc.xml".to_owned(),
            soap_action: None,
            body: String::new(),
        })
        .await
        .unwrap();

    assert_eq!(response.body, "plain /tr64desc.xml");
    let received = router.received();
    assert_eq!(received.len(), 1);
    assert!(!received[0].tls);
    assert_eq!(received[0].header("authorization"), None);
}

#[tokio::test]
async fn no_authenticated_request_without_known_https_port() {
    let router = router_with_digest();
    let t = RouterTransport::new(
        RouterSettings {
            host: "127.0.0.1".to_owned(),
            http_port: router.http_port,
            tls: right_pin(),
        },
        credentials(PASSWORD),
    )
    .unwrap();

    let error = t.send(soap("/upnp/control/deviceinfo")).await.unwrap_err();

    assert_eq!(error.kind(), "transport");
    assert!(router.received().is_empty());
}

#[test]
fn no_secret_in_debug() {
    let t = RouterTransport::new(
        RouterSettings {
            host: "127.0.0.1".to_owned(),
            http_port: 1,
            tls: right_pin(),
        },
        credentials(PASSWORD),
    )
    .unwrap();

    assert!(!format!("{t:?}").contains(PASSWORD));
    assert!(!format!("{:?}", credentials(PASSWORD)).contains(PASSWORD));
}

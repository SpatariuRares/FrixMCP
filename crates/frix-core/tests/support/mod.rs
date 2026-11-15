//! Fake router on 127.0.0.1 for the real transport tests: plain HTTP and HTTPS with
//! a test certificate. Never contacts the real router.
//!
//! Also used by the `frix-mcp` tests with `#[path = …]`.
#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::missing_panics_doc
)]

pub mod cert;

use std::fmt::Write as _;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

use md5::{Digest as _, Md5};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

/// A request received by the fake router.
#[derive(Debug, Clone)]
pub struct ReceivedRequest {
    /// Arrived over HTTPS.
    pub tls: bool,
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl ReceivedRequest {
    /// Value of a header (case-insensitive name).
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// Response of the fake router: status, extra headers, body.
pub type Response = (u16, Vec<(String, String)>, String);

/// Request handler.
pub type Handler = Arc<dyn Fn(&ReceivedRequest) -> Response + Send + Sync>;

/// Fake router listening on two local ports.
#[derive(Debug, Clone)]
pub struct FakeRouter {
    pub http_port: u16,
    pub https_port: u16,
    pub received: Arc<Mutex<Vec<ReceivedRequest>>>,
}

impl FakeRouter {
    pub fn start(handler: Handler) -> Self {
        let received = Arc::new(Mutex::new(Vec::new()));
        let http = TcpListener::bind("127.0.0.1:0").unwrap();
        let https = TcpListener::bind("127.0.0.1:0").unwrap();
        let router = Self {
            http_port: http.local_addr().unwrap().port(),
            https_port: https.local_addr().unwrap().port(),
            received: Arc::clone(&received),
        };

        let (h, r) = (Arc::clone(&handler), Arc::clone(&received));
        thread::spawn(move || {
            for stream in http.incoming().flatten() {
                serve(stream, false, &h, &r);
            }
        });
        let config = Arc::new(server_config());
        thread::spawn(move || {
            for stream in https.incoming().flatten() {
                let conn = rustls::ServerConnection::new(Arc::clone(&config)).unwrap();
                let tls = rustls::StreamOwned::new(conn, stream);
                serve(tls, true, &handler, &received);
            }
        });
        router
    }

    pub fn received(&self) -> Vec<ReceivedRequest> {
        self.received.lock().unwrap().clone()
    }
}

fn server_config() -> rustls::ServerConfig {
    let cert = CertificateDer::from(from_hex(cert::CERT_HEX));
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(from_hex(cert::KEY_HEX)));
    rustls::ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)
        .unwrap()
}

/// Bytes of the test certificate (DER).
pub fn cert_der() -> Vec<u8> {
    from_hex(cert::CERT_HEX)
}

fn from_hex(lines: &[&str]) -> Vec<u8> {
    let text: String = lines.concat();
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

/// One request per connection, then `Connection: close`. A failed handshake just closes.
fn serve<S: Read + Write>(
    stream: S,
    tls: bool,
    handler: &Handler,
    log: &Arc<Mutex<Vec<ReceivedRequest>>>,
) {
    let mut reader = BufReader::new(stream);
    let mut first = String::new();
    if reader.read_line(&mut first).unwrap_or(0) == 0 {
        return;
    }
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or_default().to_owned();
    let path = parts.next().unwrap_or_default().to_owned();
    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((n, v)) = line.split_once(':') {
            headers.push((n.trim().to_owned(), v.trim().to_owned()));
        }
    }
    let length: usize = headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case("content-length"))
        .map_or(0, |(_, v)| v.parse().unwrap());
    let mut body = vec![0; length];
    if reader.read_exact(&mut body).is_err() {
        return;
    }
    let request = ReceivedRequest {
        tls,
        method,
        path,
        headers,
        body: String::from_utf8(body).unwrap(),
    };
    log.lock().unwrap().push(request.clone());
    let (status, extra, body) = handler(&request);
    let mut response = format!(
        "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    for (n, v) in extra {
        let _ = write!(response, "{n}: {v}\r\n");
    }
    response.push_str("\r\n");
    response.push_str(&body);
    let stream = reader.get_mut();
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

/// Digest challenge of the fake router (typical parameters: `qop=auth`, MD5).
pub const REALM: &str = "F!Box SOAP-Auth";
pub const NONCE: &str = "4F1C5B3A2D9E8F70";

pub fn challenge() -> (String, String) {
    (
        "WWW-Authenticate".to_owned(),
        format!(r#"Digest realm="{REALM}", nonce="{NONCE}", algorithm=MD5, qop="auth""#),
    )
}

/// Independent check of an `Authorization: Digest` header (`qop=auth`, MD5).
pub fn valid_digest(request: &ReceivedRequest, user: &str, password: &str) -> bool {
    let Some(header) = request.header("authorization") else {
        return false;
    };
    let Some(params) = header.strip_prefix("Digest ") else {
        return false;
    };
    let field = |name: &str| -> String {
        params
            .split(", ")
            .find_map(|p| p.strip_prefix(&format!("{name}=")))
            .unwrap_or_default()
            .trim_matches('"')
            .to_owned()
    };
    let md5 = |s: String| -> String {
        Md5::digest(s.as_bytes())
            .iter()
            .fold(String::new(), |mut acc, b| {
                let _ = write!(acc, "{b:02x}");
                acc
            })
    };
    let ha1 = md5(format!("{user}:{REALM}:{password}"));
    let ha2 = md5(format!("{}:{}", request.method, field("uri")));
    let expected = md5(format!(
        "{ha1}:{}:{}:{}:auth:{ha2}",
        field("nonce"),
        field("nc"),
        field("cnonce")
    ));
    field("username") == user
        && field("uri") == request.path
        && field("nonce") == NONCE
        && field("response") == expected
}

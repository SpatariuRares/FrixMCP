//! Router finto su 127.0.0.1 per i test del trasporto reale: HTTP in chiaro e HTTPS
//! con un certificato di test. Non contatta mai il router vero.
//!
//! Usato anche dai test di `frix-mcp` con `#[path = …]`.
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

/// Una richiesta ricevuta dal router finto.
#[derive(Debug, Clone)]
pub struct Ricevuta {
    /// Arrivata su HTTPS.
    pub tls: bool,
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Ricevuta {
    /// Valore di un header (nome senza distinzione di maiuscole).
    pub fn header(&self, nome: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(nome))
            .map(|(_, v)| v.as_str())
    }
}

/// Risposta del router finto: stato, header aggiuntivi, corpo.
pub type Risposta = (u16, Vec<(String, String)>, String);

/// Gestore delle richieste.
pub type Gestore = Arc<dyn Fn(&Ricevuta) -> Risposta + Send + Sync>;

/// Router finto in ascolto su due porte locali.
#[derive(Debug, Clone)]
pub struct RouterFinto {
    pub http_port: u16,
    pub https_port: u16,
    pub ricevute: Arc<Mutex<Vec<Ricevuta>>>,
}

impl RouterFinto {
    pub fn avvia(gestore: Gestore) -> Self {
        let ricevute = Arc::new(Mutex::new(Vec::new()));
        let http = TcpListener::bind("127.0.0.1:0").unwrap();
        let https = TcpListener::bind("127.0.0.1:0").unwrap();
        let router = Self {
            http_port: http.local_addr().unwrap().port(),
            https_port: https.local_addr().unwrap().port(),
            ricevute: Arc::clone(&ricevute),
        };

        let (g, r) = (Arc::clone(&gestore), Arc::clone(&ricevute));
        thread::spawn(move || {
            for stream in http.incoming().flatten() {
                servi(stream, false, &g, &r);
            }
        });
        let config = Arc::new(server_config());
        thread::spawn(move || {
            for stream in https.incoming().flatten() {
                let conn = rustls::ServerConnection::new(Arc::clone(&config)).unwrap();
                let tls = rustls::StreamOwned::new(conn, stream);
                servi(tls, true, &gestore, &ricevute);
            }
        });
        router
    }

    pub fn ricevute(&self) -> Vec<Ricevuta> {
        self.ricevute.lock().unwrap().clone()
    }
}

fn server_config() -> rustls::ServerConfig {
    let cert = CertificateDer::from(da_hex(cert::CERT_HEX));
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(da_hex(cert::KEY_HEX)));
    rustls::ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)
        .unwrap()
}

/// Byte del certificato di test (DER).
pub fn cert_der() -> Vec<u8> {
    da_hex(cert::CERT_HEX)
}

fn da_hex(righe: &[&str]) -> Vec<u8> {
    let testo: String = righe.concat();
    (0..testo.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&testo[i..i + 2], 16).unwrap())
        .collect()
}

/// Una richiesta per connessione, poi `Connection: close`. Un handshake fallito chiude e basta.
fn servi<S: Read + Write>(
    stream: S,
    tls: bool,
    gestore: &Gestore,
    registro: &Arc<Mutex<Vec<Ricevuta>>>,
) {
    let mut lettore = BufReader::new(stream);
    let mut prima = String::new();
    if lettore.read_line(&mut prima).unwrap_or(0) == 0 {
        return;
    }
    let mut parti = prima.split_whitespace();
    let method = parti.next().unwrap_or_default().to_owned();
    let path = parti.next().unwrap_or_default().to_owned();
    let mut headers = Vec::new();
    loop {
        let mut riga = String::new();
        if lettore.read_line(&mut riga).unwrap_or(0) == 0 {
            return;
        }
        let riga = riga.trim_end();
        if riga.is_empty() {
            break;
        }
        if let Some((n, v)) = riga.split_once(':') {
            headers.push((n.trim().to_owned(), v.trim().to_owned()));
        }
    }
    let lunghezza: usize = headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case("content-length"))
        .map_or(0, |(_, v)| v.parse().unwrap());
    let mut corpo = vec![0; lunghezza];
    if lettore.read_exact(&mut corpo).is_err() {
        return;
    }
    let ricevuta = Ricevuta {
        tls,
        method,
        path,
        headers,
        body: String::from_utf8(corpo).unwrap(),
    };
    registro.lock().unwrap().push(ricevuta.clone());
    let (stato, extra, corpo) = gestore(&ricevuta);
    let mut risposta = format!(
        "HTTP/1.1 {stato} X\r\nContent-Length: {}\r\nConnection: close\r\n",
        corpo.len()
    );
    for (n, v) in extra {
        let _ = write!(risposta, "{n}: {v}\r\n");
    }
    risposta.push_str("\r\n");
    risposta.push_str(&corpo);
    let stream = lettore.get_mut();
    let _ = stream.write_all(risposta.as_bytes());
    let _ = stream.flush();
}

/// Sfida Digest del router finto (parametri tipici: `qop=auth`, MD5).
pub const REALM: &str = "F!Box SOAP-Auth";
pub const NONCE: &str = "4F1C5B3A2D9E8F70";

pub fn sfida() -> (String, String) {
    (
        "WWW-Authenticate".to_owned(),
        format!(r#"Digest realm="{REALM}", nonce="{NONCE}", algorithm=MD5, qop="auth""#),
    )
}

/// Verifica indipendente di un header `Authorization: Digest` (`qop=auth`, MD5).
pub fn digest_valido(ricevuta: &Ricevuta, utente: &str, password: &str) -> bool {
    let Some(header) = ricevuta.header("authorization") else {
        return false;
    };
    let Some(parametri) = header.strip_prefix("Digest ") else {
        return false;
    };
    let campo = |nome: &str| -> String {
        parametri
            .split(", ")
            .find_map(|p| p.strip_prefix(&format!("{nome}=")))
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
    let ha1 = md5(format!("{utente}:{REALM}:{password}"));
    let ha2 = md5(format!("{}:{}", ricevuta.method, campo("uri")));
    let atteso = md5(format!(
        "{ha1}:{}:{}:{}:auth:{ha2}",
        campo("nonce"),
        campo("nc"),
        campo("cnonce")
    ));
    campo("username") == utente
        && campo("uri") == ricevuta.path
        && campo("nonce") == NONCE
        && campo("response") == atteso
}

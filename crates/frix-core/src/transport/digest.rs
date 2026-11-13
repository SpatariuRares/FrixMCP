//! HTTP Digest (RFC 2617, RFC 7616), l'autenticazione delle chiamate TR-064
//! (docs/PROJECT.md §5.1).
//!
//! I parametri effettivi del 7590 (`qop`, `algorithm`, `opaque`) non sono ancora
//! verificati (§14.3): si supportano `qop=auth` o nessun `qop` (compatibilità RFC 2069),
//! `algorithm` `MD5`, `MD5-sess`, `SHA-256` o `SHA-256-sess` (RFC 7616), `opaque`
//! facoltativo. `qop=auth-int` (serve l'hash del corpo) e gli altri algoritmi
//! (`SHA-512-256`, …) vengono rifiutati con un errore che li nomina.

use std::fmt::Write as _;

use md5::{Digest as _, Md5};
use ring::digest::{Context as Sha256Context, SHA256};
use secrecy::{ExposeSecret as _, SecretString};

use crate::error::FrixError;

/// Algoritmo della sfida Digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Algorithm {
    /// `MD5` (anche quando `algorithm` manca).
    Md5,
    /// `MD5-sess`.
    Md5Sess,
    /// `SHA-256` (RFC 7616).
    Sha256,
    /// `SHA-256-sess` (RFC 7616).
    Sha256Sess,
}

impl Algorithm {
    fn sess(self) -> bool {
        matches!(self, Self::Md5Sess | Self::Sha256Sess)
    }

    fn name(self) -> &'static str {
        match self {
            Self::Md5 => "MD5",
            Self::Md5Sess => "MD5-sess",
            Self::Sha256 => "SHA-256",
            Self::Sha256Sess => "SHA-256-sess",
        }
    }

    /// Hash esadecimale minuscolo delle parti unite da `:`.
    fn hex(self, parti: &[&str]) -> String {
        let unite = parti.join(":");
        match self {
            Self::Md5 | Self::Md5Sess => hex(&Md5::digest(unite.as_bytes())),
            Self::Sha256 | Self::Sha256Sess => {
                let mut contesto = Sha256Context::new(&SHA256);
                contesto.update(unite.as_bytes());
                hex(contesto.finish().as_ref())
            }
        }
    }
}

/// Una sfida `WWW-Authenticate: Digest …` già interpretata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Challenge {
    realm: String,
    nonce: String,
    opaque: Option<String>,
    algorithm: Algorithm,
    /// Vero se il server offre `qop=auth`; falso se non offre `qop` (RFC 2069).
    qop_auth: bool,
    /// `stale=true`: il nonce è scaduto ma le credenziali erano giuste.
    pub(crate) stale: bool,
}

impl Challenge {
    /// Interpreta il valore di un header `WWW-Authenticate` con schema `Digest`.
    pub(crate) fn parse(header: &str) -> Result<Self, FrixError> {
        let header = header.trim();
        let (schema, resto) = header
            .split_once(char::is_whitespace)
            .unwrap_or((header, ""));
        if !schema.eq_ignore_ascii_case("Digest") {
            return Err(FrixError::Transport(format!(
                "schema di autenticazione non supportato: {schema}"
            )));
        }
        let parametri = parse_params(resto)?;
        let valore = |nome: &str| {
            parametri
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(nome))
                .map(|(_, v)| v.clone())
        };
        let manca = |nome: &str| FrixError::Transport(format!("sfida Digest senza {nome}"));
        let algorithm = match valore("algorithm") {
            None => Algorithm::Md5,
            Some(a) => [
                Algorithm::Md5,
                Algorithm::Md5Sess,
                Algorithm::Sha256,
                Algorithm::Sha256Sess,
            ]
            .into_iter()
            .find(|noto| a.eq_ignore_ascii_case(noto.name()))
            .ok_or_else(|| FrixError::Transport(format!("algoritmo Digest non supportato: {a}")))?,
        };
        let qop_auth = match valore("qop") {
            None => false,
            Some(qop)
                if qop
                    .split(',')
                    .any(|q| q.trim().eq_ignore_ascii_case("auth")) =>
            {
                true
            }
            Some(qop) => {
                return Err(FrixError::Transport(format!(
                    "qop Digest non supportato: {qop}"
                )));
            }
        };
        Ok(Self {
            realm: valore("realm").ok_or_else(|| manca("realm"))?,
            nonce: valore("nonce").ok_or_else(|| manca("nonce"))?,
            opaque: valore("opaque"),
            algorithm,
            qop_auth,
            stale: valore("stale").is_some_and(|s| s.eq_ignore_ascii_case("true")),
        })
    }

    /// Valore dell'header `Authorization` per una richiesta `method uri`.
    ///
    /// `nc` è il contatore delle richieste fatte con questo nonce (da 1), `cnonce`
    /// il nonce scelto dal client. Senza `qop` nella sfida entrambi vengono ignorati.
    pub(crate) fn authorization(
        &self,
        username: &str,
        password: &SecretString,
        method: &str,
        uri: &str,
        nc: u32,
        cnonce: &str,
    ) -> Result<String, FrixError> {
        let h = |parti: &[&str]| self.algorithm.hex(parti);
        let mut ha1 = h(&[username, &self.realm, password.expose_secret()]);
        if self.algorithm.sess() {
            ha1 = h(&[&ha1, &self.nonce, cnonce]);
        }
        let ha2 = h(&[method, uri]);
        let nc = format!("{nc:08x}");
        let response = if self.qop_auth {
            h(&[&ha1, &self.nonce, &nc, cnonce, "auth", &ha2])
        } else {
            h(&[&ha1, &self.nonce, &ha2])
        };

        let mut header = format!(
            "Digest username={}, realm={}, nonce={}, uri={}, response=\"{response}\"",
            quote(username)?,
            quote(&self.realm)?,
            quote(&self.nonce)?,
            quote(uri)?,
        );
        let _ = write!(header, ", algorithm={}", self.algorithm.name());
        if self.qop_auth {
            let _ = write!(header, ", qop=auth, nc={nc}, cnonce={}", quote(cnonce)?);
        }
        if let Some(opaque) = &self.opaque {
            let _ = write!(header, ", opaque={}", quote(opaque)?);
        }
        Ok(header)
    }
}

/// Byte in esadecimale minuscolo.
fn hex(byte: &[u8]) -> String {
    byte.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// `quoted-string` di RFC 2616: virgolette e barre rovesciate escapate, niente controlli.
fn quote(valore: &str) -> Result<String, FrixError> {
    if valore.chars().any(char::is_control) {
        return Err(FrixError::Validation(
            "carattere di controllo in un parametro Digest".into(),
        ));
    }
    Ok(format!(
        "\"{}\"",
        valore.replace('\\', "\\\\").replace('"', "\\\"")
    ))
}

/// `auth-param` separati da virgole: `nome=token` o `nome="quoted string"`.
fn parse_params(testo: &str) -> Result<Vec<(String, String)>, FrixError> {
    let errore = || FrixError::Transport("sfida Digest non interpretabile".into());
    let mut parametri = Vec::new();
    let mut caratteri = testo.chars().peekable();
    loop {
        while caratteri
            .next_if(|c| c.is_whitespace() || *c == ',')
            .is_some()
        {}
        if caratteri.peek().is_none() {
            return Ok(parametri);
        }
        let mut nome = String::new();
        while let Some(c) = caratteri.next_if(|c| *c != '=' && *c != ',' && !c.is_whitespace()) {
            nome.push(c);
        }
        while caratteri.next_if(|c| c.is_whitespace()).is_some() {}
        if caratteri.next() != Some('=') || nome.is_empty() {
            return Err(errore());
        }
        while caratteri.next_if(|c| c.is_whitespace()).is_some() {}
        let mut valore = String::new();
        if caratteri.next_if_eq(&'"').is_some() {
            loop {
                match caratteri.next().ok_or_else(errore)? {
                    '"' => break,
                    '\\' => valore.push(caratteri.next().ok_or_else(errore)?),
                    c => valore.push(c),
                }
            }
        } else {
            while let Some(c) = caratteri.next_if(|c| *c != ',' && !c.is_whitespace()) {
                valore.push(c);
            }
        }
        parametri.push((nome, valore));
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use super::*;
    use secrecy::SecretString;

    /// Vettore di esempio di RFC 2617 §3.5.
    const RFC_CHALLENGE: &str = r#"Digest realm="testrealm@host.com", qop="auth,auth-int", nonce="dcd98b7102dd2f0e8b11d0f600bfb0c093", opaque="5ccc069c403ebaf9f0171e9517f40e41""#;

    #[test]
    fn risposta_del_vettore_di_esempio_di_rfc_2617() {
        let challenge = Challenge::parse(RFC_CHALLENGE).unwrap();
        let password = SecretString::from("Circle Of Life");

        let header = challenge
            .authorization("Mufasa", &password, "GET", "/dir/index.html", 1, "0a4f113b")
            .unwrap();

        assert!(
            header.contains(r#"response="6629fae49393a05397450978507c4ef1""#),
            "{header}"
        );
        for parte in [
            r#"username="Mufasa""#,
            r#"realm="testrealm@host.com""#,
            r#"nonce="dcd98b7102dd2f0e8b11d0f600bfb0c093""#,
            r#"uri="/dir/index.html""#,
            "qop=auth",
            "nc=00000001",
            r#"cnonce="0a4f113b""#,
            r#"opaque="5ccc069c403ebaf9f0171e9517f40e41""#,
        ] {
            assert!(header.contains(parte), "manca {parte} in {header}");
        }
        assert!(header.starts_with("Digest "), "{header}");
    }

    // Valori attesi calcolati a parte con `hashlib` di Python sugli stessi input.
    #[test]
    fn senza_qop_usa_la_risposta_di_rfc_2069() {
        let challenge = Challenge::parse(
            r#"Digest realm="testrealm@host.com", nonce="dcd98b7102dd2f0e8b11d0f600bfb0c093""#,
        )
        .unwrap();
        let header = challenge
            .authorization(
                "Mufasa",
                &SecretString::from("Circle Of Life"),
                "GET",
                "/dir/index.html",
                1,
                "0a4f113b",
            )
            .unwrap();

        assert!(header.contains(r#"response="670fd8c2df070c60b045671b8b24ff02""#));
        assert!(!header.contains("qop="), "{header}");
        assert!(!header.contains("cnonce="), "{header}");
    }

    #[test]
    fn md5_sess_entra_nella_risposta() {
        let challenge = Challenge::parse(
            r#"Digest realm="testrealm@host.com", nonce="dcd98b7102dd2f0e8b11d0f600bfb0c093", qop=auth, algorithm=MD5-sess"#,
        )
        .unwrap();
        let header = challenge
            .authorization(
                "Mufasa",
                &SecretString::from("Circle Of Life"),
                "GET",
                "/dir/index.html",
                1,
                "0a4f113b",
            )
            .unwrap();

        assert!(header.contains(r#"response="8e3825c57e897f5a0dec6c2d4e5059d0""#));
        assert!(header.contains("algorithm=MD5-sess"));
    }

    /// Esempio di RFC 7616 §3.9.1: stessa sfida, con `MD5` e con `SHA-256`.
    fn rfc_7616(algorithm: &str) -> String {
        let challenge = Challenge::parse(&format!(
            r#"Digest realm="http-auth@example.org", qop="auth, auth-int", algorithm={algorithm}, nonce="7ypf/xlj9XXwfDPEoM4URrv/xwf94BcCAzFZH4GiTo0v", opaque="FQhe/qaU925kfnzjCev0ciny7QMkPqMAFRtzCUYo5tdS""#
        ))
        .unwrap();
        challenge
            .authorization(
                "Mufasa",
                &SecretString::from("Circle of Life"),
                "GET",
                "/dir/index.html",
                1,
                "f2/wE4q74E6zIJEtWaHKaf5wv/H5QzzpXusqGemxURZJ",
            )
            .unwrap()
    }

    #[test]
    fn risposta_del_vettore_sha_256_di_rfc_7616() {
        let header = rfc_7616("SHA-256");

        assert!(
            header.contains(
                r#"response="753927fa0e85d155564e2e272a28d1802ca10daf4496794697cf8db5856cb6c1""#
            ),
            "{header}"
        );
        assert!(header.contains("algorithm=SHA-256"), "{header}");
    }

    #[test]
    fn risposta_del_vettore_md5_di_rfc_7616() {
        let header = rfc_7616("MD5");

        assert!(
            header.contains(r#"response="8ca523f5e9506fed4657c9700eebdbec""#),
            "{header}"
        );
    }

    #[test]
    fn rifiuta_varianti_non_supportate() {
        for sfida in [
            r#"Basic realm="x""#,
            r#"Digest realm="x", nonce="n", qop="auth-int""#,
            r#"Digest realm="x", nonce="n", algorithm=SHA-512-256"#,
            r#"Digest realm="x""#,
            r#"Digest realm="x", nonce="n"#,
        ] {
            assert!(Challenge::parse(sfida).is_err(), "{sfida}");
        }
    }

    #[test]
    fn riconosce_stale() {
        let c = Challenge::parse(r#"Digest realm="x", nonce="n", stale=TRUE"#).unwrap();
        assert!(c.stale);
    }
}

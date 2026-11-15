//! HTTP Digest (RFC 2617, RFC 7616), the authentication of TR-064 calls
//! (docs/PROJECT.md §5.1).
//!
//! The 7590's actual parameters (`qop`, `algorithm`, `opaque`) are not verified yet
//! (§14.3): supported are `qop=auth` or no `qop` (RFC 2069 compatibility), `algorithm`
//! `MD5`, `MD5-sess`, `SHA-256` or `SHA-256-sess` (RFC 7616), optional `opaque`.
//! `qop=auth-int` (needs the body hash) and other algorithms (`SHA-512-256`, …)
//! are rejected with an error that names them.

use std::fmt::Write as _;

use md5::{Digest as _, Md5};
use ring::digest::{Context as Sha256Context, SHA256};
use secrecy::{ExposeSecret as _, SecretString};

use crate::error::FrixError;

/// Algorithm of the Digest challenge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Algorithm {
    /// `MD5` (also when `algorithm` is missing).
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

    /// Lowercase hex hash of the parts joined by `:`.
    fn hex(self, parts: &[&str]) -> String {
        let joined = parts.join(":");
        match self {
            Self::Md5 | Self::Md5Sess => hex(&Md5::digest(joined.as_bytes())),
            Self::Sha256 | Self::Sha256Sess => {
                let mut context = Sha256Context::new(&SHA256);
                context.update(joined.as_bytes());
                hex(context.finish().as_ref())
            }
        }
    }
}

/// A parsed `WWW-Authenticate: Digest …` challenge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Challenge {
    realm: String,
    nonce: String,
    opaque: Option<String>,
    algorithm: Algorithm,
    /// True if the server offers `qop=auth`; false if it offers no `qop` (RFC 2069).
    qop_auth: bool,
    /// `stale=true`: the nonce expired but the credentials were right.
    pub(crate) stale: bool,
}

impl Challenge {
    /// Parses the value of a `WWW-Authenticate` header with the `Digest` scheme.
    pub(crate) fn parse(header: &str) -> Result<Self, FrixError> {
        let header = header.trim();
        let (scheme, rest) = header
            .split_once(char::is_whitespace)
            .unwrap_or((header, ""));
        if !scheme.eq_ignore_ascii_case("Digest") {
            return Err(FrixError::Transport(format!(
                "unsupported authentication scheme: {scheme}"
            )));
        }
        let params = parse_params(rest)?;
        let value = |name: &str| {
            params
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(name))
                .map(|(_, v)| v.clone())
        };
        let missing = |name: &str| FrixError::Transport(format!("Digest challenge without {name}"));
        let algorithm = match value("algorithm") {
            None => Algorithm::Md5,
            Some(a) => [
                Algorithm::Md5,
                Algorithm::Md5Sess,
                Algorithm::Sha256,
                Algorithm::Sha256Sess,
            ]
            .into_iter()
            .find(|known| a.eq_ignore_ascii_case(known.name()))
            .ok_or_else(|| FrixError::Transport(format!("unsupported Digest algorithm: {a}")))?,
        };
        let qop_auth = match value("qop") {
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
                    "unsupported Digest qop: {qop}"
                )));
            }
        };
        Ok(Self {
            realm: value("realm").ok_or_else(|| missing("realm"))?,
            nonce: value("nonce").ok_or_else(|| missing("nonce"))?,
            opaque: value("opaque"),
            algorithm,
            qop_auth,
            stale: value("stale").is_some_and(|s| s.eq_ignore_ascii_case("true")),
        })
    }

    /// Value of the `Authorization` header for a `method uri` request.
    ///
    /// `nc` is the count of requests made with this nonce (from 1), `cnonce` the
    /// nonce chosen by the client. Without `qop` in the challenge both are ignored.
    pub(crate) fn authorization(
        &self,
        username: &str,
        password: &SecretString,
        method: &str,
        uri: &str,
        nc: u32,
        cnonce: &str,
    ) -> Result<String, FrixError> {
        let h = |parts: &[&str]| self.algorithm.hex(parts);
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

/// Bytes as lowercase hex.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// RFC 2616 `quoted-string`: quotes and backslashes escaped, no control characters.
fn quote(value: &str) -> Result<String, FrixError> {
    if value.chars().any(char::is_control) {
        return Err(FrixError::Validation(
            "control character in a Digest parameter".into(),
        ));
    }
    Ok(format!(
        "\"{}\"",
        value.replace('\\', "\\\\").replace('"', "\\\"")
    ))
}

/// Comma-separated `auth-param`s: `name=token` or `name="quoted string"`.
fn parse_params(text: &str) -> Result<Vec<(String, String)>, FrixError> {
    let invalid = || FrixError::Transport("unparsable Digest challenge".into());
    let mut params = Vec::new();
    let mut chars = text.chars().peekable();
    loop {
        while chars.next_if(|c| c.is_whitespace() || *c == ',').is_some() {}
        if chars.peek().is_none() {
            return Ok(params);
        }
        let mut name = String::new();
        while let Some(c) = chars.next_if(|c| *c != '=' && *c != ',' && !c.is_whitespace()) {
            name.push(c);
        }
        while chars.next_if(|c| c.is_whitespace()).is_some() {}
        if chars.next() != Some('=') || name.is_empty() {
            return Err(invalid());
        }
        while chars.next_if(|c| c.is_whitespace()).is_some() {}
        let mut value = String::new();
        if chars.next_if_eq(&'"').is_some() {
            loop {
                match chars.next().ok_or_else(invalid)? {
                    '"' => break,
                    '\\' => value.push(chars.next().ok_or_else(invalid)?),
                    c => value.push(c),
                }
            }
        } else {
            while let Some(c) = chars.next_if(|c| *c != ',' && !c.is_whitespace()) {
                value.push(c);
            }
        }
        params.push((name, value));
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

    /// Example vector of RFC 2617 §3.5.
    const RFC_CHALLENGE: &str = r#"Digest realm="testrealm@host.com", qop="auth,auth-int", nonce="dcd98b7102dd2f0e8b11d0f600bfb0c093", opaque="5ccc069c403ebaf9f0171e9517f40e41""#;

    #[test]
    fn response_of_the_rfc_2617_example_vector() {
        let challenge = Challenge::parse(RFC_CHALLENGE).unwrap();
        let password = SecretString::from("Circle Of Life");

        let header = challenge
            .authorization("Mufasa", &password, "GET", "/dir/index.html", 1, "0a4f113b")
            .unwrap();

        assert!(
            header.contains(r#"response="6629fae49393a05397450978507c4ef1""#),
            "{header}"
        );
        for part in [
            r#"username="Mufasa""#,
            r#"realm="testrealm@host.com""#,
            r#"nonce="dcd98b7102dd2f0e8b11d0f600bfb0c093""#,
            r#"uri="/dir/index.html""#,
            "qop=auth",
            "nc=00000001",
            r#"cnonce="0a4f113b""#,
            r#"opaque="5ccc069c403ebaf9f0171e9517f40e41""#,
        ] {
            assert!(header.contains(part), "missing {part} in {header}");
        }
        assert!(header.starts_with("Digest "), "{header}");
    }

    // Expected values computed separately with Python's `hashlib` on the same inputs.
    #[test]
    fn without_qop_uses_the_rfc_2069_response() {
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
    fn md5_sess_enters_the_response() {
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

    /// Example of RFC 7616 §3.9.1: same challenge, with `MD5` and with `SHA-256`.
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
    fn response_of_the_rfc_7616_sha_256_vector() {
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
    fn response_of_the_rfc_7616_md5_vector() {
        let header = rfc_7616("MD5");

        assert!(
            header.contains(r#"response="8ca523f5e9506fed4657c9700eebdbec""#),
            "{header}"
        );
    }

    #[test]
    fn rejects_unsupported_variants() {
        for challenge in [
            r#"Basic realm="x""#,
            r#"Digest realm="x", nonce="n", qop="auth-int""#,
            r#"Digest realm="x", nonce="n", algorithm=SHA-512-256"#,
            r#"Digest realm="x""#,
            r#"Digest realm="x", nonce="n"#,
        ] {
            assert!(Challenge::parse(challenge).is_err(), "{challenge}");
        }
    }

    #[test]
    fn recognizes_stale() {
        let c = Challenge::parse(r#"Digest realm="x", nonce="n", stale=TRUE"#).unwrap();
        assert!(c.stale);
    }
}

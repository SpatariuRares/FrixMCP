//! TLS verification of the router (docs/PROJECT.md §7.4): `pin` mode (SHA-256
//! fingerprint of the certificate, ignores name and chain) or `ca` (the router's
//! certificate as the only root). There is no mode that skips verification.

use std::fmt;
use std::str::FromStr;
use std::sync::{Arc, Mutex, PoisonError};

use rustls::client::WebPkiServerVerifier;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::WebPkiSupportedAlgorithms;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{
    CertificateError, ClientConfig, DigitallySignedStruct, RootCertStore, SignatureScheme,
};

use crate::error::FrixError;

/// SHA-256 fingerprint of a certificate, written as `AB:CD:…` (32 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fingerprint([u8; 32]);

impl Fingerprint {
    /// SHA-256 fingerprint of a certificate's DER bytes.
    #[must_use]
    pub fn of_der(der: &[u8]) -> Self {
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(ring::digest::digest(&ring::digest::SHA256, der).as_ref());
        Self(bytes)
    }
}

impl FromStr for Fingerprint {
    type Err = FrixError;

    /// Accepts `AB:CD:…`, hex without separators and the output of
    /// `openssl x509 -fingerprint -sha256` (`sha256 Fingerprint=AB:CD:…`).
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let invalid = || {
            FrixError::Validation(
                "invalid SHA-256 fingerprint: 32 hex bytes required (AB:CD:…)".into(),
            )
        };
        let text = text
            .rsplit_once('=')
            .map_or(text, |(_, fingerprint)| fingerprint);
        let digits: Vec<u8> = text
            .bytes()
            .filter(|b| *b != b':' && !b.is_ascii_whitespace())
            .collect();
        if digits.len() != 64 {
            return Err(invalid());
        }
        let mut bytes = [0u8; 32];
        let (pairs, _) = digits.as_chunks::<2>();
        for (out, pair) in bytes.iter_mut().zip(pairs) {
            let hex = std::str::from_utf8(pair).map_err(|_| invalid())?;
            *out = u8::from_str_radix(hex, 16).map_err(|_| invalid())?;
        }
        Ok(Self(bytes))
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, b) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(":")?;
            }
            write!(f, "{b:02X}")?;
        }
        Ok(())
    }
}

/// Accepts the presented certificate only if its fingerprint is the expected one.
pub(crate) fn check_pin(expected: &Fingerprint, presented: Option<&[u8]>) -> Result<(), FrixError> {
    let found = presented.map(Fingerprint::of_der);
    if found.as_ref() == Some(expected) {
        return Ok(());
    }
    Err(FrixError::TlsPinMismatch {
        expected: expected.to_string(),
        found: found.map(|f| f.to_string()),
    })
}

/// How to trust the router's certificate: the only two modes that exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TlsTrust {
    /// SHA-256 fingerprint of the certificate (default): ignores name and chain.
    Pin(Fingerprint),
    /// The router's certificate (DER) as the only root; the verified name is the configured host.
    Ca(Vec<u8>),
}

/// The last fingerprint error seen during a handshake: the HTTP client reports it
/// as a generic I/O error, the transport retrieves it from here.
pub(crate) type PinFailure = Arc<Mutex<Option<FrixError>>>;

/// rustls `ClientConfig` for the chosen mode, plus the fingerprint error slot.
pub(crate) fn client_config(
    trust: &TlsTrust,
) -> Result<(Arc<ClientConfig>, PinFailure), FrixError> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let failure = PinFailure::default();
    let builder = ClientConfig::builder_with_provider(Arc::clone(&provider))
        .with_safe_default_protocol_versions()
        .map_err(|e| FrixError::Tls(e.to_string()))?;
    let config = match trust {
        TlsTrust::Pin(expected) => builder
            // `dangerous` is the rustls API name for a custom verifier: here we verify
            // more, not less (exact fingerprint + handshake signatures).
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(PinVerifier {
                expected: *expected,
                algorithms: provider.signature_verification_algorithms,
                failure: Arc::clone(&failure),
            }))
            .with_no_client_auth(),
        TlsTrust::Ca(der) => {
            let mut roots = RootCertStore::empty();
            roots
                .add(CertificateDer::from(der.clone()))
                .map_err(|e| FrixError::Tls(format!("invalid CA certificate: {e}")))?;
            let verifier = WebPkiServerVerifier::builder_with_provider(Arc::new(roots), provider)
                .build()
                .map_err(|e| FrixError::Tls(e.to_string()))?;
            builder.with_webpki_verifier(verifier).with_no_client_auth()
        }
    };
    Ok((Arc::new(config), failure))
}

/// Verifier of `pin` mode: exact fingerprint of the leaf certificate; handshake
/// signatures are still verified with that certificate's key.
#[derive(Debug)]
struct PinVerifier {
    expected: Fingerprint,
    algorithms: WebPkiSupportedAlgorithms,
    failure: PinFailure,
}

impl ServerCertVerifier for PinVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        match check_pin(&self.expected, Some(end_entity.as_ref())) {
            Ok(()) => Ok(ServerCertVerified::assertion()),
            Err(error) => {
                *self.failure.lock().unwrap_or_else(PoisonError::into_inner) = Some(error);
                Err(rustls::Error::InvalidCertificate(
                    CertificateError::ApplicationVerificationFailure,
                ))
            }
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algorithms.supported_schemes()
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

    const CERT: &[u8] = b"any DER certificate";
    /// SHA-256 of `CERT`, computed separately with `shasum -a 256`.
    const CERT_FINGERPRINT: &str = "0A:81:1D:9A:F0:A0:3D:EC:8E:FF:CD:54:E4:E6:33:D2:B0:68:D7:24:92:AA:6A:55:2B:96:63:A7:89:17:8E:27";

    #[test]
    fn right_fingerprint_accepted() {
        let expected: Fingerprint = CERT_FINGERPRINT.parse().unwrap();
        assert_eq!(check_pin(&expected, Some(CERT)), Ok(()));
    }

    #[test]
    fn wrong_fingerprint_rejected_with_expected_and_found() {
        let expected: Fingerprint = "00".repeat(32).parse().unwrap();
        let error = check_pin(&expected, Some(CERT)).unwrap_err();

        assert_eq!(
            error,
            FrixError::TlsPinMismatch {
                expected: ["00"; 32].join(":"),
                found: Some(CERT_FINGERPRINT.to_owned()),
            }
        );
        let message = error.to_string();
        assert!(message.contains(&["00"; 32].join(":")), "{message}");
        assert!(message.contains(CERT_FINGERPRINT), "{message}");
    }

    #[test]
    fn missing_certificate_rejected() {
        let expected: Fingerprint = CERT_FINGERPRINT.parse().unwrap();
        let error = check_pin(&expected, None).unwrap_err();

        assert_eq!(
            error,
            FrixError::TlsPinMismatch {
                expected: CERT_FINGERPRINT.to_owned(),
                found: None,
            }
        );
    }

    #[test]
    fn fingerprint_accepts_openssl_format_and_hex_without_separators() {
        let with_colons: Fingerprint = CERT_FINGERPRINT.parse().unwrap();
        let without: Fingerprint = CERT_FINGERPRINT
            .replace(':', "")
            .to_lowercase()
            .parse()
            .unwrap();
        let openssl: Fingerprint = format!("sha256 Fingerprint={CERT_FINGERPRINT}")
            .parse()
            .unwrap();

        assert_eq!(with_colons, without);
        assert_eq!(with_colons, openssl);
        assert_eq!(with_colons.to_string(), CERT_FINGERPRINT);
    }

    #[test]
    fn missing_or_malformed_fingerprint_rejected() {
        for text in ["", "AB:CD", &"GG".repeat(32), &"00".repeat(33)] {
            assert!(text.parse::<Fingerprint>().is_err(), "{text:?}");
        }
    }
}

//! Verifica TLS verso il router (docs/PROJECT.md §7.4): modalità `pin` (impronta
//! SHA-256 del certificato, ignora nome e catena) o `ca` (il certificato del router
//! come unica root). Non esiste una modalità che non verifichi.

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

/// Impronta SHA-256 di un certificato, scritta come `AB:CD:…` (32 byte).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fingerprint([u8; 32]);

impl Fingerprint {
    /// Impronta SHA-256 dei byte DER di un certificato.
    #[must_use]
    pub fn of_der(der: &[u8]) -> Self {
        let mut byte = [0u8; 32];
        byte.copy_from_slice(ring::digest::digest(&ring::digest::SHA256, der).as_ref());
        Self(byte)
    }
}

impl FromStr for Fingerprint {
    type Err = FrixError;

    /// Accetta `AB:CD:…`, l'esadecimale senza separatori e l'output di
    /// `openssl x509 -fingerprint -sha256` (`sha256 Fingerprint=AB:CD:…`).
    fn from_str(testo: &str) -> Result<Self, Self::Err> {
        let errore = || {
            FrixError::Validation(
                "impronta SHA-256 non valida: servono 32 byte esadecimali (AB:CD:…)".into(),
            )
        };
        let testo = testo
            .rsplit_once('=')
            .map_or(testo, |(_, impronta)| impronta);
        let cifre: Vec<u8> = testo
            .bytes()
            .filter(|b| *b != b':' && !b.is_ascii_whitespace())
            .collect();
        if cifre.len() != 64 {
            return Err(errore());
        }
        let mut byte = [0u8; 32];
        let (gruppi, _) = cifre.as_chunks::<2>();
        for (uscita, gruppo) in byte.iter_mut().zip(gruppi) {
            let testo = std::str::from_utf8(gruppo).map_err(|_| errore())?;
            *uscita = u8::from_str_radix(testo, 16).map_err(|_| errore())?;
        }
        Ok(Self(byte))
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

/// Accetta il certificato presentato solo se la sua impronta è quella attesa.
pub(crate) fn check_pin(expected: &Fingerprint, presented: Option<&[u8]>) -> Result<(), FrixError> {
    let found = presented.map(Fingerprint::of_der);
    if found.as_ref() == Some(expected) {
        return Ok(());
    }
    Err(FrixError::TlsPinMismatch {
        expected: expected.to_string(),
        found: found.map_or_else(|| "nessun certificato".to_owned(), |f| f.to_string()),
    })
}

/// Come fidarsi del certificato del router: le sole due modalità esistenti.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TlsTrust {
    /// Impronta SHA-256 del certificato (default): ignora nome e catena.
    Pin(Fingerprint),
    /// Certificato del router (DER) come unica root; il nome verificato è l'host configurato.
    Ca(Vec<u8>),
}

/// L'ultimo errore di impronta visto durante un handshake: il client HTTP lo
/// riporta come errore di I/O generico, il trasporto lo recupera da qui.
pub(crate) type PinFailure = Arc<Mutex<Option<FrixError>>>;

/// `ClientConfig` di rustls per la modalità scelta, più il registro degli errori di impronta.
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
            // `dangerous` è il nome dell'API di rustls per un verificatore proprio:
            // qui si verifica di più, non di meno (impronta esatta + firme dell'handshake).
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
                .map_err(|e| FrixError::Tls(format!("certificato CA non valido: {e}")))?;
            let verifier = WebPkiServerVerifier::builder_with_provider(Arc::new(roots), provider)
                .build()
                .map_err(|e| FrixError::Tls(e.to_string()))?;
            builder.with_webpki_verifier(verifier).with_no_client_auth()
        }
    };
    Ok((Arc::new(config), failure))
}

/// Verificatore della modalità `pin`: impronta esatta del certificato foglia; le firme
/// dell'handshake si verificano comunque con la chiave di quel certificato.
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
            Err(errore) => {
                *self.failure.lock().unwrap_or_else(PoisonError::into_inner) = Some(errore);
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

    const CERT: &[u8] = b"un certificato DER qualunque";
    /// SHA-256 di `CERT`, calcolato a parte con `shasum -a 256`.
    const IMPRONTA_CERT: &str = "1E:FF:F5:11:EF:CC:64:DD:85:A2:E8:FB:BA:FA:A8:10:E9:97:E9:B3:88:FE:2A:41:50:E8:2A:32:1B:51:17:C8";

    #[test]
    fn impronta_giusta_accettata() {
        let atteso: Fingerprint = IMPRONTA_CERT.parse().unwrap();
        assert_eq!(check_pin(&atteso, Some(CERT)), Ok(()));
    }

    #[test]
    fn impronta_sbagliata_rifiutata_con_attesa_e_trovata() {
        let atteso: Fingerprint = "00".repeat(32).parse().unwrap();
        let errore = check_pin(&atteso, Some(CERT)).unwrap_err();

        assert_eq!(
            errore,
            FrixError::TlsPinMismatch {
                expected: ["00"; 32].join(":"),
                found: IMPRONTA_CERT.to_owned(),
            }
        );
        let messaggio = errore.to_string();
        assert!(messaggio.contains(&["00"; 32].join(":")), "{messaggio}");
        assert!(messaggio.contains(IMPRONTA_CERT), "{messaggio}");
    }

    #[test]
    fn certificato_assente_rifiutato() {
        let atteso: Fingerprint = IMPRONTA_CERT.parse().unwrap();
        let errore = check_pin(&atteso, None).unwrap_err();

        assert_eq!(
            errore,
            FrixError::TlsPinMismatch {
                expected: IMPRONTA_CERT.to_owned(),
                found: "nessun certificato".to_owned(),
            }
        );
    }

    #[test]
    fn impronta_accetta_il_formato_di_openssl_e_l_esadecimale_senza_separatori() {
        let con_due_punti: Fingerprint = IMPRONTA_CERT.parse().unwrap();
        let senza: Fingerprint = IMPRONTA_CERT
            .replace(':', "")
            .to_lowercase()
            .parse()
            .unwrap();
        let openssl: Fingerprint = format!("sha256 Fingerprint={IMPRONTA_CERT}")
            .parse()
            .unwrap();

        assert_eq!(con_due_punti, senza);
        assert_eq!(con_due_punti, openssl);
        assert_eq!(con_due_punti.to_string(), IMPRONTA_CERT);
    }

    #[test]
    fn impronta_assente_o_malformata_rifiutata() {
        for testo in ["", "AB:CD", &"GG".repeat(32), &"00".repeat(33)] {
            assert!(testo.parse::<Fingerprint>().is_err(), "{testo:?}");
        }
    }
}

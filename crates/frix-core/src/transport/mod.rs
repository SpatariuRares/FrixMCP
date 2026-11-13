//! `HttpTransport`: l'unico seam fra `frix-core` e il router (docs/PROJECT.md §6.2).
//!
//! Due implementazioni: [`RouterTransport`] (TLS pinnato e Digest) e
//! [`FixtureTransport`] per i test, che risponde da file senza rete.

mod digest;
mod fixture;
mod router;
mod tls;

use std::future::Future;
use std::pin::Pin;

pub use fixture::{FixtureTransport, RequestLog};
pub use router::{Credentials, DEFAULT_HTTP_PORT, MAX_RETRIES, RouterSettings, RouterTransport};
pub use tls::{Fingerprint, TlsTrust};

use crate::error::FrixError;

/// Future restituito da [`HttpTransport::send`].
pub type SendFuture<'a> =
    Pin<Box<dyn Future<Output = Result<HttpResponse, FrixError>> + Send + 'a>>;

/// Metodo HTTP di una richiesta verso il router.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// Descrizioni (`tr64desc.xml`, SCPD) e liste `*ListPath`.
    Get,
    /// Chiamate SOAP.
    Post,
}

/// Canale su cui il trasporto reale invia la richiesta (docs/PROJECT.md §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    /// HTTP sulla porta 49000, senza autenticazione: solo descrizioni, SCPD e `GetSecurityPort`.
    Plain,
    /// HTTPS sulla porta di `GetSecurityPort`, con TLS pinnato e Digest.
    Secure,
}

/// Richiesta verso il router, indipendente dall'host e dalla porta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    /// Metodo HTTP.
    pub method: Method,
    /// Canale (HTTP in chiaro o HTTPS autenticato).
    pub channel: Channel,
    /// Path assoluto sul router, per esempio `/upnp/control/deviceinfo`.
    pub path: String,
    /// Valore dell'header `SOAPACTION` senza virgolette (`<serviceType>#<azione>`), solo per SOAP.
    pub soap_action: Option<String>,
    /// Corpo della richiesta (vuoto per `GET`).
    pub body: String,
}

impl HttpRequest {
    /// Servizio con istanza (`DeviceInfo:1`) e azione dell'header `SOAPACTION`, se la
    /// richiesta è una chiamata SOAP TR-064 (`urn:dslforum-org:service:<servizio>#<azione>`).
    #[must_use]
    pub fn soap_service_action(&self) -> Option<(&str, &str)> {
        self.soap_action
            .as_deref()?
            .strip_prefix(crate::tr064::SERVICE_URN_PREFIX)?
            .split_once('#')
    }
}

/// Risposta del router.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    /// Stato HTTP.
    pub status: u16,
    /// Corpo della risposta.
    pub body: String,
}

impl HttpResponse {
    /// Risposta `200 OK` con il corpo dato.
    #[must_use]
    pub fn ok(body: impl Into<String>) -> Self {
        Self {
            status: 200,
            body: body.into(),
        }
    }
}

/// Un'operazione asincrona richiesta → risposta verso il router.
pub trait HttpTransport: std::fmt::Debug + Send + Sync {
    /// Invia la richiesta e restituisce la risposta, qualunque sia lo stato HTTP.
    fn send(&self, request: HttpRequest) -> SendFuture<'_>;
}

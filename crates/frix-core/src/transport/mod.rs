//! `HttpTransport`: the only seam between `frix-core` and the router (docs/PROJECT.md §6.2).
//!
//! Two implementations: [`RouterTransport`] (pinned TLS and Digest) and
//! [`FixtureTransport`] for tests, which answers from files without network.

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

/// Future returned by [`HttpTransport::send`].
pub type SendFuture<'a> =
    Pin<Box<dyn Future<Output = Result<HttpResponse, FrixError>> + Send + 'a>>;

/// HTTP method of a request to the router.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// Descriptions (`tr64desc.xml`, SCPD) and `*ListPath` lists.
    Get,
    /// SOAP calls.
    Post,
}

/// Channel the real transport sends the request on (docs/PROJECT.md §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    /// HTTP on port 49000, unauthenticated: only descriptions, SCPDs and `GetSecurityPort`.
    Plain,
    /// HTTPS on the `GetSecurityPort` port, with pinned TLS and Digest.
    Secure,
}

/// Request to the router, independent of host and port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    /// HTTP method.
    pub method: Method,
    /// Channel (plain HTTP or authenticated HTTPS).
    pub channel: Channel,
    /// Absolute path on the router, for example `/upnp/control/deviceinfo`.
    pub path: String,
    /// Value of the `SOAPACTION` header without quotes (`<serviceType>#<action>`), SOAP only.
    pub soap_action: Option<String>,
    /// Request body (empty for `GET`).
    pub body: String,
}

impl HttpRequest {
    /// Service with instance (`DeviceInfo:1`) and action of the `SOAPACTION` header, if the
    /// request is a TR-064 SOAP call (`urn:dslforum-org:service:<service>#<action>`).
    #[must_use]
    pub fn soap_service_action(&self) -> Option<(&str, &str)> {
        self.soap_action
            .as_deref()?
            .strip_prefix(crate::tr064::SERVICE_URN_PREFIX)?
            .split_once('#')
    }
}

/// Router response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    /// HTTP status.
    pub status: u16,
    /// Response body.
    pub body: String,
}

impl HttpResponse {
    /// `200 OK` response with the given body.
    #[must_use]
    pub fn ok(body: impl Into<String>) -> Self {
        Self {
            status: 200,
            body: body.into(),
        }
    }
}

/// An asynchronous request → response operation against the router.
pub trait HttpTransport: std::fmt::Debug + Send + Sync {
    /// Sends the request and returns the response, whatever the HTTP status.
    fn send(&self, request: HttpRequest) -> SendFuture<'_>;
}

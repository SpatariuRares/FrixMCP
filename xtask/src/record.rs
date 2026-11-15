//! Recording of the router's responses through the `HttpTransport` seam.
//!
//! [`RecordingTransport`] wraps the transport (real or fixture-backed) and keeps every
//! request/response pair. The password never shows up: the `Authorization` header
//! is added by `RouterTransport` internally, below this seam.
//!
//! Defence in depth: besides the Policy of `Tr064Client::call`, the recorder
//! itself rejects every SOAP `POST` that the registry does not classify `Exposed(Read)`.

use std::sync::{Arc, Mutex, PoisonError};

use frix_core::FrixError;
use frix_core::policy::{Classification, Level, classification};
use frix_core::services::{device_info, lan_config_security};
use frix_core::tr064::Tr064Client;
use frix_core::transport::{
    Credentials, HttpRequest, HttpResponse, HttpTransport, Method, RouterTransport, SendFuture,
};

use crate::reads::{self, Outcome};

/// A request to the router and its response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exchange {
    /// Request sent (without credentials).
    pub request: HttpRequest,
    /// Response received.
    pub response: HttpResponse,
}

/// Transport that records every exchange and lets through only reads.
#[derive(Debug)]
pub struct RecordingTransport {
    inner: Arc<dyn HttpTransport>,
    exchanges: Arc<Mutex<Vec<Exchange>>>,
}

impl RecordingTransport {
    /// Wraps `inner`.
    #[must_use]
    pub fn new(inner: Arc<dyn HttpTransport>) -> Self {
        Self {
            inner,
            exchanges: Arc::default(),
        }
    }

    /// Copy of the exchanges recorded so far, in order.
    #[must_use]
    pub fn exchanges(&self) -> Vec<Exchange> {
        self.exchanges
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// Allows a `POST` only if the SOAP action is classified `Exposed(Read)`.
fn reads_only(request: &HttpRequest) -> Result<(), FrixError> {
    if request.method == Method::Get {
        return Ok(());
    }
    let (service, action) = request.soap_service_action().ok_or_else(|| {
        FrixError::Policy(format!(
            "POST without a recognisable SOAP action: {}",
            request.soap_action.as_deref().unwrap_or_default()
        ))
    })?;
    match classification(service, action) {
        Some(Classification::Exposed(Level::Read)) => Ok(()),
        other => Err(FrixError::Policy(format!(
            "{service}#{action} is not a read ({other:?}): the recorder rejects it"
        ))),
    }
}

impl HttpTransport for RecordingTransport {
    fn send(&self, request: HttpRequest) -> SendFuture<'_> {
        Box::pin(async move {
            reads_only(&request)?;
            let response = self.inner.send(request.clone()).await?;
            self.exchanges
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(Exchange {
                    request,
                    response: response.clone(),
                });
            Ok(response)
        })
    }
}

/// Result of the command: recorded exchanges and outcomes of the reads.
#[derive(Debug, Clone)]
pub struct Recording {
    /// Every request and response, in order (descriptions, SCPD, SOAP, lists).
    pub exchanges: Vec<Exchange>,
    /// One outcome per entry of [`reads::READS`].
    pub outcomes: Vec<Outcome>,
    /// Names of the tools exposed by the server.
    pub server_tools: Vec<String>,
}

/// Records from the real router: config and flags as for `frix-mcp` (`args`, for example
/// `["--config", "…"]`), Router Account password from the Keychain, transport with pinned
/// TLS and Digest. Used by `cargo xtask record` and by the live test; never by the normal tests.
pub async fn record_from_router(args: Vec<String>) -> Result<Recording, FrixError> {
    let cli = frix_mcp::cli::parse(args)?;
    let config = frix_mcp::cli::load(&cli)?;
    let password = frix_core::secrets::keychain_password(&config.username)?;
    let transport = Arc::new(RouterTransport::new(
        config.settings,
        Credentials {
            username: config.username,
            password,
        },
    )?);
    let port = Arc::clone(&transport);
    record(transport, move |p| port.set_secure_port(p)).await
}

/// Discovery, HTTPS port, rights (as at `frix-mcp` startup), then all the
/// [`reads::READS`], recording every exchange with `inner`.
///
/// `secure_port` receives the port from `GetSecurityPort`: with the real transport
/// it is `RouterTransport::set_secure_port`; with fixtures it is not needed.
pub async fn record(
    inner: Arc<dyn HttpTransport>,
    secure_port: impl FnOnce(u16),
) -> Result<Recording, FrixError> {
    let recorder = Arc::new(RecordingTransport::new(inner));
    let client = Tr064Client::discover(Arc::clone(&recorder) as _).await?;
    secure_port(device_info::security_port(&client).await?);
    lan_config_security::current_user(&client).await?;
    let reads = reads::run(client).await?;
    Ok(Recording {
        exchanges: recorder.exchanges(),
        outcomes: reads.outcomes,
        server_tools: reads.server_tools,
    })
}

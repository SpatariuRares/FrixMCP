//! `RouterTransport`: the real transport to the FRITZ!Box (docs/PROJECT.md §5.1, §7.4).
//!
//! - [`Channel::Plain`]: HTTP on port 49000, no credentials (descriptions, SCPDs,
//!   `GetSecurityPort`).
//! - [`Channel::Secure`]: HTTPS on the port read from `GetSecurityPort`, TLS verified
//!   as set by [`TlsTrust`], and HTTP Digest. Credentials never travel in plain text.
//!
//! Against router lockouts (threat T8, §7.2): the Digest nonce is reused, a rejected
//! password is not retried while the process lives, and a 503 is retried only with
//! backoff and at most [`MAX_RETRIES`] times.
//!
//! `BlockTime` is deliberately not handled: it is a field of the `login_sid.lua`
//! response (SID login of the web UI, §5.2), which TR-064 with Digest does not use.
//! Whether and how the 7590 signals a block after failed Digests (401 without challenge,
//! 503, `Retry-After`, …) is to be verified on the router (§14.3): until then, no
//! invented behavior beyond not retrying after a rejection.

use std::fmt::{self, Write as _};
use std::net::IpAddr;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Duration;

use rustls::pki_types::ServerName;
use secrecy::SecretString;
use ureq::unversioned::resolver::DefaultResolver;
use ureq::unversioned::transport::{Connector, TcpConnector};

use super::digest::Challenge;
use super::tls::{self, PinFailure, TlsTrust};
use super::{Channel, HttpRequest, HttpResponse, HttpTransport, Method, SendFuture};
use crate::error::FrixError;

mod connector;

/// FRITZ!Box HTTP port for descriptions and SCPDs.
pub const DEFAULT_HTTP_PORT: u16 = 49000;

/// Maximum extra attempts after a 503.
pub const MAX_RETRIES: u32 = 3;

/// Maximum wait accepted from a `Retry-After`: beyond it, give up instead of waiting.
const MAX_RETRY_AFTER: Duration = Duration::from_secs(60);

/// Global timeout of a request.
const TIMEOUT: Duration = Duration::from_secs(20);

/// Router Account and password. The password is a `SecretString`: `Debug` redacts it.
#[derive(Debug, Clone)]
pub struct Credentials {
    /// Name of the Router Account (for example `frixmcp`).
    pub username: String,
    /// Password, read from the Keychain.
    pub password: SecretString,
}

/// Where the router is and how its certificate is verified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouterSettings {
    /// IP or name of the router (for example `192.168.178.1`).
    pub host: String,
    /// Plain HTTP port (normally [`DEFAULT_HTTP_PORT`]).
    pub http_port: u16,
    /// TLS verification mode.
    pub tls: TlsTrust,
}

/// Real transport: HTTP/1.1 with `ureq` in `spawn_blocking`, TLS with rustls.
pub struct RouterTransport {
    inner: Arc<Inner>,
}

struct Inner {
    host: String,
    http_port: u16,
    secure_port: OnceLock<u16>,
    credentials: Credentials,
    agent: ureq::Agent,
    pin_failure: PinFailure,
    digest: Mutex<DigestState>,
}

#[derive(Default)]
struct DigestState {
    /// Last challenge received and requests already made with its nonce.
    challenge: Option<(Challenge, u32)>,
    /// The router rejected the credentials: no retry.
    rejected: bool,
}

impl fmt::Debug for RouterTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RouterTransport")
            .field("host", &self.inner.host)
            .field("http_port", &self.inner.http_port)
            .field("secure_port", &self.inner.secure_port.get())
            .field("credentials", &self.inner.credentials)
            .finish_non_exhaustive()
    }
}

impl RouterTransport {
    /// Prepares the transport. Opens no connections.
    pub fn new(settings: RouterSettings, credentials: Credentials) -> Result<Self, FrixError> {
        let (tls_config, pin_failure) = tls::client_config(&settings.tls)?;
        let server_name = server_name(&settings.host)?;
        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .max_redirects(0)
            .timeout_global(Some(TIMEOUT))
            .build();
        let connector =
            TcpConnector::default().chain(connector::RustlsConnector::new(tls_config, server_name));
        let agent = ureq::Agent::with_parts(config, connector, DefaultResolver::default());
        Ok(Self {
            inner: Arc::new(Inner {
                host: settings.host,
                http_port: settings.http_port,
                secure_port: OnceLock::new(),
                credentials,
                agent,
                pin_failure,
                digest: Mutex::new(DigestState::default()),
            }),
        })
    }

    /// HTTPS port read with `DeviceInfo:GetSecurityPort`. Until it is known, every
    /// request on [`Channel::Secure`] fails without traffic. Set only once.
    pub fn set_secure_port(&self, port: u16) {
        let _ = self.inner.secure_port.set(port);
    }
}

impl HttpTransport for RouterTransport {
    fn send(&self, request: HttpRequest) -> SendFuture<'_> {
        let inner = Arc::clone(&self.inner);
        Box::pin(async move {
            tokio::task::spawn_blocking(move || inner.send(&request))
                .await
                .map_err(|e| FrixError::Transport(format!("request interrupted: {e}")))?
        })
    }
}

/// Response with the headers the transport needs.
struct Raw {
    status: u16,
    challenges: Vec<String>,
    retry_after: Option<String>,
    body: String,
}

impl Inner {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, FrixError> {
        let url = self.url(request)?;
        let mut attempts_503 = 0;
        let mut challenges_seen = 0;
        loop {
            let authorization = match request.channel {
                Channel::Plain => None,
                Channel::Secure => self.authorization(request)?,
            };
            let raw = self.execute(
                request,
                &url,
                authorization.as_ref().map(|(header, _)| header.as_str()),
            )?;

            if raw.status == 401
                && request.channel == Channel::Secure
                && self.accept_challenge(&raw, authorization.as_ref().map(|(_, nc)| *nc))?
                && challenges_seen == 0
            {
                challenges_seen += 1;
                continue;
            }
            if raw.status == 503 {
                match retry_delay(attempts_503, raw.retry_after.as_deref()) {
                    Some(delay) => {
                        attempts_503 += 1;
                        std::thread::sleep(delay);
                        continue;
                    }
                    None => {
                        return Err(FrixError::Transport(
                            "the router refuses requests (503): try again later".into(),
                        ));
                    }
                }
            }
            return Ok(HttpResponse {
                status: raw.status,
                body: raw.body,
            });
        }
    }

    fn url(&self, request: &HttpRequest) -> Result<String, FrixError> {
        if !request.path.starts_with('/') {
            return Err(FrixError::Validation(format!(
                "path is not absolute: {}",
                request.path
            )));
        }
        let host = match self.host.parse::<IpAddr>() {
            Ok(IpAddr::V6(ip)) => format!("[{ip}]"),
            _ => self.host.clone(),
        };
        Ok(match request.channel {
            Channel::Plain => format!("http://{host}:{}{}", self.http_port, request.path),
            Channel::Secure => {
                let port = self.secure_port.get().ok_or_else(|| {
                    FrixError::Transport("router HTTPS port not known yet (GetSecurityPort)".into())
                })?;
                format!("https://{host}:{port}{}", request.path)
            }
        })
    }

    /// `Authorization` header with the last known challenge and its `nc`, or `None` if
    /// there is no challenge yet. After a credentials rejection there is no retry.
    fn authorization(&self, request: &HttpRequest) -> Result<Option<(String, u32)>, FrixError> {
        let mut state = self.digest.lock().unwrap_or_else(PoisonError::into_inner);
        if state.rejected {
            return Err(FrixError::Unauthenticated);
        }
        let Some((challenge, nc)) = state.challenge.as_mut() else {
            return Ok(None);
        };
        *nc += 1;
        let method = match request.method {
            Method::Get => "GET",
            Method::Post => "POST",
        };
        challenge
            .authorization(
                &self.credentials.username,
                &self.credentials.password,
                method,
                &request.path,
                *nc,
                &cnonce()?,
            )
            .map(|header| Some((header, *nc)))
    }

    /// Records the challenge of a 401 and says whether to repeat the request (once only).
    ///
    /// `nc_sent` is the counter of the request just made with credentials, if any.
    /// A 401 on the first request with a fresh nonce (`nc` 1) and without `stale=true`
    /// means credentials rejected: no attempts from then on. With a reused nonce
    /// (`nc` > 1) the 401 only means an expired nonce.
    fn accept_challenge(&self, raw: &Raw, nc_sent: Option<u32>) -> Result<bool, FrixError> {
        let challenge = raw.challenges.iter().find_map(|h| Challenge::parse(h).ok());
        let mut state = self.digest.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(challenge) = challenge else {
            if raw.challenges.is_empty() {
                return Ok(false);
            }
            return Err(FrixError::Transport(
                "the router asks for an authentication other than HTTP Digest".into(),
            ));
        };
        if nc_sent == Some(1) && !challenge.stale {
            state.rejected = true;
            state.challenge = None;
            return Ok(false);
        }
        state.challenge = Some((challenge, 0));
        Ok(true)
    }

    fn execute(
        &self,
        request: &HttpRequest,
        url: &str,
        authorization: Option<&str>,
    ) -> Result<Raw, FrixError> {
        let result = match request.method {
            Method::Get => {
                let mut r = self.agent.get(url);
                if let Some(a) = authorization {
                    r = r.header("Authorization", a);
                }
                r.call()
            }
            Method::Post => {
                let mut r = self
                    .agent
                    .post(url)
                    .header("Content-Type", r#"text/xml; charset="utf-8""#);
                if let Some(action) = &request.soap_action {
                    r = r.header("SOAPACTION", format!("\"{action}\""));
                }
                if let Some(a) = authorization {
                    r = r.header("Authorization", a);
                }
                r.send(request.body.as_str())
            }
        };
        let mut response = result.map_err(|e| self.transport_error(&e))?;
        let headers = response.headers();
        let challenges = headers
            .get_all("www-authenticate")
            .iter()
            .filter_map(|v| v.to_str().ok().map(str::to_owned))
            .collect();
        let retry_after = headers
            .get("retry-after")
            .and_then(|v| v.to_str().ok().map(str::to_owned));
        let status = response.status().as_u16();
        let body = response
            .body_mut()
            .read_to_string()
            .map_err(|e| self.transport_error(&e))?;
        Ok(Raw {
            status,
            challenges,
            retry_after,
            body,
        })
    }

    /// A wrong fingerprint comes from ureq as an I/O error: here it becomes `TlsPinMismatch` again.
    fn transport_error(&self, error: &ureq::Error) -> FrixError {
        if let Some(pin) = self
            .pin_failure
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            return pin;
        }
        match error {
            ureq::Error::Io(e)
                if e.get_ref()
                    .is_some_and(<dyn std::error::Error + Send + Sync>::is::<rustls::Error>) =>
            {
                FrixError::Tls(e.to_string())
            }
            e => FrixError::Transport(e.to_string()),
        }
    }
}

/// Wait before attempt `attempt + 1` after a 503, or `None` to give up.
///
/// Honors `Retry-After` (seconds) if present; otherwise 1, 2, 4 s. Beyond
/// [`MAX_RETRIES`] attempts, or if the router asks for more than a minute, give up.
pub(crate) fn retry_delay(attempt: u32, retry_after: Option<&str>) -> Option<Duration> {
    if attempt >= MAX_RETRIES {
        return None;
    }
    let delay = match retry_after.and_then(|v| v.trim().parse::<u64>().ok()) {
        Some(seconds) => Duration::from_secs(seconds),
        None => Duration::from_secs(1u64 << attempt),
    };
    (delay <= MAX_RETRY_AFTER).then_some(delay)
}

fn server_name(host: &str) -> Result<ServerName<'static>, FrixError> {
    ServerName::try_from(host.to_owned())
        .map_err(|_| FrixError::Validation(format!("invalid host: {host}")))
}

fn cnonce() -> Result<String, FrixError> {
    use ring::rand::SecureRandom as _;
    let mut bytes = [0u8; 8];
    ring::rand::SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| FrixError::Transport("random generator unavailable".into()))?;
    Ok(bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    }))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn exponential_backoff_without_retry_after() {
        let delays: Vec<_> = (0..4).map(|t| retry_delay(t, None)).collect();
        assert_eq!(
            delays,
            vec![
                Some(Duration::from_secs(1)),
                Some(Duration::from_secs(2)),
                Some(Duration::from_secs(4)),
                None
            ]
        );
    }

    #[test]
    fn retry_after_honored_and_capped() {
        assert_eq!(retry_delay(0, Some("10")), Some(Duration::from_secs(10)));
        assert_eq!(retry_delay(0, Some("600")), None, "long block: give up");
        assert_eq!(retry_delay(3, Some("1")), None);
    }
}

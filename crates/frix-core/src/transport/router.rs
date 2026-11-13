//! `RouterTransport`: il trasporto reale verso il FRITZ!Box (docs/PROJECT.md §5.1, §7.4).
//!
//! - [`Channel::Plain`]: HTTP sulla porta 49000, senza credenziali (descrizioni, SCPD,
//!   `GetSecurityPort`).
//! - [`Channel::Secure`]: HTTPS sulla porta letta da `GetSecurityPort`, TLS verificato
//!   come da [`TlsTrust`] e HTTP Digest. Le credenziali non viaggiano mai in chiaro.
//!
//! Contro i blocchi del router (minaccia T8, §7.2): il nonce Digest si riusa, una
//! password rifiutata non si ritenta finché il processo vive, e un 503 si ripete solo
//! con backoff e al massimo [`MAX_RETRIES`] volte.
//!
//! `BlockTime` non è gestito di proposito: è un campo della risposta di
//! `login_sid.lua` (login a SID della UI web, §5.2), che il TR-064 con Digest non usa.
//! Se e come il 7590 segnali un blocco dopo Digest falliti (401 senza sfida, 503,
//! `Retry-After`, …) è da verificare sul router (§14.3): finché non lo si sa, nessun
//! comportamento inventato oltre al non ritentare dopo un rifiuto.

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

/// Porta HTTP del FRITZ!Box per descrizioni e SCPD.
pub const DEFAULT_HTTP_PORT: u16 = 49000;

/// Tentativi aggiuntivi al massimo dopo un 503.
pub const MAX_RETRIES: u32 = 3;

/// Attesa massima accettata da un `Retry-After`: oltre, si rinuncia invece di aspettare.
const MAX_RETRY_AFTER: Duration = Duration::from_secs(60);

/// Timeout globale di una richiesta.
const TIMEOUT: Duration = Duration::from_secs(20);

/// Router Account e password. La password è un `SecretString`: `Debug` la oscura.
#[derive(Debug, Clone)]
pub struct Credentials {
    /// Nome del Router Account (per esempio `frixmcp`).
    pub username: String,
    /// Password, letta dal Keychain.
    pub password: SecretString,
}

/// Dove sta il router e come si verifica il suo certificato.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouterSettings {
    /// IP o nome del router (per esempio `192.168.178.1`).
    pub host: String,
    /// Porta HTTP in chiaro (di norma [`DEFAULT_HTTP_PORT`]).
    pub http_port: u16,
    /// Modalità di verifica TLS.
    pub tls: TlsTrust,
}

/// Trasporto reale: HTTP/1.1 con `ureq` in `spawn_blocking`, TLS con rustls.
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
    /// Ultima sfida ricevuta e richieste già fatte con il suo nonce.
    challenge: Option<(Challenge, u32)>,
    /// Il router ha rifiutato le credenziali: non si ritenta.
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
    /// Prepara il trasporto. Non apre connessioni.
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

    /// Porta HTTPS letta con `DeviceInfo:GetSecurityPort`. Prima di saperla, ogni
    /// richiesta su [`Channel::Secure`] fallisce senza traffico. Si fissa una volta sola.
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
                .map_err(|e| FrixError::Transport(format!("richiesta interrotta: {e}")))?
        })
    }
}

/// Risposta con gli header che servono al trasporto.
struct Raw {
    status: u16,
    challenges: Vec<String>,
    retry_after: Option<String>,
    body: String,
}

impl Inner {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, FrixError> {
        let url = self.url(request)?;
        let mut tentativi_503 = 0;
        let mut sfide = 0;
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
                && sfide == 0
            {
                sfide += 1;
                continue;
            }
            if raw.status == 503 {
                match retry_delay(tentativi_503, raw.retry_after.as_deref()) {
                    Some(attesa) => {
                        tentativi_503 += 1;
                        std::thread::sleep(attesa);
                        continue;
                    }
                    None => {
                        return Err(FrixError::Transport(
                            "il router rifiuta le richieste (503): riprova più tardi".into(),
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
                "path non assoluto: {}",
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
                let porta = self.secure_port.get().ok_or_else(|| {
                    FrixError::Transport(
                        "porta HTTPS del router non ancora nota (GetSecurityPort)".into(),
                    )
                })?;
                format!("https://{host}:{porta}{}", request.path)
            }
        })
    }

    /// Header `Authorization` con l'ultima sfida nota e il suo `nc`, o `None` se non
    /// c'è ancora una sfida. Dopo un rifiuto delle credenziali non si ritenta.
    fn authorization(&self, request: &HttpRequest) -> Result<Option<(String, u32)>, FrixError> {
        let mut stato = self.digest.lock().unwrap_or_else(PoisonError::into_inner);
        if stato.rejected {
            return Err(FrixError::Unauthenticated);
        }
        let Some((challenge, nc)) = stato.challenge.as_mut() else {
            return Ok(None);
        };
        *nc += 1;
        let metodo = match request.method {
            Method::Get => "GET",
            Method::Post => "POST",
        };
        challenge
            .authorization(
                &self.credentials.username,
                &self.credentials.password,
                metodo,
                &request.path,
                *nc,
                &cnonce()?,
            )
            .map(|header| Some((header, *nc)))
    }

    /// Registra la sfida di un 401 e dice se ripetere la richiesta (una volta sola).
    ///
    /// `nc_inviato` è il contatore della richiesta appena fatta con credenziali, se c'erano.
    /// Un 401 alla prima richiesta con un nonce fresco (`nc` 1) e senza `stale=true`
    /// significa credenziali rifiutate: da lì in poi nessun tentativo. Con un nonce
    /// riusato (`nc` > 1) il 401 vuol dire solo nonce scaduto.
    fn accept_challenge(&self, raw: &Raw, nc_inviato: Option<u32>) -> Result<bool, FrixError> {
        let challenge = raw.challenges.iter().find_map(|h| Challenge::parse(h).ok());
        let mut stato = self.digest.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(challenge) = challenge else {
            if raw.challenges.is_empty() {
                return Ok(false);
            }
            return Err(FrixError::Transport(
                "il router chiede un'autenticazione diversa da HTTP Digest".into(),
            ));
        };
        if nc_inviato == Some(1) && !challenge.stale {
            stato.rejected = true;
            stato.challenge = None;
            return Ok(false);
        }
        stato.challenge = Some((challenge, 0));
        Ok(true)
    }

    fn execute(
        &self,
        request: &HttpRequest,
        url: &str,
        authorization: Option<&str>,
    ) -> Result<Raw, FrixError> {
        let esito = match request.method {
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
                if let Some(azione) = &request.soap_action {
                    r = r.header("SOAPACTION", format!("\"{azione}\""));
                }
                if let Some(a) = authorization {
                    r = r.header("Authorization", a);
                }
                r.send(request.body.as_str())
            }
        };
        let mut risposta = esito.map_err(|e| self.transport_error(&e))?;
        let headers = risposta.headers();
        let challenges = headers
            .get_all("www-authenticate")
            .iter()
            .filter_map(|v| v.to_str().ok().map(str::to_owned))
            .collect();
        let retry_after = headers
            .get("retry-after")
            .and_then(|v| v.to_str().ok().map(str::to_owned));
        let status = risposta.status().as_u16();
        let body = risposta
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

    /// Un'impronta sbagliata arriva da ureq come errore di I/O: qui torna `TlsPinMismatch`.
    fn transport_error(&self, errore: &ureq::Error) -> FrixError {
        if let Some(pin) = self
            .pin_failure
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            return pin;
        }
        match errore {
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

/// Attesa prima del tentativo `tentativo + 1` dopo un 503, o `None` per rinunciare.
///
/// Rispetta `Retry-After` (secondi) se presente; altrimenti 1, 2, 4 s. Oltre
/// [`MAX_RETRIES`] tentativi, o se il router chiede più di un minuto, si rinuncia.
pub(crate) fn retry_delay(tentativo: u32, retry_after: Option<&str>) -> Option<Duration> {
    if tentativo >= MAX_RETRIES {
        return None;
    }
    let attesa = match retry_after.and_then(|v| v.trim().parse::<u64>().ok()) {
        Some(secondi) => Duration::from_secs(secondi),
        None => Duration::from_secs(1u64 << tentativo),
    };
    (attesa <= MAX_RETRY_AFTER).then_some(attesa)
}

fn server_name(host: &str) -> Result<ServerName<'static>, FrixError> {
    ServerName::try_from(host.to_owned())
        .map_err(|_| FrixError::Validation(format!("host non valido: {host}")))
}

fn cnonce() -> Result<String, FrixError> {
    use ring::rand::SecureRandom as _;
    let mut byte = [0u8; 8];
    ring::rand::SystemRandom::new()
        .fill(&mut byte)
        .map_err(|_| FrixError::Transport("generatore casuale non disponibile".into()))?;
    Ok(byte.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    }))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn backoff_esponenziale_senza_retry_after() {
        let attese: Vec<_> = (0..4).map(|t| retry_delay(t, None)).collect();
        assert_eq!(
            attese,
            vec![
                Some(Duration::from_secs(1)),
                Some(Duration::from_secs(2)),
                Some(Duration::from_secs(4)),
                None
            ]
        );
    }

    #[test]
    fn retry_after_rispettato_e_limitato() {
        assert_eq!(retry_delay(0, Some("10")), Some(Duration::from_secs(10)));
        assert_eq!(
            retry_delay(0, Some("600")),
            None,
            "blocco lungo: si rinuncia"
        );
        assert_eq!(retry_delay(3, Some("1")), None);
    }
}

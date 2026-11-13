//! Registrazione delle risposte del router attraverso il seam `HttpTransport`.
//!
//! [`TrasportoRegistratore`] avvolge il trasporto (reale o a fixture) e conserva ogni
//! coppia richiesta/risposta. La password non compare mai: l'header `Authorization`
//! lo aggiunge `RouterTransport` al suo interno, sotto questo seam.
//!
//! Difesa in profondità: oltre alla Policy di `Tr064Client::call`, il registratore
//! rifiuta da sé ogni `POST` SOAP che il registro non classifica `Exposed(Read)`.

use std::sync::{Arc, Mutex, PoisonError};

use frix_core::FrixError;
use frix_core::policy::{Classification, Level, classification};
use frix_core::services::{device_info, lan_config_security};
use frix_core::tr064::Tr064Client;
use frix_core::transport::{
    Credentials, HttpRequest, HttpResponse, HttpTransport, Method, RouterTransport, SendFuture,
};

use crate::letture::{self, Esito};

/// Una richiesta al router e la sua risposta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scambio {
    /// Richiesta inviata (senza credenziali).
    pub richiesta: HttpRequest,
    /// Risposta ricevuta.
    pub risposta: HttpResponse,
}

/// Trasporto che registra ogni scambio e lascia passare solo letture.
#[derive(Debug)]
pub struct TrasportoRegistratore {
    interno: Arc<dyn HttpTransport>,
    scambi: Arc<Mutex<Vec<Scambio>>>,
}

impl TrasportoRegistratore {
    /// Avvolge `interno`.
    #[must_use]
    pub fn new(interno: Arc<dyn HttpTransport>) -> Self {
        Self {
            interno,
            scambi: Arc::default(),
        }
    }

    /// Copia degli scambi registrati finora, in ordine.
    #[must_use]
    pub fn scambi(&self) -> Vec<Scambio> {
        self.scambi
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// Consente un `POST` solo se l'azione SOAP è classificata `Exposed(Read)`.
fn solo_letture(richiesta: &HttpRequest) -> Result<(), FrixError> {
    if richiesta.method == Method::Get {
        return Ok(());
    }
    let (service, action) = richiesta.soap_service_action().ok_or_else(|| {
        FrixError::Policy(format!(
            "POST senza azione SOAP riconoscibile: {}",
            richiesta.soap_action.as_deref().unwrap_or_default()
        ))
    })?;
    match classification(service, action) {
        Some(Classification::Exposed(Level::Read)) => Ok(()),
        altro => Err(FrixError::Policy(format!(
            "{service}#{action} non è una lettura ({altro:?}): il registratore la rifiuta"
        ))),
    }
}

impl HttpTransport for TrasportoRegistratore {
    fn send(&self, request: HttpRequest) -> SendFuture<'_> {
        Box::pin(async move {
            solo_letture(&request)?;
            let risposta = self.interno.send(request.clone()).await?;
            self.scambi
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(Scambio {
                    richiesta: request,
                    risposta: risposta.clone(),
                });
            Ok(risposta)
        })
    }
}

/// Esito del comando: scambi registrati ed esiti delle letture.
#[derive(Debug, Clone)]
pub struct Registrazione {
    /// Ogni richiesta e risposta, in ordine (descrizioni, SCPD, SOAP, liste).
    pub scambi: Vec<Scambio>,
    /// Un esito per ogni voce di [`letture::LETTURE`].
    pub esiti: Vec<Esito>,
    /// Nomi dei tool esposti dal server.
    pub tool_del_server: Vec<String>,
}

/// Registra dal router reale: config e flag come `frix-mcp` (`args`, per esempio
/// `["--config", "…"]`), password del Router Account dal Keychain, trasporto con TLS
/// pinnato e Digest. Usata da `cargo xtask record` e dal test live; mai dai test normali.
pub async fn registra_dal_router(args: Vec<String>) -> Result<Registrazione, FrixError> {
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
    let porta = Arc::clone(&transport);
    registra(transport, move |p| porta.set_secure_port(p)).await
}

/// Discovery, porta HTTPS, diritti (come all'avvio di `frix-mcp`), poi tutte le
/// [`letture::LETTURE`], registrando ogni scambio con `interno`.
///
/// `porta_sicura` riceve la porta di `GetSecurityPort`: con il trasporto reale
/// è `RouterTransport::set_secure_port`; con le fixture non serve.
pub async fn registra(
    interno: Arc<dyn HttpTransport>,
    porta_sicura: impl FnOnce(u16),
) -> Result<Registrazione, FrixError> {
    let registratore = Arc::new(TrasportoRegistratore::new(interno));
    let client = Tr064Client::discover(Arc::clone(&registratore) as _).await?;
    porta_sicura(device_info::security_port(&client).await?);
    lan_config_security::current_user(&client).await?;
    let letture = letture::esegui(client).await?;
    Ok(Registrazione {
        scambi: registratore.scambi(),
        esiti: letture.esiti,
        tool_del_server: letture.tool_del_server,
    })
}

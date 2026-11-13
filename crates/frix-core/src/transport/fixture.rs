//! `FixtureTransport`: risponde da file, senza rete. Serve ai test (spec #1, "Testing Decisions").

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use super::{HttpRequest, HttpResponse, HttpTransport, Method, SendFuture};
use crate::error::FrixError;
use crate::tr064::SERVICE_URN_PREFIX;

/// Transport a fixture: `GET` serve i file di descrizione, `POST` le risposte SOAP
/// registrate per `SOAPACTION`. Ogni richiesta ricevuta finisce nel [`RequestLog`].
///
/// Layout delle risposte SOAP su disco: `<Servizio>-<istanza>/<Azione>.xml`,
/// per esempio `DeviceInfo-1/GetInfo.xml` o `X_AVM-DE_HostFilter-1/GetWANAccessByIP.xml`.
/// Una risposta che vale solo per certi argomenti in ingresso (azioni `Generic…` con un
/// indice) si chiama `<Azione>@<Argomento>=<valore>.xml`, per esempio
/// `GetGenericAssociatedDeviceInfo@NewAssociatedDeviceIndex=0.xml`.
#[derive(Debug, Default)]
pub struct FixtureTransport {
    files: HashMap<String, HttpResponse>,
    soap: HashMap<String, HttpResponse>,
    soap_by_args: Vec<RispostaPerArgomenti>,
    log: RequestLog,
}

/// Risposta a `soap_action` quando la richiesta contiene tutti gli argomenti `args`.
#[derive(Debug)]
struct RispostaPerArgomenti {
    soap_action: String,
    args: Vec<(String, String)>,
    response: HttpResponse,
}

/// Registro condiviso delle richieste arrivate a un [`FixtureTransport`].
#[derive(Debug, Clone, Default)]
pub struct RequestLog(Arc<Mutex<Vec<HttpRequest>>>);

impl RequestLog {
    /// Copia delle richieste ricevute finora, in ordine.
    #[must_use]
    pub fn all(&self) -> Vec<HttpRequest> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Le sole richieste SOAP (`POST`) ricevute finora.
    #[must_use]
    pub fn soap(&self) -> Vec<HttpRequest> {
        self.all()
            .into_iter()
            .filter(|r| r.method == Method::Post)
            .collect()
    }

    fn push(&self, request: HttpRequest) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request);
    }
}

impl FixtureTransport {
    /// Transport vuoto: ogni richiesta fallisce con un errore di trasporto.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Serve ogni file di `dir` in `GET /<nome-file>` (`tr64desc.xml`, SCPD).
    pub fn with_descriptions_dir(mut self, dir: &Path) -> std::io::Result<Self> {
        for voce in std::fs::read_dir(dir)? {
            let path = voce?.path();
            if let Some(nome) = path.file_name().and_then(|n| n.to_str()) {
                let contenuto = std::fs::read_to_string(&path)?;
                self.files
                    .insert(format!("/{nome}"), HttpResponse::ok(contenuto));
            }
        }
        Ok(self)
    }

    /// Serve `body` in `GET path`, sostituendo un eventuale file già presente.
    #[must_use]
    pub fn with_file(self, path: &str, body: impl Into<String>) -> Self {
        self.with_get(path, HttpResponse::ok(body))
    }

    /// Risponde a `GET path` con `response`, per esempio un errore HTTP.
    #[must_use]
    pub fn with_get(mut self, path: &str, response: HttpResponse) -> Self {
        self.files.insert(path.to_owned(), response);
        self
    }

    /// Carica le risposte SOAP `200 OK` da `dir/<Servizio>-<istanza>/<Azione>.xml`.
    pub fn with_responses_dir(mut self, dir: &Path) -> std::io::Result<Self> {
        for servizio in std::fs::read_dir(dir)? {
            let servizio = servizio?.path();
            let Some(nome) = servizio.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let Some((tipo, istanza)) = nome.rsplit_once('-') else {
                continue;
            };
            let service = format!("{tipo}:{istanza}");
            for azione in std::fs::read_dir(&servizio)? {
                let azione = azione?.path();
                if azione.extension().and_then(|e| e.to_str()) != Some("xml") {
                    continue;
                }
                let Some(stem) = azione.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                let body = HttpResponse::ok(std::fs::read_to_string(&azione)?);
                self = match stem.split_once('@') {
                    Some((action, argomenti)) => {
                        let args: Vec<(&str, &str)> = argomenti
                            .split('&')
                            .filter_map(|coppia| coppia.split_once('='))
                            .collect();
                        self.with_soap_args(&service, action, &args, body)
                    }
                    None => self.with_soap(&service, stem, body),
                };
            }
        }
        Ok(self)
    }

    /// Risposta a `service#action`, per esempio `("DeviceInfo:1", "GetInfo")`;
    /// sostituisce una risposta già presente (utile per fault e casi limite).
    #[must_use]
    pub fn with_soap(mut self, service: &str, action: &str, response: HttpResponse) -> Self {
        self.soap
            .insert(format!("{SERVICE_URN_PREFIX}{service}#{action}"), response);
        self
    }

    /// Risposta a `service#action` solo quando la richiesta porta tutti gli argomenti
    /// `args` (per esempio `[("NewAssociatedDeviceIndex", "0")]`); ha la precedenza su
    /// [`Self::with_soap`] e sostituisce una risposta già presente per gli stessi argomenti.
    /// I valori si confrontano con il corpo SOAP così come è stato inviato.
    #[must_use]
    pub fn with_soap_args(
        mut self,
        service: &str,
        action: &str,
        args: &[(&str, &str)],
        response: HttpResponse,
    ) -> Self {
        let soap_action = format!("{SERVICE_URN_PREFIX}{service}#{action}");
        let args: Vec<(String, String)> = args
            .iter()
            .map(|(nome, valore)| ((*nome).to_owned(), (*valore).to_owned()))
            .collect();
        self.soap_by_args
            .retain(|r| r.soap_action != soap_action || r.args != args);
        self.soap_by_args.push(RispostaPerArgomenti {
            soap_action,
            args,
            response,
        });
        self
    }

    /// Registro delle richieste ricevute, da tenere prima di cedere il transport.
    #[must_use]
    pub fn requests(&self) -> RequestLog {
        self.log.clone()
    }

    fn respond(&self, request: &HttpRequest) -> Result<HttpResponse, FrixError> {
        let risposta = match (request.method, &request.soap_action) {
            (Method::Post, Some(soap_action)) => self
                .soap_by_args
                .iter()
                .find(|r| {
                    &r.soap_action == soap_action
                        && r.args.iter().all(|(nome, valore)| {
                            request.body.contains(&format!("<{nome}>{valore}</{nome}>"))
                        })
                })
                .map(|r| r.response.clone())
                .or_else(|| self.soap.get(soap_action).cloned()),
            _ => self.files.get(&request.path).cloned(),
        };
        risposta.ok_or_else(|| {
            FrixError::Transport(format!(
                "nessuna fixture per {:?} {} {:?}",
                request.method, request.path, request.soap_action
            ))
        })
    }
}

impl HttpTransport for FixtureTransport {
    fn send(&self, request: HttpRequest) -> SendFuture<'_> {
        let risposta = self.respond(&request);
        self.log.push(request);
        Box::pin(std::future::ready(risposta))
    }
}

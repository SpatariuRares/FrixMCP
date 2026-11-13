//! Client TR-064: discovery e chiamata generica (servizio, azione, argomenti)
//! (docs/PROJECT.md §5.1, §6.2).
//!
//! Le stringhe grezze di servizi e azioni compaiono solo nei moduli `services::*`:
//! è per questo che [`Tr064Client::call`] è visibile solo dentro il crate.

#[cfg(test)]
mod policy_tests;
mod scpd;
mod soap;
mod xml;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::error::FrixError;
use crate::policy;
use crate::transport::{Channel, HttpRequest, HttpTransport, Method};
use scpd::ServiceDescription;

/// Prefisso dei `serviceType` TR-064 del FRITZ!Box.
pub const SERVICE_URN_PREFIX: &str = "urn:dslforum-org:service:";

/// Path della descrizione del dispositivo sulla porta 49000.
pub const DEVICE_DESCRIPTION_PATH: &str = "/tr64desc.xml";

/// Client TR-064 nato da una discovery: conosce servizi, azioni e argomenti del router.
#[derive(Debug)]
pub struct Tr064Client {
    transport: Arc<dyn HttpTransport>,
    services: Vec<ServiceDescription>,
}

/// Argomenti di uscita di una chiamata, per nome (per esempio `NewModelName`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SoapResponse(BTreeMap<String, String>);

impl SoapResponse {
    /// Valore dell'argomento di uscita, o errore di parsing se manca.
    pub(crate) fn text(&self, name: &str) -> Result<&str, FrixError> {
        self.0
            .get(name)
            .map(String::as_str)
            .ok_or_else(|| FrixError::Parse(format!("manca l'argomento di uscita {name}")))
    }

    /// Valore dell'argomento di uscita convertito con `FromStr`.
    pub(crate) fn parse<T: std::str::FromStr>(&self, name: &str) -> Result<T, FrixError> {
        let testo = self.text(name)?;
        testo
            .trim()
            .parse()
            .map_err(|_| FrixError::Parse(format!("valore non valido per {name}")))
    }

    /// Booleano TR-064 (`0`/`1`).
    pub(crate) fn flag(&self, name: &str) -> Result<bool, FrixError> {
        match self.text(name)?.trim() {
            "1" => Ok(true),
            "0" => Ok(false),
            _ => Err(FrixError::Parse(format!("valore non booleano per {name}"))),
        }
    }
}

impl Tr064Client {
    /// Scarica `tr64desc.xml` e tutti gli SCPD che referenzia (docs/PROJECT.md §5.1).
    pub async fn discover(transport: Arc<dyn HttpTransport>) -> Result<Self, FrixError> {
        let descrizione = get(transport.as_ref(), DEVICE_DESCRIPTION_PATH).await?;
        let mut services = scpd::parse_device_description(&descrizione)?;
        let mut scpd_letti: BTreeMap<String, BTreeMap<String, scpd::ActionDescription>> =
            BTreeMap::new();
        let percorsi: BTreeSet<String> = services.iter().map(|s| s.scpd_url.clone()).collect();
        for percorso in percorsi {
            let documento = get(transport.as_ref(), &percorso).await?;
            scpd_letti.insert(percorso, scpd::parse_scpd(&documento)?);
        }
        for servizio in &mut services {
            if let Some(azioni) = scpd_letti.get(&servizio.scpd_url) {
                servizio.actions.clone_from(azioni);
            }
        }
        Ok(Self {
            transport,
            services,
        })
    }

    /// Le istanze di un servizio presenti nella discovery, per esempio
    /// `["WLANConfiguration:1", "WLANConfiguration:2"]` per `"WLANConfiguration"`, in ordine.
    pub(crate) fn instances(&self, service: &str) -> Vec<String> {
        let prefisso = format!("{SERVICE_URN_PREFIX}{service}:");
        let mut istanze: Vec<(u32, String)> = self
            .services
            .iter()
            .filter_map(|s| s.service_type.strip_prefix(&prefisso))
            .filter_map(|n| n.parse::<u32>().ok().map(|i| (i, format!("{service}:{n}"))))
            .collect();
        istanze.sort();
        istanze.into_iter().map(|(_, s)| s).collect()
    }

    /// Chiama `service#action`, per esempio `("DeviceInfo:1", "GetInfo", &[])`.
    ///
    /// In ordine: azione e argomenti devono esistere nello SCPD, la Policy deve
    /// consentire l'azione, poi l'envelope parte e la risposta viene analizzata.
    /// Ogni rifiuto avviene prima di qualunque richiesta al transport.
    pub(crate) async fn call(
        &self,
        service: &str,
        action: &str,
        args: &[(&str, &str)],
    ) -> Result<SoapResponse, FrixError> {
        self.call_on(Channel::Secure, service, action, args).await
    }

    /// Come [`Self::call`], ma in chiaro (HTTP 49000, senza credenziali). Serve solo a
    /// sapere su quale porta sta l'HTTPS (docs/PROJECT.md §5.1): l'unico chiamante è
    /// `services::device_info::security_port`. Tutto il resto va su HTTPS.
    pub(crate) async fn call_plain(
        &self,
        service: &str,
        action: &str,
        args: &[(&str, &str)],
    ) -> Result<SoapResponse, FrixError> {
        self.call_on(Channel::Plain, service, action, args).await
    }

    async fn call_on(
        &self,
        channel: Channel,
        service: &str,
        action: &str,
        args: &[(&str, &str)],
    ) -> Result<SoapResponse, FrixError> {
        let service_type = format!("{SERVICE_URN_PREFIX}{service}");
        let servizio = self
            .services
            .iter()
            .find(|s| s.service_type == service_type)
            .ok_or_else(|| {
                FrixError::Validation(format!("il servizio {service} non esiste nel router"))
            })?;
        let azione = servizio.actions.get(action).ok_or_else(|| {
            FrixError::Validation(format!(
                "l'azione {action} non esiste nello SCPD di {service}"
            ))
        })?;
        azione.validate_in_args(action, args)?;
        policy::authorize(service, action)?;

        let body = soap::build_envelope(&service_type, action, args)?;
        let risposta = self
            .transport
            .send(HttpRequest {
                method: Method::Post,
                channel,
                path: servizio.control_url.clone(),
                soap_action: Some(format!("{service_type}#{action}")),
                body,
            })
            .await?;
        soap::parse_response(action, &risposta)
            .map(SoapResponse)
            .map_err(|errore| match errore {
                FrixError::NotAuthorized { description, .. } => FrixError::NotAuthorized {
                    action: format!("{service}#{action}"),
                    rights: policy::required_rights(service, action),
                    description,
                },
                altro => altro,
            })
    }

    /// `GET path` sul canale autenticato: è così che si scaricano le liste `*ListPath`
    /// (docs/PROJECT.md §5.3). Il path è quello restituito dal router, SID incluso.
    pub(crate) async fn get_secure(&self, path: &str) -> Result<String, FrixError> {
        get_on(self.transport.as_ref(), Channel::Secure, path).await
    }
}

async fn get(transport: &dyn HttpTransport, path: &str) -> Result<String, FrixError> {
    get_on(transport, Channel::Plain, path).await
}

async fn get_on(
    transport: &dyn HttpTransport,
    channel: Channel,
    path: &str,
) -> Result<String, FrixError> {
    let risposta = transport
        .send(HttpRequest {
            method: Method::Get,
            channel,
            path: path.to_owned(),
            soap_action: None,
            body: String::new(),
        })
        .await?;
    if risposta.status == 200 {
        Ok(risposta.body)
    } else {
        // Niente query nel messaggio: può contenere il SID, che non arriva mai all'Agent
        // (docs/PROJECT.md §7.2 T3, §13.6).
        let senza_query = path.split_once('?').map_or(path, |(p, _)| p);
        Err(FrixError::Transport(format!(
            "GET {senza_query}: HTTP {}",
            risposta.status
        )))
    }
}

/// Argomenti in ingresso di `action` in un envelope SOAP di richiesta, in ordine (per
/// esempio quello di un [`HttpRequest`] registrato). Stesso parser delle risposte.
pub fn soap_request_arguments(
    envelope: &str,
    action: &str,
) -> Result<Vec<(String, String)>, FrixError> {
    soap::request_args(envelope, action)
}

/// Analizza un frammento XML annidato in un argomento di uscita (per esempio i diritti
/// di `X_AVM-DE_GetCurrentUser`) e restituisce le coppie (nome locale, testo) dei figli
/// della radice, in ordine.
pub(crate) fn nested_xml_pairs(documento: &str) -> Result<Vec<(String, String)>, FrixError> {
    let radice = xml::parse(documento)?;
    Ok(radice
        .children
        .into_iter()
        .map(|c| (c.name, c.text))
        .collect())
}

/// Analizza una lista XML `*ListPath` (per esempio `<List><Item>…</Item></List>`) e
/// restituisce, per ogni figlio della radice chiamato `item`, i suoi figli come
/// mappa nome locale → testo. Gli altri figli della radice (`ItemCount`, …) si ignorano.
pub(crate) fn xml_list_items(
    documento: &str,
    item: &str,
) -> Result<Vec<BTreeMap<String, String>>, FrixError> {
    let radice = xml::parse(documento)?;
    Ok(radice
        .children
        .into_iter()
        .filter(|c| c.name == item)
        .map(|c| c.children.into_iter().map(|f| (f.name, f.text)).collect())
        .collect())
}

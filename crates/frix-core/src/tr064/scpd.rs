//! Discovery: `tr64desc.xml` e SCPD, e validazione di azioni e argomenti (docs/PROJECT.md §5.1).

use std::collections::BTreeMap;

use super::xml::{self, Element};
use crate::error::FrixError;

/// Un servizio dichiarato in `tr64desc.xml`, con le azioni del suo SCPD.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ServiceDescription {
    /// Per esempio `urn:dslforum-org:service:DeviceInfo:1`.
    pub(crate) service_type: String,
    pub(crate) control_url: String,
    pub(crate) scpd_url: String,
    pub(crate) actions: BTreeMap<String, ActionDescription>,
}

/// Argomenti di un'azione come li dichiara lo SCPD.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ActionDescription {
    pub(crate) in_args: Vec<String>,
    pub(crate) out_args: Vec<String>,
}

/// I servizi elencati da `tr64desc.xml` (a qualunque livello di `deviceList`), senza azioni.
pub(crate) fn parse_device_description(
    documento: &str,
) -> Result<Vec<ServiceDescription>, FrixError> {
    let radice = xml::parse(documento)?;
    radice
        .descendants("service")
        .into_iter()
        .map(|servizio| {
            let campo = |nome: &str| {
                servizio
                    .child_text(nome)
                    .map(|t| t.trim().to_owned())
                    .filter(|t| !t.is_empty())
                    .ok_or_else(|| FrixError::Parse(format!("tr64desc.xml: servizio senza {nome}")))
            };
            Ok(ServiceDescription {
                service_type: campo("serviceType")?,
                control_url: campo("controlURL")?,
                scpd_url: campo("SCPDURL")?,
                actions: BTreeMap::new(),
            })
        })
        .collect()
}

/// Le azioni di uno SCPD, per nome.
pub(crate) fn parse_scpd(
    documento: &str,
) -> Result<BTreeMap<String, ActionDescription>, FrixError> {
    let radice = xml::parse(documento)?;
    let Some(lista) = radice.child("actionList") else {
        return Ok(BTreeMap::new());
    };
    lista
        .children
        .iter()
        .filter(|e| e.name == "action")
        .map(|azione| {
            let nome = azione
                .child_text("name")
                .map(|t| t.trim().to_owned())
                .ok_or_else(|| FrixError::Parse("SCPD: azione senza nome".into()))?;
            Ok((nome, parse_arguments(azione)?))
        })
        .collect()
}

fn parse_arguments(azione: &Element) -> Result<ActionDescription, FrixError> {
    let mut descrizione = ActionDescription::default();
    let Some(lista) = azione.child("argumentList") else {
        return Ok(descrizione);
    };
    for argomento in lista.children.iter().filter(|e| e.name == "argument") {
        let nome = argomento
            .child_text("name")
            .map(|t| t.trim().to_owned())
            .ok_or_else(|| FrixError::Parse("SCPD: argomento senza nome".into()))?;
        match argomento.child_text("direction").map(str::trim) {
            Some("in") => descrizione.in_args.push(nome),
            Some("out") => descrizione.out_args.push(nome),
            altro => {
                return Err(FrixError::Parse(format!(
                    "SCPD: direzione {altro:?} per l'argomento {nome}"
                )));
            }
        }
    }
    Ok(descrizione)
}

impl ActionDescription {
    /// Verifica che `args` siano esattamente gli argomenti `in` dello SCPD.
    pub(crate) fn validate_in_args(
        &self,
        action: &str,
        args: &[(&str, &str)],
    ) -> Result<(), FrixError> {
        for (nome, _) in args {
            if !self.in_args.iter().any(|a| a == nome) {
                return Err(FrixError::Validation(format!(
                    "l'argomento {nome} non esiste nello SCPD di {action}"
                )));
            }
        }
        for atteso in &self.in_args {
            if !args.iter().any(|(nome, _)| nome == atteso) {
                return Err(FrixError::Validation(format!(
                    "manca l'argomento {atteso} richiesto dallo SCPD di {action}"
                )));
            }
        }
        Ok(())
    }
}

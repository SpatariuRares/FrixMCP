//! Envelope SOAP con escape XML rigoroso, parsing delle risposte e dei fault `UPnP`
//! (docs/PROJECT.md §5.1).

use std::collections::BTreeMap;

use super::xml;
use crate::error::FrixError;
use crate::transport::HttpResponse;

/// Costruisce l'envelope SOAP per `service_type#action` con gli argomenti dati.
///
/// Rifiuta i caratteri che XML 1.0 non può rappresentare (controlli diversi da
/// tabulazione e a capo) invece di inviarli al router.
pub(crate) fn build_envelope(
    service_type: &str,
    action: &str,
    args: &[(&str, &str)],
) -> Result<String, FrixError> {
    let service_type = escape(service_type)?;
    let action = nome_xml(action)?;
    let mut corpo = String::new();
    for (nome, valore) in args {
        let nome = nome_xml(nome)?;
        let valore = escape(valore)?;
        for parte in ["<", nome, ">", &valore, "</", nome, ">"] {
            corpo.push_str(parte);
        }
    }
    Ok(format!(
        concat!(
            r#"<?xml version="1.0" encoding="utf-8"?>"#,
            r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" "#,
            r#"s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">"#,
            r#"<s:Body><u:{action} xmlns:u="{service_type}">{corpo}</u:{action}></s:Body>"#,
            r#"</s:Envelope>"#
        ),
        action = action,
        service_type = service_type,
        corpo = corpo,
    ))
}

/// Escape per testo e valori di attributo XML.
pub(crate) fn escape(valore: &str) -> Result<String, FrixError> {
    let mut uscita = String::with_capacity(valore.len());
    for c in valore.chars() {
        match c {
            '&' => uscita.push_str("&amp;"),
            '<' => uscita.push_str("&lt;"),
            '>' => uscita.push_str("&gt;"),
            '"' => uscita.push_str("&quot;"),
            '\'' => uscita.push_str("&apos;"),
            '\t' | '\n' | '\r' => uscita.push(c),
            c if c.is_control() => {
                return Err(FrixError::Validation(format!(
                    "carattere di controllo U+{:04X} non ammesso in un argomento",
                    u32::from(c)
                )));
            }
            c => uscita.push(c),
        }
    }
    Ok(uscita)
}

/// I nomi di azioni e argomenti vengono dallo SCPD, ma non si mettono mai in un tag senza controllo.
fn nome_xml(nome: &str) -> Result<&str, FrixError> {
    let valido = !nome.is_empty()
        && nome
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if valido {
        Ok(nome)
    } else {
        Err(FrixError::Validation(format!(
            "nome XML non valido: {nome:?}"
        )))
    }
}

/// Argomenti in ingresso di `action` in un envelope di richiesta, in ordine.
pub(crate) fn request_args(
    envelope: &str,
    action: &str,
) -> Result<Vec<(String, String)>, FrixError> {
    let radice = xml::parse(envelope)?;
    let elemento = radice
        .descendants(action)
        .into_iter()
        .next()
        .ok_or_else(|| FrixError::Parse(format!("manca {action} nell'envelope")))?;
    Ok(elemento
        .children
        .iter()
        .map(|c| (c.name.clone(), c.text.clone()))
        .collect())
}

/// Argomenti di uscita di `action` dalla risposta, o l'errore di dominio del fault.
pub(crate) fn parse_response(
    action: &str,
    risposta: &HttpResponse,
) -> Result<BTreeMap<String, String>, FrixError> {
    if risposta.status == 401 {
        return Err(FrixError::Unauthenticated);
    }
    let documento = xml::parse(&risposta.body);
    if let Ok(radice) = &documento
        && let Some(fault) = radice.descendants("Fault").first()
    {
        return Err(fault_error(fault));
    }
    if risposta.status != 200 {
        return Err(FrixError::Transport(format!("HTTP {}", risposta.status)));
    }
    let radice = documento?;
    let atteso = format!("{action}Response");
    let elemento = radice
        .descendants(&atteso)
        .into_iter()
        .next()
        .ok_or_else(|| FrixError::Parse(format!("manca {atteso}")))?;
    Ok(elemento
        .children
        .iter()
        .map(|c| (c.name.clone(), c.text.clone()))
        .collect())
}

fn fault_error(fault: &xml::Element) -> FrixError {
    let errore = fault.descendants("UPnPError").into_iter().next();
    let codice = errore
        .and_then(|e| e.child_text("errorCode"))
        .and_then(|c| c.trim().parse::<u16>().ok());
    let description = errore
        .and_then(|e| e.child_text("errorDescription"))
        .unwrap_or_default()
        .trim()
        .to_owned();
    match codice {
        Some(401) => FrixError::Unauthenticated,
        Some(402) => FrixError::InvalidArgs { description },
        // servizio e diritti li aggiunge `Tr064Client::call`, che li conosce
        Some(606) => FrixError::NotAuthorized {
            action: String::new(),
            rights: None,
            description,
        },
        Some(714) => FrixError::NoSuchEntry { description },
        Some(820) => FrixError::RouterInternal { description },
        Some(code @ 866..=868) => FrixError::SecondFactorRequired { code },
        Some(code) => FrixError::Soap { code, description },
        None => FrixError::Parse("fault SOAP senza errorCode".into()),
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

    #[test]
    fn envelope_escapa_i_caratteri_speciali_negli_argomenti() {
        let envelope = build_envelope(
            "urn:dslforum-org:service:Hosts:1",
            "X_AVM-DE_SetHostNameByMACAddress",
            &[
                ("NewMACAddress", "02:00:00:00:00:02"),
                ("NewHostName", r#"<a href="x">Tom & Jerry's</a>"#),
            ],
        )
        .unwrap();

        assert!(envelope.contains(
            "<NewHostName>&lt;a href=&quot;x&quot;&gt;Tom &amp; Jerry&apos;s&lt;/a&gt;</NewHostName>"
        ));
        assert!(envelope.contains("<NewMACAddress>02:00:00:00:00:02</NewMACAddress>"));
        assert!(envelope.contains(
            r#"<u:X_AVM-DE_SetHostNameByMACAddress xmlns:u="urn:dslforum-org:service:Hosts:1">"#
        ));
    }

    #[test]
    fn envelope_non_permette_di_chiudere_il_tag_e_iniettarne_un_altro() {
        let envelope = build_envelope(
            "urn:dslforum-org:service:Hosts:1",
            "GetSpecificHostEntry",
            &[("NewMACAddress", "</NewMACAddress><NewEnable>1</NewEnable>")],
        )
        .unwrap();

        assert!(!envelope.contains("<NewEnable>"));
        let radice = xml::parse(&envelope).unwrap();
        let argomenti = &radice.descendants("GetSpecificHostEntry")[0].children;
        assert_eq!(argomenti.len(), 1);
        assert_eq!(
            argomenti[0].text,
            "</NewMACAddress><NewEnable>1</NewEnable>"
        );
    }

    #[test]
    fn envelope_senza_argomenti_ha_l_elemento_azione_vuoto() {
        let envelope = build_envelope(
            "urn:dslforum-org:service:DeviceInfo:1",
            "GetSecurityPort",
            &[],
        )
        .unwrap();

        assert!(envelope.contains(
            r#"<s:Body><u:GetSecurityPort xmlns:u="urn:dslforum-org:service:DeviceInfo:1"></u:GetSecurityPort></s:Body>"#
        ));
    }

    #[test]
    fn envelope_rifiuta_i_caratteri_di_controllo() {
        let errore = build_envelope(
            "urn:dslforum-org:service:Hosts:1",
            "X_AVM-DE_SetHostNameByMACAddress",
            &[("NewHostName", "nome\u{0}nascosto")],
        )
        .unwrap_err();

        assert!(matches!(errore, FrixError::Validation(_)), "{errore:?}");
    }

    #[test]
    fn envelope_rifiuta_un_nome_di_argomento_che_non_e_un_nome_xml() {
        let errore = build_envelope(
            "urn:dslforum-org:service:Hosts:1",
            "GetSpecificHostEntry",
            &[("NewMACAddress><x", "1")],
        )
        .unwrap_err();

        assert!(matches!(errore, FrixError::Validation(_)), "{errore:?}");
    }
}

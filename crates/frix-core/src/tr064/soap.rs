//! SOAP envelope with strict XML escaping, parsing of responses and `UPnP` faults
//! (docs/PROJECT.md §5.1).

use std::collections::BTreeMap;

use super::xml;
use crate::error::FrixError;
use crate::transport::HttpResponse;

/// Builds the SOAP envelope for `service_type#action` with the given arguments.
///
/// Rejects characters that XML 1.0 cannot represent (controls other than tab and
/// newline) instead of sending them to the router.
pub(crate) fn build_envelope(
    service_type: &str,
    action: &str,
    args: &[(&str, &str)],
) -> Result<String, FrixError> {
    let service_type = escape(service_type)?;
    let action = xml_name(action)?;
    let mut body = String::new();
    for (name, value) in args {
        let name = xml_name(name)?;
        let value = escape(value)?;
        for part in ["<", name, ">", &value, "</", name, ">"] {
            body.push_str(part);
        }
    }
    Ok(format!(
        concat!(
            r#"<?xml version="1.0" encoding="utf-8"?>"#,
            r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" "#,
            r#"s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">"#,
            r#"<s:Body><u:{action} xmlns:u="{service_type}">{body}</u:{action}></s:Body>"#,
            r#"</s:Envelope>"#
        ),
        action = action,
        service_type = service_type,
        body = body,
    ))
}

/// Escaping for XML text and attribute values.
pub(crate) fn escape(value: &str) -> Result<String, FrixError> {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\t' | '\n' | '\r' => out.push(c),
            c if c.is_control() => {
                return Err(FrixError::Validation(format!(
                    "control character U+{:04X} not allowed in an argument",
                    u32::from(c)
                )));
            }
            c => out.push(c),
        }
    }
    Ok(out)
}

/// Action and argument names come from the SCPD, but never go into a tag unchecked.
fn xml_name(name: &str) -> Result<&str, FrixError> {
    let valid = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if valid {
        Ok(name)
    } else {
        Err(FrixError::Validation(format!("invalid XML name: {name:?}")))
    }
}

/// Input arguments of `action` in a request envelope, in order.
pub(crate) fn request_args(
    envelope: &str,
    action: &str,
) -> Result<Vec<(String, String)>, FrixError> {
    let root = xml::parse(envelope)?;
    let element = root
        .descendants(action)
        .into_iter()
        .next()
        .ok_or_else(|| FrixError::Parse(format!("missing {action} in the envelope")))?;
    Ok(element
        .children
        .iter()
        .map(|c| (c.name.clone(), c.text.clone()))
        .collect())
}

/// Output arguments of `action` from the response, or the fault's domain error.
pub(crate) fn parse_response(
    action: &str,
    response: &HttpResponse,
) -> Result<BTreeMap<String, String>, FrixError> {
    if response.status == 401 {
        return Err(FrixError::Unauthenticated);
    }
    let document = xml::parse(&response.body);
    if let Ok(root) = &document
        && let Some(fault) = root.descendants("Fault").first()
    {
        return Err(fault_error(fault));
    }
    if response.status != 200 {
        return Err(FrixError::Transport(format!("HTTP {}", response.status)));
    }
    let root = document?;
    let expected = format!("{action}Response");
    let element = root
        .descendants(&expected)
        .into_iter()
        .next()
        .ok_or_else(|| FrixError::Parse(format!("missing {expected}")))?;
    Ok(element
        .children
        .iter()
        .map(|c| (c.name.clone(), c.text.clone()))
        .collect())
}

fn fault_error(fault: &xml::Element) -> FrixError {
    let upnp_error = fault.descendants("UPnPError").into_iter().next();
    let code = upnp_error
        .and_then(|e| e.child_text("errorCode"))
        .and_then(|c| c.trim().parse::<u16>().ok());
    let description = upnp_error
        .and_then(|e| e.child_text("errorDescription"))
        .unwrap_or_default()
        .trim()
        .to_owned();
    match code {
        Some(401) => FrixError::Unauthenticated,
        Some(402) => FrixError::InvalidArgs { description },
        // service and rights are added by `Tr064Client::call`, which knows them
        Some(606) => FrixError::NotAuthorized {
            action: String::new(),
            rights: None,
            description,
        },
        Some(714) => FrixError::NoSuchEntry { description },
        Some(820) => FrixError::RouterInternal { description },
        Some(code @ 866..=868) => FrixError::SecondFactorRequired { code },
        Some(code) => FrixError::Soap { code, description },
        None => FrixError::Parse("SOAP fault without errorCode".into()),
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
    fn envelope_escapes_special_characters_in_arguments() {
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
    fn envelope_does_not_allow_closing_the_tag_and_injecting_another() {
        let envelope = build_envelope(
            "urn:dslforum-org:service:Hosts:1",
            "GetSpecificHostEntry",
            &[("NewMACAddress", "</NewMACAddress><NewEnable>1</NewEnable>")],
        )
        .unwrap();

        assert!(!envelope.contains("<NewEnable>"));
        let root = xml::parse(&envelope).unwrap();
        let args = &root.descendants("GetSpecificHostEntry")[0].children;
        assert_eq!(args.len(), 1);
        assert_eq!(args[0].text, "</NewMACAddress><NewEnable>1</NewEnable>");
    }

    #[test]
    fn envelope_without_arguments_has_an_empty_action_element() {
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
    fn envelope_rejects_control_characters() {
        let error = build_envelope(
            "urn:dslforum-org:service:Hosts:1",
            "X_AVM-DE_SetHostNameByMACAddress",
            &[("NewHostName", "name\u{0}hidden")],
        )
        .unwrap_err();

        assert!(matches!(error, FrixError::Validation(_)), "{error:?}");
    }

    #[test]
    fn envelope_rejects_an_argument_name_that_is_not_an_xml_name() {
        let error = build_envelope(
            "urn:dslforum-org:service:Hosts:1",
            "GetSpecificHostEntry",
            &[("NewMACAddress><x", "1")],
        )
        .unwrap_err();

        assert!(matches!(error, FrixError::Validation(_)), "{error:?}");
    }
}

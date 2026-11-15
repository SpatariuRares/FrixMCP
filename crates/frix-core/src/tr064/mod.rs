//! TR-064 client: discovery and generic call (service, action, arguments)
//! (docs/PROJECT.md §5.1, §6.2).
//!
//! Raw service and action strings appear only in the `services::*` modules:
//! that is why [`Tr064Client::call`] is visible only inside the crate.

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

/// Prefix of the FRITZ!Box TR-064 `serviceType`s.
pub const SERVICE_URN_PREFIX: &str = "urn:dslforum-org:service:";

/// Path of the device description on port 49000.
pub const DEVICE_DESCRIPTION_PATH: &str = "/tr64desc.xml";

/// TR-064 client built by a discovery: knows the router's services, actions and arguments.
#[derive(Debug)]
pub struct Tr064Client {
    transport: Arc<dyn HttpTransport>,
    services: Vec<ServiceDescription>,
}

/// Output arguments of a call, by name (for example `NewModelName`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SoapResponse(BTreeMap<String, String>);

impl SoapResponse {
    /// Value of the output argument, or a parse error if missing.
    pub(crate) fn text(&self, name: &str) -> Result<&str, FrixError> {
        self.0
            .get(name)
            .map(String::as_str)
            .ok_or_else(|| FrixError::Parse(format!("missing output argument {name}")))
    }

    /// Value of the output argument converted with `FromStr`.
    pub(crate) fn parse<T: std::str::FromStr>(&self, name: &str) -> Result<T, FrixError> {
        let text = self.text(name)?;
        text.trim()
            .parse()
            .map_err(|_| FrixError::Parse(format!("invalid value for {name}")))
    }

    /// TR-064 boolean (`0`/`1`).
    pub(crate) fn flag(&self, name: &str) -> Result<bool, FrixError> {
        match self.text(name)?.trim() {
            "1" => Ok(true),
            "0" => Ok(false),
            _ => Err(FrixError::Parse(format!("non-boolean value for {name}"))),
        }
    }
}

impl Tr064Client {
    /// Downloads `tr64desc.xml` and all the SCPDs it references (docs/PROJECT.md §5.1).
    pub async fn discover(transport: Arc<dyn HttpTransport>) -> Result<Self, FrixError> {
        let description = get(transport.as_ref(), DEVICE_DESCRIPTION_PATH).await?;
        let mut services = scpd::parse_device_description(&description)?;
        let mut scpds_read: BTreeMap<String, BTreeMap<String, scpd::ActionDescription>> =
            BTreeMap::new();
        let paths: BTreeSet<String> = services.iter().map(|s| s.scpd_url.clone()).collect();
        for path in paths {
            let document = get(transport.as_ref(), &path).await?;
            scpds_read.insert(path, scpd::parse_scpd(&document)?);
        }
        for service in &mut services {
            if let Some(actions) = scpds_read.get(&service.scpd_url) {
                service.actions.clone_from(actions);
            }
        }
        Ok(Self {
            transport,
            services,
        })
    }

    /// The instances of a service found by the discovery, for example
    /// `["WLANConfiguration:1", "WLANConfiguration:2"]` for `"WLANConfiguration"`, in order.
    pub(crate) fn instances(&self, service: &str) -> Vec<String> {
        let prefix = format!("{SERVICE_URN_PREFIX}{service}:");
        let mut instances: Vec<(u32, String)> = self
            .services
            .iter()
            .filter_map(|s| s.service_type.strip_prefix(&prefix))
            .filter_map(|n| n.parse::<u32>().ok().map(|i| (i, format!("{service}:{n}"))))
            .collect();
        instances.sort();
        instances.into_iter().map(|(_, s)| s).collect()
    }

    /// Calls `service#action`, for example `("DeviceInfo:1", "GetInfo", &[])`.
    ///
    /// In order: action and arguments must exist in the SCPD, the Policy must allow
    /// the action, then the envelope is sent and the response is parsed.
    /// Every rejection happens before any request to the transport.
    pub(crate) async fn call(
        &self,
        service: &str,
        action: &str,
        args: &[(&str, &str)],
    ) -> Result<SoapResponse, FrixError> {
        self.call_on(Channel::Secure, service, action, args).await
    }

    /// Like [`Self::call`], but in plain text (HTTP 49000, no credentials). Only used to
    /// learn which port HTTPS is on (docs/PROJECT.md §5.1): the only caller is
    /// `services::device_info::security_port`. Everything else goes over HTTPS.
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
        let service_desc = self
            .services
            .iter()
            .find(|s| s.service_type == service_type)
            .ok_or_else(|| {
                FrixError::Validation(format!("service {service} does not exist on the router"))
            })?;
        let action_desc = service_desc.actions.get(action).ok_or_else(|| {
            FrixError::Validation(format!(
                "action {action} does not exist in the SCPD of {service}"
            ))
        })?;
        action_desc.validate_in_args(action, args)?;
        policy::authorize(service, action)?;

        let body = soap::build_envelope(&service_type, action, args)?;
        let response = self
            .transport
            .send(HttpRequest {
                method: Method::Post,
                channel,
                path: service_desc.control_url.clone(),
                soap_action: Some(format!("{service_type}#{action}")),
                body,
            })
            .await?;
        soap::parse_response(action, &response)
            .map(SoapResponse)
            .map_err(|error| match error {
                FrixError::NotAuthorized { description, .. } => FrixError::NotAuthorized {
                    action: format!("{service}#{action}"),
                    rights: policy::required_rights(service, action),
                    description,
                },
                other => other,
            })
    }

    /// `GET path` on the authenticated channel: this is how `*ListPath` lists are
    /// downloaded (docs/PROJECT.md §5.3). The path is the one the router returned, SID included.
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
    let response = transport
        .send(HttpRequest {
            method: Method::Get,
            channel,
            path: path.to_owned(),
            soap_action: None,
            body: String::new(),
        })
        .await?;
    if response.status == 200 {
        Ok(response.body)
    } else {
        // No query in the message: it may contain the SID, which never reaches the Agent
        // (docs/PROJECT.md §7.2 T3, §13.6).
        let without_query = path.split_once('?').map_or(path, |(p, _)| p);
        Err(FrixError::Transport(format!(
            "GET {without_query}: HTTP {}",
            response.status
        )))
    }
}

/// Input arguments of `action` in a SOAP request envelope, in order (for example
/// that of a recorded [`HttpRequest`]). Same parser as for responses.
pub fn soap_request_arguments(
    envelope: &str,
    action: &str,
) -> Result<Vec<(String, String)>, FrixError> {
    soap::request_args(envelope, action)
}

/// Parses an XML fragment nested in an output argument (for example the rights of
/// `X_AVM-DE_GetCurrentUser`) and returns the (local name, text) pairs of the root's
/// children, in order.
pub(crate) fn nested_xml_pairs(document: &str) -> Result<Vec<(String, String)>, FrixError> {
    let root = xml::parse(document)?;
    Ok(root
        .children
        .into_iter()
        .map(|c| (c.name, c.text))
        .collect())
}

/// Parses a `*ListPath` XML list (for example `<List><Item>…</Item></List>`) and
/// returns, for each root child named `item`, its children as a local name → text
/// map. Other root children (`ItemCount`, …) are ignored.
pub(crate) fn xml_list_items(
    document: &str,
    item: &str,
) -> Result<Vec<BTreeMap<String, String>>, FrixError> {
    let root = xml::parse(document)?;
    Ok(root
        .children
        .into_iter()
        .filter(|c| c.name == item)
        .map(|c| c.children.into_iter().map(|f| (f.name, f.text)).collect())
        .collect())
}

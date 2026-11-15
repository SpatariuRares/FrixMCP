//! Discovery: `tr64desc.xml` and SCPDs, and validation of actions and arguments (docs/PROJECT.md §5.1).

use std::collections::BTreeMap;

use super::xml::{self, Element};
use crate::error::FrixError;

/// A service declared in `tr64desc.xml`, with the actions of its SCPD.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ServiceDescription {
    /// For example `urn:dslforum-org:service:DeviceInfo:1`.
    pub(crate) service_type: String,
    pub(crate) control_url: String,
    pub(crate) scpd_url: String,
    pub(crate) actions: BTreeMap<String, ActionDescription>,
}

/// Arguments of an action as the SCPD declares them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ActionDescription {
    pub(crate) in_args: Vec<String>,
    pub(crate) out_args: Vec<String>,
}

/// The services listed by `tr64desc.xml` (at any level of `deviceList`), without actions.
pub(crate) fn parse_device_description(
    document: &str,
) -> Result<Vec<ServiceDescription>, FrixError> {
    let root = xml::parse(document)?;
    root.descendants("service")
        .into_iter()
        .map(|service| {
            let field = |name: &str| {
                service
                    .child_text(name)
                    .map(|t| t.trim().to_owned())
                    .filter(|t| !t.is_empty())
                    .ok_or_else(|| {
                        FrixError::Parse(format!("tr64desc.xml: service without {name}"))
                    })
            };
            Ok(ServiceDescription {
                service_type: field("serviceType")?,
                control_url: field("controlURL")?,
                scpd_url: field("SCPDURL")?,
                actions: BTreeMap::new(),
            })
        })
        .collect()
}

/// The actions of an SCPD, by name.
pub(crate) fn parse_scpd(document: &str) -> Result<BTreeMap<String, ActionDescription>, FrixError> {
    let root = xml::parse(document)?;
    let Some(list) = root.child("actionList") else {
        return Ok(BTreeMap::new());
    };
    list.children
        .iter()
        .filter(|e| e.name == "action")
        .map(|action| {
            let name = action
                .child_text("name")
                .map(|t| t.trim().to_owned())
                .ok_or_else(|| FrixError::Parse("SCPD: action without name".into()))?;
            Ok((name, parse_arguments(action)?))
        })
        .collect()
}

fn parse_arguments(action: &Element) -> Result<ActionDescription, FrixError> {
    let mut description = ActionDescription::default();
    let Some(list) = action.child("argumentList") else {
        return Ok(description);
    };
    for argument in list.children.iter().filter(|e| e.name == "argument") {
        let name = argument
            .child_text("name")
            .map(|t| t.trim().to_owned())
            .ok_or_else(|| FrixError::Parse("SCPD: argument without name".into()))?;
        match argument.child_text("direction").map(str::trim) {
            Some("in") => description.in_args.push(name),
            Some("out") => description.out_args.push(name),
            other => {
                return Err(FrixError::Parse(format!(
                    "SCPD: direction {other:?} for argument {name}"
                )));
            }
        }
    }
    Ok(description)
}

impl ActionDescription {
    /// Checks that `args` are exactly the SCPD's `in` arguments.
    pub(crate) fn validate_in_args(
        &self,
        action: &str,
        args: &[(&str, &str)],
    ) -> Result<(), FrixError> {
        for (name, _) in args {
            if !self.in_args.iter().any(|a| a == name) {
                return Err(FrixError::Validation(format!(
                    "argument {name} does not exist in the SCPD of {action}"
                )));
            }
        }
        for expected in &self.in_args {
            if !args.iter().any(|(name, _)| name == expected) {
                return Err(FrixError::Validation(format!(
                    "missing argument {expected} required by the SCPD of {action}"
                )));
            }
        }
        Ok(())
    }
}

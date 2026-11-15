//! `X_AVM-DE_HostFilter:1`: filter profiles and per-Host Internet blocking
//! (docs/PROJECT.md §4.2.7).
//!
//! The router works by IP; Hosts are given by MAC and the IP is resolved from the host
//! list at call time, with [`hosts::resolve_ip`] (ADR 0002).

use std::net::Ipv4Addr;

use crate::error::FrixError;
use crate::model::Mac;
use crate::services::hosts;
use crate::tr064::{Tr064Client, xml_list_items};

const SERVICE: &str = "X_AVM-DE_HostFilter:1";

/// A filter profile. TR-064 gives only ID and name, not schedules or lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterProfile {
    /// `FilterProfileID` (for example `filtprof1`).
    pub id: String,
    /// `Name`: untrusted data.
    pub name: String,
}

/// The filter profiles (`GetFilterProfiles`), in list order.
///
/// `NewFilterProfileList` is a nested XML list `<List><Item>…`; the exact format
/// is to be verified on the router (docs/PROJECT.md §14.3).
pub async fn filter_profiles(client: &Tr064Client) -> Result<Vec<FilterProfile>, FrixError> {
    let r = client.call(SERVICE, "GetFilterProfiles", &[]).await?;
    xml_list_items(r.text("NewFilterProfileList")?, "Item")?
        .iter()
        .map(|item| {
            let text = |name: &str| item.get(name).map(|t| t.trim().to_owned());
            Ok(FilterProfile {
                id: text("FilterProfileID").ok_or_else(|| {
                    FrixError::Parse("filter profile without FilterProfileID".into())
                })?,
                name: text("Name").unwrap_or_default(),
            })
        })
        .collect()
}

/// Access control status of a Host (`GetWANAccessByIP` + `GetHostEntryByIP`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostAccess {
    /// The given MAC: the Host's identity.
    pub mac: Mac,
    /// The IP resolved from the host list at call time.
    pub ip: Ipv4Addr,
    /// `NewHostName`: untrusted data.
    pub name: String,
    /// `NewWANAccess` from `GetWANAccessByIP`: `granted`, `denied` or `error`.
    pub wan_access: String,
    /// `NewDisallow`: Internet blocked manually for this Host.
    pub wan_access_disallowed: bool,
    /// `NewFilterProfileID`.
    pub filter_profile_id: String,
    /// `NewTimeUsed` (unit to be verified on the router).
    pub time_used: u32,
    /// `NewTimeMax` (unit to be verified on the router).
    pub time_max: u32,
    /// `NewTicketsInAdvance`.
    pub tickets_in_advance: u32,
    /// `NewTicketValid`.
    pub ticket_valid: u32,
    /// `NewIsTimeShared`.
    pub time_shared: bool,
}

/// Access control status of the Host with this MAC.
///
/// The IP is resolved before every call to `X_AVM-DE_HostFilter`: with an unknown MAC,
/// or an IP now held by another active Host, the error comes without calling the service.
pub async fn host_access(client: &Tr064Client, mac: Mac) -> Result<HostAccess, FrixError> {
    let ip = hosts::resolve_ip(client, mac).await?;
    let address = ip.to_string();
    let args = [("NewIPv4Address", address.as_str())];
    let access = client.call(SERVICE, "GetWANAccessByIP", &args).await?;
    let entry = client.call(SERVICE, "GetHostEntryByIP", &args).await?;
    Ok(HostAccess {
        mac,
        ip,
        name: entry.text("NewHostName")?.trim().to_owned(),
        wan_access: access.text("NewWANAccess")?.trim().to_owned(),
        wan_access_disallowed: access.flag("NewDisallow")?,
        filter_profile_id: entry.text("NewFilterProfileID")?.trim().to_owned(),
        time_used: entry.parse("NewTimeUsed")?,
        time_max: entry.parse("NewTimeMax")?,
        tickets_in_advance: entry.parse("NewTicketsInAdvance")?,
        ticket_valid: entry.parse("NewTicketValid")?,
        time_shared: entry.flag("NewIsTimeShared")?,
    })
}

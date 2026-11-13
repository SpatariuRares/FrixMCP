//! `X_AVM-DE_HostFilter:1`: profili di filtro e blocco di Internet per Host
//! (docs/PROJECT.md §4.2.7).
//!
//! Il router lavora per IP; gli Host si indicano per MAC e l'IP si risolve dalla lista
//! host al momento della chiamata, con [`hosts::resolve_ip`] (ADR 0002).

use std::net::Ipv4Addr;

use crate::error::FrixError;
use crate::model::Mac;
use crate::services::hosts;
use crate::tr064::{Tr064Client, xml_list_items};

const SERVICE: &str = "X_AVM-DE_HostFilter:1";

/// Un profilo di filtro. TR-064 dà solo ID e nome, non orari e liste.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterProfile {
    /// `FilterProfileID` (per esempio `filtprof1`).
    pub id: String,
    /// `Name`: dato non fidato.
    pub name: String,
}

/// I profili di filtro (`GetFilterProfiles`), nell'ordine della lista.
///
/// `NewFilterProfileList` è una lista XML annidata `<List><Item>…`; il formato esatto
/// è da verificare sul router (docs/PROJECT.md §14.3).
pub async fn filter_profiles(client: &Tr064Client) -> Result<Vec<FilterProfile>, FrixError> {
    let r = client.call(SERVICE, "GetFilterProfiles", &[]).await?;
    xml_list_items(r.text("NewFilterProfileList")?, "Item")?
        .iter()
        .map(|voce| {
            let testo = |nome: &str| voce.get(nome).map(|t| t.trim().to_owned());
            Ok(FilterProfile {
                id: testo("FilterProfileID").ok_or_else(|| {
                    FrixError::Parse("profilo di filtro senza FilterProfileID".into())
                })?,
                name: testo("Name").unwrap_or_default(),
            })
        })
        .collect()
}

/// Stato del controllo accessi di un Host (`GetWANAccessByIP` + `GetHostEntryByIP`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostAccess {
    /// Il MAC indicato: l'identità dell'Host.
    pub mac: Mac,
    /// L'IP risolto dalla lista host al momento della chiamata.
    pub ip: Ipv4Addr,
    /// `NewHostName`: dato non fidato.
    pub name: String,
    /// `NewWANAccess` di `GetWANAccessByIP`: `granted`, `denied` o `error`.
    pub wan_access: String,
    /// `NewDisallow`: Internet bloccato a mano per questo Host.
    pub wan_access_disallowed: bool,
    /// `NewFilterProfileID`.
    pub filter_profile_id: String,
    /// `NewTimeUsed` (unità da verificare sul router).
    pub time_used: u32,
    /// `NewTimeMax` (unità da verificare sul router).
    pub time_max: u32,
    /// `NewTicketsInAdvance`.
    pub tickets_in_advance: u32,
    /// `NewTicketValid`.
    pub ticket_valid: u32,
    /// `NewIsTimeShared`.
    pub time_shared: bool,
}

/// Stato del controllo accessi dell'Host con questo MAC.
///
/// L'IP si risolve prima di ogni chiamata a `X_AVM-DE_HostFilter`: con un MAC sconosciuto
/// o un IP ora di un altro Host attivo l'errore arriva senza chiamare il servizio.
pub async fn host_access(client: &Tr064Client, mac: Mac) -> Result<HostAccess, FrixError> {
    let ip = hosts::resolve_ip(client, mac).await?;
    let indirizzo = ip.to_string();
    let args = [("NewIPv4Address", indirizzo.as_str())];
    let accesso = client.call(SERVICE, "GetWANAccessByIP", &args).await?;
    let voce = client.call(SERVICE, "GetHostEntryByIP", &args).await?;
    Ok(HostAccess {
        mac,
        ip,
        name: voce.text("NewHostName")?.trim().to_owned(),
        wan_access: accesso.text("NewWANAccess")?.trim().to_owned(),
        wan_access_disallowed: accesso.flag("NewDisallow")?,
        filter_profile_id: voce.text("NewFilterProfileID")?.trim().to_owned(),
        time_used: voce.parse("NewTimeUsed")?,
        time_max: voce.parse("NewTimeMax")?,
        tickets_in_advance: voce.parse("NewTicketsInAdvance")?,
        ticket_valid: voce.parse("NewTicketValid")?,
        time_shared: voce.flag("NewIsTimeShared")?,
    })
}

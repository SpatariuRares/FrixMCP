//! `Host`: una voce della lista host del router (GLOSSARY.md, ADR 0002).

use std::net::Ipv4Addr;

use super::Mac;

/// Una voce della lista host (`X_AVM-DE_GetHostListPath`, TR-064_Hosts.pdf §4.1).
///
/// L'identità è [`Host::mac`]; l'IP è solo l'ultimo noto e può appartenere anche a
/// un'altra voce (ADR 0002). I testi (`name`, `interface_type`, …) sono dati non fidati.
// perché: i flag rispecchiano uno a uno i campi booleani della lista host del router.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Host {
    /// `MACAddress`.
    pub mac: Mac,
    /// `IPAddress`: assente se vuoto o non IPv4.
    pub ip: Option<Ipv4Addr>,
    /// `HostName`.
    pub name: String,
    /// `Active`.
    pub active: bool,
    /// `InterfaceType` (`Ethernet`, `802.11`, …).
    pub interface_type: String,
    /// `AddressSource` (`DHCP`, `Static`, …).
    pub address_source: String,
    /// `X_AVM-DE_Port`: porta LAN (0 se non cablato).
    pub port: Option<u32>,
    /// `X_AVM-DE_Speed`, in Mbit/s.
    pub speed_mbps: Option<u32>,
    /// `X_AVM-DE_Guest`.
    pub guest: bool,
    /// `X_AVM-DE_VPN`.
    pub vpn: bool,
    /// `X_AVM-DE_WANAccess` (`granted`, `denied`, `error`).
    pub wan_access: String,
    /// `X_AVM-DE_Disallow`: accesso a Internet bloccato.
    pub wan_access_disallowed: bool,
    /// `X_AVM-DE_Priority`: Host prioritario.
    pub priority: bool,
    /// `X_AVM-DE_FilterProfileID`, se la lista lo riporta.
    pub filter_profile_id: Option<String>,
    /// `X_AVM-DE_IsMeshable`.
    pub meshable: bool,
}

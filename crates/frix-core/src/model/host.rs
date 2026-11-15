//! `Host`: an entry of the router's host list (GLOSSARY.md, ADR 0002).

use std::net::Ipv4Addr;

use super::Mac;

/// An entry of the host list (`X_AVM-DE_GetHostListPath`, TR-064_Hosts.pdf §4.1).
///
/// The identity is [`Host::mac`]; the IP is only the last known one and may also belong
/// to another entry (ADR 0002). Texts (`name`, `interface_type`, …) are untrusted data.
// why: the flags mirror one to one the boolean fields of the router's host list.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Host {
    /// `MACAddress`.
    pub mac: Mac,
    /// `IPAddress`: absent if empty or not IPv4.
    pub ip: Option<Ipv4Addr>,
    /// `HostName`.
    pub name: String,
    /// `Active`.
    pub active: bool,
    /// `InterfaceType` (`Ethernet`, `802.11`, …).
    pub interface_type: String,
    /// `AddressSource` (`DHCP`, `Static`, …).
    pub address_source: String,
    /// `X_AVM-DE_Port`: LAN port (0 if not wired).
    pub port: Option<u32>,
    /// `X_AVM-DE_Speed`, in Mbit/s.
    pub speed_mbps: Option<u32>,
    /// `X_AVM-DE_Guest`.
    pub guest: bool,
    /// `X_AVM-DE_VPN`.
    pub vpn: bool,
    /// `X_AVM-DE_WANAccess` (`granted`, `denied`, `error`).
    pub wan_access: String,
    /// `X_AVM-DE_Disallow`: Internet access blocked.
    pub wan_access_disallowed: bool,
    /// `X_AVM-DE_Priority`: prioritized Host.
    pub priority: bool,
    /// `X_AVM-DE_FilterProfileID`, if the list reports it.
    pub filter_profile_id: Option<String>,
    /// `X_AVM-DE_IsMeshable`.
    pub meshable: bool,
}

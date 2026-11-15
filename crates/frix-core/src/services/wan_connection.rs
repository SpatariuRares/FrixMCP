//! `WANPPPConnection:1` / `WANIPConnection:1`, chosen at runtime via
//! `Layer3Forwarding:1` (docs/PROJECT.md §4.2.6).

use crate::error::FrixError;
use crate::services::{EntryList, EntryTruncation};
use crate::tr064::{SoapResponse, Tr064Client};

const LAYER3: &str = "Layer3Forwarding:1";
const PPP: &str = "WANPPPConnection:1";
const IP: &str = "WANIPConnection:1";

/// Type of the active Internet connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionKind {
    /// `PPPoE`: `WANPPPConnection`.
    Ppp,
    /// Direct IP: `WANIPConnection`.
    Ip,
}

impl ConnectionKind {
    fn service(self) -> &'static str {
        match self {
            Self::Ppp => PPP,
            Self::Ip => IP,
        }
    }
}

/// `Layer3Forwarding:GetDefaultConnectionService`: which service carries the Internet
/// connection (for example `1.WANPPPConnection.1`).
pub async fn active_kind(client: &Tr064Client) -> Result<ConnectionKind, FrixError> {
    let r = client
        .call(LAYER3, "GetDefaultConnectionService", &[])
        .await?;
    let service = r.text("NewDefaultConnectionService")?;
    if service.contains("WANPPPConnection") {
        Ok(ConnectionKind::Ppp)
    } else if service.contains("WANIPConnection") {
        Ok(ConnectionKind::Ip)
    } else {
        Err(FrixError::Parse(
            "unknown default connection service".into(),
        ))
    }
}

/// Internet connection status. The PPP username is never read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionInfo {
    /// Service the data comes from.
    pub kind: ConnectionKind,
    /// `NewEnable`.
    pub enabled: bool,
    /// `NewConnectionStatus` (for example `Connected`).
    pub status: String,
    /// `NewExternalIPAddress`: public IP.
    pub external_ip: String,
    /// `NewUptime`, in seconds.
    pub uptime_seconds: u64,
    /// `NewLastConnectionError` (for example `ERROR_NONE`).
    pub last_error: String,
    /// `NewDNSServers`, comma-separated in the response.
    pub dns_servers: Vec<String>,
}

/// `GetInfo` of the active connection service.
pub async fn info(client: &Tr064Client, kind: ConnectionKind) -> Result<ConnectionInfo, FrixError> {
    let r = client.call(kind.service(), "GetInfo", &[]).await?;
    Ok(ConnectionInfo {
        kind,
        enabled: r.flag("NewEnable")?,
        status: r.text("NewConnectionStatus")?.to_owned(),
        external_ip: r.text("NewExternalIPAddress")?.to_owned(),
        uptime_seconds: r.parse("NewUptime")?,
        last_error: r.text("NewLastConnectionError")?.to_owned(),
        dns_servers: r
            .text("NewDNSServers")?
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect(),
    })
}

/// A port mapping, as reported by the router.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortMapping {
    /// `NewPortMappingDescription`.
    pub description: String,
    /// `NewEnabled`.
    pub enabled: bool,
    /// `NewProtocol` (`TCP`, `UDP`).
    pub protocol: String,
    /// `NewExternalPort`.
    pub external_port: u16,
    /// `NewInternalClient`: address of the Host in the LAN.
    pub internal_client: String,
    /// `NewInternalPort`.
    pub internal_port: u16,
    /// `NewRemoteHost`: empty if the mapping applies to every remote host.
    pub remote_host: String,
    /// `NewLeaseDuration`, in seconds (0 = permanent).
    pub lease_duration_seconds: u32,
}

/// Maximum port mappings read: beyond that, the list is reported as truncated. A FRITZ!Box
/// handles a few dozen; the cap avoids thousands of calls if the router
/// declares an absurd number.
pub const MAX_PORT_MAPPINGS: usize = 256;

/// The port mappings of connection service `kind`, in index order, at most
/// [`MAX_PORT_MAPPINGS`]: `GetPortMappingNumberOfEntries`, then `GetGenericPortMappingEntry`
/// for each index. An unparsable entry is skipped and reported.
pub async fn port_mappings(
    client: &Tr064Client,
    kind: ConnectionKind,
) -> Result<EntryList<PortMapping>, FrixError> {
    let service = kind.service();
    let count: u16 = client
        .call(service, "GetPortMappingNumberOfEntries", &[])
        .await?
        .parse("NewPortMappingNumberOfEntries")?;
    let total = usize::from(count);
    let mut mappings = EntryList::default();
    for index in (0..count).take(MAX_PORT_MAPPINGS) {
        let r = client
            .call(
                service,
                "GetGenericPortMappingEntry",
                &[("NewPortMappingIndex", &index.to_string())],
            )
            .await?;
        mappings.push(index, port_mapping(&r))?;
    }
    if total > MAX_PORT_MAPPINGS {
        mappings.truncation = Some(EntryTruncation {
            limit: MAX_PORT_MAPPINGS,
            omitted: Some(total - MAX_PORT_MAPPINGS),
        });
    }
    Ok(mappings)
}

fn port_mapping(r: &SoapResponse) -> Result<PortMapping, FrixError> {
    Ok(PortMapping {
        description: r.text("NewPortMappingDescription")?.to_owned(),
        enabled: r.flag("NewEnabled")?,
        protocol: r.text("NewProtocol")?.to_owned(),
        external_port: r.parse("NewExternalPort")?,
        internal_client: r.text("NewInternalClient")?.to_owned(),
        internal_port: r.parse("NewInternalPort")?,
        remote_host: r.text("NewRemoteHost")?.to_owned(),
        lease_duration_seconds: r.parse("NewLeaseDuration")?,
    })
}

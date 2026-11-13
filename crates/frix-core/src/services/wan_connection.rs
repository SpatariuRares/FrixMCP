//! `WANPPPConnection:1` / `WANIPConnection:1`, scelti a runtime con
//! `Layer3Forwarding:1` (docs/PROJECT.md §4.2.6).

use crate::error::FrixError;
use crate::services::{EntryList, EntryTruncation};
use crate::tr064::{SoapResponse, Tr064Client};

const LAYER3: &str = "Layer3Forwarding:1";
const PPP: &str = "WANPPPConnection:1";
const IP: &str = "WANIPConnection:1";

/// Tipo della connessione Internet attiva.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionKind {
    /// `PPPoE`: `WANPPPConnection`.
    Ppp,
    /// IP diretto: `WANIPConnection`.
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

/// `Layer3Forwarding:GetDefaultConnectionService`: quale servizio porta la connessione
/// Internet (per esempio `1.WANPPPConnection.1`).
pub async fn active_kind(client: &Tr064Client) -> Result<ConnectionKind, FrixError> {
    let r = client
        .call(LAYER3, "GetDefaultConnectionService", &[])
        .await?;
    let servizio = r.text("NewDefaultConnectionService")?;
    if servizio.contains("WANPPPConnection") {
        Ok(ConnectionKind::Ppp)
    } else if servizio.contains("WANIPConnection") {
        Ok(ConnectionKind::Ip)
    } else {
        Err(FrixError::Parse(
            "servizio di connessione predefinito sconosciuto".into(),
        ))
    }
}

/// Stato della connessione Internet. Il nome utente PPP non viene mai letto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionInfo {
    /// Servizio da cui vengono i dati.
    pub kind: ConnectionKind,
    /// `NewEnable`.
    pub enabled: bool,
    /// `NewConnectionStatus` (per esempio `Connected`).
    pub status: String,
    /// `NewExternalIPAddress`: IP pubblico.
    pub external_ip: String,
    /// `NewUptime`, in secondi.
    pub uptime_seconds: u64,
    /// `NewLastConnectionError` (per esempio `ERROR_NONE`).
    pub last_error: String,
    /// `NewDNSServers`, separati da virgola nella risposta.
    pub dns_servers: Vec<String>,
}

/// `GetInfo` del servizio di connessione attivo.
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

/// Un inoltro porta, così come lo riporta il router.
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
    /// `NewInternalClient`: indirizzo dell'Host nella LAN.
    pub internal_client: String,
    /// `NewInternalPort`.
    pub internal_port: u16,
    /// `NewRemoteHost`: vuoto se l'inoltro vale per ogni host remoto.
    pub remote_host: String,
    /// `NewLeaseDuration`, in secondi (0 = permanente).
    pub lease_duration_seconds: u32,
}

/// Inoltri porta letti al massimo: oltre, la lista si dichiara troncata. Un FRITZ!Box
/// ne gestisce poche decine; il tetto evita migliaia di chiamate se il router
/// dichiara un numero assurdo.
pub const MAX_PORT_MAPPINGS: usize = 256;

/// Gli inoltri porta del servizio di connessione `kind`, in ordine di indice, al massimo
/// [`MAX_PORT_MAPPINGS`]: `GetPortMappingNumberOfEntries`, poi `GetGenericPortMappingEntry`
/// per ogni indice. Una voce non interpretabile si salta e si dichiara.
pub async fn port_mappings(
    client: &Tr064Client,
    kind: ConnectionKind,
) -> Result<EntryList<PortMapping>, FrixError> {
    let service = kind.service();
    let numero: u16 = client
        .call(service, "GetPortMappingNumberOfEntries", &[])
        .await?
        .parse("NewPortMappingNumberOfEntries")?;
    let totale = usize::from(numero);
    let mut inoltri = EntryList::default();
    for indice in (0..numero).take(MAX_PORT_MAPPINGS) {
        let r = client
            .call(
                service,
                "GetGenericPortMappingEntry",
                &[("NewPortMappingIndex", &indice.to_string())],
            )
            .await?;
        inoltri.push(indice, port_mapping(&r))?;
    }
    if totale > MAX_PORT_MAPPINGS {
        inoltri.truncation = Some(EntryTruncation {
            limit: MAX_PORT_MAPPINGS,
            omitted: Some(totale - MAX_PORT_MAPPINGS),
        });
    }
    Ok(inoltri)
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

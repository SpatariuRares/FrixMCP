//! `WANCommonInterfaceConfig:1` (docs/PROJECT.md §4.2.4).

use crate::error::FrixError;
use crate::tr064::Tr064Client;

const SERVICE: &str = "WANCommonInterfaceConfig:1";

/// Proprietà del link WAN.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkProperties {
    /// `NewWANAccessType` (per esempio `DSL`, `Ethernet`).
    pub access_type: String,
    /// `NewPhysicalLinkStatus` (per esempio `Up`).
    pub physical_status: String,
    /// `NewLayer1UpstreamMaxBitRate`, in bit/s.
    pub upstream_max_bps: u64,
    /// `NewLayer1DownstreamMaxBitRate`, in bit/s.
    pub downstream_max_bps: u64,
}

/// I contatori di traffico TR-064 sono a 32 bit: ripartono da zero a questo valore.
pub const COUNTER_WRAP: u64 = 1 << 32;

/// Un gruppo di sincronizzazione del monitor online (il "grafico di traffico" della UI AVM).
/// Le serie sono i valori degli ultimi secondi, come li restituisce il router.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnlineMonitor {
    /// `NewSyncGroupName`.
    pub sync_group_name: String,
    /// `NewSyncGroupMode` (per esempio `VDSL`).
    pub sync_group_mode: String,
    /// `Newmax_ds`.
    pub max_downstream_bps: u64,
    /// `Newmax_us`.
    pub max_upstream_bps: u64,
    /// `Newds_current_bps`.
    pub downstream_bps: Vec<u64>,
    /// `Newmc_current_bps`: multicast in download (per esempio IPTV).
    pub downstream_multicast_bps: Vec<u64>,
    /// `Newus_current_bps`.
    pub upstream_bps: Vec<u64>,
    /// `Newprio_realtime_bps`: upload a priorità tempo reale.
    pub upstream_realtime_bps: Vec<u64>,
    /// `Newprio_high_bps`.
    pub upstream_high_bps: Vec<u64>,
    /// `Newprio_default_bps`.
    pub upstream_default_bps: Vec<u64>,
    /// `Newprio_low_bps`.
    pub upstream_low_bps: Vec<u64>,
}

/// `X_AVM-DE_GetOnlineMonitor` per il primo gruppo di sincronizzazione.
pub async fn online_monitor(client: &Tr064Client) -> Result<OnlineMonitor, FrixError> {
    let r = client
        .call(
            SERVICE,
            "X_AVM-DE_GetOnlineMonitor",
            &[("NewSyncGroupIndex", "0")],
        )
        .await?;
    let serie = |nome: &str| -> Result<Vec<u64>, FrixError> {
        r.text(nome)?
            .split(',')
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(|v| {
                v.parse()
                    .map_err(|_| FrixError::Parse(format!("valore non valido in {nome}")))
            })
            .collect()
    };
    Ok(OnlineMonitor {
        sync_group_name: r.text("NewSyncGroupName")?.to_owned(),
        sync_group_mode: r.text("NewSyncGroupMode")?.to_owned(),
        max_downstream_bps: r.parse("Newmax_ds")?,
        max_upstream_bps: r.parse("Newmax_us")?,
        downstream_bps: serie("Newds_current_bps")?,
        downstream_multicast_bps: serie("Newmc_current_bps")?,
        upstream_bps: serie("Newus_current_bps")?,
        upstream_realtime_bps: serie("Newprio_realtime_bps")?,
        upstream_high_bps: serie("Newprio_high_bps")?,
        upstream_default_bps: serie("Newprio_default_bps")?,
        upstream_low_bps: serie("Newprio_low_bps")?,
    })
}

/// `GetTotalBytesSent`: byte inviati, modulo [`COUNTER_WRAP`].
pub async fn total_bytes_sent(client: &Tr064Client) -> Result<u64, FrixError> {
    let r = client.call(SERVICE, "GetTotalBytesSent", &[]).await?;
    counter(r.parse("NewTotalBytesSent")?)
}

/// `GetTotalBytesReceived`: byte ricevuti, modulo [`COUNTER_WRAP`].
pub async fn total_bytes_received(client: &Tr064Client) -> Result<u64, FrixError> {
    let r = client.call(SERVICE, "GetTotalBytesReceived", &[]).await?;
    counter(r.parse("NewTotalBytesReceived")?)
}

/// Un contatore a 32 bit oltre 2^31 può arrivare come intero con segno negativo:
/// lo si riporta nell'intervallo `0..2^32` invece di restituire un valore negativo.
fn counter(valore: i64) -> Result<u64, FrixError> {
    let modulo = i64::try_from(COUNTER_WRAP)
        .map_err(|_| FrixError::Parse("contatore fuori intervallo".into()))?;
    u64::try_from(valore.rem_euclid(modulo))
        .map_err(|_| FrixError::Parse("contatore fuori intervallo".into()))
}

/// `GetCommonLinkProperties`.
pub async fn link_properties(client: &Tr064Client) -> Result<LinkProperties, FrixError> {
    let r = client.call(SERVICE, "GetCommonLinkProperties", &[]).await?;
    Ok(LinkProperties {
        access_type: r.text("NewWANAccessType")?.to_owned(),
        physical_status: r.text("NewPhysicalLinkStatus")?.to_owned(),
        upstream_max_bps: r.parse("NewLayer1UpstreamMaxBitRate")?,
        downstream_max_bps: r.parse("NewLayer1DownstreamMaxBitRate")?,
    })
}

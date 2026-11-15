//! `WANCommonInterfaceConfig:1` (docs/PROJECT.md §4.2.4).

use crate::error::FrixError;
use crate::tr064::Tr064Client;

const SERVICE: &str = "WANCommonInterfaceConfig:1";

/// WAN link properties.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkProperties {
    /// `NewWANAccessType` (for example `DSL`, `Ethernet`).
    pub access_type: String,
    /// `NewPhysicalLinkStatus` (for example `Up`).
    pub physical_status: String,
    /// `NewLayer1UpstreamMaxBitRate`, in bit/s (see [`link_properties`] for Ethernet).
    pub upstream_max_bps: u64,
    /// `NewLayer1DownstreamMaxBitRate`, in bit/s (see [`link_properties`] for Ethernet).
    pub downstream_max_bps: u64,
}

/// TR-064 traffic counters are 32-bit: they wrap to zero at this value.
pub const COUNTER_WRAP: u64 = 1 << 32;

/// A sync group of the online monitor (the "traffic graph" of the AVM UI).
/// The series are the values of the last seconds, as returned by the router.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnlineMonitor {
    /// `NewSyncGroupName`.
    pub sync_group_name: String,
    /// `NewSyncGroupMode` (for example `VDSL`).
    pub sync_group_mode: String,
    /// `Newmax_ds`.
    pub max_downstream_bps: u64,
    /// `Newmax_us`.
    pub max_upstream_bps: u64,
    /// `Newds_current_bps`.
    pub downstream_bps: Vec<u64>,
    /// `Newmc_current_bps`: downstream multicast (for example IPTV).
    pub downstream_multicast_bps: Vec<u64>,
    /// `Newus_current_bps`.
    pub upstream_bps: Vec<u64>,
    /// `Newprio_realtime_bps`: upstream at real-time priority.
    pub upstream_realtime_bps: Vec<u64>,
    /// `Newprio_high_bps`.
    pub upstream_high_bps: Vec<u64>,
    /// `Newprio_default_bps`.
    pub upstream_default_bps: Vec<u64>,
    /// `Newprio_low_bps`.
    pub upstream_low_bps: Vec<u64>,
}

/// `X_AVM-DE_GetOnlineMonitor` for the first sync group.
pub async fn online_monitor(client: &Tr064Client) -> Result<OnlineMonitor, FrixError> {
    let r = client
        .call(
            SERVICE,
            "X_AVM-DE_GetOnlineMonitor",
            &[("NewSyncGroupIndex", "0")],
        )
        .await?;
    let series = |name: &str| -> Result<Vec<u64>, FrixError> {
        r.text(name)?
            .split(',')
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(|v| {
                v.parse()
                    .map_err(|_| FrixError::Parse(format!("invalid value in {name}")))
            })
            .collect()
    };
    Ok(OnlineMonitor {
        sync_group_name: r.text("NewSyncGroupName")?.to_owned(),
        sync_group_mode: r.text("NewSyncGroupMode")?.to_owned(),
        max_downstream_bps: r.parse("Newmax_ds")?,
        max_upstream_bps: r.parse("Newmax_us")?,
        downstream_bps: series("Newds_current_bps")?,
        downstream_multicast_bps: series("Newmc_current_bps")?,
        upstream_bps: series("Newus_current_bps")?,
        upstream_realtime_bps: series("Newprio_realtime_bps")?,
        upstream_high_bps: series("Newprio_high_bps")?,
        upstream_default_bps: series("Newprio_default_bps")?,
        upstream_low_bps: series("Newprio_low_bps")?,
    })
}

/// `GetTotalBytesSent`: bytes sent, modulo [`COUNTER_WRAP`].
pub async fn total_bytes_sent(client: &Tr064Client) -> Result<u64, FrixError> {
    let r = client.call(SERVICE, "GetTotalBytesSent", &[]).await?;
    counter(r.parse("NewTotalBytesSent")?)
}

/// `GetTotalBytesReceived`: bytes received, modulo [`COUNTER_WRAP`].
pub async fn total_bytes_received(client: &Tr064Client) -> Result<u64, FrixError> {
    let r = client.call(SERVICE, "GetTotalBytesReceived", &[]).await?;
    counter(r.parse("NewTotalBytesReceived")?)
}

/// A 32-bit counter above 2^31 may arrive as a negative signed integer:
/// it is mapped back into `0..2^32` instead of returning a negative value.
fn counter(value: i64) -> Result<u64, FrixError> {
    let modulus =
        i64::try_from(COUNTER_WRAP).map_err(|_| FrixError::Parse("counter out of range".into()))?;
    u64::try_from(value.rem_euclid(modulus))
        .map_err(|_| FrixError::Parse("counter out of range".into()))
}

/// `GetCommonLinkProperties`.
///
/// On a DSL line the Layer 1 rates are in bit/s. On an Ethernet WAN the 7590
/// (FRITZ!OS 8.25) gives the port speed in kbit/s: `1000000` for a Gigabit port, while
/// on the same line the PPP maximum is `905660` kbit/s and the online monitor's maximum
/// about 1 Gbit/s. Here both become bit/s.
pub async fn link_properties(client: &Tr064Client) -> Result<LinkProperties, FrixError> {
    let r = client.call(SERVICE, "GetCommonLinkProperties", &[]).await?;
    let access_type = r.text("NewWANAccessType")?.to_owned();
    let to_bps = |value: u64| -> Result<u64, FrixError> {
        if access_type == "Ethernet" {
            value
                .checked_mul(1000)
                .ok_or_else(|| FrixError::Parse("link rate out of range".into()))
        } else {
            Ok(value)
        }
    };
    Ok(LinkProperties {
        physical_status: r.text("NewPhysicalLinkStatus")?.to_owned(),
        upstream_max_bps: to_bps(r.parse("NewLayer1UpstreamMaxBitRate")?)?,
        downstream_max_bps: to_bps(r.parse("NewLayer1DownstreamMaxBitRate")?)?,
        access_type,
    })
}

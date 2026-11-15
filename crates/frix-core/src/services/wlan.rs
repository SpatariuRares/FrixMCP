//! `WLANConfiguration:1..3`: the router's Wi-Fi Networks (docs/PROJECT.md §4.2.3).
//!
//! Which `WLANConfiguration` instance is which Wi-Fi Network is not assumed
//! (docs/PROJECT.md §4.1): `X_AVM-DE_APType` from `X_AVM-DE_GetWLANExtInfo` and the
//! instance's Band tell, read on every call. The 7590 with FRITZ!OS 8.25 answers
//! `unknown` as `X_AVM-DE_FrequencyBand` on the guest network (§14.3): the Band is then
//! derived from the channel.
//!
//! `GetSecurityKeys` and `X_AVM-DE_GetWLANHybridMode` expose Wi-Fi keys: this
//! module does not call them, and the Policy denies them anyway (Level Dangerous).

use std::fmt;
use std::str::FromStr;

use std::collections::BTreeMap;

use crate::error::FrixError;
use crate::model::Mac;
use crate::services::list_path;
use crate::tr064::{Tr064Client, xml_list_items};

const SERVICE: &str = "WLANConfiguration";

/// One of the router's three Wi-Fi Networks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WifiNetwork {
    /// Main network at 2.4 GHz (`main_2g`).
    Main2g,
    /// Main network at 5 GHz (`main_5g`).
    Main5g,
    /// Guest network (`guest`).
    Guest,
}

impl WifiNetwork {
    /// The three Wi-Fi Networks, in the order they are returned.
    pub const ALL: [Self; 3] = [Self::Main2g, Self::Main5g, Self::Guest];

    /// Stable name for the Agent: `main_2g`, `main_5g`, `guest`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Main2g => "main_2g",
            Self::Main5g => "main_5g",
            Self::Guest => "guest",
        }
    }

    /// Agent filter: `main_2g`, `main_5g`, `guest` or `all` (the three networks).
    /// Any other value is a validation error.
    pub fn parse_filter(value: &str) -> Result<Vec<Self>, FrixError> {
        if value == "all" {
            Ok(Self::ALL.to_vec())
        } else {
            value.parse().map(|net| vec![net])
        }
    }
}

impl FromStr for WifiNetwork {
    type Err = FrixError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|net| net.as_str() == value)
            .ok_or_else(|| {
                FrixError::Validation(format!(
                    "invalid Wi-Fi Network: {value:?} (allowed: main_2g, main_5g, guest, all)"
                ))
            })
    }
}

impl fmt::Display for WifiNetwork {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The Band a Wi-Fi Network transmits on (`X_AVM-DE_FrequencyBand`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Band {
    /// `2400`.
    Ghz2_4,
    /// `5000`.
    Ghz5,
    /// `6000`.
    Ghz6,
    /// `unknown` or a value the SCPD does not define.
    Unknown,
}

impl Band {
    fn from_router(value: &str) -> Self {
        match value.trim() {
            "2400" => Self::Ghz2_4,
            "5000" => Self::Ghz5,
            "6000" => Self::Ghz6,
            _ => Self::Unknown,
        }
    }

    /// The Band of a channel: 1-14 exist only at 2.4 GHz, 32-177 only at 5 GHz.
    /// 6 GHz channels are also numbered from 1, so on a router with a 6 GHz radio
    /// this rule would not suffice; the 7590 has none.
    fn from_channel(channel: u8) -> Self {
        match channel {
            1..=14 => Self::Ghz2_4,
            32..=177 => Self::Ghz5,
            _ => Self::Unknown,
        }
    }

    /// Stable name for the Agent: `2.4GHz`, `5GHz`, `6GHz`, `unknown`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ghz2_4 => "2.4GHz",
            Self::Ghz5 => "5GHz",
            Self::Ghz6 => "6GHz",
            Self::Unknown => "unknown",
        }
    }
}

/// Status of a Wi-Fi Network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WifiStatus {
    /// The Wi-Fi Network, derived from the router's data.
    pub network: WifiNetwork,
    /// `NewEnable`.
    pub enabled: bool,
    /// `NewStatus` (`Up`, `Disabled`).
    pub status: String,
    /// `NewSSID`: untrusted data.
    pub ssid: String,
    /// `NewX_AVM-DE_FrequencyBand`.
    pub band: Band,
    /// `NewChannel` from `GetChannelInfo`.
    pub channel: u8,
    /// `NewX_AVM-DE_AutoChannelEnabled`.
    pub auto_channel: bool,
    /// `NewStandard` (`n`, `ac`, `ax`, …).
    pub standard: String,
    /// `NewBeaconType`: the encryption (`11i`, `11iandWPA3`, …).
    pub encryption: String,
    /// `NewMaxBitRate`.
    pub max_bit_rate: String,
    /// `NewTotalAssociations`: associated clients.
    pub clients: u16,
    /// Only for the guest Wi-Fi Network: timeout and isolation.
    pub guest: Option<GuestSettings>,
}

/// Settings of the guest Wi-Fi Network (`X_AVM-DE_GetWLANExtInfo`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuestSettings {
    /// `NewX_AVM-DE_TimeoutActive`: the network turns itself off.
    pub timeout_active: bool,
    /// `NewX_AVM-DE_Timeout`, in minutes; `None` if the router gives no number.
    pub timeout_minutes: Option<u32>,
    /// `NewX_AVM-DE_TimeRemain`, in minutes; `None` if the router gives no number.
    pub time_remaining_minutes: Option<u32>,
    /// `NewX_AVM-DE_NoForcedOff`: the timeout does not fire while clients are connected.
    pub no_forced_off: bool,
    /// `NewX_AVM-DE_UserIsolation`: clients cannot see each other.
    pub user_isolation: bool,
}

/// A client associated with a Wi-Fi Network (`X_AVM-DE_GetWLANDeviceListPath` or
/// `GetGenericAssociatedDeviceInfo`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WifiClient {
    /// The Wi-Fi Network it is associated with.
    pub network: WifiNetwork,
    /// The Band of the Wi-Fi Network.
    pub band: Band,
    /// `NewAssociatedDeviceMACAddress`.
    pub mac: Mac,
    /// `NewAssociatedDeviceIPAddress`.
    pub ip: String,
    /// `NewAssociatedDeviceAuthState`.
    pub authenticated: bool,
    /// `NewX_AVM-DE_Speed`, in Mbit/s.
    pub speed_mbps: u16,
    /// `NewX_AVM-DE_SignalStrength`, as reported by the router.
    pub signal_strength: u8,
    /// `NewX_AVM-DE_ChannelWidth`, in MHz.
    pub channel_width_mhz: u16,
}

/// A Wi-Fi Network found on the router, with the `WLANConfiguration:N` service serving it.
struct FoundNetwork {
    network: WifiNetwork,
    service: String,
    band: Band,
    guest: Option<GuestSettings>,
}

/// Status of the requested Wi-Fi Networks, in [`WifiNetwork::ALL`] order.
///
/// `GetInfo`, `GetChannelInfo` and `GetTotalAssociations` on each requested instance.
pub async fn status(
    client: &Tr064Client,
    networks: &[WifiNetwork],
) -> Result<Vec<WifiStatus>, FrixError> {
    let mut result = Vec::new();
    for net in found_networks(client, networks).await? {
        let info = client.call(&net.service, "GetInfo", &[]).await?;
        let channel_info = client.call(&net.service, "GetChannelInfo", &[]).await?;
        result.push(WifiStatus {
            network: net.network,
            enabled: info.flag("NewEnable")?,
            status: info.text("NewStatus")?.to_owned(),
            ssid: info.text("NewSSID")?.to_owned(),
            band: net.band,
            channel: channel_info.parse("NewChannel")?,
            auto_channel: channel_info.flag("NewX_AVM-DE_AutoChannelEnabled")?,
            standard: info.text("NewStandard")?.to_owned(),
            encryption: info.text("NewBeaconType")?.to_owned(),
            max_bit_rate: info.text("NewMaxBitRate")?.to_owned(),
            clients: total_associations(client, &net.service).await?,
            guest: net.guest,
        });
    }
    Ok(result)
}

/// The clients associated with the requested Wi-Fi Networks.
///
/// First `X_AVM-DE_GetWLANDeviceListPath` and a `GET` of the list (one call for all
/// clients). The 7590 gives every instance the same path, with the clients of all Wi-Fi
/// Networks (§14.3): the list is read once and each entry goes to the Wi-Fi Network
/// given by `AssociatedDeviceGuest` and `AssociatedDeviceChannel`. If the list
/// fails, `GetTotalAssociations` and `GetGenericAssociatedDeviceInfo` for each index
/// of each instance (§8.1); if both fail, the error is the list's.
pub async fn clients(
    client: &Tr064Client,
    networks: &[WifiNetwork],
) -> Result<Vec<WifiClient>, FrixError> {
    let nets = found_networks(client, networks).await?;
    let Some(first) = nets.first() else {
        return Ok(Vec::new());
    };
    let mut result = Vec::new();
    match entries_from_list(client, &first.service).await {
        Ok(entries) => {
            for entry in &entries {
                if let Some(net) = network_of_entry(&nets, entry)? {
                    result.push(wifi_client(net, entry)?);
                }
            }
            result.sort_by_key(|c| c.network);
        }
        Err(error) => {
            for net in &nets {
                let entries = entries_by_index(client, &net.service)
                    .await
                    .map_err(|_| error.clone())?;
                for entry in &entries {
                    result.push(wifi_client(net, entry)?);
                }
            }
        }
    }
    Ok(result)
}

/// The requested Wi-Fi Network of a list entry: guest if `AssociatedDeviceGuest`
/// is 1, otherwise the main one for the Band of `AssociatedDeviceChannel`. `None` if the
/// Wi-Fi Network is not among those requested or the channel does not tell the Band.
fn network_of_entry<'a>(
    nets: &'a [FoundNetwork],
    entry: &BTreeMap<String, String>,
) -> Result<Option<&'a FoundNetwork>, FrixError> {
    let field = |name: &str| {
        entry
            .get(name)
            .map(|t| t.trim())
            .ok_or_else(|| FrixError::Parse(format!("Wi-Fi client without {name}")))
    };
    let network = if field("AssociatedDeviceGuest")? == "1" {
        WifiNetwork::Guest
    } else {
        let channel = field("AssociatedDeviceChannel")?
            .parse()
            .map_err(|_| FrixError::Parse("invalid value for AssociatedDeviceChannel".into()))?;
        match Band::from_channel(channel) {
            Band::Ghz2_4 => WifiNetwork::Main2g,
            Band::Ghz5 => WifiNetwork::Main5g,
            _ => return Ok(None),
        }
    };
    Ok(nets.iter().find(|n| n.network == network))
}

/// The fields of a client, with the list entry names (the
/// `GetGenericAssociatedDeviceInfo` arguments without `New`).
const CLIENT_FIELDS: [&str; 6] = [
    "AssociatedDeviceMACAddress",
    "AssociatedDeviceIPAddress",
    "AssociatedDeviceAuthState",
    "X_AVM-DE_Speed",
    "X_AVM-DE_SignalStrength",
    "X_AVM-DE_ChannelWidth",
];

async fn entries_from_list(
    client: &Tr064Client,
    service: &str,
) -> Result<Vec<BTreeMap<String, String>>, FrixError> {
    let r = client
        .call(service, "X_AVM-DE_GetWLANDeviceListPath", &[])
        .await?;
    let document = list_path::download(client, r.text("NewX_AVM-DE_WLANDeviceListPath")?).await?;
    xml_list_items(&document, "Item")
}

async fn entries_by_index(
    client: &Tr064Client,
    service: &str,
) -> Result<Vec<BTreeMap<String, String>>, FrixError> {
    let mut entries = Vec::new();
    for index in 0..total_associations(client, service).await? {
        let r = client
            .call(
                service,
                "GetGenericAssociatedDeviceInfo",
                &[("NewAssociatedDeviceIndex", &index.to_string())],
            )
            .await?;
        let mut entry = BTreeMap::new();
        for field in CLIENT_FIELDS {
            entry.insert(field.to_owned(), r.text(&format!("New{field}"))?.to_owned());
        }
        entries.push(entry);
    }
    Ok(entries)
}

fn wifi_client(
    net: &FoundNetwork,
    entry: &BTreeMap<String, String>,
) -> Result<WifiClient, FrixError> {
    let text = |name: &str| {
        entry
            .get(name)
            .map(|t| t.trim())
            .ok_or_else(|| FrixError::Parse(format!("Wi-Fi client without {name}")))
    };
    let number = |name: &str| -> Result<u16, FrixError> {
        text(name)?
            .parse()
            .map_err(|_| FrixError::Parse(format!("invalid value for {name}")))
    };
    Ok(WifiClient {
        network: net.network,
        band: net.band,
        mac: text("AssociatedDeviceMACAddress")?
            .parse()
            .map_err(|_| FrixError::Parse("invalid MAC of a Wi-Fi client".into()))?,
        ip: text("AssociatedDeviceIPAddress")?.to_owned(),
        authenticated: text("AssociatedDeviceAuthState")? == "1",
        speed_mbps: number("X_AVM-DE_Speed")?,
        signal_strength: text("X_AVM-DE_SignalStrength")?
            .parse()
            .map_err(|_| FrixError::Parse("invalid value for X_AVM-DE_SignalStrength".into()))?,
        channel_width_mhz: number("X_AVM-DE_ChannelWidth")?,
    })
}

/// The Band of an instance: `X_AVM-DE_FrequencyBand` from `X_AVM-DE_GetWLANExtInfo`
/// (`from_ext_info`), then the one from `GetChannelInfo`, then the channel. The 7590 with
/// FRITZ!OS 8.25 answers `unknown` in both fields on the guest instance.
async fn instance_band(
    client: &Tr064Client,
    service: &str,
    from_ext_info: &str,
) -> Result<Band, FrixError> {
    let band = Band::from_router(from_ext_info);
    if band != Band::Unknown {
        return Ok(band);
    }
    let channel_info = client.call(service, "GetChannelInfo", &[]).await?;
    let band = channel_info
        .text("NewX_AVM-DE_FrequencyBand")
        .map_or(Band::Unknown, Band::from_router);
    if band != Band::Unknown {
        return Ok(band);
    }
    Ok(Band::from_channel(channel_info.parse("NewChannel")?))
}

async fn total_associations(client: &Tr064Client, service: &str) -> Result<u16, FrixError> {
    client
        .call(service, "GetTotalAssociations", &[])
        .await?
        .parse("NewTotalAssociations")
}

/// The requested Wi-Fi Networks present on the router, in [`WifiNetwork::ALL`] order.
///
/// Each discovered `WLANConfiguration` instance declares via `X_AVM-DE_GetWLANExtInfo`
/// the AP type (`normal`, `guest`) and the Band: guest is the `guest` AP, the main ones
/// are the `normal` APs at 2.4 and 5 GHz. An instance that fits none of the three
/// cases is ignored; two instances for the same Wi-Fi Network are an error. A Wi-Fi
/// Network requested explicitly and missing from the router is a validation error.
async fn found_networks(
    client: &Tr064Client,
    networks: &[WifiNetwork],
) -> Result<Vec<FoundNetwork>, FrixError> {
    let mut found: Vec<FoundNetwork> = Vec::new();
    for service in client.instances(SERVICE) {
        let r = client
            .call(&service, "X_AVM-DE_GetWLANExtInfo", &[])
            .await?;
        let band = instance_band(client, &service, r.text("NewX_AVM-DE_FrequencyBand")?).await?;
        let network = match (r.text("NewX_AVM-DE_APType")?.trim(), band) {
            ("guest", _) => WifiNetwork::Guest,
            ("normal", Band::Ghz2_4) => WifiNetwork::Main2g,
            ("normal", Band::Ghz5) => WifiNetwork::Main5g,
            _ => continue,
        };
        if let Some(duplicate) = found.iter().find(|i| i.network == network) {
            return Err(FrixError::Parse(format!(
                "{} and {service} both declare the Wi-Fi Network {network}",
                duplicate.service
            )));
        }
        let guest = if network == WifiNetwork::Guest {
            Some(GuestSettings {
                timeout_active: r.flag("NewX_AVM-DE_TimeoutActive")?,
                timeout_minutes: r.text("NewX_AVM-DE_Timeout")?.trim().parse().ok(),
                time_remaining_minutes: r.text("NewX_AVM-DE_TimeRemain")?.trim().parse().ok(),
                no_forced_off: r.flag("NewX_AVM-DE_NoForcedOff")?,
                user_isolation: r.flag("NewX_AVM-DE_UserIsolation")?,
            })
        } else {
            None
        };
        found.push(FoundNetwork {
            network,
            service,
            band,
            guest,
        });
    }
    if networks.len() == 1
        && let Some(net) = networks.first()
        && !found.iter().any(|i| i.network == *net)
    {
        return Err(FrixError::Validation(format!(
            "the router has no WLANConfiguration instance for the Wi-Fi Network {net}"
        )));
    }
    found.retain(|i| networks.contains(&i.network));
    found.sort_by_key(|i| i.network);
    Ok(found)
}

//! `WLANConfiguration:1..3`: le Wi-Fi Network del router (docs/PROJECT.md §4.2.3).
//!
//! Quale istanza `WLANConfiguration` sia quale Wi-Fi Network non si presume
//! (docs/PROJECT.md §4.1): lo dicono `X_AVM-DE_APType` di `X_AVM-DE_GetWLANExtInfo`
//! e la Band dell'istanza, letti a ogni chiamata. Il 7590 con FRITZ!OS 8.25 risponde
//! `unknown` come `X_AVM-DE_FrequencyBand` sulla rete Ospiti (§14.3): la Band si ricava
//! allora dal canale.
//!
//! `GetSecurityKeys` e `X_AVM-DE_GetWLANHybridMode` espongono chiavi Wi-Fi: questo
//! modulo non le chiama, e la Policy le nega comunque (Level Dangerous).

use std::fmt;
use std::str::FromStr;

use std::collections::BTreeMap;

use crate::error::FrixError;
use crate::model::Mac;
use crate::services::list_path;
use crate::tr064::{Tr064Client, xml_list_items};

const SERVICE: &str = "WLANConfiguration";

/// Una delle tre Wi-Fi Network del router.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WifiNetwork {
    /// Rete principale a 2,4 GHz (`main_2g`).
    Main2g,
    /// Rete principale a 5 GHz (`main_5g`).
    Main5g,
    /// Rete Ospiti (`guest`).
    Guest,
}

impl WifiNetwork {
    /// Le tre Wi-Fi Network, nell'ordine in cui si restituiscono.
    pub const ALL: [Self; 3] = [Self::Main2g, Self::Main5g, Self::Guest];

    /// Nome stabile verso l'Agent: `main_2g`, `main_5g`, `guest`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Main2g => "main_2g",
            Self::Main5g => "main_5g",
            Self::Guest => "guest",
        }
    }

    /// Filtro dell'Agent: `main_2g`, `main_5g`, `guest` oppure `all` (le tre reti).
    /// Ogni altro valore è un errore di validazione.
    pub fn parse_filter(valore: &str) -> Result<Vec<Self>, FrixError> {
        if valore == "all" {
            Ok(Self::ALL.to_vec())
        } else {
            valore.parse().map(|rete| vec![rete])
        }
    }
}

impl FromStr for WifiNetwork {
    type Err = FrixError;

    fn from_str(valore: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|rete| rete.as_str() == valore)
            .ok_or_else(|| {
                FrixError::Validation(format!(
                    "Wi-Fi Network non valida: {valore:?} (ammessi: main_2g, main_5g, guest, all)"
                ))
            })
    }
}

impl fmt::Display for WifiNetwork {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// La Band su cui trasmette una Wi-Fi Network (`X_AVM-DE_FrequencyBand`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Band {
    /// `2400`.
    Ghz2_4,
    /// `5000`.
    Ghz5,
    /// `6000`.
    Ghz6,
    /// `unknown` o un valore che lo SCPD non prevede.
    Unknown,
}

impl Band {
    fn from_router(valore: &str) -> Self {
        match valore.trim() {
            "2400" => Self::Ghz2_4,
            "5000" => Self::Ghz5,
            "6000" => Self::Ghz6,
            _ => Self::Unknown,
        }
    }

    /// La Band di un canale: 1-14 esistono solo a 2,4 GHz, 32-177 solo a 5 GHz.
    /// Anche i canali a 6 GHz sono numerati da 1, quindi su un router con una radio
    /// a 6 GHz questa regola non basterebbe; il 7590 non ne ha.
    fn from_channel(canale: u8) -> Self {
        match canale {
            1..=14 => Self::Ghz2_4,
            32..=177 => Self::Ghz5,
            _ => Self::Unknown,
        }
    }

    /// Nome stabile verso l'Agent: `2.4GHz`, `5GHz`, `6GHz`, `unknown`.
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

/// Stato di una Wi-Fi Network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WifiStatus {
    /// La Wi-Fi Network, ricavata dai dati del router.
    pub network: WifiNetwork,
    /// `NewEnable`.
    pub enabled: bool,
    /// `NewStatus` (`Up`, `Disabled`).
    pub status: String,
    /// `NewSSID`: dato non fidato.
    pub ssid: String,
    /// `NewX_AVM-DE_FrequencyBand`.
    pub band: Band,
    /// `NewChannel` di `GetChannelInfo`.
    pub channel: u8,
    /// `NewX_AVM-DE_AutoChannelEnabled`.
    pub auto_channel: bool,
    /// `NewStandard` (`n`, `ac`, `ax`, …).
    pub standard: String,
    /// `NewBeaconType`: la cifratura (`11i`, `11iandWPA3`, …).
    pub encryption: String,
    /// `NewMaxBitRate`.
    pub max_bit_rate: String,
    /// `NewTotalAssociations`: client associati.
    pub clients: u16,
    /// Solo per la Wi-Fi Network Ospiti: timeout e isolamento.
    pub guest: Option<GuestSettings>,
}

/// Impostazioni della Wi-Fi Network Ospiti (`X_AVM-DE_GetWLANExtInfo`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuestSettings {
    /// `NewX_AVM-DE_TimeoutActive`: la rete si spegne da sola.
    pub timeout_active: bool,
    /// `NewX_AVM-DE_Timeout`, in minuti; `None` se il router non dà un numero.
    pub timeout_minutes: Option<u32>,
    /// `NewX_AVM-DE_TimeRemain`, in minuti; `None` se il router non dà un numero.
    pub time_remaining_minutes: Option<u32>,
    /// `NewX_AVM-DE_NoForcedOff`: il timeout non scatta finché ci sono client.
    pub no_forced_off: bool,
    /// `NewX_AVM-DE_UserIsolation`: i client non si vedono fra loro.
    pub user_isolation: bool,
}

/// Un client associato a una Wi-Fi Network (`X_AVM-DE_GetWLANDeviceListPath` o
/// `GetGenericAssociatedDeviceInfo`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WifiClient {
    /// La Wi-Fi Network a cui è associato.
    pub network: WifiNetwork,
    /// La Band della Wi-Fi Network.
    pub band: Band,
    /// `NewAssociatedDeviceMACAddress`.
    pub mac: Mac,
    /// `NewAssociatedDeviceIPAddress`.
    pub ip: String,
    /// `NewAssociatedDeviceAuthState`.
    pub authenticated: bool,
    /// `NewX_AVM-DE_Speed`, in Mbit/s.
    pub speed_mbps: u16,
    /// `NewX_AVM-DE_SignalStrength`, così come lo riporta il router.
    pub signal_strength: u8,
    /// `NewX_AVM-DE_ChannelWidth`, in MHz.
    pub channel_width_mhz: u16,
}

/// Una Wi-Fi Network trovata nel router, con il servizio `WLANConfiguration:N` che la serve.
struct ReteTrovata {
    network: WifiNetwork,
    service: String,
    band: Band,
    guest: Option<GuestSettings>,
}

/// Stato delle Wi-Fi Network richieste, nell'ordine di [`WifiNetwork::ALL`].
///
/// `GetInfo`, `GetChannelInfo` e `GetTotalAssociations` su ogni istanza richiesta.
pub async fn status(
    client: &Tr064Client,
    networks: &[WifiNetwork],
) -> Result<Vec<WifiStatus>, FrixError> {
    let mut risultato = Vec::new();
    for rete in reti_trovate(client, networks).await? {
        let info = client.call(&rete.service, "GetInfo", &[]).await?;
        let canale = client.call(&rete.service, "GetChannelInfo", &[]).await?;
        risultato.push(WifiStatus {
            network: rete.network,
            enabled: info.flag("NewEnable")?,
            status: info.text("NewStatus")?.to_owned(),
            ssid: info.text("NewSSID")?.to_owned(),
            band: rete.band,
            channel: canale.parse("NewChannel")?,
            auto_channel: canale.flag("NewX_AVM-DE_AutoChannelEnabled")?,
            standard: info.text("NewStandard")?.to_owned(),
            encryption: info.text("NewBeaconType")?.to_owned(),
            max_bit_rate: info.text("NewMaxBitRate")?.to_owned(),
            clients: totale_associazioni(client, &rete.service).await?,
            guest: rete.guest,
        });
    }
    Ok(risultato)
}

/// I client associati alle Wi-Fi Network richieste.
///
/// Per ogni Wi-Fi Network, prima `X_AVM-DE_GetWLANDeviceListPath` e `GET` della lista
/// (una chiamata per tutti i client); se fallisce, `GetTotalAssociations` e
/// `GetGenericAssociatedDeviceInfo` per ogni indice (§8.1). Se falliscono entrambe,
/// l'errore è quello della lista.
pub async fn clients(
    client: &Tr064Client,
    networks: &[WifiNetwork],
) -> Result<Vec<WifiClient>, FrixError> {
    let mut risultato = Vec::new();
    for rete in reti_trovate(client, networks).await? {
        let voci = match voci_da_lista(client, &rete.service).await {
            Ok(voci) => voci,
            Err(errore) => voci_per_indice(client, &rete.service)
                .await
                .map_err(|_| errore)?,
        };
        for voce in &voci {
            risultato.push(wifi_client(&rete, voce)?);
        }
    }
    Ok(risultato)
}

/// I campi di un client, con i nomi delle voci della lista (gli argomenti di
/// `GetGenericAssociatedDeviceInfo` senza `New`).
const CAMPI_CLIENT: [&str; 6] = [
    "AssociatedDeviceMACAddress",
    "AssociatedDeviceIPAddress",
    "AssociatedDeviceAuthState",
    "X_AVM-DE_Speed",
    "X_AVM-DE_SignalStrength",
    "X_AVM-DE_ChannelWidth",
];

async fn voci_da_lista(
    client: &Tr064Client,
    service: &str,
) -> Result<Vec<BTreeMap<String, String>>, FrixError> {
    let r = client
        .call(service, "X_AVM-DE_GetWLANDeviceListPath", &[])
        .await?;
    let documento = list_path::download(client, r.text("NewX_AVM-DE_WLANDeviceListPath")?).await?;
    xml_list_items(&documento, "Item")
}

async fn voci_per_indice(
    client: &Tr064Client,
    service: &str,
) -> Result<Vec<BTreeMap<String, String>>, FrixError> {
    let mut elenco = Vec::new();
    for indice in 0..totale_associazioni(client, service).await? {
        let r = client
            .call(
                service,
                "GetGenericAssociatedDeviceInfo",
                &[("NewAssociatedDeviceIndex", &indice.to_string())],
            )
            .await?;
        let mut voce = BTreeMap::new();
        for campo in CAMPI_CLIENT {
            voce.insert(campo.to_owned(), r.text(&format!("New{campo}"))?.to_owned());
        }
        elenco.push(voce);
    }
    Ok(elenco)
}

fn wifi_client(
    rete: &ReteTrovata,
    voce: &BTreeMap<String, String>,
) -> Result<WifiClient, FrixError> {
    let testo = |nome: &str| {
        voce.get(nome)
            .map(|t| t.trim())
            .ok_or_else(|| FrixError::Parse(format!("client Wi-Fi senza {nome}")))
    };
    let numero = |nome: &str| -> Result<u16, FrixError> {
        testo(nome)?
            .parse()
            .map_err(|_| FrixError::Parse(format!("valore non valido per {nome}")))
    };
    Ok(WifiClient {
        network: rete.network,
        band: rete.band,
        mac: testo("AssociatedDeviceMACAddress")?
            .parse()
            .map_err(|_| FrixError::Parse("MAC non valido di un client Wi-Fi".into()))?,
        ip: testo("AssociatedDeviceIPAddress")?.to_owned(),
        authenticated: testo("AssociatedDeviceAuthState")? == "1",
        speed_mbps: numero("X_AVM-DE_Speed")?,
        signal_strength: testo("X_AVM-DE_SignalStrength")?.parse().map_err(|_| {
            FrixError::Parse("valore non valido per X_AVM-DE_SignalStrength".into())
        })?,
        channel_width_mhz: numero("X_AVM-DE_ChannelWidth")?,
    })
}

/// La Band di un'istanza: `X_AVM-DE_FrequencyBand` di `X_AVM-DE_GetWLANExtInfo`
/// (`da_ext_info`), poi quello di `GetChannelInfo`, poi il canale. Il 7590 con
/// FRITZ!OS 8.25 risponde `unknown` in entrambi i campi sull'istanza Ospiti.
async fn banda(client: &Tr064Client, service: &str, da_ext_info: &str) -> Result<Band, FrixError> {
    let band = Band::from_router(da_ext_info);
    if band != Band::Unknown {
        return Ok(band);
    }
    let canale = client.call(service, "GetChannelInfo", &[]).await?;
    let band = canale
        .text("NewX_AVM-DE_FrequencyBand")
        .map_or(Band::Unknown, Band::from_router);
    if band != Band::Unknown {
        return Ok(band);
    }
    Ok(Band::from_channel(canale.parse("NewChannel")?))
}

async fn totale_associazioni(client: &Tr064Client, service: &str) -> Result<u16, FrixError> {
    client
        .call(service, "GetTotalAssociations", &[])
        .await?
        .parse("NewTotalAssociations")
}

/// Le Wi-Fi Network richieste presenti nel router, nell'ordine di [`WifiNetwork::ALL`].
///
/// Ogni istanza `WLANConfiguration` della discovery dichiara con `X_AVM-DE_GetWLANExtInfo`
/// il tipo di AP (`normal`, `guest`) e la Band: Ospiti è l'AP `guest`, le principali
/// sono gli AP `normal` a 2,4 e 5 GHz. Un'istanza che non rientra in nessuno dei tre
/// casi si ignora; due istanze per la stessa Wi-Fi Network sono un errore. Una Wi-Fi
/// Network chiesta esplicitamente e assente dal router è un errore di validazione.
async fn reti_trovate(
    client: &Tr064Client,
    networks: &[WifiNetwork],
) -> Result<Vec<ReteTrovata>, FrixError> {
    let mut trovate: Vec<ReteTrovata> = Vec::new();
    for service in client.instances(SERVICE) {
        let r = client
            .call(&service, "X_AVM-DE_GetWLANExtInfo", &[])
            .await?;
        let band = banda(client, &service, r.text("NewX_AVM-DE_FrequencyBand")?).await?;
        let network = match (r.text("NewX_AVM-DE_APType")?.trim(), band) {
            ("guest", _) => WifiNetwork::Guest,
            ("normal", Band::Ghz2_4) => WifiNetwork::Main2g,
            ("normal", Band::Ghz5) => WifiNetwork::Main5g,
            _ => continue,
        };
        if let Some(doppia) = trovate.iter().find(|i| i.network == network) {
            return Err(FrixError::Parse(format!(
                "{} e {service} dichiarano entrambe la Wi-Fi Network {network}",
                doppia.service
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
        trovate.push(ReteTrovata {
            network,
            service,
            band,
            guest,
        });
    }
    if networks.len() == 1
        && let Some(rete) = networks.first()
        && !trovate.iter().any(|i| i.network == *rete)
    {
        return Err(FrixError::Validation(format!(
            "il router non ha un'istanza WLANConfiguration per la Wi-Fi Network {rete}"
        )));
    }
    trovate.retain(|i| networks.contains(&i.network));
    trovate.sort_by_key(|i| i.network);
    Ok(trovate)
}

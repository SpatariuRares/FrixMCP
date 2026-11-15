//! `WANDSLInterfaceConfig:1` (docs/PROJECT.md §4.2.5).
//!
//! On a non-DSL line (fiber, cable, LAN) the router returns empty values:
//! here they become `None` instead of a parse error.

use std::str::FromStr;

use crate::error::FrixError;
use crate::tr064::{SoapResponse, Tr064Client};

const SERVICE: &str = "WANDSLInterfaceConfig:1";

/// Values for one direction of the line.
#[derive(Debug, Clone, PartialEq)]
pub struct DslDirection {
    /// `New*CurrRate`, in kbit/s.
    pub current_rate_kbps: Option<u64>,
    /// `New*MaxRate`, in kbit/s.
    pub max_rate_kbps: Option<u64>,
    /// `New*NoiseMargin` (SNR margin), in dB.
    pub snr_margin_db: Option<f64>,
    /// `New*Attenuation`, in dB.
    pub attenuation_db: Option<f64>,
}

/// `GetInfo`: line status and values.
#[derive(Debug, Clone, PartialEq)]
pub struct DslInfo {
    /// `NewEnable`.
    pub enabled: Option<bool>,
    /// `NewStatus` (for example `Up`, `NoSignal`).
    pub status: String,
    /// `NewDataPath` (for example `Interleaved`, `Fast`).
    pub data_path: String,
    /// Downstream values.
    pub downstream: DslDirection,
    /// Upstream values.
    pub upstream: DslDirection,
}

/// `GetInfo`.
pub async fn info(client: &Tr064Client) -> Result<DslInfo, FrixError> {
    let r = client.call(SERVICE, "GetInfo", &[]).await?;
    let direction = |dir: &str| -> Result<DslDirection, FrixError> {
        Ok(DslDirection {
            current_rate_kbps: optional(&r, &format!("New{dir}CurrRate"))?,
            max_rate_kbps: optional(&r, &format!("New{dir}MaxRate"))?,
            snr_margin_db: tenths(&r, &format!("New{dir}NoiseMargin"))?,
            attenuation_db: tenths(&r, &format!("New{dir}Attenuation"))?,
        })
    };
    Ok(DslInfo {
        enabled: match r.text("NewEnable")?.trim() {
            "" => None,
            _ => Some(r.flag("NewEnable")?),
        },
        status: r.text("NewStatus")?.to_owned(),
        data_path: r.text("NewDataPath")?.to_owned(),
        downstream: direction("Downstream")?,
        upstream: direction("Upstream")?,
    })
}

/// `X_AVM-DE_GetDSLInfo`: the text part (status, modulation, profile).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DslLine {
    /// `NewLinkStatus`.
    pub link_status: String,
    /// `NewModulationType` (for example `VDSL`).
    pub modulation: String,
    /// `NewCurrentProfile` (for example `35b`).
    pub profile: String,
}

/// `X_AVM-DE_GetDSLInfo`.
pub async fn line(client: &Tr064Client) -> Result<DslLine, FrixError> {
    let r = client.call(SERVICE, "X_AVM-DE_GetDSLInfo", &[]).await?;
    Ok(DslLine {
        link_status: r.text("NewLinkStatus")?.to_owned(),
        modulation: r.text("NewModulationType")?.to_owned(),
        profile: r.text("NewCurrentProfile")?.to_owned(),
    })
}

/// `GetStatisticsTotal`: line error counters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DslStatistics {
    /// `NewCRCErrors`.
    pub crc: Option<u64>,
    /// `NewFECErrors`.
    pub fec: Option<u64>,
    /// `NewHECErrors`.
    pub hec: Option<u64>,
    /// `NewErroredSecs`.
    pub errored_seconds: Option<u64>,
    /// `NewSeverelyErroredSecs`.
    pub severely_errored_seconds: Option<u64>,
    /// `NewLinkRetrain`.
    pub link_retrains: Option<u64>,
    /// `NewLossOfFraming`.
    pub loss_of_framing: Option<u64>,
    /// `NewInitErrors`.
    pub init_errors: Option<u64>,
}

/// `GetStatisticsTotal`.
pub async fn statistics(client: &Tr064Client) -> Result<DslStatistics, FrixError> {
    let r = client.call(SERVICE, "GetStatisticsTotal", &[]).await?;
    Ok(DslStatistics {
        crc: optional(&r, "NewCRCErrors")?,
        fec: optional(&r, "NewFECErrors")?,
        hec: optional(&r, "NewHECErrors")?,
        errored_seconds: optional(&r, "NewErroredSecs")?,
        severely_errored_seconds: optional(&r, "NewSeverelyErroredSecs")?,
        link_retrains: optional(&r, "NewLinkRetrain")?,
        loss_of_framing: optional(&r, "NewLossOfFraming")?,
        init_errors: optional(&r, "NewInitErrors")?,
    })
}

/// Numeric value, `None` if the router returns it empty.
fn optional<T: FromStr>(r: &SoapResponse, name: &str) -> Result<Option<T>, FrixError> {
    if r.text(name)?.trim().is_empty() {
        Ok(None)
    } else {
        r.parse(name).map(Some)
    }
}

/// Value in tenths of dB, converted to dB.
fn tenths(r: &SoapResponse, name: &str) -> Result<Option<f64>, FrixError> {
    Ok(optional::<i32>(r, name)?.map(|v| f64::from(v) / 10.0))
}

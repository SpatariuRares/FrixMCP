//! `WANDSLInterfaceConfig:1` (docs/PROJECT.md §4.2.5).
//!
//! Su una linea che non è DSL (fibra, cavo, LAN) il router restituisce valori vuoti:
//! qui diventano `None` invece di un errore di parsing.

use std::str::FromStr;

use crate::error::FrixError;
use crate::tr064::{SoapResponse, Tr064Client};

const SERVICE: &str = "WANDSLInterfaceConfig:1";

/// Valori di una direzione della linea.
#[derive(Debug, Clone, PartialEq)]
pub struct DslDirection {
    /// `New*CurrRate`, in kbit/s.
    pub current_rate_kbps: Option<u64>,
    /// `New*MaxRate`, in kbit/s.
    pub max_rate_kbps: Option<u64>,
    /// `New*NoiseMargin` (margine SNR), in dB.
    pub snr_margin_db: Option<f64>,
    /// `New*Attenuation`, in dB.
    pub attenuation_db: Option<f64>,
}

/// `GetInfo`: stato e valori della linea.
#[derive(Debug, Clone, PartialEq)]
pub struct DslInfo {
    /// `NewEnable`.
    pub enabled: Option<bool>,
    /// `NewStatus` (per esempio `Up`, `NoSignal`).
    pub status: String,
    /// `NewDataPath` (per esempio `Interleaved`, `Fast`).
    pub data_path: String,
    /// Valori in download.
    pub downstream: DslDirection,
    /// Valori in upload.
    pub upstream: DslDirection,
}

/// `GetInfo`.
pub async fn info(client: &Tr064Client) -> Result<DslInfo, FrixError> {
    let r = client.call(SERVICE, "GetInfo", &[]).await?;
    let direzione = |verso: &str| -> Result<DslDirection, FrixError> {
        Ok(DslDirection {
            current_rate_kbps: optional(&r, &format!("New{verso}CurrRate"))?,
            max_rate_kbps: optional(&r, &format!("New{verso}MaxRate"))?,
            snr_margin_db: tenths(&r, &format!("New{verso}NoiseMargin"))?,
            attenuation_db: tenths(&r, &format!("New{verso}Attenuation"))?,
        })
    };
    Ok(DslInfo {
        enabled: match r.text("NewEnable")?.trim() {
            "" => None,
            _ => Some(r.flag("NewEnable")?),
        },
        status: r.text("NewStatus")?.to_owned(),
        data_path: r.text("NewDataPath")?.to_owned(),
        downstream: direzione("Downstream")?,
        upstream: direzione("Upstream")?,
    })
}

/// `X_AVM-DE_GetDSLInfo`: la parte testuale (stato, modulazione, profilo).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DslLine {
    /// `NewLinkStatus`.
    pub link_status: String,
    /// `NewModulationType` (per esempio `VDSL`).
    pub modulation: String,
    /// `NewCurrentProfile` (per esempio `35b`).
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

/// `GetStatisticsTotal`: contatori di errore della linea.
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

/// Valore numerico, `None` se il router lo restituisce vuoto.
fn optional<T: FromStr>(r: &SoapResponse, nome: &str) -> Result<Option<T>, FrixError> {
    if r.text(nome)?.trim().is_empty() {
        Ok(None)
    } else {
        r.parse(nome).map(Some)
    }
}

/// Valore in decimi di dB, convertito in dB.
fn tenths(r: &SoapResponse, nome: &str) -> Result<Option<f64>, FrixError> {
    Ok(optional::<i32>(r, nome)?.map(|v| f64::from(v) / 10.0))
}

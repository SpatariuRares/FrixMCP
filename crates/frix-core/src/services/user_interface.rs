//! `UserInterface:1`: aggiornamenti di FRITZ!OS (docs/PROJECT.md §4.2.16).

use crate::error::FrixError;
use crate::tr064::Tr064Client;

const SERVICE: &str = "UserInterface:1";

/// Stato dell'aggiornamento di FRITZ!OS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateInfo {
    /// `NewUpgradeAvailable`.
    pub available: bool,
    /// `NewX_AVM-DE_Version`: versione disponibile (vuota se non c'è).
    pub version: String,
    /// `NewX_AVM-DE_UpdateState`, così come lo riporta il router.
    pub state: String,
    /// `NewX_AVM-DE_InfoURL`.
    pub info_url: String,
}

/// `GetInfo`.
pub async fn update_info(client: &Tr064Client) -> Result<UpdateInfo, FrixError> {
    let r = client.call(SERVICE, "GetInfo", &[]).await?;
    Ok(UpdateInfo {
        available: r.flag("NewUpgradeAvailable")?,
        version: r.text("NewX_AVM-DE_Version")?.to_owned(),
        state: r.text("NewX_AVM-DE_UpdateState")?.to_owned(),
        info_url: r.text("NewX_AVM-DE_InfoURL")?.to_owned(),
    })
}

/// Aggiornamenti automatici di FRITZ!OS e risultato dell'ultimo aggiornamento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoUpdate {
    /// `NewX_AVM-DE_AutoUpdateMode` (`off`, `all`, `important`, `check`, …).
    pub mode: String,
    /// `NewX_AVM-DE_CurrentFwVersion`.
    pub current_firmware_version: String,
    /// `NewX_AVM-DE_LastFwVersion`: versione prima dell'ultimo aggiornamento.
    pub last_firmware_version: String,
    /// `NewX_AVM-DE_UpdateTime`: quando è avvenuto l'ultimo aggiornamento.
    pub last_update_time: String,
    /// `NewX_AVM-DE_UpdateSuccessful`, così come lo riporta il router.
    pub last_update_result: String,
}

/// `X_AVM-DE_GetInfo`.
pub async fn auto_update(client: &Tr064Client) -> Result<AutoUpdate, FrixError> {
    let r = client.call(SERVICE, "X_AVM-DE_GetInfo", &[]).await?;
    Ok(AutoUpdate {
        mode: r.text("NewX_AVM-DE_AutoUpdateMode")?.to_owned(),
        current_firmware_version: r.text("NewX_AVM-DE_CurrentFwVersion")?.to_owned(),
        last_firmware_version: r.text("NewX_AVM-DE_LastFwVersion")?.to_owned(),
        last_update_time: r.text("NewX_AVM-DE_UpdateTime")?.to_owned(),
        last_update_result: r.text("NewX_AVM-DE_UpdateSuccessful")?.to_owned(),
    })
}

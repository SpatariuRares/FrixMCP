//! `UserInterface:1`: FRITZ!OS updates (docs/PROJECT.md §4.2.16).

use crate::error::FrixError;
use crate::tr064::Tr064Client;

const SERVICE: &str = "UserInterface:1";

/// FRITZ!OS update status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateInfo {
    /// `NewUpgradeAvailable`.
    pub available: bool,
    /// `NewX_AVM-DE_Version`: available version (empty if none).
    pub version: String,
    /// `NewX_AVM-DE_UpdateState`, as reported by the router.
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

/// FRITZ!OS automatic updates and the result of the last update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoUpdate {
    /// `NewX_AVM-DE_AutoUpdateMode` (`off`, `all`, `important`, `check`, …).
    pub mode: String,
    /// `NewX_AVM-DE_CurrentFwVersion`.
    pub current_firmware_version: String,
    /// `NewX_AVM-DE_LastFwVersion`: version before the last update, without the build.
    pub last_firmware_version: String,
    /// The build after the comma in `NewX_AVM-DE_LastFwVersion` (`154.08.20,122916`),
    /// if the router gives it.
    pub last_firmware_build: Option<String>,
    /// `NewX_AVM-DE_UpdateTime`: when the last update happened.
    pub last_update_time: String,
    /// `NewX_AVM-DE_UpdateSuccessful`, as reported by the router.
    pub last_update_result: String,
}

/// `X_AVM-DE_GetInfo`.
pub async fn auto_update(client: &Tr064Client) -> Result<AutoUpdate, FrixError> {
    let r = client.call(SERVICE, "X_AVM-DE_GetInfo", &[]).await?;
    let last = r.text("NewX_AVM-DE_LastFwVersion")?.trim();
    let (last_version, last_build) = match last.split_once(',') {
        Some((version, build)) => (version.trim(), Some(build.trim().to_owned())),
        None => (last, None),
    };
    Ok(AutoUpdate {
        mode: r.text("NewX_AVM-DE_AutoUpdateMode")?.to_owned(),
        current_firmware_version: r.text("NewX_AVM-DE_CurrentFwVersion")?.to_owned(),
        last_firmware_version: last_version.to_owned(),
        last_firmware_build: last_build.filter(|b| !b.is_empty()),
        last_update_time: r.text("NewX_AVM-DE_UpdateTime")?.to_owned(),
        last_update_result: r.text("NewX_AVM-DE_UpdateSuccessful")?.to_owned(),
    })
}

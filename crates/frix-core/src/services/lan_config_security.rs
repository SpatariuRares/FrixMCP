//! `LANConfigSecurity:1`: the Router Account and rights `FrixMCP` logged in with
//! (docs/PROJECT.md §7.5).

use crate::error::FrixError;
use crate::tr064::{Tr064Client, nested_xml_pairs};

const SERVICE: &str = "LANConfigSecurity:1";

/// The current Router Account and its rights.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentUser {
    /// `NewX_AVM-DE_CurrentUsername`.
    pub username: String,
    /// Rights in response order.
    pub rights: Vec<Right>,
}

/// A Router Account right, as reported by the router.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Right {
    /// Area (`BoxAdmin`, `Phone`, `NAS`, `HomeAuto`, `App`, …).
    pub area: String,
    /// Access (`none`, `readonly`, `readwrite`, …).
    pub access: String,
}

/// `X_AVM-DE_GetCurrentUser`.
///
/// `NewX_AVM-DE_CurrentUserRights` is a nested XML document with consecutive
/// `<path>`/`<access>` pairs.
pub async fn current_user(client: &Tr064Client) -> Result<CurrentUser, FrixError> {
    let r = client.call(SERVICE, "X_AVM-DE_GetCurrentUser", &[]).await?;
    let mut rights = Vec::new();
    let mut area: Option<String> = None;
    for (name, text) in nested_xml_pairs(r.text("NewX_AVM-DE_CurrentUserRights")?)? {
        match (name.as_str(), area.take()) {
            ("path", _) => area = Some(text.trim().to_owned()),
            ("access", Some(area)) => rights.push(Right {
                area,
                access: text.trim().to_owned(),
            }),
            _ => {
                return Err(FrixError::Parse("unparsable Router Account rights".into()));
            }
        }
    }
    Ok(CurrentUser {
        username: r.text("NewX_AVM-DE_CurrentUsername")?.to_owned(),
        rights,
    })
}

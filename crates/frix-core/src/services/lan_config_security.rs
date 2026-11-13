//! `LANConfigSecurity:1`: con quale Router Account e quali diritti `FrixMCP` è entrato
//! (docs/PROJECT.md §7.5).

use crate::error::FrixError;
use crate::tr064::{Tr064Client, nested_xml_pairs};

const SERVICE: &str = "LANConfigSecurity:1";

/// Il Router Account corrente e i suoi diritti.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentUser {
    /// `NewX_AVM-DE_CurrentUsername`.
    pub username: String,
    /// Diritti in ordine di risposta.
    pub rights: Vec<Right>,
}

/// Un diritto del Router Account, così come lo riporta il router.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Right {
    /// Area (`BoxAdmin`, `Phone`, `NAS`, `HomeAuto`, `App`, …).
    pub area: String,
    /// Accesso (`none`, `readonly`, `readwrite`, …).
    pub access: String,
}

/// `X_AVM-DE_GetCurrentUser`.
///
/// `NewX_AVM-DE_CurrentUserRights` è un documento XML annidato con coppie
/// `<path>`/`<access>` consecutive.
pub async fn current_user(client: &Tr064Client) -> Result<CurrentUser, FrixError> {
    let r = client.call(SERVICE, "X_AVM-DE_GetCurrentUser", &[]).await?;
    let mut rights = Vec::new();
    let mut area: Option<String> = None;
    for (nome, testo) in nested_xml_pairs(r.text("NewX_AVM-DE_CurrentUserRights")?)? {
        match (nome.as_str(), area.take()) {
            ("path", _) => area = Some(testo.trim().to_owned()),
            ("access", Some(area)) => rights.push(Right {
                area,
                access: testo.trim().to_owned(),
            }),
            _ => {
                return Err(FrixError::Parse(
                    "diritti del Router Account non interpretabili".into(),
                ));
            }
        }
    }
    Ok(CurrentUser {
        username: r.text("NewX_AVM-DE_CurrentUsername")?.to_owned(),
        rights,
    })
}

//! `get_system_info` (docs/PROJECT.md §8.1).

use frix_core::FrixError;
use frix_core::services::{device_info, lan_config_security, user_interface};
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, tool, tool_router};
use serde::Serialize;

use crate::response;
use crate::server::FrixServer;

#[derive(Debug, Serialize)]
struct SystemInfo {
    manufacturer: String,
    model: String,
    description: String,
    product_class: String,
    /// `null` se l'Operator lo oscura (`hide_serial_number` in `config.toml`).
    serial_number: Option<String>,
    firmware_version: String,
    hardware_version: String,
    uptime_seconds: u64,
    update: Update,
    current_user: CurrentUser,
}

#[derive(Debug, Serialize)]
struct Update {
    available: bool,
    version: String,
    state: String,
    info_url: String,
}

#[derive(Debug, Serialize)]
struct CurrentUser {
    username: String,
    rights: Vec<Right>,
}

#[derive(Debug, Serialize)]
struct Right {
    area: String,
    access: String,
}

#[tool_router(router = system_router, vis = "pub(crate)")]
impl FrixServer {
    #[tool(
        name = "get_system_info",
        description = concat!(
            "Modello, firmware, numero di serie (null se l'Operator lo oscura), uptime, aggiornamento disponibile e diritti del Router Account del FRITZ!Box (senza il log di sistema). ",
            dati_della_rete!()
        ),
        annotations(
            title = "Informazioni di sistema",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_system_info(&self) -> Result<CallToolResult, ErrorData> {
        Ok(response::from_result(self.system_info().await))
    }
}

impl FrixServer {
    async fn system_info(&self) -> Result<SystemInfo, FrixError> {
        let info = device_info::info(&self.client).await?;
        let update = user_interface::update_info(&self.client).await?;
        let utente = lan_config_security::current_user(&self.client).await?;
        Ok(SystemInfo {
            manufacturer: info.manufacturer,
            model: info.model,
            description: info.description,
            product_class: info.product_class,
            serial_number: (!self.hide_serial_number).then_some(info.serial_number),
            firmware_version: info.software_version,
            hardware_version: info.hardware_version,
            uptime_seconds: info.uptime_seconds,
            update: Update {
                available: update.available,
                version: update.version,
                state: update.state,
                info_url: update.info_url,
            },
            current_user: CurrentUser {
                username: utente.username,
                rights: utente
                    .rights
                    .into_iter()
                    .map(|r| Right {
                        area: r.area,
                        access: r.access,
                    })
                    .collect(),
            },
        })
    }
}

//! Configurazione e stato del router: `get_lan_config`, `list_port_forwardings`,
//! `get_device_log`, `get_time_and_update` (docs/PROJECT.md §8.1).

use frix_core::FrixError;
use frix_core::services::EntryTruncation;
use frix_core::services::wan_connection::{self, ConnectionKind};
use frix_core::services::{device_info, lan_host_config, time, user_interface};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::schemars::JsonSchema;
use rmcp::{ErrorData, tool, tool_router};
use serde::{Deserialize, Serialize};

use crate::response::{self, VoceSaltata};
use crate::server::FrixServer;

#[derive(Debug, Serialize)]
struct LanConfig {
    dhcp_server_enabled: bool,
    dhcp_range: DhcpRange,
    dns_servers: Vec<String>,
    subnet_mask: String,
    domain_name: String,
    routers: Vec<String>,
}

#[derive(Debug, Serialize)]
struct DhcpRange {
    min: String,
    max: String,
}

#[derive(Debug, Serialize)]
struct TimeAndUpdate {
    local_time: String,
    time_zone: String,
    daylight_saving: bool,
    ntp_servers: Vec<String>,
    auto_update: AutoUpdate,
}

#[derive(Debug, Serialize)]
struct AutoUpdate {
    mode: String,
    current_firmware_version: String,
    last_firmware_version: String,
    last_update_time: String,
    last_update_result: String,
}

#[derive(Debug, Serialize)]
#[expect(
    clippy::struct_field_names,
    reason = "port_forwardings è la chiave JSON che vede l'Agent"
)]
struct PortForwardings {
    connection: &'static str,
    port_forwardings: Vec<PortForwarding>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    skipped: Vec<VoceSaltata>,
}

#[derive(Debug, Serialize)]
struct PortForwarding {
    description: String,
    enabled: bool,
    protocol: String,
    external_port: u16,
    internal_client: String,
    internal_port: u16,
    remote_host: String,
    lease_duration_seconds: u32,
}

/// Righe al massimo in una pagina di `get_device_log`.
const MAX_RIGHE_LOG: u32 = 1000;

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
struct PaginaLog {
    /// Quante righe restituire (1–1000).
    #[serde(default = "righe_predefinite")]
    lines: u32,
    /// Quante righe saltare, contando dalla più recente.
    #[serde(default)]
    offset: u32,
}

const fn righe_predefinite() -> u32 {
    100
}

#[derive(Debug, Serialize)]
struct DeviceLog {
    total_lines: usize,
    offset: u32,
    lines: Vec<String>,
}

#[tool_router(router = config_router, vis = "pub(crate)")]
impl FrixServer {
    #[tool(
        name = "get_lan_config",
        description = concat!(
            "Configurazione della LAN del FRITZ!Box: server DHCP e intervallo di indirizzi, DNS, maschera di sottorete, dominio, router. ",
            dati_della_rete!()
        ),
        annotations(
            title = "Configurazione LAN",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_lan_config(&self) -> Result<CallToolResult, ErrorData> {
        Ok(response::from_result(self.lan_config().await))
    }

    #[tool(
        name = "get_time_and_update",
        description = concat!(
            "Ora locale del FRITZ!Box, fuso orario, server NTP e stato degli aggiornamenti automatici di FRITZ!OS (modalità, ultima versione installata). ",
            dati_della_rete!()
        ),
        annotations(
            title = "Ora e aggiornamenti",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_time_and_update(&self) -> Result<CallToolResult, ErrorData> {
        Ok(response::from_result(self.time_and_update().await))
    }

    #[tool(
        name = "get_device_log",
        description = concat!(
            "Log di sistema del FRITZ!Box, una pagina di righe dalla più recente: `lines` righe (predefinito 100, massimo 1000) a partire da `offset` (predefinito 0). `total_lines` dice quante righe ha il log. Le righe sono testo libero scritto dal router e dai dispositivi della rete. ",
            dati_della_rete!()
        ),
        annotations(
            title = "Log di sistema",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_device_log(
        &self,
        Parameters(pagina): Parameters<PaginaLog>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(response::from_result_righe(self.device_log(pagina).await))
    }

    #[tool(
        name = "list_port_forwardings",
        description = concat!(
            "Inoltri porta (port forwarding) della connessione Internet del FRITZ!Box: descrizione, abilitato, protocollo, porta esterna, Host e porta interni. Legge il servizio della connessione attiva (PPP o IP). ",
            dati_della_rete!()
        ),
        annotations(
            title = "Inoltri porta",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_port_forwardings(&self) -> Result<CallToolResult, ErrorData> {
        Ok(response::from_result_con_tetto(
            self.port_forwardings().await,
        ))
    }
}

impl FrixServer {
    async fn lan_config(&self) -> Result<LanConfig, FrixError> {
        let lan = lan_host_config::lan_config(&self.client).await?;
        Ok(LanConfig {
            dhcp_server_enabled: lan.dhcp_server_enabled,
            dhcp_range: DhcpRange {
                min: lan.dhcp_min_address,
                max: lan.dhcp_max_address,
            },
            dns_servers: lan.dns_servers,
            subnet_mask: lan.subnet_mask,
            domain_name: lan.domain_name,
            routers: lan.routers,
        })
    }

    async fn time_and_update(&self) -> Result<TimeAndUpdate, FrixError> {
        let ora = time::info(&self.client).await?;
        let update = user_interface::auto_update(&self.client).await?;
        Ok(TimeAndUpdate {
            local_time: ora.local_time,
            time_zone: ora.time_zone,
            daylight_saving: ora.daylight_saving,
            ntp_servers: ora.ntp_servers,
            auto_update: AutoUpdate {
                mode: update.mode,
                current_firmware_version: update.current_firmware_version,
                last_firmware_version: update.last_firmware_version,
                last_update_time: update.last_update_time,
                last_update_result: update.last_update_result,
            },
        })
    }

    async fn port_forwardings(
        &self,
    ) -> Result<(PortForwardings, Option<EntryTruncation>), FrixError> {
        let connessione = wan_connection::active_kind(&self.client).await?;
        let inoltri = wan_connection::port_mappings(&self.client, connessione).await?;
        let dati = PortForwardings {
            connection: match connessione {
                ConnectionKind::Ppp => "ppp",
                ConnectionKind::Ip => "ip",
            },
            skipped: inoltri.skipped.into_iter().map(Into::into).collect(),
            port_forwardings: inoltri
                .entries
                .into_iter()
                .map(|i| PortForwarding {
                    description: i.description,
                    enabled: i.enabled,
                    protocol: i.protocol,
                    external_port: i.external_port,
                    internal_client: i.internal_client,
                    internal_port: i.internal_port,
                    remote_host: i.remote_host,
                    lease_duration_seconds: i.lease_duration_seconds,
                })
                .collect(),
        };
        Ok((dati, inoltri.truncation))
    }

    async fn device_log(&self, pagina: PaginaLog) -> Result<DeviceLog, FrixError> {
        if !(1..=MAX_RIGHE_LOG).contains(&pagina.lines) {
            return Err(FrixError::Validation(format!(
                "lines deve essere tra 1 e {MAX_RIGHE_LOG}"
            )));
        }
        let righe = device_info::log(&self.client).await?;
        let total_lines = righe.len();
        let lines = righe
            .into_iter()
            .skip(usize::try_from(pagina.offset).unwrap_or(usize::MAX))
            .take(usize::try_from(pagina.lines).unwrap_or(usize::MAX))
            .collect();
        Ok(DeviceLog {
            total_lines,
            offset: pagina.offset,
            lines,
        })
    }
}

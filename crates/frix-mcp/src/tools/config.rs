//! Router configuration and state: `get_lan_config`, `list_port_forwardings`,
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

use crate::response::{self, SkippedEntryOut};
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
    reason = "port_forwardings is the JSON key the Agent sees"
)]
struct PortForwardings {
    connection: &'static str,
    port_forwardings: Vec<PortForwarding>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    skipped: Vec<SkippedEntryOut>,
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

/// Maximum number of lines in a `get_device_log` page.
const MAX_LOG_LINES: u32 = 1000;

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
struct LogPage {
    /// How many lines to return (1–1000).
    #[serde(default = "default_lines")]
    lines: u32,
    /// How many lines to skip, counting from the most recent.
    #[serde(default)]
    offset: u32,
}

const fn default_lines() -> u32 {
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
            "FRITZ!Box LAN configuration: DHCP server and address range, DNS, subnet mask, domain, routers. ",
            network_data!()
        ),
        annotations(
            title = "LAN configuration",
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
            "FRITZ!Box local time, time zone, NTP servers and state of the automatic FRITZ!OS updates (mode, last installed version). ",
            network_data!()
        ),
        annotations(
            title = "Time and updates",
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
            "FRITZ!Box system log, one page of lines starting from the most recent: `lines` lines (default 100, maximum 1000) starting at `offset` (default 0). `total_lines` says how many lines the log has. The lines are free text written by the router and by the devices on the network. ",
            network_data!()
        ),
        annotations(
            title = "System log",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_device_log(
        &self,
        Parameters(page): Parameters<LogPage>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(response::from_result_lines(self.device_log(page).await))
    }

    #[tool(
        name = "list_port_forwardings",
        description = concat!(
            "Port forwarding rules of the FRITZ!Box Internet connection: description, enabled, protocol, external port, internal Host and port. Reads the service of the active connection (PPP or IP). ",
            network_data!()
        ),
        annotations(
            title = "Port forwarding",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_port_forwardings(&self) -> Result<CallToolResult, ErrorData> {
        Ok(response::from_result_capped(self.port_forwardings().await))
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
        let clock = time::info(&self.client).await?;
        let update = user_interface::auto_update(&self.client).await?;
        Ok(TimeAndUpdate {
            local_time: clock.local_time,
            time_zone: clock.time_zone,
            daylight_saving: clock.daylight_saving,
            ntp_servers: clock.ntp_servers,
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
        let connection = wan_connection::active_kind(&self.client).await?;
        let forwardings = wan_connection::port_mappings(&self.client, connection).await?;
        let data = PortForwardings {
            connection: match connection {
                ConnectionKind::Ppp => "ppp",
                ConnectionKind::Ip => "ip",
            },
            skipped: forwardings.skipped.into_iter().map(Into::into).collect(),
            port_forwardings: forwardings
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
        Ok((data, forwardings.truncation))
    }

    async fn device_log(&self, page: LogPage) -> Result<DeviceLog, FrixError> {
        if !(1..=MAX_LOG_LINES).contains(&page.lines) {
            return Err(FrixError::Validation(format!(
                "lines must be between 1 and {MAX_LOG_LINES}"
            )));
        }
        let all_lines = device_info::log(&self.client).await?;
        let total_lines = all_lines.len();
        let lines = all_lines
            .into_iter()
            .skip(usize::try_from(page.offset).unwrap_or(usize::MAX))
            .take(usize::try_from(page.lines).unwrap_or(usize::MAX))
            .collect();
        Ok(DeviceLog {
            total_lines,
            offset: page.offset,
            lines,
        })
    }
}

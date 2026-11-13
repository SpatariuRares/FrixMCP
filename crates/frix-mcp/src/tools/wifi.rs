//! `get_wifi_status`, `list_wifi_clients` (docs/PROJECT.md §8.1).

use frix_core::FrixError;
use frix_core::services::wlan::{self, WifiNetwork};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};

use crate::response;
use crate::server::FrixServer;

/// Parametri comuni ai tool Wi-Fi.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct NetworkParams {
    /// Wi-Fi Network: `main_2g` (principale 2,4 GHz), `main_5g` (principale 5 GHz),
    /// `guest` (Ospiti) oppure `all` (default).
    #[schemars(extend("enum" = ["main_2g", "main_5g", "guest", "all"]))]
    network: Option<String>,
}

impl NetworkParams {
    fn networks(&self) -> Result<Vec<WifiNetwork>, FrixError> {
        WifiNetwork::parse_filter(self.network.as_deref().unwrap_or("all"))
    }
}

#[derive(Debug, Serialize)]
struct WifiStatusList {
    networks: Vec<WifiStatus>,
}

#[derive(Debug, Serialize)]
struct WifiStatus {
    network: &'static str,
    enabled: bool,
    status: String,
    ssid: String,
    band: &'static str,
    channel: u8,
    auto_channel: bool,
    standard: String,
    encryption: String,
    max_bit_rate: String,
    clients: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    guest: Option<GuestSettings>,
}

#[derive(Debug, Serialize)]
struct GuestSettings {
    timeout_active: bool,
    timeout_minutes: Option<u32>,
    time_remaining_minutes: Option<u32>,
    no_forced_off: bool,
    user_isolation: bool,
}

#[derive(Debug, Serialize)]
struct WifiClientList {
    clients: Vec<WifiClient>,
}

#[derive(Debug, Serialize)]
struct WifiClient {
    network: &'static str,
    band: &'static str,
    mac: String,
    ip: String,
    authenticated: bool,
    speed_mbps: u16,
    signal_strength: u8,
    channel_width_mhz: u16,
}

#[tool_router(router = wifi_router, vis = "pub(crate)")]
impl FrixServer {
    #[tool(
        name = "get_wifi_status",
        description = concat!(
            "Stato delle Wi-Fi Network del FRITZ!Box (main_2g, main_5g, guest o all): accesa, SSID, Band, canale, standard, cifratura, numero di client; per Ospiti anche timeout e isolamento. Non restituisce mai password o chiavi Wi-Fi. ",
            dati_della_rete!()
        ),
        annotations(
            title = "Stato Wi-Fi",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_wifi_status(
        &self,
        Parameters(params): Parameters<NetworkParams>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(response::from_result(self.wifi_status(&params).await))
    }

    #[tool(
        name = "list_wifi_clients",
        description = concat!(
            "Client associati alle Wi-Fi Network del FRITZ!Box (main_2g, main_5g, guest o all): MAC, IP, segnale, velocità, larghezza di canale e Band. ",
            dati_della_rete!()
        ),
        annotations(
            title = "Client Wi-Fi",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_wifi_clients(
        &self,
        Parameters(params): Parameters<NetworkParams>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(response::from_result(self.wifi_clients(&params).await))
    }
}

impl FrixServer {
    async fn wifi_status(&self, params: &NetworkParams) -> Result<WifiStatusList, FrixError> {
        let reti = wlan::status(&self.client, &params.networks()?).await?;
        Ok(WifiStatusList {
            networks: reti
                .into_iter()
                .map(|r| WifiStatus {
                    network: r.network.as_str(),
                    enabled: r.enabled,
                    status: r.status,
                    ssid: r.ssid,
                    band: r.band.as_str(),
                    channel: r.channel,
                    auto_channel: r.auto_channel,
                    standard: r.standard,
                    encryption: r.encryption,
                    max_bit_rate: r.max_bit_rate,
                    clients: r.clients,
                    guest: r.guest.map(|g| GuestSettings {
                        timeout_active: g.timeout_active,
                        timeout_minutes: g.timeout_minutes,
                        time_remaining_minutes: g.time_remaining_minutes,
                        no_forced_off: g.no_forced_off,
                        user_isolation: g.user_isolation,
                    }),
                })
                .collect(),
        })
    }

    async fn wifi_clients(&self, params: &NetworkParams) -> Result<WifiClientList, FrixError> {
        let client = wlan::clients(&self.client, &params.networks()?).await?;
        Ok(WifiClientList {
            clients: client
                .into_iter()
                .map(|c| WifiClient {
                    network: c.network.as_str(),
                    band: c.band.as_str(),
                    mac: c.mac.to_string(),
                    ip: c.ip,
                    authenticated: c.authenticated,
                    speed_mbps: c.speed_mbps,
                    signal_strength: c.signal_strength,
                    channel_width_mhz: c.channel_width_mhz,
                })
                .collect(),
        })
    }
}

//! `get_access_control` (docs/PROJECT.md §4.2.7, §8.1; ADR 0002).

use frix_core::FrixError;
use frix_core::model::Mac;
use frix_core::services::host_filter::{self, HostAccess};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};

use crate::response;
use crate::server::FrixServer;

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct AccessControlParams {
    /// MAC dell'Host di cui leggere lo stato di blocco, per esempio `AA:BB:CC:DD:EE:FF`.
    /// Gli Host si indicano solo per MAC, mai per IP. Senza: solo i profili di filtro.
    #[serde(default)]
    mac: Option<String>,
}

#[derive(Debug, Serialize)]
struct AccessControl {
    profiles: Vec<ProfileOut>,
    /// Solo con `mac`.
    #[serde(skip_serializing_if = "Option::is_none")]
    host: Option<HostAccessOut>,
}

#[derive(Debug, Serialize)]
struct ProfileOut {
    id: String,
    name: String,
}

#[derive(Debug, Serialize)]
struct HostAccessOut {
    mac: String,
    ip: String,
    name: String,
    wan_access: String,
    wan_access_disallowed: bool,
    filter_profile_id: String,
    time_used: u32,
    time_max: u32,
    tickets_in_advance: u32,
    ticket_valid: u32,
    time_shared: bool,
}

impl From<HostAccess> for HostAccessOut {
    fn from(h: HostAccess) -> Self {
        Self {
            mac: h.mac.to_string(),
            ip: h.ip.to_string(),
            name: h.name,
            wan_access: h.wan_access,
            wan_access_disallowed: h.wan_access_disallowed,
            filter_profile_id: h.filter_profile_id,
            time_used: h.time_used,
            time_max: h.time_max,
            tickets_in_advance: h.tickets_in_advance,
            ticket_valid: h.ticket_valid,
            time_shared: h.time_shared,
        }
    }
}

#[tool_router(router = access_control_router, vis = "pub(crate)")]
impl FrixServer {
    #[tool(
        name = "get_access_control",
        description = concat!(
            "Profili di filtro del controllo accessi (ID e nome) e, con `mac`, lo stato di blocco di quell'Host: accesso a Internet, blocco manuale, profilo, tempo usato e massimo, ticket. L'IP dell'Host si ricava dalla lista host al momento della chiamata. ",
            dati_della_rete!()
        ),
        annotations(
            title = "Controllo accessi",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_access_control(
        &self,
        Parameters(params): Parameters<AccessControlParams>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(response::from_result(
            self.access_control(params.mac.as_deref()).await,
        ))
    }
}

impl FrixServer {
    async fn access_control(&self, mac: Option<&str>) -> Result<AccessControl, FrixError> {
        // il MAC si valida e si risolve prima di qualunque chiamata a HostFilter
        let host = match mac {
            Some(mac) => {
                let mac: Mac = mac.parse()?;
                Some(host_filter::host_access(&self.client, mac).await?.into())
            }
            None => None,
        };
        Ok(AccessControl {
            profiles: host_filter::filter_profiles(&self.client)
                .await?
                .into_iter()
                .map(|p| ProfileOut {
                    id: p.id,
                    name: p.name,
                })
                .collect(),
            host,
        })
    }
}

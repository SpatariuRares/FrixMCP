//! `get_wan_status`, `get_bandwidth`, `get_dsl_status` (docs/PROJECT.md §8.1).

use frix_core::FrixError;
use frix_core::services::wan_connection::{self, ConnectionKind};
use frix_core::services::{wan_common, wan_dsl};
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, tool, tool_router};
use serde::Serialize;

use crate::response;
use crate::server::FrixServer;

#[derive(Debug, Serialize)]
struct WanStatus {
    connection_type: &'static str,
    enabled: bool,
    connected: bool,
    status: String,
    external_ip: String,
    uptime_seconds: u64,
    last_error: String,
    dns_servers: Vec<String>,
    link: Link,
}

#[derive(Debug, Serialize)]
struct Link {
    access_type: String,
    physical_status: String,
    upstream_max_bps: u64,
    downstream_max_bps: u64,
}

#[derive(Debug, Serialize)]
struct Bandwidth {
    sync_group: SyncGroup,
    max_downstream_bps: u64,
    max_upstream_bps: u64,
    current_downstream_bps: Option<u64>,
    current_upstream_bps: Option<u64>,
    series: Series,
    total_bytes_sent: u64,
    total_bytes_received: u64,
    counters_wrap_at_bytes: u64,
}

#[derive(Debug, Serialize)]
struct SyncGroup {
    name: String,
    mode: String,
}

#[derive(Debug, Serialize)]
struct Series {
    downstream_bps: Vec<u64>,
    downstream_multicast_bps: Vec<u64>,
    upstream_bps: Vec<u64>,
    upstream_by_priority: Priorities,
}

#[derive(Debug, Serialize)]
struct Priorities {
    realtime: Vec<u64>,
    high: Vec<u64>,
    default: Vec<u64>,
    low: Vec<u64>,
}

#[derive(Debug, Serialize)]
struct DslStatus {
    access_type: String,
    enabled: Option<bool>,
    status: String,
    link_status: String,
    modulation: String,
    profile: String,
    data_path: String,
    downstream: DslDirection,
    upstream: DslDirection,
    errors: DslErrors,
}

#[derive(Debug, Serialize)]
struct DslDirection {
    current_rate_kbps: Option<u64>,
    max_rate_kbps: Option<u64>,
    snr_margin_db: Option<f64>,
    attenuation_db: Option<f64>,
}

impl From<wan_dsl::DslDirection> for DslDirection {
    fn from(d: wan_dsl::DslDirection) -> Self {
        Self {
            current_rate_kbps: d.current_rate_kbps,
            max_rate_kbps: d.max_rate_kbps,
            snr_margin_db: d.snr_margin_db,
            attenuation_db: d.attenuation_db,
        }
    }
}

#[derive(Debug, Serialize)]
struct DslErrors {
    crc: Option<u64>,
    fec: Option<u64>,
    hec: Option<u64>,
    errored_seconds: Option<u64>,
    severely_errored_seconds: Option<u64>,
    link_retrains: Option<u64>,
    loss_of_framing: Option<u64>,
    init_errors: Option<u64>,
}

#[tool_router(router = wan_router, vis = "pub(crate)")]
impl FrixServer {
    #[tool(
        name = "get_dsl_status",
        description = concat!(
            "Salute della linea DSL del FRITZ!Box: sync rate e rate massimi (kbit/s), margine SNR e attenuazione (dB), errori CRC/FEC/HEC, modulazione e profilo. Su una linea non DSL (vedi access_type) i valori sono vuoti. ",
            dati_della_rete!()
        ),
        annotations(
            title = "Stato della linea DSL",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_dsl_status(&self) -> Result<CallToolResult, ErrorData> {
        Ok(response::from_result(self.dsl_status().await))
    }

    #[tool(
        name = "get_bandwidth",
        description = concat!(
            "Traffico Internet del FRITZ!Box: utilizzo corrente in download e upload, serie degli ultimi secondi (upload anche per priorità: realtime, high, default, low) e contatori totali di byte, che ripartono da zero a 4 GiB. ",
            dati_della_rete!()
        ),
        annotations(
            title = "Traffico Internet",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_bandwidth(&self) -> Result<CallToolResult, ErrorData> {
        Ok(response::from_result(self.bandwidth().await))
    }

    #[tool(
        name = "get_wan_status",
        description = concat!(
            "Stato della connessione Internet del FRITZ!Box: tipo (PPP o IP, scoperto dal router), connessa o no, IP pubblico, uptime, ultimo errore, DNS e velocità massime del link. ",
            dati_della_rete!()
        ),
        annotations(
            title = "Stato della connessione Internet",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_wan_status(&self) -> Result<CallToolResult, ErrorData> {
        Ok(response::from_result(self.wan_status().await))
    }
}

impl FrixServer {
    async fn dsl_status(&self) -> Result<DslStatus, FrixError> {
        let link = wan_common::link_properties(&self.client).await?;
        let info = wan_dsl::info(&self.client).await?;
        let linea = wan_dsl::line(&self.client).await?;
        let stats = wan_dsl::statistics(&self.client).await?;
        Ok(DslStatus {
            access_type: link.access_type,
            enabled: info.enabled,
            status: info.status,
            link_status: linea.link_status,
            modulation: linea.modulation,
            profile: linea.profile,
            data_path: info.data_path,
            downstream: info.downstream.into(),
            upstream: info.upstream.into(),
            errors: DslErrors {
                crc: stats.crc,
                fec: stats.fec,
                hec: stats.hec,
                errored_seconds: stats.errored_seconds,
                severely_errored_seconds: stats.severely_errored_seconds,
                link_retrains: stats.link_retrains,
                loss_of_framing: stats.loss_of_framing,
                init_errors: stats.init_errors,
            },
        })
    }

    async fn bandwidth(&self) -> Result<Bandwidth, FrixError> {
        let m = wan_common::online_monitor(&self.client).await?;
        let sent = wan_common::total_bytes_sent(&self.client).await?;
        let received = wan_common::total_bytes_received(&self.client).await?;
        Ok(Bandwidth {
            sync_group: SyncGroup {
                name: m.sync_group_name,
                mode: m.sync_group_mode,
            },
            max_downstream_bps: m.max_downstream_bps,
            max_upstream_bps: m.max_upstream_bps,
            // il primo valore della serie è il campione più recente
            current_downstream_bps: m.downstream_bps.first().copied(),
            current_upstream_bps: m.upstream_bps.first().copied(),
            series: Series {
                downstream_bps: m.downstream_bps,
                downstream_multicast_bps: m.downstream_multicast_bps,
                upstream_bps: m.upstream_bps,
                upstream_by_priority: Priorities {
                    realtime: m.upstream_realtime_bps,
                    high: m.upstream_high_bps,
                    default: m.upstream_default_bps,
                    low: m.upstream_low_bps,
                },
            },
            total_bytes_sent: sent,
            total_bytes_received: received,
            counters_wrap_at_bytes: wan_common::COUNTER_WRAP,
        })
    }

    async fn wan_status(&self) -> Result<WanStatus, FrixError> {
        let kind = wan_connection::active_kind(&self.client).await?;
        let info = wan_connection::info(&self.client, kind).await?;
        let link = wan_common::link_properties(&self.client).await?;
        Ok(WanStatus {
            connection_type: match kind {
                ConnectionKind::Ppp => "PPP",
                ConnectionKind::Ip => "IP",
            },
            enabled: info.enabled,
            connected: info.status == "Connected",
            status: info.status,
            external_ip: info.external_ip,
            uptime_seconds: info.uptime_seconds,
            last_error: info.last_error,
            dns_servers: info.dns_servers,
            link: Link {
                access_type: link.access_type,
                physical_status: link.physical_status,
                upstream_max_bps: link.upstream_max_bps,
                downstream_max_bps: link.downstream_max_bps,
            },
        })
    }
}

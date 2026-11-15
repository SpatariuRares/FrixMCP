//! WAN tools (`get_wan_status`, `get_bandwidth`, `get_dsl_status`) through the
//! MCP protocol, on top of the fixture transport.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use frix_core::transport::HttpResponse;
use serde_json::json;

/// 7590 transport with the default connection on `WANIPConnection`.
fn transport_ip() -> frix_core::transport::FixtureTransport {
    common::transport_7590().with_soap(
        "Layer3Forwarding:1",
        "GetDefaultConnectionService",
        HttpResponse::ok(
            common::fixture("responses/Layer3Forwarding-1/GetDefaultConnectionService.xml")
                .replace("1.WANPPPConnection.1", "1.WANIPConnection.1"),
        ),
    )
}

#[tokio::test]
async fn get_wan_status_on_ip_connection_detected_from_the_router() {
    let transport = transport_ip();
    let log = transport.requests();
    let mcp = common::start(transport).await;

    let outcome = common::call(&mcp, "get_wan_status", json!({})).await;

    assert!(!common::is_error(&outcome), "{outcome:?}");
    let data = common::data(&outcome);
    assert_eq!(data["connection_type"], "IP");
    assert_eq!(data["connected"], true);
    assert_eq!(data["external_ip"], "198.51.100.7");
    assert_eq!(data["uptime_seconds"], 3600);
    assert_eq!(data["dns_servers"], json!(["198.51.100.1"]));
    let actions: Vec<String> = log
        .soap()
        .iter()
        .filter_map(|r| r.soap_action.clone())
        .collect();
    assert!(
        !actions.iter().any(|a| a.contains("WANPPPConnection")),
        "{actions:?}"
    );
}

#[tokio::test]
async fn get_wan_status_never_returns_the_ppp_username() {
    let mcp = common::start(common::transport_7590()).await;

    let outcome = common::call(&mcp, "get_wan_status", json!({})).await;

    assert!(!common::is_error(&outcome), "{outcome:?}");
    let text = common::text(&outcome);
    assert!(!text.contains("utente-ppp-segreto"), "{text}");
    assert!(!text.contains("provider.example"), "{text}");
    assert!(!text.to_lowercase().contains("user"), "{text}");
}

#[tokio::test]
async fn get_dsl_status_returns_sync_rate_snr_attenuation_errors_and_profile() {
    let mcp = common::start(common::transport_7590()).await;

    let outcome = common::call(&mcp, "get_dsl_status", json!({})).await;

    assert!(!common::is_error(&outcome), "{outcome:?}");
    assert_eq!(
        common::data(&outcome),
        json!({
            "access_type": "DSL",
            "enabled": true,
            "status": "Up",
            "link_status": "Up",
            "modulation": "VDSL",
            "profile": "35b",
            "data_path": "Interleaved",
            "downstream": {
                "current_rate_kbps": 250_000,
                "max_rate_kbps": 283_500,
                "snr_margin_db": 6.5,
                "attenuation_db": 14.3
            },
            "upstream": {
                "current_rate_kbps": 40_000,
                "max_rate_kbps": 46_200,
                "snr_margin_db": 9.0,
                "attenuation_db": 0.0
            },
            "errors": {
                "crc": 17,
                "fec": 52_000,
                "hec": 0,
                "errored_seconds": 12,
                "severely_errored_seconds": 1,
                "link_retrains": 2,
                "loss_of_framing": 0,
                "init_errors": 0
            }
        })
    );
}

/// The response fixture with every output argument empty (`<NewX></NewX>`).
fn empty_response(relative: &str) -> HttpResponse {
    let body: Vec<String> = common::fixture(relative)
        .lines()
        .map(
            |line| match (line.strip_prefix("<New"), line.find('>'), line.rfind("</")) {
                (Some(_), Some(open), Some(close)) if open < close => {
                    format!("{}{}", &line[..=open], &line[close..])
                }
                _ => line.to_owned(),
            },
        )
        .collect();
    HttpResponse::ok(body.join("\n"))
}

#[tokio::test]
async fn get_dsl_status_on_non_dsl_line_returns_empty_values_without_error() {
    let dsl = "WANDSLInterfaceConfig:1";
    let transport = common::transport_7590()
        .with_soap(
            "WANCommonInterfaceConfig:1",
            "GetCommonLinkProperties",
            HttpResponse::ok(
                common::fixture("responses/WANCommonInterfaceConfig-1/GetCommonLinkProperties.xml")
                    .replace(">DSL<", ">Ethernet<"),
            ),
        )
        .with_soap(
            dsl,
            "GetInfo",
            empty_response("responses/WANDSLInterfaceConfig-1/GetInfo.xml"),
        )
        .with_soap(
            dsl,
            "X_AVM-DE_GetDSLInfo",
            empty_response("responses/WANDSLInterfaceConfig-1/X_AVM-DE_GetDSLInfo.xml"),
        )
        .with_soap(
            dsl,
            "GetStatisticsTotal",
            empty_response("responses/WANDSLInterfaceConfig-1/GetStatisticsTotal.xml"),
        );
    let mcp = common::start(transport).await;

    let outcome = common::call(&mcp, "get_dsl_status", json!({})).await;

    assert!(!common::is_error(&outcome), "{outcome:?}");
    let empty = json!({
        "current_rate_kbps": null,
        "max_rate_kbps": null,
        "snr_margin_db": null,
        "attenuation_db": null
    });
    assert_eq!(
        common::data(&outcome),
        json!({
            "access_type": "Ethernet",
            "enabled": null,
            "status": "",
            "link_status": "",
            "modulation": "",
            "profile": "",
            "data_path": "",
            "downstream": empty,
            "upstream": empty,
            "errors": {
                "crc": null,
                "fec": null,
                "hec": null,
                "errored_seconds": null,
                "severely_errored_seconds": null,
                "link_retrains": null,
                "loss_of_framing": null,
                "init_errors": null
            }
        })
    );
}

#[tokio::test]
async fn get_bandwidth_returns_current_usage_and_series_by_priority() {
    let mcp = common::start(common::transport_7590()).await;

    let outcome = common::call(&mcp, "get_bandwidth", json!({})).await;

    assert!(!common::is_error(&outcome), "{outcome:?}");
    assert_eq!(
        common::data(&outcome),
        json!({
            "sync_group": { "name": "sync_dsl", "mode": "VDSL" },
            "max_downstream_bps": 31_250_000,
            "max_upstream_bps": 5_000_000,
            "current_downstream_bps": 1_200_000,
            "current_upstream_bps": 90_000,
            "series": {
                "downstream_bps": [1_200_000, 800_000, 650_000, 0, 15_000],
                "downstream_multicast_bps": [0, 0, 0, 0, 0],
                "upstream_bps": [90_000, 40_000, 35_000, 0, 1_000],
                "upstream_by_priority": {
                    "realtime": [50_000, 20_000, 20_000, 0, 0],
                    "high": [10_000, 5_000, 0, 0, 0],
                    "default": [30_000, 15_000, 15_000, 0, 1_000],
                    "low": [0, 0, 0, 0, 0]
                }
            },
            "total_bytes_sent": 123_456_789,
            "total_bytes_received": 2_345_678_901_u64,
            "counters_wrap_at_bytes": 4_294_967_296_u64
        })
    );
}

#[tokio::test]
async fn get_bandwidth_never_returns_negative_counters_after_the_wrap() {
    let transport = common::transport_7590().with_soap(
        "WANCommonInterfaceConfig:1",
        "GetTotalBytesReceived",
        HttpResponse::ok(
            common::fixture("responses/WANCommonInterfaceConfig-1/GetTotalBytesReceived.xml")
                .replace("2345678901", "-1294967296"),
        ),
    );
    let mcp = common::start(transport).await;

    let outcome = common::call(&mcp, "get_bandwidth", json!({})).await;

    assert!(!common::is_error(&outcome), "{outcome:?}");
    assert_eq!(
        common::data(&outcome)["total_bytes_received"],
        3_000_000_000_u64
    );
}

#[tokio::test]
async fn get_wan_status_on_ppp_connection() {
    let mcp = common::start(common::transport_7590()).await;

    let outcome = common::call(&mcp, "get_wan_status", json!({})).await;

    assert!(!common::is_error(&outcome), "{outcome:?}");
    assert_eq!(
        common::data(&outcome),
        json!({
            "connection_type": "PPP",
            "enabled": true,
            "connected": true,
            "status": "Connected",
            "external_ip": "203.0.113.45",
            "uptime_seconds": 86_400,
            "last_error": "ERROR_NONE",
            "dns_servers": ["198.51.100.53", "198.51.100.54"],
            "link": {
                "access_type": "DSL",
                "physical_status": "Up",
                "upstream_max_bps": 40_000_000,
                "downstream_max_bps": 250_000_000
            }
        })
    );
}

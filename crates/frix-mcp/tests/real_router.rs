//! The 15 tools of §8.1 on the redacted real responses of the 7590 (`fixtures/real/`,
//! recorded with `cargo xtask record`; issue #15).
//!
//! The other tests use hand-written fixtures to cover cases this router does not
//! produce (DSL line, IP connection, `SmartHome` Devices, counter wrap). Here we
//! check that the real responses are interpreted, and the facts observed on the router
//! (docs/PROJECT.md §14.3): PPP connection over Ethernet, instance 3 = guest network with
//! `X_AVM-DE_FrequencyBand` `unknown`, no `SmartHome` Device.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use frix_core::transport::FixtureTransport;
use serde_json::{Value, json};

/// The 7590 without `SmartHome` Devices answers HTTP 500 with fault 713 already at index 0
/// (observed with `cargo xtask record`, which discards faults).
fn transport() -> FixtureTransport {
    common::transport_real().with_soap_args(
        "X_AVM-DE_Homeauto:1",
        "GetGenericDeviceInfos",
        &[("NewIndex", "0")],
        common::fault(713, "SpecifiedArrayIndexInvalid"),
    )
}

async fn tool_data(tool: &str, arguments: Value) -> Value {
    let mcp = common::start(transport()).await;
    let outcome = common::call(&mcp, tool, arguments).await;
    assert!(!common::is_error(&outcome), "{tool}: {outcome:?}");
    common::data(&outcome)
}

fn active_host(hosts: &Value) -> String {
    hosts["hosts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["active"] == true)
        .map(|h| h["mac"].as_str().unwrap().to_owned())
        .expect("at least one active Host")
}

#[tokio::test]
async fn tools_list_has_only_the_15_read_tools() {
    let mcp = common::start(transport()).await;
    let mut names: Vec<String> = mcp
        .list_all_tools()
        .await
        .unwrap()
        .into_iter()
        .map(|t| t.name.to_string())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "get_access_control",
            "get_bandwidth",
            "get_device_log",
            "get_dsl_status",
            "get_host",
            "get_lan_config",
            "get_mesh_topology",
            "get_system_info",
            "get_time_and_update",
            "get_wan_status",
            "get_wifi_status",
            "list_hosts",
            "list_port_forwardings",
            "list_smarthome_devices",
            "list_wifi_clients",
        ]
    );
}

#[tokio::test]
async fn get_system_info_reports_model_and_router_account() {
    let d = tool_data("get_system_info", json!({})).await;
    assert_eq!(d["model"], "FRITZ!Box 7590");
    assert_eq!(d["firmware_version"], "154.08.25");
    assert_eq!(d["current_user"]["username"], "frixmcp");
    let nas = d["current_user"]["rights"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["area"] == "NAS")
        .expect("NAS right");
    assert_eq!(nas["access"], "none");
}

#[tokio::test]
async fn get_wan_status_ppp_connection_over_ethernet() {
    let d = tool_data("get_wan_status", json!({})).await;
    assert_eq!(d["connection_type"], "PPP");
    assert_eq!(d["connected"], true);
    assert_eq!(d["link"]["access_type"], "Ethernet");
    assert_eq!(d["dns_servers"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn get_bandwidth_returns_the_series_of_the_last_seconds() {
    let d = tool_data("get_bandwidth", json!({})).await;
    assert_ne!(d["series"]["downstream_bps"], json!([]));
    assert!(d["max_downstream_bps"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn get_dsl_status_on_ethernet_access_has_the_dsl_line_off() {
    let d = tool_data("get_dsl_status", json!({})).await;
    assert_eq!(d["access_type"], "Ethernet");
    assert_eq!(d["enabled"], false);
}

#[tokio::test]
async fn list_hosts_and_get_host_on_the_real_list() {
    let hosts = tool_data(
        "list_hosts",
        json!({ "active_only": true, "include_mesh": true }),
    )
    .await;
    assert!(
        hosts["hosts"]
            .as_array()
            .unwrap()
            .iter()
            .all(|h| h["active"] == true)
    );
    let mac = active_host(&hosts);

    let host = tool_data("get_host", json!({ "mac": mac })).await;
    assert_eq!(host["mac"], mac);
}

#[tokio::test]
async fn get_mesh_topology_has_nodes_and_links() {
    let d = tool_data("get_mesh_topology", json!({})).await;
    assert_ne!(d["nodes"], json!([]));
    assert_ne!(d["links"], json!([]));
}

#[tokio::test]
async fn get_wifi_status_finds_the_three_networks_and_the_guest_band_from_the_channel() {
    let d = tool_data("get_wifi_status", json!({})).await;
    let networks: Vec<(&str, &str)> = d["networks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["network"].as_str().unwrap(), r["band"].as_str().unwrap()))
        .collect();
    assert_eq!(
        networks,
        [
            ("main_2g", "2.4GHz"),
            ("main_5g", "5GHz"),
            ("guest", "2.4GHz")
        ]
    );
}

#[tokio::test]
async fn list_wifi_clients_assigns_each_client_of_the_list_once() {
    // the three instances give the same path: 14 entries, no guest (guest network off)
    let d = tool_data("list_wifi_clients", json!({})).await;
    let clients = d["clients"].as_array().unwrap();
    assert_eq!(clients.len(), 14);
    let mut mac: Vec<&str> = clients.iter().map(|c| c["mac"].as_str().unwrap()).collect();
    mac.sort_unstable();
    mac.dedup();
    assert_eq!(mac.len(), 14);
    assert!(clients.iter().all(|c| c["network"] != "guest"));
    assert!(clients.iter().any(|c| c["network"] == "main_2g"));
    assert!(clients.iter().any(|c| c["network"] == "main_5g"));
    assert!(
        clients
            .iter()
            .filter(|c| c["network"] == "main_2g")
            .all(|c| c["band"] == "2.4GHz" && c["channel_width_mhz"].as_u64().unwrap() <= 40)
    );
}

#[tokio::test]
async fn get_lan_config_reports_dhcp_and_router() {
    let d = tool_data("get_lan_config", json!({})).await;
    assert_eq!(d["routers"], json!(["192.168.178.1"]));
    assert_eq!(d["dhcp_server_enabled"], true);
}

#[tokio::test]
async fn get_device_log_pages_the_real_log() {
    let d = tool_data("get_device_log", json!({ "lines": 2 })).await;
    assert_eq!(d["lines"].as_array().unwrap().len(), 2);
    assert!(d["total_lines"].as_u64().unwrap() >= 2);
}

#[tokio::test]
async fn get_time_and_update_reports_ntp_and_updates() {
    let d = tool_data("get_time_and_update", json!({})).await;
    assert_ne!(d["ntp_servers"], json!([]));
    assert_eq!(d["auto_update"]["current_firmware_version"], "154.08.25");
}

#[tokio::test]
async fn list_port_forwardings_reads_wanpppconnection() {
    let d = tool_data("list_port_forwardings", json!({})).await;
    assert_eq!(d["connection"], "ppp");
    assert_ne!(d["port_forwardings"], json!([]));
}

#[tokio::test]
async fn get_access_control_with_and_without_mac() {
    tool_data("get_access_control", json!({})).await;
    let hosts = tool_data("list_hosts", json!({ "active_only": true })).await;
    let mac = active_host(&hosts);

    let d = tool_data("get_access_control", json!({ "mac": mac })).await;
    assert_eq!(d["host"]["mac"], mac);
}

#[tokio::test]
async fn list_smarthome_devices_without_devices_returns_an_empty_list() {
    let d = tool_data("list_smarthome_devices", json!({})).await;
    assert_eq!(d["devices"], json!([]));
}

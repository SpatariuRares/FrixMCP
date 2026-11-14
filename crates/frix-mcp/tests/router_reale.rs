//! I 15 tool della §8.1 sulle risposte reali oscurate del 7590 (`fixtures/reali/`,
//! registrate con `cargo xtask record`; issue #15).
//!
//! Gli altri test usano fixture scritte a mano per coprire casi che questo router non
//! produce (linea DSL, connessione IP, `SmartHome` Device, wrap dei contatori). Qui si
//! verifica che le risposte vere si interpretino, e i fatti osservati sul router
//! (docs/PROJECT.md §14.3): connessione PPP su Ethernet, istanza 3 = Ospiti con
//! `X_AVM-DE_FrequencyBand` `unknown`, nessun `SmartHome` Device.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use frix_core::transport::FixtureTransport;
use serde_json::{Value, json};

/// Il 7590 senza `SmartHome` Device risponde HTTP 500 con il fault 713 già all'indice 0
/// (osservato con `cargo xtask record`, che scarta i fault).
fn transport() -> FixtureTransport {
    common::transport_reale().with_soap_args(
        "X_AVM-DE_Homeauto:1",
        "GetGenericDeviceInfos",
        &[("NewIndex", "0")],
        common::fault(713, "SpecifiedArrayIndexInvalid"),
    )
}

async fn dati(tool: &str, argomenti: Value) -> Value {
    let mcp = common::avvia(transport()).await;
    let esito = common::chiama(&mcp, tool, argomenti).await;
    assert!(!common::is_errore(&esito), "{tool}: {esito:?}");
    common::dati(&esito)
}

fn host_attivo(hosts: &Value) -> String {
    hosts["hosts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["active"] == true)
        .map(|h| h["mac"].as_str().unwrap().to_owned())
        .expect("almeno un Host attivo")
}

#[tokio::test]
async fn tools_list_ha_solo_i_15_tool_di_lettura() {
    let mcp = common::avvia(transport()).await;
    let mut nomi: Vec<String> = mcp
        .list_all_tools()
        .await
        .unwrap()
        .into_iter()
        .map(|t| t.name.to_string())
        .collect();
    nomi.sort();
    assert_eq!(
        nomi,
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
async fn get_system_info_riporta_modello_e_router_account() {
    let d = dati("get_system_info", json!({})).await;
    assert_eq!(d["model"], "FRITZ!Box 7590");
    assert_eq!(d["firmware_version"], "154.08.25");
    assert_eq!(d["current_user"]["username"], "frixmcp");
    let nas = d["current_user"]["rights"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["area"] == "NAS")
        .expect("diritto NAS");
    assert_eq!(nas["access"], "none");
}

#[tokio::test]
async fn get_wan_status_connessione_ppp_su_ethernet() {
    let d = dati("get_wan_status", json!({})).await;
    assert_eq!(d["connection_type"], "PPP");
    assert_eq!(d["connected"], true);
    assert_eq!(d["link"]["access_type"], "Ethernet");
    assert_eq!(d["dns_servers"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn get_bandwidth_restituisce_la_serie_degli_ultimi_secondi() {
    let d = dati("get_bandwidth", json!({})).await;
    assert_ne!(d["series"]["downstream_bps"], json!([]));
    assert!(d["max_downstream_bps"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn get_dsl_status_su_accesso_ethernet_ha_la_linea_dsl_spenta() {
    let d = dati("get_dsl_status", json!({})).await;
    assert_eq!(d["access_type"], "Ethernet");
    assert_eq!(d["enabled"], false);
}

#[tokio::test]
async fn list_hosts_e_get_host_sulla_lista_reale() {
    let hosts = dati(
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
    let mac = host_attivo(&hosts);

    let host = dati("get_host", json!({ "mac": mac })).await;
    assert_eq!(host["mac"], mac);
}

#[tokio::test]
async fn get_mesh_topology_ha_nodi_e_link() {
    let d = dati("get_mesh_topology", json!({})).await;
    assert_ne!(d["nodes"], json!([]));
    assert_ne!(d["links"], json!([]));
}

#[tokio::test]
async fn get_wifi_status_trova_le_tre_reti_e_la_band_degli_ospiti_dal_canale() {
    let d = dati("get_wifi_status", json!({})).await;
    let reti: Vec<(&str, &str)> = d["networks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["network"].as_str().unwrap(), r["band"].as_str().unwrap()))
        .collect();
    assert_eq!(
        reti,
        [
            ("main_2g", "2.4GHz"),
            ("main_5g", "5GHz"),
            ("guest", "2.4GHz")
        ]
    );
}

#[tokio::test]
async fn list_wifi_clients_legge_i_client_delle_reti_principali() {
    let d = dati("list_wifi_clients", json!({})).await;
    let clients = d["clients"].as_array().unwrap();
    assert!(
        clients
            .iter()
            .any(|c| c["network"] == "main_2g" || c["network"] == "main_5g")
    );
}

#[tokio::test]
async fn get_lan_config_riporta_dhcp_e_router() {
    let d = dati("get_lan_config", json!({})).await;
    assert_eq!(d["routers"], json!(["192.168.178.1"]));
    assert_eq!(d["dhcp_server_enabled"], true);
}

#[tokio::test]
async fn get_device_log_pagina_il_log_reale() {
    let d = dati("get_device_log", json!({ "lines": 2 })).await;
    assert_eq!(d["lines"].as_array().unwrap().len(), 2);
    assert!(d["total_lines"].as_u64().unwrap() >= 2);
}

#[tokio::test]
async fn get_time_and_update_riporta_ntp_e_aggiornamenti() {
    let d = dati("get_time_and_update", json!({})).await;
    assert_ne!(d["ntp_servers"], json!([]));
    assert_eq!(d["auto_update"]["current_firmware_version"], "154.08.25");
}

#[tokio::test]
async fn list_port_forwardings_legge_wanpppconnection() {
    let d = dati("list_port_forwardings", json!({})).await;
    assert_eq!(d["connection"], "ppp");
    assert_ne!(d["port_forwardings"], json!([]));
}

#[tokio::test]
async fn get_access_control_con_e_senza_mac() {
    dati("get_access_control", json!({})).await;
    let hosts = dati("list_hosts", json!({ "active_only": true })).await;
    let mac = host_attivo(&hosts);

    let d = dati("get_access_control", json!({ "mac": mac })).await;
    assert_eq!(d["host"]["mac"], mac);
}

#[tokio::test]
async fn list_smarthome_devices_senza_dispositivi_restituisce_una_lista_vuota() {
    let d = dati("list_smarthome_devices", json!({})).await;
    assert_eq!(d["devices"], json!([]));
}

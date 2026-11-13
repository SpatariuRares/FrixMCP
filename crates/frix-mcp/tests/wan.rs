//! Tool WAN (`get_wan_status`, `get_bandwidth`, `get_dsl_status`) attraverso il
//! protocollo MCP, sopra il transport a fixture.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use frix_core::transport::HttpResponse;
use serde_json::json;

/// Transport del 7590 con la connessione predefinita su `WANIPConnection`.
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
async fn get_wan_status_su_connessione_ip_scoperta_dal_router() {
    let transport = transport_ip();
    let registro = transport.requests();
    let mcp = common::avvia(transport).await;

    let esito = common::chiama(&mcp, "get_wan_status", json!({})).await;

    assert!(!common::is_errore(&esito), "{esito:?}");
    let dati = common::dati(&esito);
    assert_eq!(dati["connection_type"], "IP");
    assert_eq!(dati["connected"], true);
    assert_eq!(dati["external_ip"], "198.51.100.7");
    assert_eq!(dati["uptime_seconds"], 3600);
    assert_eq!(dati["dns_servers"], json!(["198.51.100.1"]));
    let azioni: Vec<String> = registro
        .soap()
        .iter()
        .filter_map(|r| r.soap_action.clone())
        .collect();
    assert!(
        !azioni.iter().any(|a| a.contains("WANPPPConnection")),
        "{azioni:?}"
    );
}

#[tokio::test]
async fn get_wan_status_non_restituisce_mai_il_nome_utente_ppp() {
    let mcp = common::avvia(common::transport_7590()).await;

    let esito = common::chiama(&mcp, "get_wan_status", json!({})).await;

    assert!(!common::is_errore(&esito), "{esito:?}");
    let testo = common::testo(&esito);
    assert!(!testo.contains("utente-ppp-segreto"), "{testo}");
    assert!(!testo.contains("provider.example"), "{testo}");
    assert!(!testo.to_lowercase().contains("user"), "{testo}");
}

#[tokio::test]
async fn get_dsl_status_restituisce_sync_rate_snr_attenuazione_errori_e_profilo() {
    let mcp = common::avvia(common::transport_7590()).await;

    let esito = common::chiama(&mcp, "get_dsl_status", json!({})).await;

    assert!(!common::is_errore(&esito), "{esito:?}");
    assert_eq!(
        common::dati(&esito),
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

/// La fixture di risposta con ogni argomento di uscita vuoto (`<NewX></NewX>`).
fn risposta_vuota(relativo: &str) -> HttpResponse {
    let corpo: Vec<String> = common::fixture(relativo)
        .lines()
        .map(
            |riga| match (riga.strip_prefix("<New"), riga.find('>'), riga.rfind("</")) {
                (Some(_), Some(apertura), Some(chiusura)) if apertura < chiusura => {
                    format!("{}{}", &riga[..=apertura], &riga[chiusura..])
                }
                _ => riga.to_owned(),
            },
        )
        .collect();
    HttpResponse::ok(corpo.join("\n"))
}

#[tokio::test]
async fn get_dsl_status_su_linea_non_dsl_restituisce_valori_vuoti_senza_errore() {
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
            risposta_vuota("responses/WANDSLInterfaceConfig-1/GetInfo.xml"),
        )
        .with_soap(
            dsl,
            "X_AVM-DE_GetDSLInfo",
            risposta_vuota("responses/WANDSLInterfaceConfig-1/X_AVM-DE_GetDSLInfo.xml"),
        )
        .with_soap(
            dsl,
            "GetStatisticsTotal",
            risposta_vuota("responses/WANDSLInterfaceConfig-1/GetStatisticsTotal.xml"),
        );
    let mcp = common::avvia(transport).await;

    let esito = common::chiama(&mcp, "get_dsl_status", json!({})).await;

    assert!(!common::is_errore(&esito), "{esito:?}");
    let vuota = json!({
        "current_rate_kbps": null,
        "max_rate_kbps": null,
        "snr_margin_db": null,
        "attenuation_db": null
    });
    assert_eq!(
        common::dati(&esito),
        json!({
            "access_type": "Ethernet",
            "enabled": null,
            "status": "",
            "link_status": "",
            "modulation": "",
            "profile": "",
            "data_path": "",
            "downstream": vuota,
            "upstream": vuota,
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
async fn get_bandwidth_restituisce_utilizzo_corrente_e_serie_per_priorita() {
    let mcp = common::avvia(common::transport_7590()).await;

    let esito = common::chiama(&mcp, "get_bandwidth", json!({})).await;

    assert!(!common::is_errore(&esito), "{esito:?}");
    assert_eq!(
        common::dati(&esito),
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
async fn get_bandwidth_non_restituisce_contatori_negativi_dopo_il_wrap() {
    let transport = common::transport_7590().with_soap(
        "WANCommonInterfaceConfig:1",
        "GetTotalBytesReceived",
        HttpResponse::ok(
            common::fixture("responses/WANCommonInterfaceConfig-1/GetTotalBytesReceived.xml")
                .replace("2345678901", "-1294967296"),
        ),
    );
    let mcp = common::avvia(transport).await;

    let esito = common::chiama(&mcp, "get_bandwidth", json!({})).await;

    assert!(!common::is_errore(&esito), "{esito:?}");
    assert_eq!(
        common::dati(&esito)["total_bytes_received"],
        3_000_000_000_u64
    );
}

#[tokio::test]
async fn get_wan_status_su_connessione_ppp() {
    let mcp = common::avvia(common::transport_7590()).await;

    let esito = common::chiama(&mcp, "get_wan_status", json!({})).await;

    assert!(!common::is_errore(&esito), "{esito:?}");
    assert_eq!(
        common::dati(&esito),
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

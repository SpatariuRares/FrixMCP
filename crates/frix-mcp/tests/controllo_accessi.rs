//! Tool `get_access_control` attraverso il protocollo MCP, sopra il transport a fixture
//! (docs/PROJECT.md §4.2.7, §8.1; ADR 0002: l'Host si indica per MAC, l'IP si risolve
//! dalla lista host al momento della chiamata).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use frix_core::transport::{FixtureTransport, RequestLog};
use serde_json::json;

/// Path restituito dalla fixture `Hosts-1/X_AVM-DE_GetHostListPath.xml`, SID incluso.
const HOST_LIST: &str = "/devicehostlist.lua?sid=0123456789abcdef";

/// Il 7590 con la lista host servita al path che il router restituisce.
fn transport_con_lista_host() -> FixtureTransport {
    common::transport_7590().with_file(HOST_LIST, common::fixture("lists/devicehostlist.xml"))
}

/// Le azioni `X_AVM-DE_HostFilter` arrivate al transport, in ordine.
fn azioni_host_filter(registro: &RequestLog) -> Vec<String> {
    registro
        .soap()
        .iter()
        .filter_map(|r| r.soap_action.as_deref())
        .filter_map(|a| a.strip_prefix("urn:dslforum-org:service:X_AVM-DE_HostFilter:1#"))
        .map(str::to_owned)
        .collect()
}

#[tokio::test]
async fn get_access_control_senza_parametri_restituisce_i_profili_di_filtro() {
    let transport = transport_con_lista_host();
    let registro = transport.requests();
    let mcp = common::avvia(transport).await;

    let esito = common::chiama(&mcp, "get_access_control", json!({})).await;

    assert!(!common::is_errore(&esito), "{esito:?}");
    assert_eq!(
        common::dati(&esito),
        json!({
            "profiles": [
                { "id": "filtprof1", "name": "Standard" },
                { "id": "filtprof2", "name": "Gast" },
                { "id": "filtprof3", "name": "Gesperrt" }
            ]
        })
    );
    assert_eq!(azioni_host_filter(&registro), ["GetFilterProfiles"]);
}

#[tokio::test]
async fn get_access_control_con_mac_risolve_l_ip_dalla_lista_host_e_restituisce_lo_stato_dell_host()
{
    let transport = transport_con_lista_host();
    let registro = transport.requests();
    let mcp = common::avvia(transport).await;

    // iPad-Cucina (attivo) ha 192.168.178.30, lo stesso IP della Smart-TV inattiva
    let esito = common::chiama(
        &mcp,
        "get_access_control",
        json!({ "mac": "02:00:00:00:10:02" }),
    )
    .await;

    assert!(!common::is_errore(&esito), "{esito:?}");
    let dati = common::dati(&esito);
    assert_eq!(dati["profiles"].as_array().unwrap().len(), 3);
    assert_eq!(
        dati["host"],
        json!({
            "mac": "02:00:00:00:10:02",
            "ip": "192.168.178.30",
            "name": "iPad-Cucina",
            "wan_access": "granted",
            "wan_access_disallowed": false,
            "filter_profile_id": "filtprof1",
            "time_used": 35,
            "time_max": 120,
            "tickets_in_advance": 1,
            "ticket_valid": 0,
            "time_shared": true
        })
    );
    let per_ip: Vec<_> = registro
        .soap()
        .into_iter()
        .filter(|r| {
            r.soap_action.as_deref().is_some_and(|a| {
                a.ends_with("HostFilter:1#GetWANAccessByIP")
                    || a.ends_with("HostFilter:1#GetHostEntryByIP")
            })
        })
        .collect();
    assert_eq!(per_ip.len(), 2);
    for richiesta in per_ip {
        assert!(
            richiesta
                .body
                .contains("<NewIPv4Address>192.168.178.30</NewIPv4Address>"),
            "{}",
            richiesta.body
        );
    }
}

#[tokio::test]
async fn get_access_control_con_un_mac_sconosciuto_da_errore_senza_chiamare_host_filter() {
    let transport = transport_con_lista_host();
    let registro = transport.requests();
    let mcp = common::avvia(transport).await;

    let esito = common::chiama(
        &mcp,
        "get_access_control",
        json!({ "mac": "02:00:00:00:99:99" }),
    )
    .await;

    assert!(common::is_errore(&esito), "{esito:?}");
    let errore = common::dati(&esito);
    assert_eq!(errore["error"], "no_such_entry");
    assert!(
        errore["message"]
            .as_str()
            .unwrap()
            .contains("02:00:00:00:99:99"),
        "{errore}"
    );
    assert_eq!(azioni_host_filter(&registro), Vec::<String>::new());
}

#[tokio::test]
async fn get_access_control_rifiuta_un_mac_il_cui_ip_e_ora_di_un_altro_host_attivo() {
    let transport = transport_con_lista_host();
    let registro = transport.requests();
    let mcp = common::avvia(transport).await;

    // Smart-TV (inattiva) ha ancora 192.168.178.30, che ora è dell'iPad-Cucina attivo
    let esito = common::chiama(
        &mcp,
        "get_access_control",
        json!({ "mac": "02:00:00:00:10:03" }),
    )
    .await;

    assert!(common::is_errore(&esito), "{esito:?}");
    assert_eq!(common::dati(&esito)["error"], "validation");
    assert_eq!(azioni_host_filter(&registro), Vec::<String>::new());
}

#[tokio::test]
async fn get_access_control_rifiuta_un_mac_malformato_senza_richieste_al_router() {
    let transport = transport_con_lista_host();
    let registro = transport.requests();
    let mcp = common::avvia(transport).await;

    let esito = common::chiama(
        &mcp,
        "get_access_control",
        json!({ "mac": "192.168.178.30" }),
    )
    .await;

    assert!(common::is_errore(&esito), "{esito:?}");
    assert_eq!(common::dati(&esito)["error"], "validation");
    assert!(registro.soap().is_empty(), "{:?}", registro.soap());
}

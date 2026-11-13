//! Un'azione o un argomento che lo SCPD del router non dichiara vengono
//! rifiutati in locale, prima di qualunque richiesta SOAP (docs/PROJECT.md §5.1).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use serde_json::json;

const SCPD_DEVICE_INFO: &str = "/deviceinfoSCPD.xml";

#[tokio::test]
async fn azione_assente_dallo_scpd_rifiutata_senza_richieste_soap() {
    // un FRITZ!OS futuro che ha rinominato DeviceInfo:GetInfo
    let scpd = common::fixture("scpd/deviceinfoSCPD.xml")
        .replace("<name>GetInfo</name>", "<name>GetInfoV2</name>");
    let transport = common::transport_7590().with_file(SCPD_DEVICE_INFO, scpd);
    let registro = transport.requests();
    let mcp = common::avvia(transport).await;

    let esito = common::chiama(&mcp, "get_system_info", json!({})).await;

    assert!(common::is_errore(&esito));
    assert_eq!(common::dati(&esito)["error"], "validation");
    assert!(registro.soap().is_empty(), "{:?}", registro.soap());
}

#[tokio::test]
async fn argomento_richiesto_dallo_scpd_ma_non_inviato_rifiutato_senza_richieste_soap() {
    // un FRITZ!OS futuro che aggiunge un argomento di ingresso a GetInfo
    let scpd = common::fixture("scpd/deviceinfoSCPD.xml").replacen(
        "<argumentList>",
        "<argumentList><argument><name>NewFilter</name><direction>in</direction>\
         <relatedStateVariable>Description</relatedStateVariable></argument>",
        1,
    );
    let transport = common::transport_7590().with_file(SCPD_DEVICE_INFO, scpd);
    let registro = transport.requests();
    let mcp = common::avvia(transport).await;

    let esito = common::chiama(&mcp, "get_system_info", json!({})).await;

    assert!(common::is_errore(&esito));
    assert_eq!(common::dati(&esito)["error"], "validation");
    assert!(registro.soap().is_empty(), "{:?}", registro.soap());
}

#[tokio::test]
async fn servizio_assente_dalla_descrizione_rifiutato_senza_richieste_soap() {
    let descrizione = common::fixture("scpd/tr64desc.xml").replace(
        "urn:dslforum-org:service:DeviceInfo:1",
        "urn:dslforum-org:service:DeviceInfo:2",
    );
    let transport = common::transport_7590().with_file("/tr64desc.xml", descrizione);
    let registro = transport.requests();
    let mcp = common::avvia(transport).await;

    let esito = common::chiama(&mcp, "get_system_info", json!({})).await;

    assert!(common::is_errore(&esito));
    assert_eq!(common::dati(&esito)["error"], "validation");
    assert!(registro.soap().is_empty(), "{:?}", registro.soap());
}

#[tokio::test]
async fn la_richiesta_soap_usa_control_url_e_soapaction_della_descrizione() {
    let transport = common::transport_7590();
    let registro = transport.requests();
    let mcp = common::avvia(transport).await;

    common::chiama(&mcp, "get_system_info", json!({})).await;

    let prima = registro
        .soap()
        .into_iter()
        .next()
        .expect("una richiesta SOAP");
    assert_eq!(prima.path, "/upnp/control/deviceinfo");
    assert_eq!(
        prima.soap_action.as_deref(),
        Some("urn:dslforum-org:service:DeviceInfo:1#GetInfo")
    );
}

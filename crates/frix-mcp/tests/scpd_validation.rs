//! An action or argument that the router's SCPD does not declare is
//! rejected locally, before any SOAP request (docs/PROJECT.md §5.1).
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
async fn action_missing_from_scpd_rejected_without_soap_requests() {
    // a future FRITZ!OS that renamed DeviceInfo:GetInfo
    let scpd = common::fixture("scpd/deviceinfoSCPD.xml")
        .replace("<name>GetInfo</name>", "<name>GetInfoV2</name>");
    let transport = common::transport_7590().with_file(SCPD_DEVICE_INFO, scpd);
    let log = transport.requests();
    let mcp = common::start(transport).await;

    let outcome = common::call(&mcp, "get_system_info", json!({})).await;

    assert!(common::is_error(&outcome));
    assert_eq!(common::data(&outcome)["error"], "validation");
    assert!(log.soap().is_empty(), "{:?}", log.soap());
}

#[tokio::test]
async fn argument_required_by_scpd_but_not_sent_rejected_without_soap_requests() {
    // a future FRITZ!OS that adds an input argument to GetInfo
    let scpd = common::fixture("scpd/deviceinfoSCPD.xml").replacen(
        "<argumentList>",
        "<argumentList><argument><name>NewFilter</name><direction>in</direction>\
         <relatedStateVariable>Description</relatedStateVariable></argument>",
        1,
    );
    let transport = common::transport_7590().with_file(SCPD_DEVICE_INFO, scpd);
    let log = transport.requests();
    let mcp = common::start(transport).await;

    let outcome = common::call(&mcp, "get_system_info", json!({})).await;

    assert!(common::is_error(&outcome));
    assert_eq!(common::data(&outcome)["error"], "validation");
    assert!(log.soap().is_empty(), "{:?}", log.soap());
}

#[tokio::test]
async fn service_missing_from_description_rejected_without_soap_requests() {
    let description = common::fixture("scpd/tr64desc.xml").replace(
        "urn:dslforum-org:service:DeviceInfo:1",
        "urn:dslforum-org:service:DeviceInfo:2",
    );
    let transport = common::transport_7590().with_file("/tr64desc.xml", description);
    let log = transport.requests();
    let mcp = common::start(transport).await;

    let outcome = common::call(&mcp, "get_system_info", json!({})).await;

    assert!(common::is_error(&outcome));
    assert_eq!(common::data(&outcome)["error"], "validation");
    assert!(log.soap().is_empty(), "{:?}", log.soap());
}

#[tokio::test]
async fn soap_request_uses_control_url_and_soapaction_from_description() {
    let transport = common::transport_7590();
    let log = transport.requests();
    let mcp = common::start(transport).await;

    common::call(&mcp, "get_system_info", json!({})).await;

    let first = log.soap().into_iter().next().expect("one SOAP request");
    assert_eq!(first.path, "/upnp/control/deviceinfo");
    assert_eq!(
        first.soap_action.as_deref(),
        Some("urn:dslforum-org:service:DeviceInfo:1#GetInfo")
    );
}

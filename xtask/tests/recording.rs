//! Record command (ticket #8) on the `FixtureTransport`: no real router.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use frix_core::policy::{Classification, Level, classification};
use frix_core::tr064::SERVICE_URN_PREFIX;
use frix_core::transport::{
    Channel, FixtureTransport, HttpRequest, HttpResponse, HttpTransport, Method, RequestLog,
};
use xtask::reads::READS;
use xtask::record::{RecordingTransport, record};
use xtask::save::save;

const HOST_LIST: &str = "/devicehostlist.lua?sid=0123456789abcdef";
const MESH_LIST: &str = "/meshlist.lua?sid=0123456789abcdef";

const SECURITY_PORT: &str = r#"<?xml version="1.0"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
<s:Body>
<u:GetSecurityPortResponse xmlns:u="urn:dslforum-org:service:DeviceInfo:1">
<NewSecurityPort>49443</NewSecurityPort>
</u:GetSecurityPortResponse>
</s:Body>
</s:Envelope>
"#;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../crates/frix-core/tests/fixtures")
}

fn read_fixture(relative: &str) -> String {
    std::fs::read_to_string(fixture_dir().join(relative)).unwrap()
}

/// The 7590 fixtures plus the host and mesh lists, as in `frix-mcp/tests/host.rs`.
fn transport_7590(scpd: &Path, responses: &Path, lists: &[(&str, String)]) -> FixtureTransport {
    let mut transport = FixtureTransport::new()
        .with_descriptions_dir(scpd)
        .unwrap()
        .with_soap(
            "DeviceInfo:1",
            "GetSecurityPort",
            HttpResponse::ok(SECURITY_PORT),
        )
        .with_responses_dir(responses)
        .unwrap();
    for (path, body) in lists {
        transport = transport.with_file(path, body.clone());
    }
    transport
}

/// Access control by MAC uses the IP of the first active Host: the `…ByIP` fixtures
/// apply to any IP.
fn fixture_transport() -> FixtureTransport {
    let by_ip = |action: &str| {
        HttpResponse::ok(read_fixture(&format!(
            "responses/X_AVM-DE_HostFilter-1/{action}@NewIPv4Address=192.168.178.30.xml"
        )))
    };
    transport_7590(
        &fixture_dir().join("scpd"),
        &fixture_dir().join("responses"),
        &[
            (HOST_LIST, read_fixture("lists/devicehostlist.xml")),
            (MESH_LIST, read_fixture("lists/meshlist.json")),
        ],
    )
    .with_soap(
        "X_AVM-DE_HostFilter:1",
        "GetWANAccessByIP",
        by_ip("GetWANAccessByIP"),
    )
    .with_soap(
        "X_AVM-DE_HostFilter:1",
        "GetHostEntryByIP",
        by_ip("GetHostEntryByIP"),
    )
}

fn soap(service: &str, action: &str) -> HttpRequest {
    HttpRequest {
        method: Method::Post,
        channel: Channel::Secure,
        path: "/upnp/control/hosts".into(),
        soap_action: Some(format!("{SERVICE_URN_PREFIX}{service}#{action}")),
        body: String::new(),
    }
}

fn service_action(request: &HttpRequest) -> (String, String) {
    let (service, action) = request.soap_service_action().unwrap();
    (service.to_owned(), action.to_owned())
}

#[tokio::test]
async fn the_recording_transport_rejects_every_action_that_is_not_read() {
    let inner = FixtureTransport::new();
    let arrived: RequestLog = inner.requests();
    let recorder = RecordingTransport::new(Arc::new(inner));

    for (service, action) in [
        ("Hosts:1", "X_AVM-DE_SetFriendlyNameByMAC"), // Write
        ("DeviceConfig:1", "Reboot"),                 // Dangerous
        ("DeviceInfo:1", "SetProvisioningCode"),      // Forbidden
        ("DeviceInfo:1", "MadeUpAction"),             // no Classification
    ] {
        let error = recorder.send(soap(service, action)).await.unwrap_err();
        assert_eq!(error.kind(), "policy", "{service}#{action}");
    }

    assert!(arrived.all().is_empty(), "no request must go out");
    assert_eq!(recorder.exchanges(), []);
}

#[tokio::test]
async fn the_command_runs_every_tool_read_and_records_only_read_actions() {
    let recording = record(Arc::new(fixture_transport()), |_| {}).await.unwrap();

    for outcome in &recording.outcomes {
        assert!(
            outcome.error.is_none(),
            "{}: {:?}",
            outcome.tool,
            outcome.error
        );
    }
    let soap: Vec<_> = recording
        .exchanges
        .iter()
        .filter(|e| e.request.method == Method::Post)
        .collect();
    assert_ne!(soap.len(), 0);
    for exchange in soap {
        let (service, action) = service_action(&exchange.request);
        assert_eq!(
            classification(&service, &action),
            Some(Classification::Exposed(Level::Read)),
            "{service}#{action}"
        );
    }
    let lists: Vec<&str> = recording
        .exchanges
        .iter()
        .filter(|e| e.request.channel == Channel::Secure && e.request.method == Method::Get)
        .map(|e| e.request.path.as_str())
        .collect();
    assert!(
        lists.contains(&HOST_LIST) && lists.contains(&MESH_LIST),
        "{lists:?}"
    );
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("xtask-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[tokio::test]
async fn save_writes_unredacted_responses_and_redacted_copies_ready_as_fixtures() {
    let recording = record(Arc::new(fixture_transport()), |_| {}).await.unwrap();
    let dir = temp_dir("save");

    save(&recording.exchanges, &dir).unwrap();

    let unredacted = dir.join("unredacted");
    let redacted = dir.join("redacted");
    for relative in [
        "scpd/tr64desc.xml",
        "responses/DeviceInfo-1/GetInfo.xml",
        "responses/WLANConfiguration-1/GetGenericAssociatedDeviceInfo@NewAssociatedDeviceIndex=0.xml",
        "lists/devicehostlist.lua",
        "lists/meshlist.lua",
    ] {
        assert!(unredacted.join(relative).is_file(), "unredacted/{relative}");
        assert!(redacted.join(relative).is_file(), "redacted/{relative}");
    }
    // the MACs of the unredacted host list do not appear in the redacted copy
    let unredacted_list =
        std::fs::read_to_string(unredacted.join("lists/devicehostlist.lua")).unwrap();
    let redacted_list = std::fs::read_to_string(redacted.join("lists/devicehostlist.lua")).unwrap();
    let macs: Vec<&str> = unredacted_list
        .split("<MACAddress>")
        .skip(1)
        .filter_map(|r| r.split("</MACAddress>").next())
        .collect();
    assert_ne!(macs.len(), 0);
    for mac in macs {
        assert!(!redacted_list.contains(mac), "{mac} survives");
    }

    // the redacted copies are enough to run all the reads again
    let lists = [
        (HOST_LIST, redacted_list),
        (
            MESH_LIST,
            std::fs::read_to_string(redacted.join("lists/meshlist.lua")).unwrap(),
        ),
    ];
    let again = record(
        Arc::new(transport_7590(
            &redacted.join("scpd"),
            &redacted.join("responses"),
            &lists,
        )),
        |_| {},
    )
    .await
    .unwrap();
    for outcome in &again.outcomes {
        assert!(
            outcome.error.is_none(),
            "{}: {:?}",
            outcome.tool,
            outcome.error
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn reads_cover_every_server_tool() {
    let recording = record(Arc::new(fixture_transport()), |_| {}).await.unwrap();

    let mut covered: Vec<&str> = READS.iter().map(|r| r.tool).collect();
    covered.sort_unstable();
    covered.dedup();
    let mut tools = recording.server_tools.clone();
    tools.sort_unstable();
    assert_eq!(
        covered, tools,
        "a new tool must be added to xtask::reads::READS"
    );
}

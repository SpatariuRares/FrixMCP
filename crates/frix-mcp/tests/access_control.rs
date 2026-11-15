//! The `get_access_control` tool through the MCP protocol, on top of the fixture transport
//! (docs/PROJECT.md §4.2.7, §8.1; ADR 0002: the Host is identified by MAC, the IP is
//! resolved from the host list at call time).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use frix_core::transport::{FixtureTransport, RequestLog};
use serde_json::json;

/// Path returned by the `Hosts-1/X_AVM-DE_GetHostListPath.xml` fixture, SID included.
const HOST_LIST: &str = "/devicehostlist.lua?sid=0123456789abcdef";

/// The 7590 with the host list served at the path the router returns.
fn transport_with_host_list() -> FixtureTransport {
    common::transport_7590().with_file(HOST_LIST, common::fixture("lists/devicehostlist.xml"))
}

/// The `X_AVM-DE_HostFilter` actions that reached the transport, in order.
fn host_filter_actions(log: &RequestLog) -> Vec<String> {
    log.soap()
        .iter()
        .filter_map(|r| r.soap_action.as_deref())
        .filter_map(|a| a.strip_prefix("urn:dslforum-org:service:X_AVM-DE_HostFilter:1#"))
        .map(str::to_owned)
        .collect()
}

#[tokio::test]
async fn get_access_control_without_parameters_returns_the_filter_profiles() {
    let transport = transport_with_host_list();
    let log = transport.requests();
    let mcp = common::start(transport).await;

    let outcome = common::call(&mcp, "get_access_control", json!({})).await;

    assert!(!common::is_error(&outcome), "{outcome:?}");
    assert_eq!(
        common::data(&outcome),
        json!({
            "profiles": [
                { "id": "filtprof1", "name": "Standard" },
                { "id": "filtprof2", "name": "Gast" },
                { "id": "filtprof3", "name": "Gesperrt" }
            ]
        })
    );
    assert_eq!(host_filter_actions(&log), ["GetFilterProfiles"]);
}

#[tokio::test]
async fn get_access_control_with_mac_resolves_the_ip_from_the_host_list_and_returns_the_host_state()
{
    let transport = transport_with_host_list();
    let log = transport.requests();
    let mcp = common::start(transport).await;

    // iPad-Cucina (active) has 192.168.178.30, the same IP as the inactive Smart-TV
    let outcome = common::call(
        &mcp,
        "get_access_control",
        json!({ "mac": "02:00:00:00:10:02" }),
    )
    .await;

    assert!(!common::is_error(&outcome), "{outcome:?}");
    let data = common::data(&outcome);
    assert_eq!(data["profiles"].as_array().unwrap().len(), 3);
    assert_eq!(
        data["host"],
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
    let per_ip: Vec<_> = log
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
    for request in per_ip {
        assert!(
            request
                .body
                .contains("<NewIPv4Address>192.168.178.30</NewIPv4Address>"),
            "{}",
            request.body
        );
    }
}

#[tokio::test]
async fn get_access_control_with_an_unknown_mac_errors_without_calling_host_filter() {
    let transport = transport_with_host_list();
    let log = transport.requests();
    let mcp = common::start(transport).await;

    let outcome = common::call(
        &mcp,
        "get_access_control",
        json!({ "mac": "02:00:00:00:99:99" }),
    )
    .await;

    assert!(common::is_error(&outcome), "{outcome:?}");
    let error = common::data(&outcome);
    assert_eq!(error["error"], "no_such_entry");
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains("02:00:00:00:99:99"),
        "{error}"
    );
    assert_eq!(host_filter_actions(&log), Vec::<String>::new());
}

#[tokio::test]
async fn get_access_control_rejects_a_mac_whose_ip_now_belongs_to_another_active_host() {
    let transport = transport_with_host_list();
    let log = transport.requests();
    let mcp = common::start(transport).await;

    // Smart-TV (inactive) still has 192.168.178.30, which now belongs to the active iPad-Cucina
    let outcome = common::call(
        &mcp,
        "get_access_control",
        json!({ "mac": "02:00:00:00:10:03" }),
    )
    .await;

    assert!(common::is_error(&outcome), "{outcome:?}");
    assert_eq!(common::data(&outcome)["error"], "validation");
    assert_eq!(host_filter_actions(&log), Vec::<String>::new());
}

#[tokio::test]
async fn get_access_control_rejects_a_malformed_mac_without_router_requests() {
    let transport = transport_with_host_list();
    let log = transport.requests();
    let mcp = common::start(transport).await;

    let outcome = common::call(
        &mcp,
        "get_access_control",
        json!({ "mac": "192.168.178.30" }),
    )
    .await;

    assert!(common::is_error(&outcome), "{outcome:?}");
    assert_eq!(common::data(&outcome)["error"], "validation");
    assert!(log.soap().is_empty(), "{:?}", log.soap());
}

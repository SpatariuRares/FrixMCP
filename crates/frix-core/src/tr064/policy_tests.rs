//! Policy on the real 7590 data: every action of the fixture SCPDs has a Classification
//! and only Read-Level actions reach the transport (docs/PROJECT.md §4.2, §6.2).
//!
//! They live under `tr064` because the MCP seam cannot reach a non-Read action
//! (no tool calls one): here we go through [`Tr064Client::call`], which is `pub(crate)`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use super::{SERVICE_URN_PREFIX, Tr064Client};
use crate::policy::{
    Classification, Level, actions_with_rights, classification, registered_actions,
};
use crate::transport::{FixtureTransport, RequestLog};

async fn client_7590() -> (Tr064Client, RequestLog) {
    let scpd = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/scpd");
    let transport = FixtureTransport::new()
        .with_descriptions_dir(&scpd)
        .expect("SCPD fixture");
    let log = transport.requests();
    let client = Tr064Client::discover(Arc::new(transport))
        .await
        .expect("discovery on the fixtures");
    (client, log)
}

#[tokio::test]
async fn every_action_of_the_7590_scpds_has_a_classification() {
    let (client, _) = client_7590().await;

    let mut missing = Vec::new();
    for service in &client.services {
        let kind = service
            .service_type
            .strip_prefix(SERVICE_URN_PREFIX)
            .unwrap_or(&service.service_type);
        for action in service.actions.keys() {
            if classification(kind, action).is_none() {
                missing.push(format!("{kind}#{action}"));
            }
        }
    }

    assert!(client.services.len() >= 35, "{}", client.services.len());
    assert!(
        missing.is_empty(),
        "actions without Classification: {missing:#?}"
    );
}

#[tokio::test]
async fn every_registry_row_is_an_scpd_action_and_appears_once() {
    let (client, _) = client_7590().await;
    let existing: BTreeSet<(String, String)> = client
        .services
        .iter()
        .flat_map(|s| {
            let kind = s
                .service_type
                .strip_prefix(SERVICE_URN_PREFIX)
                .and_then(|t| t.rsplit_once(':'))
                .map_or(s.service_type.clone(), |(t, _)| t.to_owned());
            s.actions.keys().map(move |a| (kind.clone(), a.clone()))
        })
        .collect();

    let mut seen = BTreeSet::new();
    let mut duplicates = Vec::new();
    let mut orphans = Vec::new();
    for (service, action) in registered_actions() {
        let key = (service.to_owned(), action.to_owned());
        if !existing.contains(&key) {
            orphans.push(format!("{service}#{action}"));
        }
        if !seen.insert(key) {
            duplicates.push(format!("{service}#{action}"));
        }
    }

    assert!(
        orphans.is_empty(),
        "rows without an action in the SCPD: {orphans:#?}"
    );
    assert!(duplicates.is_empty(), "duplicate rows: {duplicates:#?}");
}

#[test]
fn every_rights_row_is_a_read_action_of_the_registry_and_appears_once() {
    let mut seen = BTreeSet::new();
    for (service, action) in actions_with_rights() {
        assert_eq!(
            classification(service, action),
            Some(Classification::Exposed(Level::Read)),
            "{service}#{action}"
        );
        assert!(
            seen.insert((service, action)),
            "duplicate: {service}#{action}"
        );
    }
}

#[test]
fn classifications_match_section_4_2() {
    let r = Some(Classification::Exposed(Level::Read));
    let w = Some(Classification::Exposed(Level::Write));
    let d = Some(Classification::Exposed(Level::Dangerous));
    let x = Some(Classification::Forbidden);
    let expected = [
        // §4.2.1
        ("DeviceInfo:1", "GetSecurityPort", r),
        ("DeviceInfo:1", "SetProvisioningCode", x),
        // §4.2.2
        ("Hosts:1", "X_AVM-DE_GetMeshListPath", r),
        ("Hosts:1", "X_AVM-DE_SetHostNameByMACAddress", w),
        ("Hosts:1", "X_AVM-DE_HostDoUpdate", d),
        // §4.2.3: the Wi-Fi password, guest network included, is Dangerous
        ("WLANConfiguration:1", "GetSecurityKeys", d),
        ("WLANConfiguration:3", "GetSecurityKeys", d),
        ("WLANConfiguration:1", "SetSecurityKeys", d),
        ("WLANConfiguration:3", "SetSecurityKeys", d),
        ("WLANConfiguration:3", "X_AVM-DE_GetWLANExtInfo", r),
        ("WLANConfiguration:2", "X_AVM-DE_SetWLANGlobalEnable", d),
        ("WLANConfiguration:1", "SetDefaultWEPKeyIndex", x),
        // §4.2.4–4.2.6
        ("WANCommonInterfaceConfig:1", "X_AVM-DE_GetOnlineMonitor", r),
        ("WANCommonInterfaceConfig:1", "X_AVM-DE_SetWANAccessType", x),
        ("WANDSLInterfaceConfig:1", "X_AVM-DE_GetDSLInfo", r),
        ("WANPPPConnection:1", "GetGenericPortMappingEntry", r),
        ("WANPPPConnection:1", "AddPortMapping", d),
        ("WANPPPConnection:1", "SetPassword", x),
        ("WANIPConnection:1", "ForceTermination", d),
        // §4.2.7–4.2.8
        ("X_AVM-DE_HostFilter:1", "GetFilterProfiles", r),
        ("X_AVM-DE_HostFilter:1", "DisallowWANAccessByIP", w),
        ("X_AVM-DE_Homeauto:1", "SetSwitch", w),
        // §4.2.14
        ("DeviceConfig:1", "X_AVM-DE_CreateUrlSID", r),
        ("DeviceConfig:1", "X_AVM-DE_GetConfigFile", x),
        // §4.2.15
        ("X_AVM-DE_RemoteAccess:1", "SetEnable", x),
        ("ManagementServer:1", "SetManagementServerPassword", x),
        // §4.2.16
        ("LANConfigSecurity:1", "X_AVM-DE_GetCurrentUser", r),
        ("LANConfigSecurity:1", "SetConfigPassword", x),
        ("Time:1", "SetNTPServers", d),
        ("UserInterface:1", "X_AVM-DE_CheckUpdate", w),
        ("UserInterface:1", "X_AVM-DE_DoUpdate", d),
        ("UserInterface:1", "X_AVM-DE_DoManualUpdate", x),
        ("X_AVM-DE_Storage:1", "SetFTPServerWAN", x),
        ("X_VoIP:1", "X_AVM-DE_GetVoIPAccount", x),
        // nonexistent action: no Classification
        ("DeviceInfo:1", "GetInfoV2", None),
    ];

    for (service, action, want) in expected {
        assert_eq!(classification(service, action), want, "{service}#{action}");
    }
}

/// Calls an action that exists in the SCPD with valid arguments and checks that the
/// Policy denies it before any SOAP request.
async fn denied_without_traffic(service: &str, action: &str, args: &[(&str, &str)]) {
    let (client, log) = client_7590().await;

    let result = client.call(service, action, args).await;

    let error = result.expect_err("the Policy must deny the action");
    assert_eq!(error.kind(), "policy", "{service}#{action}: {error}");
    assert!(log.soap().is_empty(), "{:?}", log.soap());
}

#[tokio::test]
async fn write_action_denied_without_traffic_to_the_transport() {
    denied_without_traffic(
        "Hosts:1",
        "X_AVM-DE_WakeOnLANByMACAddress",
        &[("NewMACAddress", "02:00:00:00:00:02")],
    )
    .await;
}

#[tokio::test]
async fn dangerous_action_denied_without_traffic_to_the_transport() {
    denied_without_traffic("DeviceConfig:1", "Reboot", &[]).await;
}

#[tokio::test]
async fn forbidden_action_denied_without_traffic_to_the_transport() {
    denied_without_traffic("DeviceConfig:1", "FactoryReset", &[]).await;
}

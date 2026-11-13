//! Policy sui dati reali del 7590: ogni azione degli SCPD fixture ha una Classification
//! e solo le azioni di Level Read raggiungono il transport (docs/PROJECT.md §4.2, §6.2).
//!
//! Stanno sotto `tr064` perché il seam MCP non può raggiungere un'azione non Read
//! (nessun tool la chiama): qui si passa da [`Tr064Client::call`], che è `pub(crate)`.
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
    let registro = transport.requests();
    let client = Tr064Client::discover(Arc::new(transport))
        .await
        .expect("discovery sulle fixture");
    (client, registro)
}

#[tokio::test]
async fn ogni_azione_degli_scpd_del_7590_ha_una_classification() {
    let (client, _) = client_7590().await;

    let mut senza = Vec::new();
    for servizio in &client.services {
        let tipo = servizio
            .service_type
            .strip_prefix(SERVICE_URN_PREFIX)
            .unwrap_or(&servizio.service_type);
        for azione in servizio.actions.keys() {
            if classification(tipo, azione).is_none() {
                senza.push(format!("{tipo}#{azione}"));
            }
        }
    }

    assert!(client.services.len() >= 35, "{}", client.services.len());
    assert!(senza.is_empty(), "azioni senza Classification: {senza:#?}");
}

#[tokio::test]
async fn ogni_riga_del_registro_e_un_azione_degli_scpd_e_compare_una_volta_sola() {
    let (client, _) = client_7590().await;
    let esistenti: BTreeSet<(String, String)> = client
        .services
        .iter()
        .flat_map(|s| {
            let tipo = s
                .service_type
                .strip_prefix(SERVICE_URN_PREFIX)
                .and_then(|t| t.rsplit_once(':'))
                .map_or(s.service_type.clone(), |(t, _)| t.to_owned());
            s.actions.keys().map(move |a| (tipo.clone(), a.clone()))
        })
        .collect();

    let mut viste = BTreeSet::new();
    let mut doppie = Vec::new();
    let mut orfane = Vec::new();
    for (servizio, azione) in registered_actions() {
        let chiave = (servizio.to_owned(), azione.to_owned());
        if !esistenti.contains(&chiave) {
            orfane.push(format!("{servizio}#{azione}"));
        }
        if !viste.insert(chiave) {
            doppie.push(format!("{servizio}#{azione}"));
        }
    }

    assert!(
        orfane.is_empty(),
        "righe senza azione nello SCPD: {orfane:#?}"
    );
    assert!(doppie.is_empty(), "righe duplicate: {doppie:#?}");
}

#[test]
fn ogni_riga_dei_diritti_e_un_azione_read_del_registro_e_compare_una_volta_sola() {
    let mut viste = BTreeSet::new();
    for (servizio, azione) in actions_with_rights() {
        assert_eq!(
            classification(servizio, azione),
            Some(Classification::Exposed(Level::Read)),
            "{servizio}#{azione}"
        );
        assert!(
            viste.insert((servizio, azione)),
            "doppia: {servizio}#{azione}"
        );
    }
}

#[test]
fn le_classification_corrispondono_alla_sezione_4_2() {
    let r = Some(Classification::Exposed(Level::Read));
    let w = Some(Classification::Exposed(Level::Write));
    let d = Some(Classification::Exposed(Level::Dangerous));
    let x = Some(Classification::Forbidden);
    let attese = [
        // §4.2.1
        ("DeviceInfo:1", "GetSecurityPort", r),
        ("DeviceInfo:1", "SetProvisioningCode", x),
        // §4.2.2
        ("Hosts:1", "X_AVM-DE_GetMeshListPath", r),
        ("Hosts:1", "X_AVM-DE_SetHostNameByMACAddress", w),
        ("Hosts:1", "X_AVM-DE_HostDoUpdate", d),
        // §4.2.3: la password Wi-Fi, anche della rete ospiti, è Dangerous
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
        // azione inesistente: nessuna Classification
        ("DeviceInfo:1", "GetInfoV2", None),
    ];

    for (servizio, azione, attesa) in attese {
        assert_eq!(
            classification(servizio, azione),
            attesa,
            "{servizio}#{azione}"
        );
    }
}

/// Chiama un'azione che esiste nello SCPD con argomenti validi e verifica che la
/// Policy la neghi prima di qualunque richiesta SOAP.
async fn negata_senza_traffico(service: &str, action: &str, args: &[(&str, &str)]) {
    let (client, registro) = client_7590().await;

    let esito = client.call(service, action, args).await;

    let errore = esito.expect_err("la Policy deve negare l'azione");
    assert_eq!(errore.kind(), "policy", "{service}#{action}: {errore}");
    assert!(registro.soap().is_empty(), "{:?}", registro.soap());
}

#[tokio::test]
async fn azione_write_negata_senza_traffico_verso_il_transport() {
    negata_senza_traffico(
        "Hosts:1",
        "X_AVM-DE_WakeOnLANByMACAddress",
        &[("NewMACAddress", "02:00:00:00:00:02")],
    )
    .await;
}

#[tokio::test]
async fn azione_dangerous_negata_senza_traffico_verso_il_transport() {
    negata_senza_traffico("DeviceConfig:1", "Reboot", &[]).await;
}

#[tokio::test]
async fn azione_forbidden_negata_senza_traffico_verso_il_transport() {
    negata_senza_traffico("DeviceConfig:1", "FactoryReset", &[]).await;
}

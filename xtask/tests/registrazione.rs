//! Comando di registrazione (ticket #8) sul `FixtureTransport`: nessun router reale.
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
use xtask::letture::LETTURE;
use xtask::registra::{TrasportoRegistratore, registra};
use xtask::salva::salva;

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

fn leggi(relativo: &str) -> String {
    std::fs::read_to_string(fixture_dir().join(relativo)).unwrap()
}

/// Le fixture del 7590 più le liste host e mesh, come in `frix-mcp/tests/host.rs`.
fn transport_7590(scpd: &Path, risposte: &Path, liste: &[(&str, String)]) -> FixtureTransport {
    let mut transport = FixtureTransport::new()
        .with_descriptions_dir(scpd)
        .unwrap()
        .with_soap(
            "DeviceInfo:1",
            "GetSecurityPort",
            HttpResponse::ok(SECURITY_PORT),
        )
        .with_responses_dir(risposte)
        .unwrap();
    for (path, corpo) in liste {
        transport = transport.with_file(path, corpo.clone());
    }
    transport
}

/// Il controllo accessi per MAC usa l'IP del primo Host attivo: le fixture `…ByIP`
/// valgono per qualunque IP.
fn transport_fixture() -> FixtureTransport {
    let per_ip = |azione: &str| {
        HttpResponse::ok(leggi(&format!(
            "responses/X_AVM-DE_HostFilter-1/{azione}@NewIPv4Address=192.168.178.30.xml"
        )))
    };
    transport_7590(
        &fixture_dir().join("scpd"),
        &fixture_dir().join("responses"),
        &[
            (HOST_LIST, leggi("lists/devicehostlist.xml")),
            (MESH_LIST, leggi("lists/meshlist.json")),
        ],
    )
    .with_soap(
        "X_AVM-DE_HostFilter:1",
        "GetWANAccessByIP",
        per_ip("GetWANAccessByIP"),
    )
    .with_soap(
        "X_AVM-DE_HostFilter:1",
        "GetHostEntryByIP",
        per_ip("GetHostEntryByIP"),
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

fn azione(richiesta: &HttpRequest) -> (String, String) {
    let (service, action) = richiesta.soap_service_action().unwrap();
    (service.to_owned(), action.to_owned())
}

#[tokio::test]
async fn il_trasporto_di_registrazione_rifiuta_ogni_azione_che_non_e_read() {
    let interno = FixtureTransport::new();
    let arrivate: RequestLog = interno.requests();
    let registratore = TrasportoRegistratore::new(Arc::new(interno));

    for (service, action) in [
        ("Hosts:1", "X_AVM-DE_SetFriendlyNameByMAC"), // Write
        ("DeviceConfig:1", "Reboot"),                 // Dangerous
        ("DeviceInfo:1", "SetProvisioningCode"),      // Forbidden
        ("DeviceInfo:1", "AzioneInventata"),          // senza Classification
    ] {
        let errore = registratore.send(soap(service, action)).await.unwrap_err();
        assert_eq!(errore.kind(), "policy", "{service}#{action}");
    }

    assert!(arrivate.all().is_empty(), "nessuna richiesta deve partire");
    assert_eq!(registratore.scambi(), []);
}

#[tokio::test]
async fn il_comando_esegue_le_letture_di_tutti_i_tool_e_registra_solo_azioni_read() {
    let registrazione = registra(Arc::new(transport_fixture()), |_| {})
        .await
        .unwrap();

    for esito in &registrazione.esiti {
        assert!(esito.errore.is_none(), "{}: {:?}", esito.tool, esito.errore);
    }
    let soap: Vec<_> = registrazione
        .scambi
        .iter()
        .filter(|s| s.richiesta.method == Method::Post)
        .collect();
    assert_ne!(soap.len(), 0);
    for scambio in soap {
        let (service, action) = azione(&scambio.richiesta);
        assert_eq!(
            classification(&service, &action),
            Some(Classification::Exposed(Level::Read)),
            "{service}#{action}"
        );
    }
    let liste: Vec<&str> = registrazione
        .scambi
        .iter()
        .filter(|s| s.richiesta.channel == Channel::Secure && s.richiesta.method == Method::Get)
        .map(|s| s.richiesta.path.as_str())
        .collect();
    assert!(
        liste.contains(&HOST_LIST) && liste.contains(&MESH_LIST),
        "{liste:?}"
    );
}

fn cartella_temporanea(nome: &str) -> PathBuf {
    let cartella = std::env::temp_dir().join(format!("xtask-{nome}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cartella);
    cartella
}

#[tokio::test]
async fn salva_le_risposte_grezze_e_copie_oscurate_pronte_come_fixture() {
    let registrazione = registra(Arc::new(transport_fixture()), |_| {})
        .await
        .unwrap();
    let cartella = cartella_temporanea("salva");

    salva(&registrazione.scambi, &cartella).unwrap();

    let grezze = cartella.join("grezze");
    let oscurate = cartella.join("oscurate");
    for relativo in [
        "scpd/tr64desc.xml",
        "responses/DeviceInfo-1/GetInfo.xml",
        "responses/WLANConfiguration-1/GetGenericAssociatedDeviceInfo@NewAssociatedDeviceIndex=0.xml",
        "lists/devicehostlist.lua",
        "lists/meshlist.lua",
    ] {
        assert!(grezze.join(relativo).is_file(), "grezze/{relativo}");
        assert!(oscurate.join(relativo).is_file(), "oscurate/{relativo}");
    }
    // i MAC della lista host grezza non compaiono nella copia oscurata
    let lista_grezza = std::fs::read_to_string(grezze.join("lists/devicehostlist.lua")).unwrap();
    let lista_oscurata =
        std::fs::read_to_string(oscurate.join("lists/devicehostlist.lua")).unwrap();
    let macs: Vec<&str> = lista_grezza
        .split("<MACAddress>")
        .skip(1)
        .filter_map(|r| r.split("</MACAddress>").next())
        .collect();
    assert_ne!(macs.len(), 0);
    for mac in macs {
        assert!(!lista_oscurata.contains(mac), "{mac} sopravvive");
    }

    // le copie oscurate bastano a rieseguire tutte le letture
    let liste = [
        (HOST_LIST, lista_oscurata),
        (
            MESH_LIST,
            std::fs::read_to_string(oscurate.join("lists/meshlist.lua")).unwrap(),
        ),
    ];
    let di_nuovo = registra(
        Arc::new(transport_7590(
            &oscurate.join("scpd"),
            &oscurate.join("responses"),
            &liste,
        )),
        |_| {},
    )
    .await
    .unwrap();
    for esito in &di_nuovo.esiti {
        assert!(esito.errore.is_none(), "{}: {:?}", esito.tool, esito.errore);
    }
    let _ = std::fs::remove_dir_all(&cartella);
}

#[tokio::test]
async fn le_letture_coprono_ogni_tool_del_server() {
    let registrazione = registra(Arc::new(transport_fixture()), |_| {})
        .await
        .unwrap();

    let mut coperti: Vec<&str> = LETTURE.iter().map(|l| l.tool).collect();
    coperti.sort_unstable();
    coperti.dedup();
    let mut tool = registrazione.tool_del_server.clone();
    tool.sort_unstable();
    assert_eq!(
        coperti, tool,
        "un tool nuovo va aggiunto a xtask::letture::LETTURE"
    );
}

//! Oscuramento delle risposte registrate, su dati sintetici (ticket #8).
//!
//! I valori "personali" qui sotto sono inventati: servono a verificare che nessuno
//! sopravviva, che XML e JSON restino validi e che un MAC abbia sempre lo stesso sostituto.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::Value;
use xtask::oscura::oscura;

const MAC: &str = "3C:A6:2F:12:34:56";
const MAC_HOST: &str = "A4:83:E7:AB:CD:EF";
const SERIALE: &str = "3CA62F123456";
const SSID: &str = "Casa Spatariu 5G";
const HOSTNAME: &str = "MacBook-di-Rares";
const SMART_HOME: &str = "Presa Soggiorno";
const IP_PUBBLICO: &str = "87.12.34.56";
const IPV6_PUBBLICO: &str = "2a02:810d:4b3f:9a00::1";

fn device_info() -> String {
    format!(
        r#"<?xml version="1.0"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
<s:Body>
<u:GetInfoResponse xmlns:u="urn:dslforum-org:service:DeviceInfo:1">
<NewManufacturerName>AVM</NewManufacturerName>
<NewModelName>FRITZ!Box 7590</NewModelName>
<NewSerialNumber>{SERIALE}</NewSerialNumber>
<NewDeviceLog>12.11.26 10:00:01 WLAN-Gerät angemeldet, {HOSTNAME}, {mac_minuscolo}.
12.11.26 09:00:00 Internetverbindung hergestellt. IP-Adresse: {IP_PUBBLICO}</NewDeviceLog>
</u:GetInfoResponse>
</s:Body>
</s:Envelope>
"#,
        mac_minuscolo = MAC_HOST.to_lowercase()
    )
}

fn wlan() -> String {
    format!(
        r#"<?xml version="1.0"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/">
<s:Body>
<u:GetInfoResponse xmlns:u="urn:dslforum-org:service:WLANConfiguration:2">
<NewSSID>{SSID}</NewSSID>
<NewBSSID>{MAC}</NewBSSID>
<NewChannel>36</NewChannel>
</u:GetInfoResponse>
</s:Body>
</s:Envelope>
"#
    )
}

fn wan() -> String {
    format!(
        r#"<?xml version="1.0"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/">
<s:Body>
<u:GetInfoResponse xmlns:u="urn:dslforum-org:service:WANPPPConnection:1">
<NewExternalIPAddress>{IP_PUBBLICO}</NewExternalIPAddress>
<NewExternalIPv6Address>{IPV6_PUBBLICO}</NewExternalIPv6Address>
<NewUpTime>3600</NewUpTime>
</u:GetInfoResponse>
</s:Body>
</s:Envelope>
"#
    )
}

fn host_list() -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<List>
<Item><Index>1</Index><IPAddress>192.168.178.20</IPAddress><MACAddress>{MAC_HOST}</MACAddress><HostName>{HOSTNAME}</HostName><Active>1</Active></Item>
<Item><Index>2</Index><IPAddress>192.168.178.30</IPAddress><MACAddress>02:11:22:33:44:55</MACAddress><HostName>Tom &amp; Jerry&apos;s TV</HostName><Active>0</Active></Item>
</List>
"#
    )
}

fn mesh() -> String {
    format!(
        r#"{{"schema_version": "8.7", "nodes": [
  {{"uid": "n-1", "device_name": "{HOSTNAME}", "device_mac_address": "{mac_minuscolo}",
    "node_interfaces": [{{"name": "WLAN", "mac_address": "{MAC}", "ssid": "{SSID}"}}]}},
  {{"uid": "n-2", "device_name": "{SMART_HOME}", "device_mac_address": "02:11:22:33:44:55", "node_interfaces": []}}
]}}"#,
        mac_minuscolo = MAC_HOST.to_lowercase()
    )
}

fn documenti() -> Vec<String> {
    vec![device_info(), wlan(), wan(), host_list(), mesh()]
}

fn originali() -> Vec<String> {
    vec![
        MAC.into(),
        MAC.to_lowercase(),
        MAC_HOST.into(),
        MAC_HOST.to_lowercase(),
        "02:11:22:33:44:55".into(),
        SERIALE.into(),
        SSID.into(),
        HOSTNAME.into(),
        SMART_HOME.into(),
        "Tom & Jerry's TV".into(),
        "Tom &amp; Jerry&apos;s TV".into(),
        IP_PUBBLICO.into(),
        IPV6_PUBBLICO.into(),
    ]
}

fn xml_valido(documento: &str) {
    let mut reader = Reader::from_str(documento);
    let mut profondita = 0_i32;
    loop {
        match reader.read_event() {
            Ok(Event::Start(_)) => profondita += 1,
            Ok(Event::End(_)) => profondita -= 1,
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => panic!("XML non valido dopo l'oscuramento ({e}):\n{documento}"),
        }
    }
    assert_eq!(profondita, 0, "elementi non bilanciati:\n{documento}");
}

#[test]
fn nessun_valore_originale_sopravvive() {
    let oscurati = oscura(&documenti()).unwrap();

    for (documento, originale) in oscurati
        .iter()
        .flat_map(|d| originali().into_iter().map(move |o| (d, o)))
    {
        assert!(
            !documento.contains(&originale),
            "{originale:?} sopravvive in:\n{documento}"
        );
    }
}

#[test]
fn xml_e_json_restano_validi_e_conservano_i_valori_non_sensibili() {
    let oscurati = oscura(&documenti()).unwrap();

    for documento in &oscurati[..4] {
        xml_valido(documento);
    }
    assert!(oscurati[0].contains("<NewModelName>FRITZ!Box 7590</NewModelName>"));
    assert!(oscurati[1].contains("<NewChannel>36</NewChannel>"));
    assert!(oscurati[3].contains("<IPAddress>192.168.178.20</IPAddress>"));
    let mesh: Value = serde_json::from_str(&oscurati[4]).expect("JSON valido");
    assert_eq!(mesh["schema_version"], "8.7");
    assert_eq!(mesh["nodes"][0]["node_interfaces"][0]["name"], "WLAN");
}

#[test]
fn lo_stesso_mac_ha_sempre_lo_stesso_sostituto() {
    let oscurati = oscura(&documenti()).unwrap();

    let lista: Vec<&str> = oscurati[3]
        .split("<MACAddress>")
        .skip(1)
        .map(|resto| resto.split("</MACAddress>").next().unwrap())
        .collect();
    let mesh: Value = serde_json::from_str(&oscurati[4]).unwrap();
    let nodi = mesh["nodes"].as_array().unwrap();
    let host = lista[0];
    assert_ne!(host, lista[1], "MAC diversi, sostituti diversi");
    assert!(host.parse::<frix_core::model::Mac>().is_ok(), "{host}");
    // stesso MAC: maiuscolo nella lista host, minuscolo nella mesh e nel log
    assert_eq!(
        nodi[0]["device_mac_address"]
            .as_str()
            .unwrap()
            .to_uppercase(),
        host
    );
    assert!(
        oscurati[0].contains(&host.to_lowercase()),
        "{}",
        oscurati[0]
    );
    assert_eq!(nodi[1]["device_mac_address"], lista[1]);
    // il seriale del 7590 è il MAC del router senza separatori: stesso sostituto del BSSID
    let bssid = oscurati[1]
        .split("<NewBSSID>")
        .nth(1)
        .and_then(|r| r.split("</NewBSSID>").next())
        .unwrap();
    let seriale = format!(
        "<NewSerialNumber>{}</NewSerialNumber>",
        bssid.replace(':', "")
    );
    assert!(oscurati[0].contains(&seriale), "{}", oscurati[0]);
}

#[test]
fn un_valore_che_sopravvive_blocca_l_oscuramento() {
    // l'hostname compare anche come nome di elemento: la struttura non si tocca, quindi resta
    let documenti = vec![
        "<List><Item><HostName>NomeSegreto</HostName></Item></List>".to_owned(),
        "<NomeSegreto>1</NomeSegreto>".to_owned(),
    ];

    let errore = oscura(&documenti).unwrap_err();

    assert!(errore.0.contains("Nome"), "{errore}");
    assert!(
        !errore.0.contains("NomeSegreto"),
        "l'errore non ripete il valore: {errore}"
    );
}

#[test]
fn un_documento_non_valido_e_un_errore() {
    assert!(oscura(&["<List><Item></List>".to_owned()]).is_err());
    assert!(oscura(&["{\"nodes\": [".to_owned()]).is_err());
}

#[test]
fn un_nome_corto_si_oscura_solo_nel_suo_campo() {
    let documenti =
        vec!["<List><Item><HostName>1</HostName><Active>1</Active></Item></List>".to_owned()];

    let oscurati = oscura(&documenti).unwrap();

    assert!(
        !oscurati[0].contains("<HostName>1</HostName>"),
        "{}",
        oscurati[0]
    );
    assert!(
        oscurati[0].contains("<Active>1</Active>"),
        "{}",
        oscurati[0]
    );
}

#[test]
fn i_commenti_xml_si_oscurano_come_il_testo() {
    let documenti = vec![format!(
        "<!-- lista di {HOSTNAME} -->\n<List><Item><HostName>{HOSTNAME}</HostName></Item></List>"
    )];

    let oscurati = oscura(&documenti).unwrap();

    assert!(!oscurati[0].contains(HOSTNAME), "{}", oscurati[0]);
    xml_valido(&oscurati[0]);
}

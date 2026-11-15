//! Redaction of recorded responses, on synthetic data (ticket #8).
//!
//! The "personal" values below are made up: they check that none of them
//! survives, that XML and JSON stay valid and that a MAC always gets the same substitute.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::Value;
use xtask::redact::redact;

const MAC: &str = "3C:A6:2F:12:34:56";
const HOST_MAC: &str = "A4:83:E7:AB:CD:EF";
const SERIAL: &str = "3CA62F123456";
const SSID: &str = "Smith Home 5G";
const HOSTNAME: &str = "Johns-MacBook";
const SMART_HOME: &str = "Living Room Plug";
const PUBLIC_IP: &str = "87.12.34.56";
const PUBLIC_IPV6: &str = "2a02:810d:4b3f:9a00::1";

fn device_info() -> String {
    format!(
        r#"<?xml version="1.0"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
<s:Body>
<u:GetInfoResponse xmlns:u="urn:dslforum-org:service:DeviceInfo:1">
<NewManufacturerName>AVM</NewManufacturerName>
<NewModelName>FRITZ!Box 7590</NewModelName>
<NewSerialNumber>{SERIAL}</NewSerialNumber>
<NewDeviceLog>12.11.26 10:00:01 WLAN-Gerät angemeldet, {HOSTNAME}, {lowercase_mac}.
12.11.26 09:00:00 Internetverbindung hergestellt. IP-Adresse: {PUBLIC_IP}</NewDeviceLog>
</u:GetInfoResponse>
</s:Body>
</s:Envelope>
"#,
        lowercase_mac = HOST_MAC.to_lowercase()
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
<NewExternalIPAddress>{PUBLIC_IP}</NewExternalIPAddress>
<NewExternalIPv6Address>{PUBLIC_IPV6}</NewExternalIPv6Address>
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
<Item><Index>1</Index><IPAddress>192.168.178.20</IPAddress><MACAddress>{HOST_MAC}</MACAddress><HostName>{HOSTNAME}</HostName><Active>1</Active></Item>
<Item><Index>2</Index><IPAddress>192.168.178.30</IPAddress><MACAddress>02:11:22:33:44:55</MACAddress><HostName>Tom &amp; Jerry&apos;s TV</HostName><Active>0</Active></Item>
</List>
"#
    )
}

fn mesh() -> String {
    format!(
        r#"{{"schema_version": "8.7", "nodes": [
  {{"uid": "n-1", "device_name": "{HOSTNAME}", "device_mac_address": "{lowercase_mac}",
    "node_interfaces": [{{"name": "WLAN", "mac_address": "{MAC}", "ssid": "{SSID}"}}]}},
  {{"uid": "n-2", "device_name": "{SMART_HOME}", "device_mac_address": "02:11:22:33:44:55", "node_interfaces": []}}
]}}"#,
        lowercase_mac = HOST_MAC.to_lowercase()
    )
}

fn documents() -> Vec<String> {
    vec![device_info(), wlan(), wan(), host_list(), mesh()]
}

fn originals() -> Vec<String> {
    vec![
        MAC.into(),
        MAC.to_lowercase(),
        HOST_MAC.into(),
        HOST_MAC.to_lowercase(),
        "02:11:22:33:44:55".into(),
        SERIAL.into(),
        SSID.into(),
        HOSTNAME.into(),
        SMART_HOME.into(),
        "Tom & Jerry's TV".into(),
        "Tom &amp; Jerry&apos;s TV".into(),
        PUBLIC_IP.into(),
        PUBLIC_IPV6.into(),
    ]
}

fn assert_valid_xml(document: &str) {
    let mut reader = Reader::from_str(document);
    let mut depth = 0_i32;
    loop {
        match reader.read_event() {
            Ok(Event::Start(_)) => depth += 1,
            Ok(Event::End(_)) => depth -= 1,
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => panic!("invalid XML after redaction ({e}):\n{document}"),
        }
    }
    assert_eq!(depth, 0, "unbalanced elements:\n{document}");
}

#[test]
fn no_original_value_survives() {
    let redacted = redact(&documents()).unwrap();

    for (document, original) in redacted
        .iter()
        .flat_map(|d| originals().into_iter().map(move |o| (d, o)))
    {
        assert!(
            !document.contains(&original),
            "{original:?} survives in:\n{document}"
        );
    }
}

#[test]
fn xml_and_json_stay_valid_and_keep_non_sensitive_values() {
    let redacted = redact(&documents()).unwrap();

    for document in &redacted[..4] {
        assert_valid_xml(document);
    }
    assert!(redacted[0].contains("<NewModelName>FRITZ!Box 7590</NewModelName>"));
    assert!(redacted[1].contains("<NewChannel>36</NewChannel>"));
    assert!(redacted[3].contains("<IPAddress>192.168.178.20</IPAddress>"));
    let mesh: Value = serde_json::from_str(&redacted[4]).expect("valid JSON");
    assert_eq!(mesh["schema_version"], "8.7");
    assert_eq!(mesh["nodes"][0]["node_interfaces"][0]["name"], "WLAN");
}

#[test]
fn the_same_mac_always_gets_the_same_substitute() {
    let redacted = redact(&documents()).unwrap();

    let list: Vec<&str> = redacted[3]
        .split("<MACAddress>")
        .skip(1)
        .map(|rest| rest.split("</MACAddress>").next().unwrap())
        .collect();
    let mesh: Value = serde_json::from_str(&redacted[4]).unwrap();
    let nodes = mesh["nodes"].as_array().unwrap();
    let host = list[0];
    assert_ne!(host, list[1], "different MACs, different substitutes");
    assert!(host.parse::<frix_core::model::Mac>().is_ok(), "{host}");
    // same MAC: uppercase in the host list, lowercase in the mesh and in the log
    assert_eq!(
        nodes[0]["device_mac_address"]
            .as_str()
            .unwrap()
            .to_uppercase(),
        host
    );
    assert!(
        redacted[0].contains(&host.to_lowercase()),
        "{}",
        redacted[0]
    );
    assert_eq!(nodes[1]["device_mac_address"], list[1]);
    // the 7590's serial is the router's MAC without separators: same substitute as the BSSID
    let bssid = redacted[1]
        .split("<NewBSSID>")
        .nth(1)
        .and_then(|r| r.split("</NewBSSID>").next())
        .unwrap();
    let serial = format!(
        "<NewSerialNumber>{}</NewSerialNumber>",
        bssid.replace(':', "")
    );
    assert!(redacted[0].contains(&serial), "{}", redacted[0]);
}

#[test]
fn a_surviving_value_blocks_redaction() {
    // the hostname also appears as an element name: the structure is not touched, so it stays
    let documents = vec![
        "<List><Item><HostName>SecretValue</HostName></Item></List>".to_owned(),
        "<SecretValue>1</SecretValue>".to_owned(),
    ];

    let error = redact(&documents).unwrap_err();

    assert!(error.0.contains("Name"), "{error}");
    assert!(
        !error.0.contains("SecretValue"),
        "the error does not repeat the value: {error}"
    );
}

#[test]
fn an_invalid_document_is_an_error() {
    assert!(redact(&["<List><Item></List>".to_owned()]).is_err());
    assert!(redact(&["{\"nodes\": [".to_owned()]).is_err());
}

#[test]
fn a_short_name_is_redacted_only_in_its_own_field() {
    let documents =
        vec!["<List><Item><HostName>1</HostName><Active>1</Active></Item></List>".to_owned()];

    let redacted = redact(&documents).unwrap();

    assert!(
        !redacted[0].contains("<HostName>1</HostName>"),
        "{}",
        redacted[0]
    );
    assert!(
        redacted[0].contains("<Active>1</Active>"),
        "{}",
        redacted[0]
    );
}

#[test]
fn xml_comments_are_redacted_like_text() {
    let documents = vec![format!(
        "<!-- list of {HOSTNAME} -->\n<List><Item><HostName>{HOSTNAME}</HostName></Item></List>"
    )];

    let redacted = redact(&documents).unwrap();

    assert!(!redacted[0].contains(HOSTNAME), "{}", redacted[0]);
    assert_valid_xml(&redacted[0]);
}

#[test]
fn provider_dns_servers_are_redacted_and_lan_ones_stay() {
    let dns = |service: &str, value: &str| {
        format!(
            r#"<?xml version="1.0"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/">
<s:Body>
<u:GetInfoResponse xmlns:u="urn:dslforum-org:service:{service}:1">
<NewDNSServers>{value}</NewDNSServers>
</u:GetInfoResponse>
</s:Body>
</s:Envelope>
"#
        )
    };
    let documents = [
        dns("WANPPPConnection", "87.12.0.70, 87.12.4.70"),
        dns("LANHostConfigManagement", "192.168.178.1"),
    ];

    let redacted = redact(&documents).unwrap();

    for original in ["87.12.0.70", "87.12.4.70"] {
        assert!(!redacted[0].contains(original), "{original} survives");
    }
    let wan = redacted[0]
        .split("<NewDNSServers>")
        .nth(1)
        .and_then(|rest| rest.split('<').next())
        .unwrap();
    assert_eq!(wan.split(',').count(), 2, "{wan}");
    assert!(redacted[1].contains("<NewDNSServers>192.168.178.1</NewDNSServers>"));
}

#[test]
fn a_host_named_none_does_not_change_the_other_nones() {
    let list = r#"<?xml version="1.0" encoding="utf-8"?>
<List>
<Item><Index>1</Index><IPAddress>192.168.178.21</IPAddress><MACAddress>02:11:22:33:44:56</MACAddress><HostName>none</HostName><Active>1</Active></Item>
</List>
"#;
    let rights = r#"<?xml version="1.0"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/">
<s:Body>
<u:X_AVM-DE_GetCurrentUserResponse xmlns:u="urn:dslforum-org:service:LANConfigSecurity:1">
<NewX_AVM-DE_CurrentUserRights>&lt;rights&gt;&lt;path&gt;NAS&lt;/path&gt;&lt;access&gt;none&lt;/access&gt;&lt;/rights&gt;</NewX_AVM-DE_CurrentUserRights>
</u:X_AVM-DE_GetCurrentUserResponse>
</s:Body>
</s:Envelope>
"#;

    let redacted = redact(&[list.to_owned(), rights.to_owned()]).unwrap();

    assert!(
        redacted[0].contains("<HostName>none</HostName>"),
        "{}",
        redacted[0]
    );
    assert!(
        redacted[1].contains("&lt;access&gt;none&lt;"),
        "{}",
        redacted[1]
    );
}

#[test]
fn ipv6_addresses_are_redacted_in_any_field_with_the_same_substitute() {
    // EUI-64: the interface ID contains a MAC; the global prefix identifies the line.
    let global = "2a02:810d:4b3f:9a00:3ea6:2fff:fe12:3456";
    let link_local = "fe80::3ea6:2fff:fe12:3456";
    let ula = "fd12:3456:789a::1";
    let mesh = format!(
        r#"{{"nodes":[{{"node_interfaces":[{{"ipv6":"{global}","peer":"{link_local}","dns":["{ula}","::1","::ffff:192.168.178.1"]}}]}}],"note":"seen {link_local}. again {global}"}}"#
    );

    let redacted = redact(&[mesh]).unwrap();
    let text = &redacted[0];

    for original in [global, link_local, ula] {
        assert!(!text.contains(original), "{original} survived: {text}");
    }
    let value: Value = serde_json::from_str(text).unwrap();
    let interface = &value["nodes"][0]["node_interfaces"][0];
    let global_sub = interface["ipv6"].as_str().unwrap();
    let local_sub = interface["peer"].as_str().unwrap();
    assert!(global_sub.starts_with("2001:db8::"), "{global_sub}");
    assert!(local_sub.starts_with("fe80::"), "{local_sub}");
    assert!(!local_sub.contains("ff:fe"), "{local_sub}");
    assert!(interface["dns"][0].as_str().unwrap().starts_with("fd00::"));
    assert_eq!(interface["dns"][1], "::1");
    assert_eq!(interface["dns"][2], "::ffff:192.168.178.1");
    let note = value["note"].as_str().unwrap();
    assert_eq!(note, format!("seen {local_sub}. again {global_sub}"));
}

#[test]
fn macs_and_times_are_not_taken_for_ipv6() {
    let log = "12.11.26 10:00:01 WLAN-Gerät angemeldet, aa:bb:cc:dd:ee:ff.";
    let redacted = redact(&[log.to_owned()]).unwrap();
    assert!(redacted[0].starts_with("12.11.26 10:00:01 WLAN-Gerät angemeldet, 02:00:00:00:"));
}

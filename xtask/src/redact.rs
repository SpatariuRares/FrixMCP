//! Redaction of recorded responses (docs/PROJECT.md §6.6, ticket #8).
//!
//! Two passes over all the documents of a recording:
//! 1. **collect**: the values of the sensitive fields ([`FIELDS`]), every MAC and every
//!    IPv6 address found in the text;
//! 2. **replace**: every collected value is replaced wherever it appears (even inside
//!    the log or in another document) with a substitute of its category.
//!
//! The structure does not change: in XML only text and attribute values are rewritten
//! (never element names), in JSON only strings (never keys). The same MAC always
//! gets the same substitute, in any document and with any case or separator.
//! IPv6 addresses are recognised by their shape in any field, like MACs: the provider's
//! prefix identifies the line, and an EUI-64 interface ID (`…ff:fe…`) contains the MAC,
//! even in a link-local `fe80::` address.
//! At the end every document is checked again: if a collected value survives,
//! redaction fails and no fixture is written.
//!
//! Limit: a value shorter than [`MIN_SUBSTRING`] characters is replaced only
//! when it is the whole value of a field, not as a substring of free text.

use std::borrow::Cow;
use std::collections::BTreeMap;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::Value;

/// Redaction error: the document cannot be published as a fixture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

/// What a sensitive value represents; decides the shape of its substitute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Category {
    Mac,
    Serial,
    Ssid,
    Name,
    PublicIp,
    Sid,
}

/// Sensitive fields by local name (XML element or attribute, JSON key).
/// MACs are not needed here: they are recognised by their shape in any field.
const FIELDS: &[(&str, Category)] = &[
    // serials and device identifiers
    ("NewSerialNumber", Category::Serial),
    ("serialNumber", Category::Serial),
    ("NewProvisioningCode", Category::Serial),
    ("NewAIN", Category::Serial),
    ("AIN", Category::Serial),
    ("ain", Category::Serial),
    ("identifier", Category::Serial),
    // SSID
    ("NewSSID", Category::Ssid),
    ("ssid", Category::Ssid),
    // hostnames, Host and SmartHome Device names, provider account
    ("friendlyName", Category::Name),
    ("HostName", Category::Name),
    ("NewHostName", Category::Name),
    ("X_AVM-DE_FriendlyName", Category::Name),
    ("NewX_AVM-DE_FriendlyName", Category::Name),
    ("device_name", Category::Name),
    ("NewDeviceName", Category::Name),
    ("NewUserName", Category::Name),
    ("NewPortMappingDescription", Category::Name),
    // public IP
    ("NewExternalIPAddress", Category::PublicIp),
    ("NewExternalIPv6Address", Category::PublicIp),
    ("NewIPv6Prefix", Category::PublicIp),
    ("NewPrefix", Category::PublicIp),
];

/// Fields holding a comma-separated list of IPs: only the public IPs are redacted
/// (the provider's DNS servers reveal the operator; the LAN ones stay).
const IP_LIST_FIELDS: &[&str] = &["NewDNSServers"];

/// Values the router puts in place of a missing name: they identify no one,
/// and replacing them as substrings would change unrelated fields (`<access>none`).
const MISSING_NAME_VALUES: &[&str] = &["none", "unknown"];

/// Minimum length (in characters) for a value to be replaced inside free text too.
pub const MIN_SUBSTRING: usize = 4;

/// Fixed substitute for the SID in list paths (same value as the hand-written fixtures).
const FIXED_SID: &str = "0123456789abcdef";

/// Format of a recorded document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// SOAP envelopes, descriptions, XML lists.
    Xml,
    /// Mesh topology.
    Json,
    /// Any other text (for example a log downloaded from a path).
    Text,
}

impl Format {
    /// Detects the format from the first significant character.
    #[must_use]
    pub fn detect(document: &str) -> Self {
        match document.trim_start().chars().next() {
            Some('<') => Self::Xml,
            Some('{' | '[') => Self::Json,
            _ => Self::Text,
        }
    }
}

/// Redacts all the documents of a recording together and returns the redacted
/// copies, in the same order. Fails if a document is not valid XML/JSON or if a
/// collected value survives; the error names the document (from 0) without
/// repeating the value.
pub fn redact(documents: &[String]) -> Result<Vec<String>, Error> {
    let in_document = |i: usize| move |e: Error| Error(format!("document {i}: {e}"));
    let mut redactor = Redactor::default();
    for (i, document) in documents.iter().enumerate() {
        walk(document, &mut |field, text| {
            redactor.collect(field, text);
            text.to_owned()
        })
        .map_err(in_document(i))?;
    }
    let redactor = redactor.ready();
    documents
        .iter()
        .enumerate()
        .map(|(i, document)| {
            let redacted = walk(document, &mut |field, text| redactor.replace(field, text))
                .map_err(in_document(i))?;
            redactor.check(&redacted).map_err(in_document(i))?;
            Ok(redacted)
        })
        .collect()
}

/// Collected values and their substitutes.
#[derive(Debug, Default)]
struct Redactor {
    /// original → (category, substitute)
    values: BTreeMap<String, (Category, String)>,
    /// normalised MAC (`AA:BB:…`) → substitute
    macs: BTreeMap<String, String>,
    /// canonical IPv6 address → substitute; replaced as whole tokens, never as
    /// substrings (`fe80::1` is a substring of `fe80::1872:…`)
    ipv6: BTreeMap<std::net::Ipv6Addr, String>,
    counters: BTreeMap<Category, u32>,
    /// Replacements to apply as substrings, longest first.
    by_length: Vec<(String, String)>,
}

impl Redactor {
    fn next_index(&mut self, category: Category) -> u32 {
        let n = self.counters.entry(category).or_insert(0);
        *n += 1;
        *n
    }

    fn collect(&mut self, field: &str, text: &str) {
        for mac in find_macs(text) {
            self.add_mac(&mac.normalized);
        }
        for ip in find_ipv6(text) {
            self.add_ipv6(ip.address);
        }
        for sid in find_sids(text) {
            self.values
                .entry(sid.to_owned())
                .or_insert((Category::Sid, FIXED_SID.to_owned()));
        }
        if IP_LIST_FIELDS.contains(&field) {
            for ip in text.split(',').map(str::trim) {
                if ip
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| !is_local_ip(ip))
                {
                    self.add(Category::PublicIp, ip);
                }
            }
            return;
        }
        let Some(category) = field_category(field) else {
            return;
        };
        self.add(category, text);
    }

    fn add(&mut self, category: Category, text: &str) {
        let value = text.trim();
        if let Ok(address) = value.parse::<std::net::Ipv6Addr>() {
            if category == Category::PublicIp {
                self.add_ipv6(address);
            }
            return;
        }
        if value.is_empty()
            || MISSING_NAME_VALUES.contains(&value.to_lowercase().as_str())
            || self.values.contains_key(value)
        {
            return;
        }
        // the 7590's serial is the MAC without separators: same substitute as the MAC
        if category == Category::Serial
            && let Some(mac) = mac_from_hex(value)
        {
            self.add_mac(&mac);
            return;
        }
        let n = self.next_index(category);
        let substitute = match category {
            Category::Serial => format!("SERIALE{n:05}"),
            Category::Ssid => format!("WLAN-{n:02}"),
            Category::PublicIp => format!("203.0.113.{}", n % 254 + 1),
            Category::Name | Category::Mac | Category::Sid => format!("nome-{n:03}"),
        };
        self.values.insert(value.to_owned(), (category, substitute));
    }

    fn add_mac(&mut self, normalized: &str) {
        if self.macs.contains_key(normalized) {
            return;
        }
        let n = self.next_index(Category::Mac) + 0x0f;
        let substitute = format!("02:00:00:00:{:02X}:{:02X}", (n >> 8) & 0xff, n & 0xff);
        let without_separators = normalized.replace(':', "");
        let substitute_without = substitute.replace(':', "");
        for form in [
            without_separators.clone(),
            without_separators.to_lowercase(),
        ] {
            self.values
                .insert(form, (Category::Mac, substitute_without.clone()));
        }
        self.macs.insert(normalized.to_owned(), substitute);
    }

    fn add_ipv6(&mut self, address: std::net::Ipv6Addr) {
        if self.ipv6.contains_key(&address) || !is_identifying_ipv6(address) {
            return;
        }
        let n = self.next_index(Category::PublicIp);
        let first = address.segments().first().copied().unwrap_or(0);
        // same scope as the original, in a range that identifies no one
        let substitute = if (first & 0xffc0) == 0xfe80 {
            format!("fe80::{n:x}")
        } else if (first & 0xfe00) == 0xfc00 {
            format!("fd00::{n:x}")
        } else {
            format!("2001:db8::{n:x}")
        };
        self.ipv6.insert(address, substitute);
    }

    fn ready(mut self) -> Self {
        let mut by_length: Vec<(String, String)> = self
            .values
            .iter()
            .filter(|(original, _)| original.chars().count() >= MIN_SUBSTRING)
            .map(|(original, (_, substitute))| (original.clone(), substitute.clone()))
            .collect();
        by_length.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.0.cmp(&b.0)));
        self.by_length = by_length;
        self
    }

    fn replace(&self, field: &str, text: &str) -> String {
        // sensitive field: the whole value is replaced, even if short
        if field_category(field).is_some()
            && let Some((_, substitute)) = self.values.get(text.trim())
        {
            return text.replace(text.trim(), substitute);
        }
        let mut result = self.replace_ipv6(&self.replace_macs(text));
        for (original, substitute) in &self.by_length {
            if result.contains(original.as_str()) {
                result = result.replace(original.as_str(), substitute);
            }
        }
        result
    }

    fn replace_macs(&self, text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        let mut last = 0;
        for mac in find_macs(text) {
            let Some(substitute) = self.macs.get(&mac.normalized) else {
                continue;
            };
            let substitute = if mac.separator == '-' {
                substitute.replace(':', "-")
            } else {
                substitute.clone()
            };
            result.push_str(text.get(last..mac.start).unwrap_or_default());
            if mac.lowercase {
                result.push_str(&substitute.to_lowercase());
            } else {
                result.push_str(&substitute);
            }
            last = mac.end;
        }
        result.push_str(text.get(last..).unwrap_or_default());
        result
    }

    fn replace_ipv6(&self, text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        let mut last = 0;
        for ip in find_ipv6(text) {
            let Some(substitute) = self.ipv6.get(&ip.address) else {
                continue;
            };
            result.push_str(text.get(last..ip.start).unwrap_or_default());
            result.push_str(substitute);
            last = ip.end;
        }
        result.push_str(text.get(last..).unwrap_or_default());
        result
    }

    /// No collected value (plain, XML-escaped or JSON-escaped), no unredacted MAC and
    /// no identifying IPv6 address that is not a substitute may remain in the document.
    fn check(&self, document: &str) -> Result<(), Error> {
        for (original, (category, _)) in &self.values {
            if original.chars().count() < MIN_SUBSTRING || *category == Category::Sid {
                continue;
            }
            let xml_escaped = quick_xml::escape::escape(original.as_str());
            let json_escaped = serde_json::to_string(original).unwrap_or_default();
            let json_escaped = json_escaped.trim_matches('"');
            if document.contains(original.as_str())
                || document.contains(xml_escaped.as_ref())
                || document.contains(json_escaped)
            {
                return Err(Error(format!(
                    "a value of category {category:?} survived redaction"
                )));
            }
        }
        let substitutes: Vec<&String> = self.macs.values().collect();
        if let Some(mac) = find_macs(document)
            .into_iter()
            .find(|m| !substitutes.contains(&&m.normalized))
        {
            return Err(Error(format!(
                "an unredacted MAC remains in the document (position {})",
                mac.start
            )));
        }
        let ipv6_substitutes: Vec<&String> = self.ipv6.values().collect();
        if let Some(ip) = find_ipv6(document).into_iter().find(|ip| {
            is_identifying_ipv6(ip.address) && !ipv6_substitutes.contains(&&ip.address.to_string())
        }) {
            return Err(Error(format!(
                "an unredacted IPv6 address remains in the document (position {})",
                ip.start
            )));
        }
        Ok(())
    }
}

/// Any IPv6 address except loopback, unspecified and IPv4-mapped (the IPv4 part is
/// handled as IPv4). Link-local and ULA count too: their interface ID can be EUI-64,
/// that is the device's MAC.
fn is_identifying_ipv6(address: std::net::Ipv6Addr) -> bool {
    !address.is_loopback() && !address.is_unspecified() && address.to_ipv4_mapped().is_none()
}

/// An IPv6 address found in a text.
#[derive(Debug)]
struct FoundIpv6 {
    start: usize,
    end: usize,
    address: std::net::Ipv6Addr,
}

/// IPv6 addresses in a text: runs of hex digits, `:` and `.` that parse as an IPv6
/// address. A MAC (`AA:BB:CC:DD:EE:FF`) or a time (`12:29:10`) does not parse.
fn find_ipv6(text: &str) -> Vec<FoundIpv6> {
    let is_part = |c: u8| c.is_ascii_hexdigit() || c == b':' || c == b'.';
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes.get(i).copied().is_some_and(is_part) {
            i += 1;
            continue;
        }
        let start = i;
        while bytes.get(i).copied().is_some_and(is_part) {
            i += 1;
        }
        // a sentence-ending dot is not part of the address
        let mut end = i;
        while end > start && bytes.get(end - 1) == Some(&b'.') {
            end -= 1;
        }
        let candidate = text.get(start..end).unwrap_or_default();
        if candidate.matches(':').count() >= 2
            && let Ok(address) = candidate.parse::<std::net::Ipv6Addr>()
        {
            found.push(FoundIpv6 {
                start,
                end,
                address,
            });
        }
    }
    found
}

/// Home network or non-routable IP: identifies no one outside the LAN.
fn is_local_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            v4.is_private() || v4.is_loopback() || v4.is_link_local() || v4.is_unspecified()
        }
        std::net::IpAddr::V6(v6) => {
            let first = v6.segments().first().copied().unwrap_or(0);
            v6.is_loopback()
                || v6.is_unspecified()
                || (first & 0xfe00) == 0xfc00
                || (first & 0xffc0) == 0xfe80
        }
    }
}

fn field_category(field: &str) -> Option<Category> {
    FIELDS
        .iter()
        .find(|(name, _)| *name == field)
        .map(|(_, category)| *category)
}

/// A MAC found in a text.
#[derive(Debug)]
struct FoundMac {
    start: usize,
    end: usize,
    /// `AA:BB:CC:DD:EE:FF` form.
    normalized: String,
    separator: char,
    lowercase: bool,
}

/// MACs with a `:` or `-` separator (six hex pairs), not adjacent to other hex digits.
fn find_macs(text: &str) -> Vec<FoundMac> {
    const LENGTH: usize = 17;
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut i = 0;
    while i + LENGTH <= bytes.len() {
        let window = bytes.get(i..i + LENGTH).unwrap_or_default();
        let separator = window.get(2).copied().unwrap_or_default();
        let mac_shape = (separator == b':' || separator == b'-')
            && window.iter().enumerate().all(|(j, c)| {
                if j % 3 == 2 {
                    *c == separator
                } else {
                    c.is_ascii_hexdigit()
                }
            });
        let free_before = i == 0
            || bytes
                .get(i - 1)
                .is_none_or(|c| !c.is_ascii_hexdigit() && *c != separator);
        let free_after = bytes
            .get(i + LENGTH)
            .is_none_or(|c| !c.is_ascii_hexdigit() && *c != separator);
        if mac_shape && free_before && free_after {
            let original = text.get(i..i + LENGTH).unwrap_or_default();
            found.push(FoundMac {
                start: i,
                end: i + LENGTH,
                normalized: original.to_uppercase().replace('-', ":"),
                separator: char::from(separator),
                lowercase: original.chars().any(|c| c.is_ascii_lowercase()),
            });
            i += LENGTH;
        } else {
            i += 1;
        }
    }
    found
}

/// Normalised MAC from 12 hex digits without separators, if the value has that shape.
fn mac_from_hex(value: &str) -> Option<String> {
    if value.len() != 12 || !value.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let upper = value.to_uppercase();
    let pairs: Vec<&str> = (0..6).filter_map(|k| upper.get(k * 2..k * 2 + 2)).collect();
    Some(pairs.join(":"))
}

/// Values of `sid=` in list paths.
fn find_sids(text: &str) -> Vec<&str> {
    text.match_indices("sid=")
        .filter_map(|(i, key)| {
            let rest = text.get(i + key.len()..)?;
            let end = rest
                .find(|c: char| !c.is_ascii_hexdigit())
                .unwrap_or(rest.len());
            rest.get(..end).filter(|s| !s.is_empty())
        })
        .collect()
}

/// Applies `f(field, text)` to every text value of the document and returns the
/// document rewritten with the same structure.
fn walk(document: &str, f: &mut dyn FnMut(&str, &str) -> String) -> Result<String, Error> {
    match Format::detect(document) {
        Format::Xml => walk_xml(document, f),
        Format::Json => walk_json(document, f),
        Format::Text => Ok(f("", document)),
    }
}

fn walk_json(document: &str, f: &mut dyn FnMut(&str, &str) -> String) -> Result<String, Error> {
    fn visit(value: &mut Value, field: &str, f: &mut dyn FnMut(&str, &str) -> String) {
        match value {
            Value::String(s) => *s = f(field, s),
            Value::Array(items) => {
                for item in items {
                    visit(item, field, f);
                }
            }
            Value::Object(map) => {
                for (key, item) in map.iter_mut() {
                    visit(item, key, f);
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) => {}
        }
    }
    let mut value: Value =
        serde_json::from_str(document).map_err(|e| Error(format!("invalid JSON: {e}")))?;
    visit(&mut value, "", f);
    let mut text = serde_json::to_string_pretty(&value)
        .map_err(|e| Error(format!("JSON not serializable: {e}")))?;
    text.push('\n');
    Ok(text)
}

fn walk_xml(document: &str, f: &mut dyn FnMut(&str, &str) -> String) -> Result<String, Error> {
    let error = |e: &dyn std::fmt::Display| Error(format!("invalid XML: {e}"));
    let mut reader = Reader::from_str(document);
    let mut output = String::with_capacity(document.len());
    let mut elements: Vec<String> = Vec::new();
    let mut text = String::new();
    loop {
        let start = position(reader.buffer_position())?;
        let event = reader.read_event().map_err(|e| error(&e))?;
        let end = position(reader.buffer_position())?;
        let raw = document.get(start..end).unwrap_or_default();
        match event {
            Event::Text(t) => {
                text.push_str(&t.xml10_content());
                continue;
            }
            Event::CData(c) => {
                text.push_str(&c.into_inner());
                continue;
            }
            Event::GeneralRef(r) => {
                let reference = format!("&{};", r.xml10_content());
                let resolved = quick_xml::escape::unescape(&reference).map_err(|e| error(&e))?;
                text.push_str(&resolved);
                continue;
            }
            _ => {}
        }
        if !text.is_empty() {
            let field = elements.last().map_or("", String::as_str);
            output.push_str(&quick_xml::escape::escape(f(field, &text)));
            text.clear();
        }
        match event {
            Event::Start(e) => {
                output.push_str(&rewrite_tag(&e, false, f)?);
                elements.push(local_name(&e));
            }
            Event::Empty(e) => output.push_str(&rewrite_tag(&e, true, f)?),
            Event::End(_) => {
                elements.pop();
                output.push_str(raw);
            }
            Event::Comment(c) => {
                output.push_str("<!--");
                output.push_str(&f("", &c.xml10_content()));
                output.push_str("-->");
            }
            Event::Eof => break,
            _ => output.push_str(raw),
        }
    }
    Ok(output)
}

fn position(p: u64) -> Result<usize, Error> {
    usize::try_from(p).map_err(|_| Error("document too large".into()))
}

fn local_name(e: &BytesStart<'_>) -> String {
    e.local_name().as_ref().to_owned()
}

/// Rewrites `<name attr="…">` applying `f` to the attribute values only.
fn rewrite_tag(
    e: &BytesStart<'_>,
    empty: bool,
    f: &mut dyn FnMut(&str, &str) -> String,
) -> Result<String, Error> {
    let mut tag = format!("<{}", e.name().as_ref());
    for attribute in e.attributes() {
        let attribute = attribute.map_err(|e| Error(format!("invalid XML attribute: {e}")))?;
        let value: Cow<'_, str> = attribute
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .map_err(|e| Error(format!("invalid XML attribute: {e}")))?;
        let field = attribute.key.local_name().as_ref().to_owned();
        tag.push(' ');
        tag.push_str(attribute.key.as_ref());
        tag.push_str("=\"");
        tag.push_str(&quick_xml::escape::escape(f(&field, &value)));
        tag.push('"');
    }
    tag.push_str(if empty { "/>" } else { ">" });
    Ok(tag)
}

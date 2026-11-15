//! `Hosts:1`: host list and mesh topology (docs/PROJECT.md §4.2.2).
//!
//! Hosts are identified by MAC (ADR 0002): [`find`] and [`resolve_ip`] always start
//! from the MAC and read the host list at call time.

use std::collections::{BTreeMap, BTreeSet};
use std::net::Ipv4Addr;

use serde::Deserialize;

use crate::error::FrixError;
use crate::model::{Host, Mac, MeshEnd, MeshLink, MeshNode, MeshTopology};
use crate::services::list_path;
use crate::tr064::{Tr064Client, xml_list_items};

const SERVICE: &str = "Hosts:1";

/// All Hosts known to the router, in list order.
///
/// First `X_AVM-DE_GetHostListPath` and a `GET` of the list (a single call, all
/// fields); if that fails, `GetHostNumberOfEntries` and `GetGenericHostEntry` by index
/// (§8.1), which give only the base fields. If both fail, the error is the
/// list's.
pub async fn list(client: &Tr064Client) -> Result<Vec<Host>, FrixError> {
    match list_from_path(client).await {
        Ok(hosts) => Ok(hosts),
        Err(error) => list_by_index(client).await.map_err(|_| error),
    }
}

async fn list_from_path(client: &Tr064Client) -> Result<Vec<Host>, FrixError> {
    let r = client
        .call(SERVICE, "X_AVM-DE_GetHostListPath", &[])
        .await?;
    let document = list_path::download(client, r.text("NewX_AVM-DE_HostListPath")?).await?;
    xml_list_items(&document, "Item")?
        .iter()
        .map(host)
        .collect()
}

/// The fields of `GetGenericHostEntry`, with the same names as the host list entries.
const GENERIC_HOST_ENTRY_FIELDS: [&str; 6] = [
    "IPAddress",
    "AddressSource",
    "MACAddress",
    "InterfaceType",
    "Active",
    "HostName",
];

async fn list_by_index(client: &Tr064Client) -> Result<Vec<Host>, FrixError> {
    let count: u16 = client
        .call(SERVICE, "GetHostNumberOfEntries", &[])
        .await?
        .parse("NewHostNumberOfEntries")?;
    let mut hosts = Vec::with_capacity(usize::from(count));
    for index in 0..count {
        let r = client
            .call(
                SERVICE,
                "GetGenericHostEntry",
                &[("NewIndex", &index.to_string())],
            )
            .await?;
        let mut entry = BTreeMap::new();
        for field in GENERIC_HOST_ENTRY_FIELDS {
            entry.insert(field.to_owned(), r.text(&format!("New{field}"))?.to_owned());
        }
        hosts.push(host(&entry)?);
    }
    Ok(hosts)
}

/// The mesh topology (`X_AVM-DE_GetMeshListPath`, then a `GET` of the JSON, schema v8.7).
///
/// The real JSON format is to be verified on the router (docs/PROJECT.md §14.3):
/// missing fields default to empty or zero, unknown ones are ignored.
pub async fn mesh(client: &Tr064Client) -> Result<MeshTopology, FrixError> {
    let r = client
        .call(SERVICE, "X_AVM-DE_GetMeshListPath", &[])
        .await?;
    let document = list_path::download(client, r.text("NewX_AVM-DE_MeshListPath")?).await?;
    let raw: MeshJson = serde_json::from_str(&document)
        .map_err(|e| FrixError::Parse(format!("unparsable mesh topology: {e}")))?;
    Ok(topology(&raw))
}

/// The Host with this MAC, read now from the host list.
///
/// Error [`FrixError::NoSuchEntry`] if the router does not know the MAC.
pub async fn find(client: &Tr064Client, mac: Mac) -> Result<Host, FrixError> {
    list(client)
        .await?
        .into_iter()
        .find(|h| h.mac == mac)
        .ok_or_else(|| unknown_mac(mac))
}

/// Resolves MAC → IP at call time, for router actions that work by IP
/// (ADR 0002).
///
/// Errors:
/// - [`FrixError::NoSuchEntry`] if the MAC is not in the host list or has no IPv4;
/// - [`FrixError::Validation`] if the same IP now belongs to another active entry: the IP
///   would point to another Host, so it is not used.
pub async fn resolve_ip(client: &Tr064Client, mac: Mac) -> Result<Ipv4Addr, FrixError> {
    ip_of(&list(client).await?, mac)
}

fn ip_of(hosts: &[Host], mac: Mac) -> Result<Ipv4Addr, FrixError> {
    let host = hosts
        .iter()
        .find(|h| h.mac == mac)
        .ok_or_else(|| unknown_mac(mac))?;
    let ip = host.ip.ok_or_else(|| FrixError::NoSuchEntry {
        description: format!("Host {mac} has no IPv4 address"),
    })?;
    if hosts
        .iter()
        .any(|other| other.mac != mac && other.active && other.ip == Some(ip))
    {
        return Err(FrixError::Validation(format!(
            "IP {ip} of Host {mac} is now assigned to another active Host"
        )));
    }
    Ok(ip)
}

fn unknown_mac(mac: Mac) -> FrixError {
    FrixError::NoSuchEntry {
        description: format!("no Host with MAC {mac} in the host list"),
    }
}

/// An `<Item>` entry of the host list. Missing fields: empty text, false flags, no numbers.
fn host(entry: &BTreeMap<String, String>) -> Result<Host, FrixError> {
    let text = |name: &str| entry.get(name).map_or("", |t| t.trim()).to_owned();
    let flag = |name: &str| entry.get(name).is_some_and(|t| t.trim() == "1");
    let number = |name: &str| entry.get(name).and_then(|t| t.trim().parse().ok());
    let mac = entry
        .get("MACAddress")
        .ok_or_else(|| FrixError::Parse("host list entry without MACAddress".into()))?
        .trim()
        .parse()
        .map_err(|_| FrixError::Parse("invalid MACAddress in the host list".into()))?;
    let filter_profile_id = Some(text("X_AVM-DE_FilterProfileID")).filter(|t| !t.is_empty());
    Ok(Host {
        mac,
        ip: entry.get("IPAddress").and_then(|t| t.trim().parse().ok()),
        name: text("HostName"),
        active: flag("Active"),
        interface_type: text("InterfaceType"),
        address_source: text("AddressSource"),
        port: number("X_AVM-DE_Port"),
        speed_mbps: number("X_AVM-DE_Speed"),
        guest: flag("X_AVM-DE_Guest"),
        vpn: flag("X_AVM-DE_VPN"),
        wan_access: text("X_AVM-DE_WANAccess"),
        wan_access_disallowed: flag("X_AVM-DE_Disallow"),
        priority: flag("X_AVM-DE_Priority"),
        filter_profile_id,
        meshable: flag("X_AVM-DE_IsMeshable"),
    })
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct MeshJson {
    nodes: Vec<NodeJson>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct NodeJson {
    uid: String,
    device_name: String,
    device_model: String,
    device_mac_address: String,
    mesh_role: String,
    is_meshed: bool,
    node_interfaces: Vec<InterfaceJson>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct InterfaceJson {
    uid: String,
    name: String,
    mac_address: String,
    node_links: Vec<LinkJson>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LinkJson {
    uid: String,
    #[serde(rename = "type")]
    link_type: String,
    state: String,
    node_1_uid: String,
    node_2_uid: String,
    node_interface_1_uid: String,
    node_interface_2_uid: String,
    max_data_rate_rx: u64,
    max_data_rate_tx: u64,
    cur_data_rate_rx: u64,
    cur_data_rate_tx: u64,
}

fn topology(raw: &MeshJson) -> MeshTopology {
    let interfaces: BTreeMap<&str, &str> = raw
        .nodes
        .iter()
        .flat_map(|n| &n.node_interfaces)
        .map(|i| (i.uid.as_str(), i.name.as_str()))
        .collect();
    let interface_name = |uid: &str| interfaces.get(uid).copied().unwrap_or("").to_owned();
    let mut seen = BTreeSet::new();
    let mut links = Vec::new();
    for link in raw
        .nodes
        .iter()
        .flat_map(|n| &n.node_interfaces)
        .flat_map(|i| &i.node_links)
    {
        if !seen.insert(link.uid.as_str()) {
            continue;
        }
        links.push(MeshLink {
            uid: link.uid.clone(),
            link_type: link.link_type.clone(),
            state: link.state.clone(),
            node_1: MeshEnd {
                node_uid: link.node_1_uid.clone(),
                interface: interface_name(&link.node_interface_1_uid),
            },
            node_2: MeshEnd {
                node_uid: link.node_2_uid.clone(),
                interface: interface_name(&link.node_interface_2_uid),
            },
            max_rate_rx_kbps: link.max_data_rate_rx,
            max_rate_tx_kbps: link.max_data_rate_tx,
            cur_rate_rx_kbps: link.cur_data_rate_rx,
            cur_rate_tx_kbps: link.cur_data_rate_tx,
        });
    }
    let nodes = raw
        .nodes
        .iter()
        .map(|n| MeshNode {
            uid: n.uid.clone(),
            name: n.device_name.clone(),
            model: n.device_model.clone(),
            mac: n.device_mac_address.trim().parse().ok(),
            role: n.mesh_role.clone(),
            meshed: n.is_meshed,
            interface_macs: n
                .node_interfaces
                .iter()
                .filter_map(|i| i.mac_address.trim().parse().ok())
                .collect(),
        })
        .collect();
    MeshTopology { nodes, links }
}

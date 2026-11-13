//! `Hosts:1`: lista host e topologia mesh (docs/PROJECT.md §4.2.2).
//!
//! Gli Host si identificano per MAC (ADR 0002): [`find`] e [`resolve_ip`] partono
//! sempre dal MAC e leggono la lista host al momento della chiamata.

use std::collections::{BTreeMap, BTreeSet};
use std::net::Ipv4Addr;

use serde::Deserialize;

use crate::error::FrixError;
use crate::model::{Host, Mac, MeshEnd, MeshLink, MeshNode, MeshTopology};
use crate::services::list_path;
use crate::tr064::{Tr064Client, xml_list_items};

const SERVICE: &str = "Hosts:1";

/// Tutti gli Host noti al router, nell'ordine della lista.
///
/// Prima `X_AVM-DE_GetHostListPath` e `GET` della lista (una sola chiamata, tutti i
/// campi); se fallisce, `GetHostNumberOfEntries` e `GetGenericHostEntry` per indice
/// (§8.1), che danno solo i campi base. Se falliscono entrambe, l'errore è quello
/// della lista.
pub async fn list(client: &Tr064Client) -> Result<Vec<Host>, FrixError> {
    match list_da_path(client).await {
        Ok(hosts) => Ok(hosts),
        Err(errore) => list_per_indice(client).await.map_err(|_| errore),
    }
}

async fn list_da_path(client: &Tr064Client) -> Result<Vec<Host>, FrixError> {
    let r = client
        .call(SERVICE, "X_AVM-DE_GetHostListPath", &[])
        .await?;
    let documento = list_path::download(client, r.text("NewX_AVM-DE_HostListPath")?).await?;
    xml_list_items(&documento, "Item")?
        .iter()
        .map(host)
        .collect()
}

/// I campi di `GetGenericHostEntry`, con gli stessi nomi delle voci della lista host.
const CAMPI_GENERIC_HOST_ENTRY: [&str; 6] = [
    "IPAddress",
    "AddressSource",
    "MACAddress",
    "InterfaceType",
    "Active",
    "HostName",
];

async fn list_per_indice(client: &Tr064Client) -> Result<Vec<Host>, FrixError> {
    let numero: u16 = client
        .call(SERVICE, "GetHostNumberOfEntries", &[])
        .await?
        .parse("NewHostNumberOfEntries")?;
    let mut hosts = Vec::with_capacity(usize::from(numero));
    for indice in 0..numero {
        let r = client
            .call(
                SERVICE,
                "GetGenericHostEntry",
                &[("NewIndex", &indice.to_string())],
            )
            .await?;
        let mut voce = BTreeMap::new();
        for campo in CAMPI_GENERIC_HOST_ENTRY {
            voce.insert(campo.to_owned(), r.text(&format!("New{campo}"))?.to_owned());
        }
        hosts.push(host(&voce)?);
    }
    Ok(hosts)
}

/// La topologia mesh (`X_AVM-DE_GetMeshListPath`, poi `GET` del JSON, schema v8.7).
///
/// Il formato reale del JSON è da verificare sul router (docs/PROJECT.md §14.3): i
/// campi assenti valgono vuoto o zero, quelli sconosciuti si ignorano.
pub async fn mesh(client: &Tr064Client) -> Result<MeshTopology, FrixError> {
    let r = client
        .call(SERVICE, "X_AVM-DE_GetMeshListPath", &[])
        .await?;
    let documento = list_path::download(client, r.text("NewX_AVM-DE_MeshListPath")?).await?;
    let grezzo: MeshJson = serde_json::from_str(&documento)
        .map_err(|e| FrixError::Parse(format!("topologia mesh non interpretabile: {e}")))?;
    Ok(topologia(&grezzo))
}

/// L'Host con questo MAC, letto ora dalla lista host.
///
/// Errore [`FrixError::NoSuchEntry`] se il router non conosce il MAC.
pub async fn find(client: &Tr064Client, mac: Mac) -> Result<Host, FrixError> {
    list(client)
        .await?
        .into_iter()
        .find(|h| h.mac == mac)
        .ok_or_else(|| sconosciuto(mac))
}

/// Risolve MAC → IP al momento della chiamata, per le azioni del router che lavorano
/// per IP (ADR 0002).
///
/// Errori:
/// - [`FrixError::NoSuchEntry`] se il MAC non è nella lista host o non ha un IPv4;
/// - [`FrixError::Validation`] se lo stesso IP è ora di un'altra voce attiva: l'IP
///   indicherebbe un altro Host, quindi non si usa.
pub async fn resolve_ip(client: &Tr064Client, mac: Mac) -> Result<Ipv4Addr, FrixError> {
    ip_di(&list(client).await?, mac)
}

fn ip_di(hosts: &[Host], mac: Mac) -> Result<Ipv4Addr, FrixError> {
    let host = hosts
        .iter()
        .find(|h| h.mac == mac)
        .ok_or_else(|| sconosciuto(mac))?;
    let ip = host.ip.ok_or_else(|| FrixError::NoSuchEntry {
        description: format!("l'Host {mac} non ha un indirizzo IPv4"),
    })?;
    if hosts
        .iter()
        .any(|altro| altro.mac != mac && altro.active && altro.ip == Some(ip))
    {
        return Err(FrixError::Validation(format!(
            "l'IP {ip} dell'Host {mac} è ora assegnato a un altro Host attivo"
        )));
    }
    Ok(ip)
}

fn sconosciuto(mac: Mac) -> FrixError {
    FrixError::NoSuchEntry {
        description: format!("nessun Host con MAC {mac} nella lista host"),
    }
}

/// Una voce `<Item>` della lista host. Campi assenti: testo vuoto, flag falsi, numeri assenti.
fn host(voce: &BTreeMap<String, String>) -> Result<Host, FrixError> {
    let testo = |nome: &str| voce.get(nome).map_or("", |t| t.trim()).to_owned();
    let flag = |nome: &str| voce.get(nome).is_some_and(|t| t.trim() == "1");
    let numero = |nome: &str| voce.get(nome).and_then(|t| t.trim().parse().ok());
    let mac = voce
        .get("MACAddress")
        .ok_or_else(|| FrixError::Parse("voce della lista host senza MACAddress".into()))?
        .trim()
        .parse()
        .map_err(|_| FrixError::Parse("MACAddress non valido nella lista host".into()))?;
    let filter_profile_id = Some(testo("X_AVM-DE_FilterProfileID")).filter(|t| !t.is_empty());
    Ok(Host {
        mac,
        ip: voce.get("IPAddress").and_then(|t| t.trim().parse().ok()),
        name: testo("HostName"),
        active: flag("Active"),
        interface_type: testo("InterfaceType"),
        address_source: testo("AddressSource"),
        port: numero("X_AVM-DE_Port"),
        speed_mbps: numero("X_AVM-DE_Speed"),
        guest: flag("X_AVM-DE_Guest"),
        vpn: flag("X_AVM-DE_VPN"),
        wan_access: testo("X_AVM-DE_WANAccess"),
        wan_access_disallowed: flag("X_AVM-DE_Disallow"),
        priority: flag("X_AVM-DE_Priority"),
        filter_profile_id,
        meshable: flag("X_AVM-DE_IsMeshable"),
    })
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct MeshJson {
    nodes: Vec<NodoJson>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct NodoJson {
    uid: String,
    device_name: String,
    device_model: String,
    device_mac_address: String,
    mesh_role: String,
    is_meshed: bool,
    node_interfaces: Vec<InterfacciaJson>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct InterfacciaJson {
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

fn topologia(grezzo: &MeshJson) -> MeshTopology {
    let interfacce: BTreeMap<&str, &str> = grezzo
        .nodes
        .iter()
        .flat_map(|n| &n.node_interfaces)
        .map(|i| (i.uid.as_str(), i.name.as_str()))
        .collect();
    let nome_interfaccia = |uid: &str| interfacce.get(uid).copied().unwrap_or("").to_owned();
    let mut visti = BTreeSet::new();
    let mut links = Vec::new();
    for link in grezzo
        .nodes
        .iter()
        .flat_map(|n| &n.node_interfaces)
        .flat_map(|i| &i.node_links)
    {
        if !visti.insert(link.uid.as_str()) {
            continue;
        }
        links.push(MeshLink {
            uid: link.uid.clone(),
            link_type: link.link_type.clone(),
            state: link.state.clone(),
            node_1: MeshEnd {
                node_uid: link.node_1_uid.clone(),
                interface: nome_interfaccia(&link.node_interface_1_uid),
            },
            node_2: MeshEnd {
                node_uid: link.node_2_uid.clone(),
                interface: nome_interfaccia(&link.node_interface_2_uid),
            },
            max_rate_rx_kbps: link.max_data_rate_rx,
            max_rate_tx_kbps: link.max_data_rate_tx,
            cur_rate_rx_kbps: link.cur_data_rate_rx,
            cur_rate_tx_kbps: link.cur_data_rate_tx,
        });
    }
    let nodes = grezzo
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

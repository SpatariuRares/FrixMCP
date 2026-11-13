//! `list_hosts`, `get_host`, `get_mesh_topology` (docs/PROJECT.md §8.1; ADR 0002).

use frix_core::FrixError;
use frix_core::model::{Host, Mac, MeshEnd, MeshLink, MeshTopology};
use frix_core::services::hosts;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};

use crate::response;
use crate::server::FrixServer;

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct ListHostsParams {
    /// Solo gli Host attivi adesso (default `false`).
    #[serde(default)]
    active_only: bool,
    /// Aggiunge a ogni Host il nodo mesh e i link con cui è collegato (default `false`).
    #[serde(default)]
    include_mesh: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct GetHostParams {
    /// MAC dell'Host, per esempio `AA:BB:CC:DD:EE:FF`. Gli Host si indicano solo per MAC, mai per IP.
    mac: String,
}

#[derive(Debug, Serialize)]
struct HostList {
    hosts: Vec<HostOut>,
}

// perché: i flag rispecchiano uno a uno quelli della lista host del router.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Serialize)]
struct HostOut {
    mac: String,
    ip: Option<String>,
    name: String,
    active: bool,
    interface_type: String,
    address_source: String,
    port: Option<u32>,
    speed_mbps: Option<u32>,
    guest: bool,
    vpn: bool,
    wan_access: String,
    wan_access_disallowed: bool,
    priority: bool,
    filter_profile_id: Option<String>,
    meshable: bool,
    /// Solo con `include_mesh`.
    #[serde(flatten)]
    mesh: Option<CampoMesh>,
}

#[derive(Debug, Serialize)]
struct CampoMesh {
    /// `null` se l'Host non compare nella mesh.
    mesh: Option<HostMesh>,
}

#[derive(Debug, Serialize)]
struct HostMesh {
    node_uid: String,
    role: String,
    links: Vec<HostMeshLink>,
}

#[derive(Debug, Serialize)]
struct HostMeshLink {
    peer: String,
    peer_mac: Option<String>,
    #[serde(rename = "type")]
    link_type: String,
    state: String,
    band: Option<&'static str>,
}

#[derive(Debug, Serialize)]
struct Mesh {
    nodes: Vec<MeshNodeOut>,
    links: Vec<MeshLinkOut>,
}

#[derive(Debug, Serialize)]
struct MeshNodeOut {
    uid: String,
    name: String,
    model: String,
    mac: Option<String>,
    role: String,
    meshed: bool,
}

#[derive(Debug, Serialize)]
struct MeshLinkOut {
    uid: String,
    #[serde(rename = "type")]
    link_type: String,
    state: String,
    node_1: MeshEndOut,
    node_2: MeshEndOut,
    band: Option<&'static str>,
    max_rate_rx_kbps: u64,
    max_rate_tx_kbps: u64,
    cur_rate_rx_kbps: u64,
    cur_rate_tx_kbps: u64,
}

#[derive(Debug, Serialize)]
struct MeshEndOut {
    uid: String,
    name: String,
    interface: String,
}

impl From<Host> for HostOut {
    fn from(h: Host) -> Self {
        Self {
            mac: h.mac.to_string(),
            ip: h.ip.map(|ip| ip.to_string()),
            name: h.name,
            active: h.active,
            interface_type: h.interface_type,
            address_source: h.address_source,
            port: h.port,
            speed_mbps: h.speed_mbps,
            guest: h.guest,
            vpn: h.vpn,
            wan_access: h.wan_access,
            wan_access_disallowed: h.wan_access_disallowed,
            priority: h.priority,
            filter_profile_id: h.filter_profile_id,
            meshable: h.meshable,
            mesh: None,
        }
    }
}

/// Nodo e link mesh dell'Host con questo MAC, se compare nella mesh.
fn host_mesh(mesh: &MeshTopology, mac: Mac) -> Option<HostMesh> {
    let nodo = mesh.node_of(mac)?;
    let links = mesh
        .links_of(&nodo.uid)
        .map(|(link, altro)| {
            let peer = mesh.node(&altro.node_uid);
            HostMeshLink {
                peer: peer.map(|n| n.name.clone()).unwrap_or_default(),
                peer_mac: peer.and_then(|n| n.mac).map(|m| m.to_string()),
                link_type: link.link_type.clone(),
                state: link.state.clone(),
                band: link.band(),
            }
        })
        .collect();
    Some(HostMesh {
        node_uid: nodo.uid.clone(),
        role: nodo.role.clone(),
        links,
    })
}

fn mesh_end(mesh: &MeshTopology, capo: &MeshEnd) -> MeshEndOut {
    MeshEndOut {
        uid: capo.node_uid.clone(),
        name: mesh
            .node(&capo.node_uid)
            .map(|n| n.name.clone())
            .unwrap_or_default(),
        interface: capo.interface.clone(),
    }
}

fn mesh_link(mesh: &MeshTopology, link: &MeshLink) -> MeshLinkOut {
    MeshLinkOut {
        uid: link.uid.clone(),
        link_type: link.link_type.clone(),
        state: link.state.clone(),
        node_1: mesh_end(mesh, &link.node_1),
        node_2: mesh_end(mesh, &link.node_2),
        band: link.band(),
        max_rate_rx_kbps: link.max_rate_rx_kbps,
        max_rate_tx_kbps: link.max_rate_tx_kbps,
        cur_rate_rx_kbps: link.cur_rate_rx_kbps,
        cur_rate_tx_kbps: link.cur_rate_tx_kbps,
    }
}

#[tool_router(router = hosts_router, vis = "pub(crate)")]
impl FrixServer {
    #[tool(
        name = "list_hosts",
        description = concat!(
            "Host noti al FRITZ!Box, identificati per MAC: nome, IP, attivo, interfaccia, porta e velocità, ospite, accesso a Internet, profilo filtro, priorità. ",
            dati_della_rete!()
        ),
        annotations(
            title = "Elenco degli Host",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_hosts(
        &self,
        Parameters(params): Parameters<ListHostsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(response::from_result(self.host_list(&params).await))
    }

    #[tool(
        name = "get_host",
        description = concat!(
            "Dettaglio di un Host indicato per MAC (mai per IP: un IP può passare a un altro Host). ",
            dati_della_rete!()
        ),
        annotations(
            title = "Dettaglio di un Host",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_host(
        &self,
        Parameters(params): Parameters<GetHostParams>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(response::from_result(self.host(&params.mac).await))
    }

    #[tool(
        name = "get_mesh_topology",
        description = concat!(
            "Nodi (router, repeater, Host) e link della mesh FRITZ!: tipo, stato, Band e velocità in kbit/s. ",
            dati_della_rete!()
        ),
        annotations(
            title = "Topologia mesh",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_mesh_topology(&self) -> Result<CallToolResult, ErrorData> {
        Ok(response::from_result(self.mesh_topology().await))
    }
}

impl FrixServer {
    async fn host(&self, mac: &str) -> Result<HostOut, FrixError> {
        let mac: Mac = mac.parse()?;
        Ok(hosts::find(&self.client, mac).await?.into())
    }

    async fn host_list(&self, params: &ListHostsParams) -> Result<HostList, FrixError> {
        let elenco = hosts::list(&self.client).await?;
        let mesh = if params.include_mesh {
            Some(hosts::mesh(&self.client).await?)
        } else {
            None
        };
        Ok(HostList {
            hosts: elenco
                .into_iter()
                .filter(|h| h.active || !params.active_only)
                .map(|h| {
                    let nella_mesh = mesh.as_ref().map(|m| CampoMesh {
                        mesh: host_mesh(m, h.mac),
                    });
                    HostOut {
                        mesh: nella_mesh,
                        ..HostOut::from(h)
                    }
                })
                .collect(),
        })
    }

    async fn mesh_topology(&self) -> Result<Mesh, FrixError> {
        let mesh = hosts::mesh(&self.client).await?;
        Ok(Mesh {
            nodes: mesh
                .nodes
                .iter()
                .map(|n| MeshNodeOut {
                    uid: n.uid.clone(),
                    name: n.name.clone(),
                    model: n.model.clone(),
                    mac: n.mac.map(|m| m.to_string()),
                    role: n.role.clone(),
                    meshed: n.meshed,
                })
                .collect(),
            links: mesh.links.iter().map(|l| mesh_link(&mesh, l)).collect(),
        })
    }
}

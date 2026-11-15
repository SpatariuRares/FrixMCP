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
    /// Only the Hosts active right now (default `false`).
    #[serde(default)]
    active_only: bool,
    /// Adds to each Host the mesh node and the links it is connected through (default `false`).
    #[serde(default)]
    include_mesh: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct GetHostParams {
    /// MAC of the Host, for example `AA:BB:CC:DD:EE:FF`. Hosts are identified only by MAC, never by IP.
    mac: String,
}

#[derive(Debug, Serialize)]
struct HostList {
    hosts: Vec<HostOut>,
}

// why: the flags mirror one to one those of the router's host list.
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
    /// Only with `include_mesh`.
    #[serde(flatten)]
    mesh: Option<MeshField>,
}

#[derive(Debug, Serialize)]
struct MeshField {
    /// `null` if the Host does not appear in the mesh.
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

/// Mesh node and links of the Host with this MAC, if it appears in the mesh.
fn host_mesh(mesh: &MeshTopology, mac: Mac) -> Option<HostMesh> {
    let node = mesh.node_of(mac)?;
    let links = mesh
        .links_of(&node.uid)
        .map(|(link, other)| {
            let peer = mesh.node(&other.node_uid);
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
        node_uid: node.uid.clone(),
        role: node.role.clone(),
        links,
    })
}

fn mesh_end(mesh: &MeshTopology, end: &MeshEnd) -> MeshEndOut {
    MeshEndOut {
        uid: end.node_uid.clone(),
        name: mesh
            .node(&end.node_uid)
            .map(|n| n.name.clone())
            .unwrap_or_default(),
        interface: end.interface.clone(),
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
            "Hosts known to the FRITZ!Box, identified by MAC: name, IP, active, interface, port and speed, guest, Internet access, filter profile, priority. ",
            network_data!()
        ),
        annotations(
            title = "Host list",
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
            "Details of one Host identified by MAC (never by IP: an IP can move to another Host). ",
            network_data!()
        ),
        annotations(
            title = "Host details",
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
            "Nodes (router, repeaters, Hosts) and links of the FRITZ! mesh: type, state, Band and speed in kbit/s. ",
            network_data!()
        ),
        annotations(
            title = "Mesh topology",
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
        let all_hosts = hosts::list(&self.client).await?;
        let mesh = if params.include_mesh {
            Some(hosts::mesh(&self.client).await?)
        } else {
            None
        };
        Ok(HostList {
            hosts: all_hosts
                .into_iter()
                .filter(|h| h.active || !params.active_only)
                .map(|h| {
                    let in_mesh = mesh.as_ref().map(|m| MeshField {
                        mesh: host_mesh(m, h.mac),
                    });
                    HostOut {
                        mesh: in_mesh,
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

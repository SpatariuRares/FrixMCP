//! Mesh topology (`X_AVM-DE_GetMeshListPath`, JSON schema v8.7).

use super::Mac;

/// Mesh nodes and links. Texts are untrusted data.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MeshTopology {
    /// Nodes in list order: router, repeaters and connected Hosts.
    pub nodes: Vec<MeshNode>,
    /// Links, once each (in the JSON they appear under both interfaces).
    pub links: Vec<MeshLink>,
}

/// A mesh node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeshNode {
    /// `uid`.
    pub uid: String,
    /// `device_name`.
    pub name: String,
    /// `device_model`.
    pub model: String,
    /// `device_mac_address`, if valid.
    pub mac: Option<Mac>,
    /// `mesh_role` (`master`, `slave`, `unknown`).
    pub role: String,
    /// `is_meshed`.
    pub meshed: bool,
    /// MACs of all the node's interfaces (`node_interfaces[].mac_address`).
    pub interface_macs: Vec<Mac>,
}

/// One end of a link: node and interface name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeshEnd {
    /// `node_N_uid`.
    pub node_uid: String,
    /// Interface name (`AP:5G:0`, `LAN:1`, …); empty if unknown.
    pub interface: String,
}

/// A link between two nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeshLink {
    /// `uid`.
    pub uid: String,
    /// `type` (`WLAN`, `LAN`, …).
    pub link_type: String,
    /// `state` (`CONNECTED`, `DISCONNECTED`).
    pub state: String,
    /// End `node_1`.
    pub node_1: MeshEnd,
    /// End `node_2`.
    pub node_2: MeshEnd,
    /// `max_data_rate_rx`, in kbit/s.
    pub max_rate_rx_kbps: u64,
    /// `max_data_rate_tx`, in kbit/s.
    pub max_rate_tx_kbps: u64,
    /// `cur_data_rate_rx`, in kbit/s.
    pub cur_rate_rx_kbps: u64,
    /// `cur_data_rate_tx`, in kbit/s.
    pub cur_rate_tx_kbps: u64,
}

impl MeshLink {
    /// The link's Band (`2.4 GHz`, `5 GHz`, `6 GHz`), derived from the interface
    /// names (`AP:2G:0`, `UPLINK:5G:0`); `None` for wired links or if unknown.
    #[must_use]
    pub fn band(&self) -> Option<&'static str> {
        [&self.node_1.interface, &self.node_2.interface]
            .into_iter()
            .flat_map(|name| name.split(':'))
            .find_map(|part| match part {
                "2G" => Some("2.4 GHz"),
                "5G" => Some("5 GHz"),
                "6G" => Some("6 GHz"),
                _ => None,
            })
    }
}

impl MeshTopology {
    /// The node representing the Host with this MAC (the device's MAC or one of
    /// its interfaces).
    #[must_use]
    pub fn node_of(&self, mac: Mac) -> Option<&MeshNode> {
        self.nodes
            .iter()
            .find(|n| n.mac == Some(mac))
            .or_else(|| self.nodes.iter().find(|n| n.interface_macs.contains(&mac)))
    }

    /// The node with this `uid`.
    #[must_use]
    pub fn node(&self, uid: &str) -> Option<&MeshNode> {
        self.nodes.iter().find(|n| n.uid == uid)
    }

    /// The links of a node, each with the opposite end.
    pub fn links_of<'a>(
        &'a self,
        uid: &'a str,
    ) -> impl Iterator<Item = (&'a MeshLink, &'a MeshEnd)> + 'a {
        self.links.iter().filter_map(move |l| {
            if l.node_1.node_uid == uid {
                Some((l, &l.node_2))
            } else if l.node_2.node_uid == uid {
                Some((l, &l.node_1))
            } else {
                None
            }
        })
    }
}

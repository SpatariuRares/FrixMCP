//! Topologia mesh (`X_AVM-DE_GetMeshListPath`, schema JSON v8.7).

use super::Mac;

/// Nodi e link della mesh. I testi sono dati non fidati.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MeshTopology {
    /// Nodi in ordine di lista: router, repeater e Host collegati.
    pub nodes: Vec<MeshNode>,
    /// Link, una volta sola ciascuno (nel JSON compaiono sotto entrambe le interfacce).
    pub links: Vec<MeshLink>,
}

/// Un nodo della mesh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeshNode {
    /// `uid`.
    pub uid: String,
    /// `device_name`.
    pub name: String,
    /// `device_model`.
    pub model: String,
    /// `device_mac_address`, se valido.
    pub mac: Option<Mac>,
    /// `mesh_role` (`master`, `slave`, `unknown`).
    pub role: String,
    /// `is_meshed`.
    pub meshed: bool,
    /// MAC di tutte le interfacce del nodo (`node_interfaces[].mac_address`).
    pub interface_macs: Vec<Mac>,
}

/// Un capo di un link: nodo e nome dell'interfaccia.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeshEnd {
    /// `node_N_uid`.
    pub node_uid: String,
    /// Nome dell'interfaccia (`AP:5G:0`, `LAN:1`, …); vuoto se sconosciuto.
    pub interface: String,
}

/// Un link fra due nodi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeshLink {
    /// `uid`.
    pub uid: String,
    /// `type` (`WLAN`, `LAN`, …).
    pub link_type: String,
    /// `state` (`CONNECTED`, `DISCONNECTED`).
    pub state: String,
    /// Capo `node_1`.
    pub node_1: MeshEnd,
    /// Capo `node_2`.
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
    /// La Band del link (`2,4 GHz`, `5 GHz`, `6 GHz`), ricavata dai nomi delle
    /// interfacce (`AP:2G:0`, `UPLINK:5G:0`); `None` per i link cablati o se ignota.
    #[must_use]
    pub fn band(&self) -> Option<&'static str> {
        [&self.node_1.interface, &self.node_2.interface]
            .into_iter()
            .flat_map(|nome| nome.split(':'))
            .find_map(|parte| match parte {
                "2G" => Some("2,4 GHz"),
                "5G" => Some("5 GHz"),
                "6G" => Some("6 GHz"),
                _ => None,
            })
    }
}

impl MeshTopology {
    /// Il nodo che rappresenta l'Host con questo MAC (MAC del dispositivo o di una
    /// sua interfaccia).
    #[must_use]
    pub fn node_of(&self, mac: Mac) -> Option<&MeshNode> {
        self.nodes
            .iter()
            .find(|n| n.mac == Some(mac))
            .or_else(|| self.nodes.iter().find(|n| n.interface_macs.contains(&mac)))
    }

    /// Il nodo con questo `uid`.
    #[must_use]
    pub fn node(&self, uid: &str) -> Option<&MeshNode> {
        self.nodes.iter().find(|n| n.uid == uid)
    }

    /// I link di un nodo, ciascuno con il capo opposto.
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

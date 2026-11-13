//! `LANHostConfigManagement:1`: indirizzamento della LAN (docs/PROJECT.md §4.2.16).

use crate::error::FrixError;
use crate::tr064::Tr064Client;

const SERVICE: &str = "LANHostConfigManagement:1";

/// Configurazione della LAN: DHCP, DNS, maschera, dominio, router.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanConfig {
    /// `NewDHCPServerEnable`.
    pub dhcp_server_enabled: bool,
    /// `NewMinAddress`: primo indirizzo assegnato dal DHCP.
    pub dhcp_min_address: String,
    /// `NewMaxAddress`: ultimo indirizzo assegnato dal DHCP.
    pub dhcp_max_address: String,
    /// `NewDNSServers`, separati da virgola nella risposta.
    pub dns_servers: Vec<String>,
    /// `NewSubnetMask`.
    pub subnet_mask: String,
    /// `NewDomainName`.
    pub domain_name: String,
    /// `NewIPRouters`, separati da virgola nella risposta.
    pub routers: Vec<String>,
}

/// `GetInfo`.
pub async fn lan_config(client: &Tr064Client) -> Result<LanConfig, FrixError> {
    let r = client.call(SERVICE, "GetInfo", &[]).await?;
    Ok(LanConfig {
        dhcp_server_enabled: r.flag("NewDHCPServerEnable")?,
        dhcp_min_address: r.text("NewMinAddress")?.to_owned(),
        dhcp_max_address: r.text("NewMaxAddress")?.to_owned(),
        dns_servers: lista(r.text("NewDNSServers")?),
        subnet_mask: r.text("NewSubnetMask")?.to_owned(),
        domain_name: r.text("NewDomainName")?.to_owned(),
        routers: lista(r.text("NewIPRouters")?),
    })
}

/// Lista separata da virgole, senza voci vuote.
fn lista(testo: &str) -> Vec<String> {
    testo
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

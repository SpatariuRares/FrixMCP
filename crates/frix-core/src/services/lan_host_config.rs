//! `LANHostConfigManagement:1`: LAN addressing (docs/PROJECT.md §4.2.16).

use crate::error::FrixError;
use crate::tr064::Tr064Client;

const SERVICE: &str = "LANHostConfigManagement:1";

/// LAN configuration: DHCP, DNS, mask, domain, routers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanConfig {
    /// `NewDHCPServerEnable`.
    pub dhcp_server_enabled: bool,
    /// `NewMinAddress`: first address assigned by DHCP.
    pub dhcp_min_address: String,
    /// `NewMaxAddress`: last address assigned by DHCP.
    pub dhcp_max_address: String,
    /// `NewDNSServers`, comma-separated in the response.
    pub dns_servers: Vec<String>,
    /// `NewSubnetMask`.
    pub subnet_mask: String,
    /// `NewDomainName`.
    pub domain_name: String,
    /// `NewIPRouters`, comma-separated in the response.
    pub routers: Vec<String>,
}

/// `GetInfo`.
pub async fn lan_config(client: &Tr064Client) -> Result<LanConfig, FrixError> {
    let r = client.call(SERVICE, "GetInfo", &[]).await?;
    Ok(LanConfig {
        dhcp_server_enabled: r.flag("NewDHCPServerEnable")?,
        dhcp_min_address: r.text("NewMinAddress")?.to_owned(),
        dhcp_max_address: r.text("NewMaxAddress")?.to_owned(),
        dns_servers: comma_list(r.text("NewDNSServers")?),
        subnet_mask: r.text("NewSubnetMask")?.to_owned(),
        domain_name: r.text("NewDomainName")?.to_owned(),
        routers: comma_list(r.text("NewIPRouters")?),
    })
}

/// Comma-separated list, without empty entries.
fn comma_list(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

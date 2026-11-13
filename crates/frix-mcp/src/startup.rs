//! Avvio sul router vero (docs/PROJECT.md §6.3): trasporto reale, discovery in chiaro,
//! porta HTTPS da `GetSecurityPort`, poi Router Account e diritti con
//! `LANConfigSecurity:X_AVM-DE_GetCurrentUser`, da scrivere su stderr.

use std::fmt::Write as _;
use std::sync::Arc;

use frix_core::FrixError;
use frix_core::services::lan_config_security::CurrentUser;
use frix_core::services::{device_info, lan_config_security};
use frix_core::tr064::Tr064Client;
use frix_core::transport::{Credentials, RouterSettings, RouterTransport};

use crate::FrixServer;

/// Server pronto a servire MCP, con il Router Account con cui è entrato.
#[derive(Debug)]
pub struct Started {
    /// Il server MCP sopra il trasporto reale.
    pub server: FrixServer,
    /// Router Account e diritti letti all'avvio.
    pub user: CurrentUser,
}

/// Collega `frix-mcp` al router: nessun tool parte finché questo non riesce.
pub async fn connect(
    settings: RouterSettings,
    credentials: Credentials,
) -> Result<Started, FrixError> {
    let transport = Arc::new(RouterTransport::new(settings, credentials)?);
    let client = Tr064Client::discover(Arc::clone(&transport) as _).await?;
    transport.set_secure_port(device_info::security_port(&client).await?);
    let user = lan_config_security::current_user(&client).await?;
    Ok(Started {
        server: FrixServer::new(client),
        user,
    })
}

/// Riga per stderr: con quale Router Account e con quali diritti `frix-mcp` è entrato.
#[must_use]
pub fn banner(user: &CurrentUser) -> String {
    let mut riga = format!("frix-mcp: Router Account: {}; diritti:", user.username);
    if user.rights.is_empty() {
        riga.push_str(" nessuno");
    }
    for diritto in &user.rights {
        let _ = write!(riga, " {}={}", diritto.area, diritto.access);
    }
    riga
}

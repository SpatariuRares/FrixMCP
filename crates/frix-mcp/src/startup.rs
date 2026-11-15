//! Startup on the real router (docs/PROJECT.md §6.3): real transport, plaintext discovery,
//! HTTPS port from `GetSecurityPort`, then Router Account and rights with
//! `LANConfigSecurity:X_AVM-DE_GetCurrentUser`, to be written on stderr.

use std::fmt::Write as _;
use std::sync::Arc;

use frix_core::FrixError;
use frix_core::language::Language;
use frix_core::services::lan_config_security::CurrentUser;
use frix_core::services::{device_info, lan_config_security};
use frix_core::tr064::Tr064Client;
use frix_core::transport::{Credentials, RouterSettings, RouterTransport};

use crate::FrixServer;

/// Server ready to serve MCP, with the Router Account it logged in with.
#[derive(Debug)]
pub struct Started {
    /// The MCP server on top of the real transport.
    pub server: FrixServer,
    /// Router Account and rights read at startup.
    pub user: CurrentUser,
}

/// Connects `frix-mcp` to the router: no tool runs until this succeeds.
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

/// Line for stderr, in `language`: which Router Account and which rights `frix-mcp`
/// logged in with. Right names and access levels are the router's, not translated.
#[must_use]
pub fn banner(user: &CurrentUser, language: Language) -> String {
    let (rights, none) = match language {
        Language::En => ("rights", "none"),
        Language::It => ("diritti", "nessuno"),
    };
    let mut line = format!("frix-mcp: Router Account: {}; {rights}:", user.username);
    if user.rights.is_empty() {
        let _ = write!(line, " {none}");
    }
    for right in &user.rights {
        let _ = write!(line, " {}={}", right.area, right.access);
    }
    line
}

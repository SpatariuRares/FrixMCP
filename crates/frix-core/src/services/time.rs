//! `Time:1`: ora del router e server NTP (docs/PROJECT.md §4.2.16).

use crate::error::FrixError;
use crate::tr064::Tr064Client;

const SERVICE: &str = "Time:1";

/// Ora locale del router e sua sorgente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeInfo {
    /// `NewCurrentLocalTime`, così come lo riporta il router (ISO 8601).
    pub local_time: String,
    /// `NewLocalTimeZoneName`.
    pub time_zone: String,
    /// `NewDaylightSavingsUsed`.
    pub daylight_saving: bool,
    /// `NewNTPServer1` e `NewNTPServer2`, senza quelli vuoti.
    pub ntp_servers: Vec<String>,
}

/// `GetInfo`.
pub async fn info(client: &Tr064Client) -> Result<TimeInfo, FrixError> {
    let r = client.call(SERVICE, "GetInfo", &[]).await?;
    let mut ntp_servers = Vec::new();
    for nome in ["NewNTPServer1", "NewNTPServer2"] {
        let server = r.text(nome)?.trim();
        if !server.is_empty() {
            ntp_servers.push(server.to_owned());
        }
    }
    Ok(TimeInfo {
        local_time: r.text("NewCurrentLocalTime")?.to_owned(),
        time_zone: r.text("NewLocalTimeZoneName")?.to_owned(),
        daylight_saving: r.flag("NewDaylightSavingsUsed")?,
        ntp_servers,
    })
}

//! `Time:1`: router time and NTP servers (docs/PROJECT.md §4.2.16).

use crate::error::FrixError;
use crate::tr064::Tr064Client;

const SERVICE: &str = "Time:1";

/// Router local time and its source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeInfo {
    /// `NewCurrentLocalTime`, as reported by the router (ISO 8601).
    pub local_time: String,
    /// `NewLocalTimeZoneName`.
    pub time_zone: String,
    /// `NewDaylightSavingsUsed`.
    pub daylight_saving: bool,
    /// `NewNTPServer1` and `NewNTPServer2`, without empty ones.
    pub ntp_servers: Vec<String>,
}

/// `GetInfo`.
pub async fn info(client: &Tr064Client) -> Result<TimeInfo, FrixError> {
    let r = client.call(SERVICE, "GetInfo", &[]).await?;
    let mut ntp_servers = Vec::new();
    for name in ["NewNTPServer1", "NewNTPServer2"] {
        let server = r.text(name)?.trim();
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

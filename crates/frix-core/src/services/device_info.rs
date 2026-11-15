//! `DeviceInfo:1` (docs/PROJECT.md §4.2.1).

use crate::error::FrixError;
use crate::tr064::Tr064Client;

const SERVICE: &str = "DeviceInfo:1";

/// Router identity and status, without the system log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    /// `NewManufacturerName`.
    pub manufacturer: String,
    /// `NewModelName`.
    pub model: String,
    /// `NewDescription`.
    pub description: String,
    /// `NewProductClass`.
    pub product_class: String,
    /// `NewSerialNumber`.
    pub serial_number: String,
    /// `NewSoftwareVersion`: FRITZ!OS version.
    pub software_version: String,
    /// `NewHardwareVersion`.
    pub hardware_version: String,
    /// `NewUpTime`, in seconds.
    pub uptime_seconds: u64,
}

/// `GetInfo`. The response's `NewDeviceLog` is discarded: it is large and untrusted.
pub async fn info(client: &Tr064Client) -> Result<DeviceInfo, FrixError> {
    let r = client.call(SERVICE, "GetInfo", &[]).await?;
    Ok(DeviceInfo {
        manufacturer: r.text("NewManufacturerName")?.to_owned(),
        model: r.text("NewModelName")?.to_owned(),
        description: r.text("NewDescription")?.to_owned(),
        product_class: r.text("NewProductClass")?.to_owned(),
        serial_number: r.text("NewSerialNumber")?.to_owned(),
        software_version: r.text("NewSoftwareVersion")?.to_owned(),
        hardware_version: r.text("NewHardwareVersion")?.to_owned(),
        uptime_seconds: r.parse("NewUpTime")?,
    })
}

/// `GetSecurityPort`: the TR-064 HTTPS port. The only action sent in plain text and
/// without credentials (docs/PROJECT.md §5.1).
pub async fn security_port(client: &Tr064Client) -> Result<u16, FrixError> {
    client
        .call_plain(SERVICE, "GetSecurityPort", &[])
        .await?
        .parse("NewSecurityPort")
}

/// `GetDeviceLog`: the system log lines, newest first, without empty lines.
///
/// The content is free text written by the router and by Hosts on the network:
/// untrusted output (docs/PROJECT.md §7.3).
pub async fn log(client: &Tr064Client) -> Result<Vec<String>, FrixError> {
    let r = client.call(SERVICE, "GetDeviceLog", &[]).await?;
    Ok(r.text("NewDeviceLog")?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_owned)
        .collect())
}

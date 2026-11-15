//! Download of `*ListPath` lists (docs/PROJECT.md §5.3).
//!
//! Actions such as `Hosts:X_AVM-DE_GetHostListPath` return the relative path of a
//! Lua script to fetch with `GET` over the authenticated channel. Whether the path
//! already holds the SID is to be verified on the router (§14.3), so both cases are
//! supported: with `sid` in the query the path is used as is, otherwise the SID from
//! `DeviceConfig:X_AVM-DE_CreateUrlSID` is appended.

use crate::error::FrixError;
use crate::services::device_config;
use crate::tr064::Tr064Client;

/// Downloads the list at the `path` returned by a `*ListPath` action and returns its body.
pub(crate) async fn download(client: &Tr064Client, path: &str) -> Result<String, FrixError> {
    let path = path.trim();
    if !path.starts_with('/') || path.starts_with("//") {
        return Err(FrixError::Parse(
            "the router returned a non-relative list path".into(),
        ));
    }
    if has_sid(path) {
        return client.get_secure(path).await;
    }
    let sid = device_config::url_sid(client).await?;
    let separator = if path.contains('?') { '&' } else { '?' };
    client
        .get_secure(&format!("{path}{separator}{}", sid.query_param()))
        .await
}

/// True if the path's query already has a `sid` parameter.
fn has_sid(path: &str) -> bool {
    path.split_once('?').is_some_and(|(_, query)| {
        query
            .split('&')
            .any(|p| p.split_once('=').map_or(p, |(name, _)| name) == "sid")
    })
}

//! `DeviceConfig:1`, read-only (docs/PROJECT.md §4.2.14).

use secrecy::{ExposeSecret as _, SecretString};

use crate::error::FrixError;
use crate::tr064::Tr064Client;

const SERVICE: &str = "DeviceConfig:1";

/// Session SID for router URLs: a secret that stays inside `frix-core`
/// (docs/PROJECT.md §13.6). It has no `Display` and its `Debug` redacts it.
#[derive(Debug)]
pub(crate) struct UrlSid(SecretString);

impl UrlSid {
    /// The `sid=…` query parameter, only for building the request URL.
    pub(crate) fn query_param(&self) -> String {
        format!("sid={}", self.0.expose_secret())
    }
}

/// `X_AVM-DE_CreateUrlSID`: the SID to append to a router URL.
pub(crate) async fn url_sid(client: &Tr064Client) -> Result<UrlSid, FrixError> {
    let r = client.call(SERVICE, "X_AVM-DE_CreateUrlSID", &[]).await?;
    let value = r.text("NewX_AVM-DE_UrlSID")?.trim();
    let sid = value.strip_prefix("sid=").unwrap_or(value);
    if sid.is_empty() || !sid.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Err(FrixError::Parse(
            "X_AVM-DE_CreateUrlSID: unparsable SID".into(),
        ));
    }
    Ok(UrlSid(SecretString::from(sid)))
}

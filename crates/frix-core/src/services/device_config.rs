//! `DeviceConfig:1`, solo in lettura (docs/PROJECT.md §4.2.14).

use secrecy::{ExposeSecret as _, SecretString};

use crate::error::FrixError;
use crate::tr064::Tr064Client;

const SERVICE: &str = "DeviceConfig:1";

/// SID di sessione per gli URL del router: un segreto che resta dentro `frix-core`
/// (docs/PROJECT.md §13.6). Non ha `Display` e il suo `Debug` lo oscura.
#[derive(Debug)]
pub(crate) struct UrlSid(SecretString);

impl UrlSid {
    /// Il parametro di query `sid=…`, da usare solo per comporre l'URL della richiesta.
    pub(crate) fn query_param(&self) -> String {
        format!("sid={}", self.0.expose_secret())
    }
}

/// `X_AVM-DE_CreateUrlSID`: il SID da aggiungere a un URL del router.
pub(crate) async fn url_sid(client: &Tr064Client) -> Result<UrlSid, FrixError> {
    let r = client.call(SERVICE, "X_AVM-DE_CreateUrlSID", &[]).await?;
    let valore = r.text("NewX_AVM-DE_UrlSID")?.trim();
    let sid = valore.strip_prefix("sid=").unwrap_or(valore);
    if sid.is_empty() || !sid.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Err(FrixError::Parse(
            "X_AVM-DE_CreateUrlSID: SID non interpretabile".into(),
        ));
    }
    Ok(UrlSid(SecretString::from(sid)))
}

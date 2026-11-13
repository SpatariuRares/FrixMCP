//! Download delle liste `*ListPath` (docs/PROJECT.md §5.3).
//!
//! Azioni come `Hosts:X_AVM-DE_GetHostListPath` restituiscono il path relativo di uno
//! script Lua da scaricare con `GET` sul canale autenticato. Se il path contenga già
//! il SID è da verificare sul router (§14.3), quindi si supportano entrambi i casi:
//! con `sid` nella query il path si usa così com'è, altrimenti si aggiunge il SID
//! di `DeviceConfig:X_AVM-DE_CreateUrlSID`.

use crate::error::FrixError;
use crate::services::device_config;
use crate::tr064::Tr064Client;

/// Scarica la lista al `path` restituito da un'azione `*ListPath` e ne restituisce il corpo.
pub(crate) async fn download(client: &Tr064Client, path: &str) -> Result<String, FrixError> {
    let path = path.trim();
    if !path.starts_with('/') || path.starts_with("//") {
        return Err(FrixError::Parse(
            "il router ha restituito un path di lista non relativo".into(),
        ));
    }
    if ha_sid(path) {
        return client.get_secure(path).await;
    }
    let sid = device_config::url_sid(client).await?;
    let separatore = if path.contains('?') { '&' } else { '?' };
    client
        .get_secure(&format!("{path}{separatore}{}", sid.query_param()))
        .await
}

/// Vero se la query del path ha già un parametro `sid`.
fn ha_sid(path: &str) -> bool {
    path.split_once('?').is_some_and(|(_, query)| {
        query
            .split('&')
            .any(|p| p.split_once('=').map_or(p, |(nome, _)| nome) == "sid")
    })
}

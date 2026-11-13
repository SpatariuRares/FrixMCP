//! Le letture eseguite dal comando di registrazione e dal test live: i tool MCP della
//! docs/PROJECT.md §8.1, chiamati con gli stessi parametri dell'Agent sul server vero.
//!
//! Passare dai tool (e non dai moduli servizio) garantisce che ogni azione arrivi al
//! router attraverso `Tr064Client::call`, cioè SCPD e Policy: solo azioni di Level Read.

use frix_core::FrixError;
use frix_core::tr064::Tr064Client;
use frix_mcp::FrixServer;
use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use serde_json::Value;

/// Segnaposto negli argomenti: il MAC del primo Host attivo restituito da `list_hosts`.
pub const MAC_HOST_ATTIVO: &str = "{mac}";

/// Una chiamata di tool: nome e argomenti JSON.
#[derive(Debug, Clone, Copy)]
pub struct Lettura {
    /// Nome del tool MCP.
    pub tool: &'static str,
    /// Argomenti JSON; [`MAC_HOST_ATTIVO`] si sostituisce con un MAC reale.
    pub argomenti: &'static str,
}

const fn lettura(tool: &'static str, argomenti: &'static str) -> Lettura {
    Lettura { tool, argomenti }
}

/// Tutte le letture, in ordine. Un tool nuovo si aggiunge con una riga: il test
/// `le_letture_coprono_ogni_tool_del_server` fallisce finché manca.
/// `list_hosts` precede le letture che usano [`MAC_HOST_ATTIVO`].
pub const LETTURE: &[Lettura] = &[
    // sistema e WAN
    lettura("get_system_info", "{}"),
    lettura("get_wan_status", "{}"),
    lettura("get_bandwidth", "{}"),
    lettura("get_dsl_status", "{}"),
    // Host e mesh
    lettura("list_hosts", r#"{"include_mesh": true}"#),
    lettura("get_host", r#"{"mac": "{mac}"}"#),
    lettura("get_mesh_topology", "{}"),
    // Wi-Fi
    lettura("get_wifi_status", r#"{"network": "all"}"#),
    lettura("list_wifi_clients", r#"{"network": "all"}"#),
    // LAN, log, ora, port forwarding
    lettura("get_lan_config", "{}"),
    lettura("get_device_log", r#"{"lines": 1000}"#),
    lettura("get_time_and_update", "{}"),
    lettura("list_port_forwardings", "{}"),
    // controllo accessi e Smart Home
    lettura("get_access_control", "{}"),
    lettura("get_access_control", r#"{"mac": "{mac}"}"#),
    lettura("list_smarthome_devices", "{}"),
];

/// Esito di una lettura.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Esito {
    /// Nome del tool.
    pub tool: &'static str,
    /// `None` se il tool ha risposto senza errore; altrimenti il testo dell'errore.
    pub errore: Option<String>,
}

/// Risultato di [`esegui`]: gli esiti e i tool che il server espone.
#[derive(Debug, Clone)]
pub struct Letture {
    /// Un esito per ogni voce di [`LETTURE`], nello stesso ordine.
    pub esiti: Vec<Esito>,
    /// Nomi dei tool di `tools/list`.
    pub tool_del_server: Vec<String>,
}

/// Avvia il server MCP sopra `client` nello stesso processo ed esegue [`LETTURE`].
pub async fn esegui(client: Tr064Client) -> Result<Letture, FrixError> {
    let errore_mcp = |e: &dyn std::fmt::Display| FrixError::Transport(format!("client MCP: {e}"));
    let (lato_server, lato_client) = tokio::io::duplex(64 * 1024);
    let server = FrixServer::new(client);
    let in_ascolto = tokio::spawn(async move {
        if let Ok(servizio) = server.serve(lato_server).await {
            let _ = servizio.waiting().await;
        }
    });
    let mcp = ().serve(lato_client).await.map_err(|e| errore_mcp(&e))?;
    let tool_del_server = mcp
        .list_all_tools()
        .await
        .map_err(|e| errore_mcp(&e))?
        .into_iter()
        .map(|t| t.name.into_owned())
        .collect();

    let mut mac: Option<String> = None;
    let mut esiti = Vec::with_capacity(LETTURE.len());
    for lettura in LETTURE {
        let argomenti = match (&mac, lettura.argomenti.contains(MAC_HOST_ATTIVO)) {
            (Some(m), true) => lettura.argomenti.replace(MAC_HOST_ATTIVO, m),
            (None, true) => {
                esiti.push(Esito {
                    tool: lettura.tool,
                    errore: Some("nessun Host attivo da list_hosts: lettura saltata".into()),
                });
                continue;
            }
            (_, false) => lettura.argomenti.to_owned(),
        };
        let argomenti: Value = serde_json::from_str(&argomenti)
            .map_err(|e| FrixError::Validation(format!("argomenti di {}: {e}", lettura.tool)))?;
        let mut richiesta = CallToolRequestParams::new(lettura.tool.to_owned());
        if let Value::Object(mappa) = argomenti {
            richiesta = richiesta.with_arguments(mappa);
        }
        let risposta = mcp.call_tool(richiesta).await.map_err(|e| errore_mcp(&e))?;
        if lettura.tool == "list_hosts" {
            mac = primo_host_attivo(&risposta);
        }
        esiti.push(Esito {
            tool: lettura.tool,
            errore: (risposta.is_error == Some(true)).then(|| testo(&risposta)),
        });
    }
    let _ = mcp.cancel().await;
    in_ascolto.abort();
    Ok(Letture {
        esiti,
        tool_del_server,
    })
}

fn testo(esito: &CallToolResult) -> String {
    esito
        .content
        .first()
        .and_then(|c| c.as_text())
        .map(|t| t.text.clone())
        .unwrap_or_default()
}

/// MAC del primo Host attivo nella risposta di `list_hosts` (JSON dopo la riga d'intestazione).
fn primo_host_attivo(esito: &CallToolResult) -> Option<String> {
    let testo = testo(esito);
    let (_, json) = testo.split_once('\n')?;
    let risposta: Value = serde_json::from_str(json).ok()?;
    risposta
        .get("data")?
        .get("hosts")?
        .as_array()?
        .iter()
        .find(|h| h.get("active").and_then(Value::as_bool) == Some(true))?
        .get("mac")?
        .as_str()
        .map(str::to_owned)
}

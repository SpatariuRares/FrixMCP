//! The reads run by the record command and by the live test: the MCP tools of
//! docs/PROJECT.md §8.1, called with the same parameters as the Agent, on the real server.
//!
//! Going through the tools (and not the service modules) guarantees that every action
//! reaches the router through `Tr064Client::call`, that is SCPD and Policy: only Level Read actions.

use frix_core::FrixError;
use frix_core::tr064::Tr064Client;
use frix_mcp::FrixServer;
use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use serde_json::Value;

/// Placeholder in the arguments: the MAC of the first active Host returned by `list_hosts`.
pub const ACTIVE_HOST_MAC: &str = "{mac}";

/// A tool call: name and JSON arguments.
#[derive(Debug, Clone, Copy)]
pub struct ToolRead {
    /// MCP tool name.
    pub tool: &'static str,
    /// JSON arguments; [`ACTIVE_HOST_MAC`] is replaced with a real MAC.
    pub args: &'static str,
}

const fn read(tool: &'static str, args: &'static str) -> ToolRead {
    ToolRead { tool, args }
}

/// All reads, in order. A new tool takes one line: the test
/// `reads_cover_every_server_tool` fails until it is added.
/// `list_hosts` comes before the reads that use [`ACTIVE_HOST_MAC`].
pub const READS: &[ToolRead] = &[
    // system and WAN
    read("get_system_info", "{}"),
    read("get_wan_status", "{}"),
    read("get_bandwidth", "{}"),
    read("get_dsl_status", "{}"),
    // Hosts and mesh
    read("list_hosts", r#"{"include_mesh": true}"#),
    read("get_host", r#"{"mac": "{mac}"}"#),
    read("get_mesh_topology", "{}"),
    // Wi-Fi
    read("get_wifi_status", r#"{"network": "all"}"#),
    read("list_wifi_clients", r#"{"network": "all"}"#),
    // LAN, log, time, port forwarding
    read("get_lan_config", "{}"),
    read("get_device_log", r#"{"lines": 1000}"#),
    read("get_time_and_update", "{}"),
    read("list_port_forwardings", "{}"),
    // access control and Smart Home
    read("get_access_control", "{}"),
    read("get_access_control", r#"{"mac": "{mac}"}"#),
    read("list_smarthome_devices", "{}"),
];

/// Outcome of one read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// Tool name.
    pub tool: &'static str,
    /// `None` if the tool answered without an error; otherwise the error text.
    pub error: Option<String>,
}

/// Result of [`run`]: the outcomes and the tools the server exposes.
#[derive(Debug, Clone)]
pub struct Reads {
    /// One outcome per entry of [`READS`], in the same order.
    pub outcomes: Vec<Outcome>,
    /// Tool names from `tools/list`.
    pub server_tools: Vec<String>,
}

/// Starts the MCP server on top of `client` in the same process and runs [`READS`].
pub async fn run(client: Tr064Client) -> Result<Reads, FrixError> {
    let mcp_error = |e: &dyn std::fmt::Display| FrixError::Transport(format!("MCP client: {e}"));
    let (server_side, client_side) = tokio::io::duplex(64 * 1024);
    let server = FrixServer::new(client);
    let listening = tokio::spawn(async move {
        if let Ok(service) = server.serve(server_side).await {
            let _ = service.waiting().await;
        }
    });
    let mcp = ().serve(client_side).await.map_err(|e| mcp_error(&e))?;
    let server_tools = mcp
        .list_all_tools()
        .await
        .map_err(|e| mcp_error(&e))?
        .into_iter()
        .map(|t| t.name.into_owned())
        .collect();

    let mut mac: Option<String> = None;
    let mut outcomes = Vec::with_capacity(READS.len());
    for read in READS {
        let args = match (&mac, read.args.contains(ACTIVE_HOST_MAC)) {
            (Some(m), true) => read.args.replace(ACTIVE_HOST_MAC, m),
            (None, true) => {
                outcomes.push(Outcome {
                    tool: read.tool,
                    error: Some("no active Host from list_hosts: read skipped".into()),
                });
                continue;
            }
            (_, false) => read.args.to_owned(),
        };
        let args: Value = serde_json::from_str(&args)
            .map_err(|e| FrixError::Validation(format!("arguments of {}: {e}", read.tool)))?;
        let mut request = CallToolRequestParams::new(read.tool.to_owned());
        if let Value::Object(map) = args {
            request = request.with_arguments(map);
        }
        let response = mcp.call_tool(request).await.map_err(|e| mcp_error(&e))?;
        if read.tool == "list_hosts" {
            mac = first_active_host(&response);
        }
        outcomes.push(Outcome {
            tool: read.tool,
            error: (response.is_error == Some(true)).then(|| text(&response)),
        });
    }
    let _ = mcp.cancel().await;
    listening.abort();
    Ok(Reads {
        outcomes,
        server_tools,
    })
}

fn text(result: &CallToolResult) -> String {
    result
        .content
        .first()
        .and_then(|c| c.as_text())
        .map(|t| t.text.clone())
        .unwrap_or_default()
}

/// MAC of the first active Host in the `list_hosts` response (JSON after the header line).
fn first_active_host(result: &CallToolResult) -> Option<String> {
    let text = text(result);
    let (_, json) = text.split_once('\n')?;
    let response: Value = serde_json::from_str(json).ok()?;
    response
        .get("data")?
        .get("hosts")?
        .as_array()?
        .iter()
        .find(|h| h.get("active").and_then(Value::as_bool) == Some(true))?
        .get("mac")?
        .as_str()
        .map(str::to_owned)
}

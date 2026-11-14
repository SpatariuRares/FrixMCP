# FrixMCP

An MCP server that lets an AI assistant read the state of a FRITZ!Box router: connected devices, Wi-Fi networks, mesh, bandwidth, WAN status, logs. Written in Rust, read-only.

Ask Claude "who is connected right now?" or "why is the internet slow?" and it answers from your router's live data instead of you digging through the AVM web interface.

- **Read-only.** Version 0.1 has no tool that changes anything on the router. Every action of the router's 35 TR-064 services is classified, and anything that writes is denied by the policy in `frix-core`.
- **No secrets in files.** The router password lives in the macOS Keychain.
- **Pinned TLS.** The server talks to the router only over HTTPS on its TR-064 port and accepts only the certificate whose SHA-256 fingerprint you configured.
- **Nothing exposed.** The server speaks MCP over stdio; it opens no network port.

Tested on a FRITZ!Box 7590 with FRITZ!OS 8.25. Other models with TR-064 probably work, but are not tested.

## Requirements

- macOS (the password is read from the Keychain with `/usr/bin/security`).
- Rust 1.88 or newer (`rustup`).
- A FRITZ!Box with "Access for applications" (TR-064) enabled: Home Network → Network → Network Settings.
- An MCP client, for example Claude Desktop or Claude Code.

## Install

```sh
cargo install --git https://github.com/SpatariuRares/FrixMCP --tag v0.1.0 frix-mcp
```

This puts `frix-mcp` in `~/.cargo/bin`.

## Setup

**1. Create a dedicated router user.** In the FRITZ!Box web interface: System → FRITZ!Box Users → Add User.

- Name it, for example, `frixmcp`, and give it a generated password (`openssl rand -base64 24`).
- Grant only the "FRITZ!Box Settings" right, and leave "Access from the internet" unchecked.

FRITZ!OS also grants the Phone, Dial, Smart Home and App rights together with "FRITZ!Box Settings", and they cannot be removed. The read-only policy is what limits the server, not the router account.

**2. Store the password in the Keychain.** The command asks for it interactively:

```sh
security add-generic-password -s FrixMCP -a frixmcp -w
```

The account (`-a`) must match the `username` in the config.

**3. Read the router's certificate fingerprint:**

```sh
openssl s_client -connect 192.168.178.1:49443 </dev/null 2>/dev/null \
  | openssl x509 -noout -fingerprint -sha256
```

**4. Write the config** at `~/.config/frixmcp/config.toml`:

```toml
host = "192.168.178.1"
username = "frixmcp"
# hide_serial_number = true   # get_system_info omits the serial number

[tls]
mode = "pin"
pinned_sha256 = "AB:CD:..."   # the fingerprint from step 3
```

Unknown keys are an error. Command-line flags override the file: run `frix-mcp --help`.

**5. Add the server to your MCP client.** For Claude Code, create `.mcp.json` in the folder you work from (or add the same entry to Claude Desktop's `claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "frix": {
      "command": "/Users/<you>/.cargo/bin/frix-mcp",
      "args": ["--config", "/Users/<you>/.config/frixmcp/config.toml"]
    }
  }
}
```

On startup the server prints the router account and its rights on stderr. In Claude Code, `/mcp` should show `frix` as connected with 15 tools.

## Tools

| Tool | What it returns |
|------|-----------------|
| `get_system_info` | model, firmware, uptime, router account and its rights |
| `get_wan_status` | connection type and state, public IP, DNS servers |
| `get_bandwidth` | current traffic and the last 20 samples, line maximums, byte counters |
| `get_dsl_status` | DSL line sync, SNR, attenuation, errors (empty on non-DSL connections) |
| `list_hosts` | devices known to the router, optionally only active ones, with mesh info |
| `get_host` | one device by MAC address |
| `get_mesh_topology` | mesh nodes and links with band and speed |
| `get_wifi_status` | each Wi-Fi network (2.4 GHz, 5 GHz, guest): SSID, band, channel, state, clients |
| `list_wifi_clients` | clients of each Wi-Fi network with signal, speed and channel width |
| `get_lan_config` | LAN addressing and DHCP server |
| `get_device_log` | the router's event log, paged |
| `get_time_and_update` | time, NTP servers, firmware update state |
| `list_port_forwardings` | port forwarding rules |
| `get_access_control` | parental control profiles, or the profile of one device |
| `list_smarthome_devices` | Smart Home devices (read-only) |

Tool descriptions and messages are in Italian; MCP clients and models handle them without issue.

## Security model

The guarantees hold only if the AI has no shell on your Mac. A model that can run commands can read the password from the Keychain and talk to the router directly, bypassing this server. Use Claude Desktop, or Claude Code with Bash denied, for example with this `.claude/settings.json` next to `.mcp.json`:

```json
{
  "enabledMcpjsonServers": ["frix"],
  "permissions": { "deny": ["Bash", "Read", "Edit", "Write", "WebFetch", "WebSearch"] }
}
```

Everything the router returns (device names, SSIDs, log lines) is passed to the model as untrusted data, with a fixed header that says so. See `SECURITY.md`.

## Not included

- Write operations (renaming devices, blocking internet access, turning the guest Wi-Fi on or off) are designed in `docs/PROJECT.md` §8.2 but not implemented.
- Smart Home write tools will not be built: there is no device to test them on (#16).
- The `frix-web` dashboard crate is an empty placeholder.

## Development

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features      # no router or network needed
cargo deny check
```

The tests run against recorded router responses with personal data removed (`crates/frix-core/tests/fixtures/`). To record your own, see `cargo xtask record`. The design document, glossary and decision records (`docs/PROJECT.md`, `GLOSSARY.md`, `docs/adr/`) are in Italian.

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT), at your option.

FRITZ!Box and FRITZ!OS are trademarks of AVM / FRITZ! GmbH. This project is not affiliated with them.

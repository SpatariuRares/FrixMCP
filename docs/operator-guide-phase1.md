# Operator guide: closing Phase 1 on the real router

Steps to do by hand, connected to the FRITZ!Box, to close tickets #3 and #9 and unblock #15.
All commands are run from the repo root. References: `docs/PROJECT.md` §12 and §14.3.

## Part 1: Router Account and config (#3)

**1. Router Account.** In the web interface: System → FRITZ!Box Users → Add User.

- Name: `frixmcp`.
- Password: generate it with `openssl rand -base64 24` and do not store it anywhere else; it goes only in the keyring (step 3).
- Rights: **only** "FRITZ!Box Settings".
- Uncheck "Access from the internet allowed".

**2. Certificate fingerprint and SAN** (no credentials needed):

```sh
openssl s_client -connect 192.168.178.1:49443 </dev/null 2>/dev/null \
  | openssl x509 -noout -fingerprint -sha256
openssl s_client -connect 192.168.178.1:49443 </dev/null 2>/dev/null \
  | openssl x509 -noout -text | grep -A1 'Subject Alternative Name'
```

(The openssl on macOS is LibreSSL and does not know `-ext`: that is why the SAN is read from `-text`.)

Note down the fingerprint (`AA:BB:…`) and whether the IP `192.168.178.1` appears in the SAN.

**3. Password in the keyring.** The command asks for it interactively. On macOS (Keychain):

```sh
security add-generic-password -s FrixMCP -a frixmcp -w
```

On Linux the password goes in the Secret Service (GNOME Keyring or KWallet):

```sh
secret-tool store --label=FrixMCP service FrixMCP account frixmcp
```

**4. Config** in `~/.config/frixmcp/config.toml`, outside the repo:

```sh
mkdir -p ~/.config/frixmcp && nano ~/.config/frixmcp/config.toml
```

```toml
host = "192.168.178.1"
username = "frixmcp"
# hide_serial_number = true

[tls]
mode = "pin"
pinned_sha256 = "AA:BB:…"   # the fingerprint from step 2
```

Unknown keys are an error: do not add `[audit]` or other sections.

**5. Digest parameters** from the 401 response, without credentials:

```sh
curl -sk -i -X POST https://192.168.178.1:49443/upnp/control/deviceinfo \
  -H 'Content-Type: text/xml; charset="utf-8"' \
  -H 'SOAPACTION: "urn:dslforum-org:service:DeviceInfo:1#GetInfo"' \
  -d '<?xml version="1.0"?><s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/"><s:Body><u:GetInfo xmlns:u="urn:dslforum-org:service:DeviceInfo:1"/></s:Body></s:Envelope>' \
  | grep -i www-authenticate
```

Without the `SOAPACTION` header the router does not answer with the Digest challenge. Note down `qop`, `algorithm` and whether `opaque` is present. The `-k` is only for this manual test; the code always uses pinning.

## Part 2: live test and recording (#9)

**6. Live test**, which only reads:

```sh
cargo test -p xtask --features live --test live -- --ignored --nocapture
```

If it passes, on startup it prints the Router Account and its rights on stderr: check that the Router Account is `frixmcp` and that it has no access from the internet.

| Error | Cause |
|---|---|
| `tls_pin_mismatch` | wrong fingerprint in the config |
| 401 | wrong user or password |
| 606 | missing rights |
| `config file not found` | step 4 is missing |

**7. Recording the fixtures:**

```sh
cargo xtask record
```

- The raw responses end up in `crates/frix-core/tests/fixtures/raw/unredacted/`, which is ignored by git and must never be committed.
- The redacted copies end up in `crates/frix-core/tests/fixtures/raw/redacted/`.

**8. Checking the redacted copies by hand.** Redaction starts from a hand-written list of fields, so an unexpected field could slip through. Search for your real data:

```sh
D=crates/frix-core/tests/fixtures/raw/redacted
grep -rni '<router hostname>' $D
grep -rni '<router MAC, with and without colons>' $D
grep -rni '<your SSID>' $D
grep -rni '<the name of one of your devices>' $D
grep -rnE '([0-9]{1,3}\.){3}[0-9]{1,3}' $D | grep -v '192\.168\.' | head
```

Replace each `<…>` with your real value before running the command. The last command looks for public IPs (in zsh a `#` at the end of a line is not a comment, so do not add one).

All the greps must return nothing, or only replaced values.

**9. Data for the comment on #3.** It is already in the redacted copies:

```sh
grep -rh -A1 'APType\|X_AVM-DE_FrequencyBand' $D/responses/WLANConfiguration-*/X_AVM-DE_GetWLANExtInfo.xml
grep -rh 'NewDefaultConnectionService' $D/responses/Layer3Forwarding-1/
grep -rh 'NewWANAccessType' $D/responses/WANCommonInterfaceConfig-1/
```

- The first tells you which instance 1/2/3 is `main_2g`, `main_5g` or `guest`. Also check that the values really are `normal` and `guest`: the code assumes them.
- The second tells you whether the connection is PPP or IP.
- The third gives you the `WANAccessType`.

**10. Comment on #3 and closing the two issues.** The comment must not contain passwords or the full fingerprint:

```sh
gh issue comment 3 --body "Digest: qop=…, algorithm=…, opaque=yes/no. SAN: IP present yes/no. WLAN: 1=…, 2=…, 3=…. Connection: PPP/IP. WANAccessType: …"
gh issue close 3
gh issue close 9 --comment "Live test passed read-only; redacted fixtures recorded and checked by hand."
```

There is no need to commit the fixtures now: they stay in the ignored folder and #15 moves them.

## Part 3: aligning with the real fixtures (#15)

**11.** Ask Claude Code to run `/implement-spec 1`: with #9 closed, #15 is unblocked. In #15 an agent:

- moves the redacted fixtures into `tests/fixtures/`, renaming `devicehostlist.lua` to `.xml` and `meshlist.lua` to `.json`;
- fixes the code where the real responses differ from the hand-written ones;
- updates §14.3 of `docs/PROJECT.md` with what you verified.

In the same round, confirm the Classification choices that depart from §4.2:

- `X_AVM-DE_GetWLANHybridMode` is Dangerous because it returns the passphrase;
- `WLANConfiguration:SetEnable` is currently Dangerous on all three instances in the registry; from Phase 2 the Level moves to the tools (`docs/PROJECT.md` §6.2) and `set_guest_wifi` will be Write on the guest network instance verified in step 9;
- the "outside v1" writes and AppSetup, WANMobileConnection and `X_AVM-DE_DoPrepareCGI` are Forbidden.

## Part 4: using it from Claude Code (optional)

**12.** Install the MCP server **outside the repo**, so that changing the code does not change the program that talks to the router (`docs/PROJECT.md` §7.0):

```sh
cargo install --path crates/frix-mcp --root ~/.local     # → ~/.local/bin/frix-mcp
cp .mcp.json.example .mcp.json                           # .mcp.json is ignored by git
```

In `.mcp.json` replace `<you>` in the two paths (binary and config, written out in full). After every code change you want to use, run `cargo install` again.

FrixMCP's guarantees hold only if the AI has no shell on the Mac: use it from Claude Desktop, or from Claude Code with Bash denied or sandboxed. Today there are only reads, so the practical risk is low, but the rule matters from Phase 2. Then open the client and check that `frix` shows as connected (in Claude Code: `/mcp`).

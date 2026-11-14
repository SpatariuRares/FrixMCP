# Goldfish Review Report

**Target:** `/Users/raresspatariu/FrixMCP/docs/PROJECT.md` (`lines 1-1302`, full file)

Stage 5: Initial Zoom Out Review. Baseline context given to each reviewer: `GLOSSARY.md`, `docs/adr/0001-out-of-band-confirmation.md`, `docs/adr/0002-hosts-addressed-by-mac.md`.

## Reviewer Status

| Reviewer | Status |
| --- | --- |
| `egm-zoom-out` | completed (15 findings; verdict: Approve with changes) |
| `egm-inventive-catalyst` | completed (8 findings) |

**Severity breakdown (after deduplication and triage):** 1 Blocker, 14 Major, 8 Nit (23 total).

**Triage notes**
- The inventive catalyst's persona allows only Major or Nit in REVIEW mode. It raised its F1 (same-user Agent) as Major and recommended making it a Blocker. The synthesizer agreed and made it a Blocker, because it disproves the document's main security claim against the primary client (Claude Code).
- Line numbers are the reviewers' citations against the file as it stood at review time. The synthesizer did not check each one again.
- **Tension between the lenses.** Zoom-out pushes to remove safety machinery that has no demonstrated threat behind it (rate limits, the TLS `ca` mode, the generic snapshot mapping). Inventive-catalyst pushes to add machinery to make safety claims hold (a shared lock, a write-ahead audit, TOCTOU re-checks, multi-MAC identity). The two are compatible if the Operator first fixes which guarantees the document actually promises (the Blocker), then keeps only the machinery those promises need.

## Prioritized Findings (Blockers -> Major -> Nits)

### The Agent is a same-user process, which the threat model excludes - Severity: Blocker
* **Reviewer Lens:** `egm-inventive-catalyst` (F1; Major in the reviewer's report, escalated by the synthesizer). Related: `egm-zoom-out` F7 (where Confirmation lives).
* **Affected Scope:** §4.3 (L394), §6.4 (L614), §8.0, §12 step 5 (L928), §13.5 (L1110), §13.7 SECURITY.md (L1162), ADR 0001, GLOSSARY "Confirmation".
* **Discrepancy Description**: The document says "Il vero confine di sicurezza è quindi **la policy in `frix-core`**", and the glossary says Confirmation runs "su un canale che l'IA non può leggere né azionare". SECURITY.md, however, puts "Attacchi da un processo già in esecuzione come lo stesso utente macOS" out of scope. With Claude Code, the Agent is exactly such a process, and it has a shell. With that shell it can:
  - (a) read the Keychain item with `security find-generic-password`. The item was created with the default ACL by `security add-generic-password -s FrixMCP -a frixmcp -w`.
  - (b) edit `.mcp.json` to add `--allow-writes` / `--allow-dangerous`.
  - (c) start `frix-web` and `curl` its forms. CSRF, SameSite and Origin checks only stop browsers. The Confirmation there is "il dialog della UI" and the audit records Initiator `operator`.
  - (d) possibly script the native dialog through `osascript`.
  - (e) rewrite `audit.jsonl`.
* **Rationale & Recommendation**: Either make the claim true or state it honestly.
  - To make it true:
    - Keychain item ACL restricted to the signed binaries (`-T`) or user-presence access control.
    - Touch ID (LocalAuthentication) for Dangerous Confirmation.
    - One Confirmation mechanism in `frix-core` behind a trait, used by both front-ends. "Came from loopback" must not count as "Operator present".
    - Flags read from a location the Agent cannot edit.
    - Optionally, the router's own physical-button 2FA for the highest-risk subset.
  - At minimum: add a threat-model row ("T13: Agent con shell sullo stesso utente"), rewrite the SECURITY.md out-of-scope line, and soften §4.3/GLOSSARY accordingly.
  - Costs: Keychain re-prompts after every rebuild, a Touch ID hardware dependency, and less autonomy if router-button 2FA is used.

### Document claims the repo is empty while Phase 0/1 are committed and "open" decisions are settled - Severity: Major
* **Reviewer Lens:** `egm-zoom-out` (F1)
* **Affected Scope:** header (L5), §2.1 (L57-63), §12 heading (L914), §14.2 items 1 and 3 (L1231, L1233); compare L1245.
* **Discrepancy Description**: "scritto il 2026-11-10 prima della prima riga di codice" / "Cartella `~/FrixMCP` **vuota**". The reqwest-vs-ureq choice is already made in `Cargo.toml` (ureq). The keyring-vs-`security` choice is already made in `crates/frix-core/src/secrets.rs` (`security` CLI). Meanwhile L1245 already cites `transport/router.rs`, so the document is half pre-code and half post-code.
* **Rationale & Recommendation**: Add a "Stato attuale" block. Mark §14.2.1 and §14.2.3 as decided, with a date and a pointer to the code or an ADR. Move §12 into `docs/guida-operator-fase1.md` or mark it historical.

### §13 copies whole repo files, and the copies have drifted from the real files - Severity: Major
* **Reviewer Lens:** `egm-zoom-out` (F2)
* **Affected Scope:** §13.1-§13.10 (L963-1212), §6.5 (L616-638).
* **Discrepancy Description**: The copies list reqwest, keyring, pbkdf2, axum, rmcp 3.9, a workspace without `xtask`, and a file named `CLAUDE.md`. The real repo has ureq, rmcp 3.10, `xtask`, and `AGENTS.md`. The real `Cargo.toml` says "Versioni candidate in docs/PROJECT.md §6.5", so the code points back at a table that is now wrong.
* **Rationale & Recommendation**: Replace §13 with a list of paths, each with a one-line statement of what the file must guarantee. Cut §6.5 down to the selection rules (`# perché:`, rustls-only, the dependency-count threshold) and leave versions to `Cargo.toml`.

### Two sources of truth for action Levels: §4 tables versus `policy::registry` - Severity: Major
* **Reviewer Lens:** `egm-zoom-out` (F3)
* **Affected Scope:** §4.2 (L190-388), §6.2 (L590), §13.6 (L1119).
* **Discrepancy Description**: About 200 lines of per-action tables duplicate `crates/frix-core/src/policy/registry.rs`, and the coverage test checks the registry, not the document. Even so, the agent rules say "la sezione 4 è la fonte di verità".
* **Rationale & Recommendation**: Make the registry the only source of truth. In §4, keep the classification rules and the non-obvious cases (`GetSecurityKeys` readable with only "App", `SetEnable` W on the guest network and D elsewhere, "C is effectively admin"). If a full table is wanted, generate it from the registry with an xtask command.

### Many actions marked Dangerous with no tool using them - Severity: Major
* **Reviewer Lens:** `egm-zoom-out` (F4)
* **Affected Scope:** §4.2.3 (L253-257), §4.2.6 (L300), §4.2.16 (L379-382), L254.
* **Discrepancy Description**: §8.3 defines 8 Dangerous tools, but dozens of other writes are classified D (`SetSSID`, `SetChannel`, `SetConnectionType`, `SetNTPServers`, ...). That makes the reachable surface larger than the planned features. "W/D" is not a valid Classification.
* **Rationale & Recommendation**: Rule: an action gets a Level only when a §8 tool uses it, and every other write is Forbidden. Resolve "W/D" to a single value.

### Multi-router "keep the door open" contradicts the non-goal - Severity: Major
* **Reviewer Lens:** `egm-zoom-out` (F5)
* **Affected Scope:** §14.2 item 6 (L1236) versus §1.3 (L46).
* **Discrepancy Description**: "ma `Config` può già essere una lista per non chiudere la porta" versus "Nessuna astrazione \"generica per router\"."
* **Rationale & Recommendation**: Delete the clause. Turning a single-entry config into a list later is trivial.

### SID login, PBKDF2 and an MD5 fallback are fully specified with no v1 consumer - Severity: Major
* **Reviewer Lens:** `egm-zoom-out` (F6). Related: `egm-inventive-catalyst` F2 (SID count across processes).
* **Affected Scope:** §5.2 (L443-474), §5.4-§5.5 (L482-488), §6.1 `sid.rs` (L530), §6.5 pbkdf2/sha2/hmac (L629).
* **Discrepancy Description**: AHA and REST are non-goals, and `data.lua` is Phase 4. `*ListPath` downloads already have a TR-064 route (`X_AVM-DE_CreateUrlSID`). The MD5 fallback "sul 8.25 non dovrebbe servire ma costa poco implementarlo" is YAGNI.
* **Rationale & Recommendation**: Cut these sections to a short deferred note with links. Remove `sid.rs` and the pbkdf2/hmac/sha2 dependencies from the planned layout. Drop the MD5 fallback.

### Policy, snapshots and audit placed inside the TR-064 client, with stale code signatures - Severity: Major
* **Reviewer Lens:** `egm-zoom-out` (F7). Related: `egm-inventive-catalyst` F1, F3, F4.
* **Affected Scope:** §6.2 (L575-590).
* **Discrepancy Description**: The `Tr064Client { transport, desc, creds, policy }` snippet makes the RPC client own confirmation, flags and audit I/O. Confirmation differs per front-end (ADR 0001), so it cannot sit in a core RPC call without callbacks leaking through. The generic Set→Get snapshot mapping is heavy when each tool in §8.2 already knows its own "before" read. The real `Tr064Client` (`crates/frix-core/src/tr064/mod.rs` L29-32) holds only `transport` and `services`.
* **Rationale & Recommendation**: Keep `Tr064Client::call` as a pure RPC call with SCPD validation. Add a separate `policy::guard` / write executor in `frix-core` that takes a typed write request with its own before/after reads, plus a `Confirm` trait implemented by each front-end. Remove the Rust snippet. This is also the natural place to fix the TOCTOU and audit-ordering findings below.

### Per-process policy state breaks "un solo punto di controllo" - Severity: Major
* **Reviewer Lens:** `egm-inventive-catalyst` (F2)
* **Affected Scope:** §2.3 (L92), §5.2 (L472), §6.1 (L569), §6.2 (L597), §6.4.
* **Discrepancy Description**: "Un solo punto di controllo per le scritture", but `frix-mcp` (possibly one process per Claude Code session) and `frix-web` each hold their own rate-limit counters, last-dangerous timestamp, SID, nonce and poller. The limits multiply by the number of processes, "Un solo SID per router per programma" no longer holds, and `audit.jsonl` has concurrent appenders with no lock. This makes T8 and T11 worse.
* **Rationale & Recommendation**: Lighter option: shared guard state and audit appends serialized with `flock` under `~/Library/Application Support/FrixMCP/`. Heavier option: a single `frix-core` daemon with both front-ends as thin clients. The heavier option adds launchd lifecycle and a new local socket to protect (see the Blocker).

### 120 s Confirmation window reopens the TOCTOU that ADR 0002 closes - Severity: Major
* **Reviewer Lens:** `egm-inventive-catalyst` (F3)
* **Affected Scope:** §6.2 `call` sequence (L586), §8.0 (L723), ADR 0002.
* **Discrepancy Description**: The Preview is approved at t0, and the write runs up to 120 s later. MAC→IP resolution or the target state can change in between. The "prima" snapshot is taken after confirmation (step 3 comes after step 2), so the audited state can differ from what the Operator saw.
* **Rationale & Recommendation**: Snapshot and resolve before the Preview. Bind the Confirmation to a digest of {IP, snapshot, `X_AVM-DE_GetChangeCounter`}. Re-check right after confirmation and abort on mismatch. Record the confirmed Preview in the audit. Cost: occasional spurious aborts.

### Audit is written after the effect, so writes can go unaudited - Severity: Major
* **Reviewer Lens:** `egm-inventive-catalyst` (F4)
* **Affected Scope:** §6.2 step (7) (L586), §4.1 (L142), §8.3 (`reboot_router`, `reconnect_wan`, `apply_firmware_update`).
* **Discrepancy Description**: "(7) se livello ≥ Write, rilegge lo stato \"dopo\" e scrive l'audit." For tools that cut connectivity, the "dopo" read fails by design. A crash or audit-write failure after the SOAP call leaves an executed write with no record, even though the document says "audit obbligatorio".
* **Rationale & Recommendation**: Write an `intent` entry before the call and fail closed if it cannot be written. Then write a `result` entry. For connectivity-cutting tools, allow `after: pending` and reconcile on the next contact.

### Port forwarding stores an IP, so the MAC guarantee lapses after the write - Severity: Major
* **Reviewer Lens:** `egm-inventive-catalyst` (F5)
* **Affected Scope:** §8.3 `add_port_forwarding` (L772), ADR 0002.
* **Discrepancy Description**: "l'Host bersaglio dovrebbe avere un IP fisso" is only a hope. FrixMCP cannot set static leases (LANHostConfigManagement `Set*` is X), and nothing detects drift, so a forward can silently start exposing a different device.
* **Rationale & Recommendation**: Store the intended MAC (in a `frix:<mac>:` description prefix or in the audit). Flag drift in `list_port_forwardings` and the dashboard. Optionally refuse the add unless the Host's `AddressSource` is static, with an Operator override.

### Control Host identified by a single MAC, though devices have several - Severity: Major
* **Reviewer Lens:** `egm-inventive-catalyst` (F6). Related: `egm-zoom-out` F14 (the lookup method is implementation detail).
* **Affected Scope:** §6.2 `policy::guard` (L596), §8.2 `set_guest_wifi`, §8.3 `set_wifi_enabled`, ADR 0002, GLOSSARY "Lockout".
* **Discrepancy Description**: The glossary says Lockout is prevented "sempre, a qualunque Level", but a Mac with both Ethernet and Wi-Fi, or with a private Wi-Fi address, has several MACs. A write aimed at the other interface passes the check.
* **Rationale & Recommendation**: Build the Control Host identity as a set: all local interface MACs (via `getifaddrs`) matched against the router host list, plus the request-client entry. Accept that virtual or VPN interfaces may cause false positives. In the document, describe this at the level of the guarantee, not the lookup call (per zoom-out F14).

### Contradiction about mobile access to the web UI - Severity: Major
* **Reviewer Lens:** `egm-zoom-out` (F9)
* **Affected Scope:** §9.1 (L792) versus Phase 3 item 3 (L898).
* **Discrepancy Description**: "l'accesso da altri apparecchi, telefono incluso, è un non-obiettivo della v1" versus "3. Accessibilità e layout mobile."
* **Rationale & Recommendation**: Drop "layout mobile", or reduce it to "non si rompe su finestre strette".

### frix-web v1 scope is broad for a single-user loopback companion - Severity: Major
* **Reviewer Lens:** `egm-zoom-out` (F10). Related: `egm-inventive-catalyst` F2 (a second process adds a SID and polling load).
* **Affected Scope:** §9.2 (L797-807), §9.3 (L816), §6.4 (L614), Phase 3 (L894-899).
* **Discrepancy Description**: Phase 3 includes nine screens, SSE, a 2-5 s poller, ring-buffer persistence, a speculative channel-scan chart and a full write path with its own CSRF and Confirmation. The 2 s poller in a second process works against T8 and against the limited number of SID sessions.
* **Rationale & Recommendation**: Minimal v1: read-only Overview, Hosts and Audit, with polling only while a browser is connected. Defer the history, channel scan, Smart Home screen and web-initiated Dangerous actions. This also removes the web path from the Blocker for v1.

### Rate limits layered on top of per-call human Confirmation - Severity: Nit
* **Reviewer Lens:** `egm-zoom-out` (F8). See also `egm-inventive-catalyst` F8 (key the limits by Initiator) and F2 (limits multiply across processes).
* **Affected Scope:** §6.2 (L597), §8.3 (L767).
* **Discrepancy Description**: There are three throttles (writes/min, 1 dangerous/60 s, `reconnect_wan` 1/10 min), but every Dangerous call already needs a click within 120 s.
* **Rationale & Recommendation**: Keep one write rate limit as protection against agent loops, applied to Initiator `agent` only (combining both lenses). Drop the Dangerous-specific and per-tool limits unless a concrete scenario justifies them.

### Write flags fixed per process: all-off or all-session-on - Severity: Nit
* **Reviewer Lens:** `egm-inventive-catalyst` (F8)
* **Affected Scope:** §4.1 (L142), §13.5, §6.2 (L597), §6.4.
* **Discrepancy Description**: Enabling one write means restarting with `--allow-writes` for the whole session. The Agent-oriented rate limit also slows the Operator in the dashboard ("Stessa `Policy`").
* **Rationale & Recommendation**: Let the flag set the maximum allowed Level, with time-boxed write windows granted through Confirmation inside that. Key limits by Initiator. Weigh this against zoom-out's push for less policy state.

### Startup depends on the router being reachable - Severity: Nit
* **Reviewer Lens:** `egm-inventive-catalyst` (F7)
* **Affected Scope:** §6.3 (L608), §5.1, §7.4 step 4 (L702), §8.3 `apply_firmware_update` (L774).
* **Discrepancy Description**: If the router is down or the pin is broken, `frix-mcp` fails to start and cannot even report the problem. `apply_firmware_update` is itself a documented cause of certificate regeneration (`TlsPinMismatch`), so a FrixMCP tool can break FrixMCP's own connectivity.
* **Rationale & Recommendation**: Lazy discovery with the SCPD cache, tools always registered, and structured errors. Warn about re-pinning in the firmware Preview. Optionally offer an Operator-confirmed re-pin that shows the old and new fingerprints.

### Craft-anything plan is heavyweight for an equivalent DevTools path - Severity: Nit
* **Reviewer Lens:** `egm-zoom-out` (F11)
* **Affected Scope:** §10 (L820-865), §2.3 (L83), Phase 4 item 2 (L904).
* **Discrepancy Description**: An alpha tool, a second router account and an `unstable-ui` flag serve gaps that no §8 tool needs. The document says DevTools "fa esattamente lo stesso lavoro".
* **Rationale & Recommendation**: Make DevTools the plan. Shrink §10 to the `ENDPOINTS.md` format plus the rule "TR-064 wins". Defer `unstable-ui`.

### Phase 1 Smart Home tool versus Phase 1 rights - Severity: Nit
* **Reviewer Lens:** `egm-zoom-out` (F12)
* **Affected Scope:** §8.1 (L743), §7.5 (L709), §12 step 2 (L919).
* **Discrepancy Description**: `list_smarthome_devices` is in Phase 1, but Smart Home rights are "fase 2" / "nessun diritto ... Smart Home ... per ora".
* **Rationale & Recommendation**: Move the tool to Phase 2, or state that C is enough for reading and H is needed only for `SetSwitch`.

### Two TLS modes where one is enough - Severity: Nit
* **Reviewer Lens:** `egm-zoom-out` (F13)
* **Affected Scope:** §7.4 (L700-703), §14.2 item 2 (L1232).
* **Discrepancy Description**: The document itself says "`pin` è più semplice", yet it keeps a `ca` mode.
* **Rationale & Recommendation**: Ship `pin` only, and add `ca` when a need appears.

### Leftover alternatives and implementation details - Severity: Nit
* **Reviewer Lens:** `egm-zoom-out` (F14)
* **Affected Scope:** §6.1 (L566, L569), §6.2 (L596), §6.5 (L635).
* **Discrepancy Description**: The text still offers "o da `cargo xtask layers`" (the rule is already enforced by `crates/frix-core/tests/regola_dipendenze.rs`), `wiremock` as an alternative to the existing `FixtureTransport`, and the details of the Control Host MAC lookup.
* **Rationale & Recommendation**: Say "enforced by test". Drop `xtask layers` and `wiremock`. Describe the guarantee rather than the lookup.

### Optional MCP resources and prompts are speculative - Severity: Nit
* **Reviewer Lens:** `egm-zoom-out` (F15)
* **Affected Scope:** §8.4 (L778-781).
* **Discrepancy Description**: `frix://hosts` and `frix://status` duplicate existing tools, for a hypothetical client preference.
* **Rationale & Recommendation**: Remove them, or move them to an "idee future" list outside the roadmap.

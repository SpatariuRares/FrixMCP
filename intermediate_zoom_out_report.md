# Goldfish Review Report

**Target:** `/Users/raresspatariu/FrixMCP/docs/PROJECT.md` (`Lines 1-1302`, full file)

Stage 8, Intermediate Zoom Out. This round builds on `zoom_out_report.md` (Stage 5), `comprehension_report.md` (Stage 6) and `critic_report.md` (Stage 7). PROJECT.md has not changed since those reports.

**Verdict:** approve with changes. The three-crate shape still works. Three cuts remove the need for most of the hardening the earlier reviews asked for:
- state the trust boundary as a deployment constraint;
- make `frix-web` read-only in v1;
- give Levels to tools, not to router actions.

After those cuts, three things still need to be added: a write executor in core, a `Confirm` trait, and an audit entry written before each write.

## Reviewer Status

| Reviewer | Status |
| --- | --- |
| `egm-zoom-out` | completed |

Severity breakdown: 1 Blocker, 8 Major, 3 Nit.

Besides the target, the reviewer read GLOSSARY.md, both ADRs and the three earlier reports. For the reuse check it also read `crates/frix-core/src/policy/{mod.rs,registry.rs}`, `crates/frix-core/src/secrets.rs`, `crates/frix-core/src/tr064/mod.rs`, `crates/frix-web/src/main.rs` and `SECURITY.md`, and confirmed that `docs/guida-operator-fase1.md` exists.

## Root-cause clusters

| Cluster | Root cause | Earlier findings it covers |
| --- | --- | --- |
| C1 Trust boundary | The design treats the in-process policy as the security boundary, but the Agent runs as the same macOS user and has a shell. | zoom_out_report "The Agent is a same-user process..."; critic_report B1, M4, M17; the "append-only" claim; comprehension 3 (in part) |
| C2 Second front-end that can write | `frix-web` was given writes, Dangerous actions and its own Confirmation in v1. | critic_report B2, half of M12; zoom_out_report "frix-web v1 scope is broad...", "Contradiction about mobile access..."; comprehension 6; inventive F2 |
| C3 Write pipeline placement and order | A single generic `Tr064Client::call` is expected to validate, confirm, snapshot, execute and audit. | comprehension 3; critic_report M13, M15; zoom_out_report "Policy, snapshots and audit placed inside the TR-064 client...", "120 s Confirmation window...", "Audit is written after the effect..." |
| C4 Level assigned in two places | Levels are set per router action (§4.2 and the registry) and again per tool (§8). | critic_report M1, N3; zoom_out_report "Two sources of truth...", "Many actions marked Dangerous with no tool..." |
| C5 Shared state across processes | Counters and session state that exist in each process are described as if there were only one process. | critic_report M12; zoom_out_report "Per-process policy state...", "Rate limits layered...", "Write flags fixed per process..." |
| C6 Speculative research tracks inside v1 | SID login, AHA, REST, `data.lua`, the craft-anything plan, MCP resources, multi-router support and the TLS `ca` mode are all in v1. | zoom_out_report "SID login, PBKDF2...", "Multi-router...", "Craft-anything plan...", "Two TLS modes...", "Optional MCP resources..."; critic_report M16 (port 443), M18, N4, N8 |
| C7 Document is plan, spec, registry, config files and runbook at once | The copies in the document drift from the code. | comprehension 1; critic_report M2, M3, N9; zoom_out_report "Document claims the repo is empty...", "§13 copies whole repo files..." |
| C8 Inconsistent terms | Can be fixed in one mechanical pass. | critic_report M5, M6, M7, N2; comprehension 2, 8 |

## Prioritized Findings (Blockers -> Major -> Nits)

### F1. Make the trust boundary a deployment constraint, not more machinery - Severity: Blocker
* **Reviewer Lens:** `egm-zoom-out` (cluster C1)
* **Affected Scope:** §4.3 L394 ("Il vero confine di sicurezza è quindi **la policy in `frix-core`**"); O3 L39 ("mai eseguibili dall'IA senza conferma esplicita della persona"); §7.1 asset 5 L664; §7.2 L668-681 (no threat row covers this case); §13.5 L1102 (`"command": "/Users/raresspatariu/FrixMCP/target/release/frix-mcp"`); §13.7 L1162 ("Attacchi da un processo già in esecuzione come lo stesso utente macOS."); GLOSSARY "Confirmation".
* **Discrepancy Description**: The earlier fixes were a Keychain ACL, Touch ID, flags the Agent cannot edit, and a hash-chained audit log. Each one closes a single path, and hardening against a same-user Agent with a shell never ends. The Agent does not even need the Keychain. `.mcp.json` runs a binary built from the working tree, and an Agent that edits this repo can rebuild `frix-mcp` without the guard.
* **Rationale & Recommendation**:
  1. Restate the claim: the policy limits Agent mistakes and prompt-injected tool calls (T1, T2). It does not stop an Agent that can run arbitrary code as the same user.
  2. In §7, name the supported deployments: Claude Desktop, or Claude Code with Bash and with file writes outside the project denied or sandboxed.
  3. Add T13 with the status "fuori ambito per costruzione, mitigato da configurazione del client".
  4. Point `.mcp.json.example` at an installed binary outside the repo, not at `target/release`.
  5. Make O3, L394, SECURITY.md L1162 and GLOSSARY "Confirmation" say the same thing.

  These text changes plus one path change replace all five hardening items in critic_report B1.

### F2. The `security` CLI, already used in the code, rules out the cheapest Keychain mitigation - Severity: Major
* **Reviewer Lens:** `egm-zoom-out` (cluster C1)
* **Affected Scope:** §6.5 L631, §14.2 item 3 L1233, §12 step 5 L928; code: `crates/frix-core/src/secrets.rs` L4-10, L22.
* **Discrepancy Description**: A Keychain ACL set with `-T` trusts the program that reads the item. Here that program is `/usr/bin/security` both for `frix-mcp` and for the Agent, so the ACL proposed in zoom_out_report (Blocker, option a) protects nothing. §14.2.3 still lists the crate-versus-CLI choice as open and only on dependency grounds, but the code has already settled it.
* **Rationale & Recommendation**: Mark §14.2.3 as decided ("CLI `security`, vedi `secrets.rs`"). Add one line on the consequence: Keychain ACLs restricted to a specific binary are not available. Do not switch to `security-framework` for this reason alone, because F1 makes it unnecessary.

### F3. Make `frix-web` read-only in v1, which removes the need for dashboard authentication - Severity: Major
* **Reviewer Lens:** `egm-zoom-out` (cluster C2)
* **Affected Scope:** §2.3 L92; §6.4 L614 (writes, dashboard Confirmation, Initiator `operator`, CSRF); §9.1 L791-792; §9.3 L815; T9 L678; Phase 3 L896-899; ADR 0001 consequence 3.
* **Discrepancy Description**: critic_report B2 asks for an auth token or a Unix socket with peer credentials, plus out-of-band Confirmation from the web. That is a second authentication and confirmation stack for a companion UI whose stated value is visibility (O5 L41: "dispositivi, banda in tempo reale, Wi-Fi, ospiti e blocchi"). A read-only dashboard on loopback exposes nothing that an unauthenticated LAN client (§5.1 L441) or a same-user process (F1) cannot already read.
* **Rationale & Recommendation**:
  - Limit v1 `frix-web` to read-only pages: Overview, Hosts and Audit.
  - Drop CSRF, the forms and the web Confirmation.
  - Make ADR 0001 consequence 3 depend on a future ADR for web writes.
  - Delete "layout mobile" at L898.
  - Remove the write impact from T9 and keep XSS and CSP.

  This resolves critic_report B2, comprehension 6 and half of M12 without adding anything.

### F4. One write executor in core with an injected `Confirm` trait, and `call` stays plain RPC - Severity: Major
* **Reviewer Lens:** `egm-zoom-out` (cluster C3)
* **Affected Scope:** §6.2 L575-586 (the struct and the 7-step `call`), L592-598 (`policy::guard`); §6.1 L541-545, L556 (`confirm.rs`), L569 (dependency rule); §8.0 L720-723.
* **Discrepancy Description**: Comprehension 3 (who asks the Operator), critic_report M13 (step order), and the zoom-out and inventive findings on TOCTOU and audit order all come from the same cause: every step is pushed into `call`. The real `Tr064Client` already keeps only a Read gate (`policy::authorize` at `tr064/mod.rs` L150 and `policy/mod.rs` L94-95). That is the right minimal choke point.
* **Rationale & Recommendation**: Replace the Rust snippet (L577-584) and the step list (L586) with one paragraph that describes two paths.
  - **Writes** go through a typed write request to an executor in `frix-core`, which runs these steps in order:
    1. validate;
    2. resolve MAC to IP (`Active=1`, single IP, otherwise refuse; critic_report M15);
    3. read the current state;
    4. Preview;
    5. Lockout check;
    6. for Dangerous actions only, call the `Confirm` trait, which the front-end implements;
    7. resolve and read again, and abort if anything changed;
    8. write the audit `intent` entry, and refuse the write if that fails;
    9. make the SOAP call;
    10. read the new state (best effort);
    11. write the audit `result` entry.
  - **Reads** go straight to `call`.

  Do not add inventive F3's digest binding over the ChangeCounter: the second read after Confirmation gives the same guarantee. Each tool supplies its own state read, so the registry needs no generic Set-to-Get map.

### F5. Levels belong to tools; the registry only decides allowed or Forbidden - Severity: Major
* **Reviewer Lens:** `egm-zoom-out` (cluster C4)
* **Affected Scope:** §4.1 L139-144; the "Livello" columns in §4.2 (e.g. L252 "W su istanza 3 (ospiti), **D** su 1 e 2", L254 "W/D"); §6.2 L590; §8.0 L719 ("Ogni tool dichiara `level`"); §13.6 L1119.
* **Discrepancy Description**: Two places assign Levels, and they already conflict. `registry.rs` (L1-3, L10) ignores the instance and picks "il più restrittivo", so `SetEnable` is D on every instance. §8.2 L758 wants `set_guest_wifi` to be W. critic_report M1's fix, keying the registry by `(service, instance, action)`, only grows a structure that should go away. "Many actions marked Dangerous..." in zoom_out_report points the same way: many D entries are used by no tool.
* **Rationale & Recommendation**:
  - Make the registry an allowlist: an action is either used by a §8 tool or Forbidden. Keep the coverage test.
  - Make the Level a property of the §8 tool, and have the tool fix its own instance (`set_guest_wifi` always uses instance 3).
  - In §4.2, replace the Level columns with "usato da: <tool>" or Forbidden, and keep only the notes that are not obvious (e.g. L251).
  - Change L1119 to "il registro e la §8 sono la fonte di verità".

  This settles M1, N3, "W/D" and the per-instance key together.

### F6. Do not build shared state across processes; state what each process guarantees - Severity: Major
* **Reviewer Lens:** `egm-zoom-out` (cluster C5)
* **Affected Scope:** §2.3 L92 ("Un solo punto di controllo per le scritture"); §5.2 L472; §6.2 L597, L600; T8 L677; §8.3 L767.
* **Discrepancy Description**: After F3 and F7, the only processes that write are `frix-mcp` instances, one per client session.
  - **Audit integrity** needs only one `write(2)` per JSONL line on an `O_APPEND` descriptor.
  - **The rate limit** exists to stop a single Agent loop, so counting per process is the intended behaviour.
  - **No SID** is used in v1.

  A daemon would add a launchd lifecycle and a local socket to protect, with no proven need.
* **Rationale & Recommendation**:
  - Change L92 to "una sola Policy (in `frix-core`)".
  - At L597, keep one per-process write limit for the `agent` Initiator.
  - Drop "una sola `dangerous` ogni 60 s" and the 1-per-10-minutes limit on `reconnect_wan` (L767), since every Dangerous call already needs a click.
  - At L600, specify the single-write append.
  - Record in §14.2 that a daemon was considered and rejected.
  - Drop inventive F8's time-limited write windows.

### F7. Move the SID, AHA, REST, `data.lua`, craft-anything and resources tracks out of v1 - Severity: Major
* **Reviewer Lens:** `egm-zoom-out` (cluster C6)
* **Affected Scope:** §5.2-§5.6 L443-492; §6.1 `sid.rs` L530; §6.5 L629; §7.4 L700-703 (`ca` mode); §8.4 L778-781; §10 L820-865; Phase 4 item 2 L904; §14.2 items 5-6 L1235-1236.
* **Discrepancy Description**: No v1 tool in §8.1-§8.2 uses a SID. `*ListPath` goes through TR-064 (`X_AVM-DE_CreateUrlSID`, L356). Cutting these tracks also removes, at no cost:
  - critic_report M16's port 443 pinning;
  - M18;
  - N4;
  - N8;
  - the multi-SID part of M12.
* **Rationale & Recommendation**:
  - Collapse §5.2-§5.6 and §10 into a short "Rinviato" appendix with the links. Keep the `ENDPOINTS.md` format and the rule "TR-064 vince".
  - Make DevTools the plan instead of craft-anything.
  - Delete the multi-router clause (L1236) and `tls.mode = "ca"`.
  - Delete the §8.4 resources and prompts, or set them aside.

### F8. Cut the v1 Dangerous catalogue until a Confirmation the Agent cannot script is proven - Severity: Major
* **Reviewer Lens:** `egm-zoom-out` (clusters C1, C3)
* **Affected Scope:** §8.3 L763-776; ADR 0001; Phase 4 L901-906; critic_report M17.
* **Discrepancy Description**: Three things about the native dialog are still unknown:
  - whether a stdio child process can show it;
  - whether the Agent can script it;
  - whether a 120 s wait fits within the MCP client's timeout.

  Three tools also bring risks that each need their own design:
  - `add_port_forwarding` causes MAC drift, because the router stores an IP and FrixMCP cannot set static leases (inventive F5).
  - `apply_firmware_update` can change the router's certificate and break the pin (L702, L774).
  - `start_wps` opens a pairing window for any nearby device (L773).
* **Rationale & Recommendation**:
  - Start Phase 4 with a Confirmation feasibility spike that has an explicit exit criterion.
  - Have ADR 0001 describe the property it needs (a channel the Agent cannot read or act on) and mark the mechanism [DA VERIFICARE].
  - Ship at most `reboot_router` and `reconnect_wan`, plus Wi-Fi enable and channel if Lockout covers them.
  - Make port forwarding, WPS and firmware apply Forbidden in v1. This removes inventive F5 and part of critic_report M14 without new code.

### F11. The document drifts from the code because it holds content that lives elsewhere - Severity: Major
* **Reviewer Lens:** `egm-zoom-out` (cluster C7)
* **Affected Scope:** header L5; §2.1 L57-63; §6.5 L616-638; §12 L914-959; §13 L963-1212; §14.2 items 1 and 3 (L1231, L1233).
* **Discrepancy Description**: `docs/guida-operator-fase1.md` already exists, so §12 duplicates it. §13 copies `Cargo.toml`, `deny.toml`, `SECURITY.md` and `CLAUDE.md`, and the real files have since changed (AGENTS.md, ureq, rmcp 3.10).
* **Rationale & Recommendation**:
  - Add a "Ultimo aggiornamento / stato" block and label §2.1 as a dated snapshot.
  - Replace §12 with a link to the operator guide.
  - Replace §13 with one line per file stating what that file must guarantee.
  - Cut §6.5 down to the selection rules.
  - Mark §14.2 items 1 and 3 as decided.

  This also covers comprehension 1 and critic_report M2, M3 and N9.

### F9. Lockout: define the guarantee and a rule per tool, not the lookup - Severity: Nit
* **Reviewer Lens:** `egm-zoom-out` (cluster C3)
* **Affected Scope:** §6.2 L596; §4.2.7 L316 (the rule is stated by IP, against ADR 0002); §8.2 L754, L758; §8.3 L769-771.
* **Discrepancy Description**: The Lockout rule is spread across free-text notes and identifies the Control Host by IP, which contradicts ADR 0002 (hosts are addressed by MAC).
* **Rationale & Recommendation**:
  - Define the Control Host as the set of router Host entries whose IP is one of the Control Host's local interface addresses (the simplest form of inventive F6 and critic_report M14).
  - Add a "Lockout" column to the §8 tables in place of the free-text notes.
  - Delete the `X_AVM-DE_RequestClient` detail.

  After F8, only five or six tools need a rule.

### F10. The objectives promise more than the reduced catalogue delivers - Severity: Nit
* **Reviewer Lens:** `egm-zoom-out` (cluster C1, wording)
* **Affected Scope:** L3 vs L50; O1 L37; O3 L39; O5 L41; T1 L670 ("cambia DNS").
* **Discrepancy Description**: The objectives (e.g. DNS changes and remote access) go beyond what F1, F3 and F8 leave in v1.
* **Rationale & Recommendation**:
  - Make O3 list exactly the §8.3 tools and add "il resto è Forbidden". Drop DNS and remote access.
  - Change L3 to "copre i casi d'uso quotidiani".
  - Have O5 use the 2-second criterion from L789.

  This covers critic_report M4, M11 and M19. Instead of a separate "Alternative considerate" section (M10), link the ADRs and §14.2, and add ADRs for the F3 and F6 decisions.

### F12. Fix the terms in one pass, after the content cuts - Severity: Nit
* **Reviewer Lens:** `egm-zoom-out` (cluster C8)
* **Affected Scope:** the locations listed in critic_report M5-M7 and N2.
* **Discrepancy Description**: The term fixes overlap with sections that F5, F7 and F11 delete or rewrite.
* **Rationale & Recommendation**: Do the terminology pass after F5, F7 and F11, which remove most of the affected lines (the §4.2 Level columns, §10, §12, §13). Rename "Livello" to "Classification" only where that column survives F5.

## What to do with each proposed addition (part a)

| Proposed addition | Decision |
| --- | --- |
| Touch ID / LocalAuthentication | Defer to the F8 spike |
| Keychain ACL with `-T` | Do not adopt: it cannot work with the `security` CLI (F2), and F1 makes it unnecessary |
| Flags the Agent cannot edit | Cut; the F1 deployment constraint replaces them |
| Hash-chained audit log | Cut; drop "append-only" (L1157, SECURITY.md L15) and say "tracciabilità, non prova di manomissione" |
| Dashboard auth token or Unix socket | Not needed after F3 |
| `Confirm` trait in core | Adopt (F4) |
| Audit entry before the write | Adopt (F4) |
| Digest binding against TOCTOU | Replace with a second read after Confirmation (F4) |
| `flock` or a daemon | Reject; use a single-write append instead (F6) |
| Control Host with several MACs | Adopt in its simplest form (F9) |
| Detecting MAC drift for port forwards | Not needed; port forwarding is Forbidden in v1 (F8) |
| Port 443 pinning | Not needed after F7 |
| Registry keyed by instance | Not needed after F5 |
| Time-limited write windows | Cut |

## Overall shape (part b)

The design still works. The read-only Phase 1 code already matches the slimmed-down design: the Read-only gate inside `call`, the registry coverage test, and no SID. Phase 2 adds one executor and one audit module. Phase 3 becomes a read-only dashboard. Phase 4 waits on the Confirmation spike. The only structural change is to move Level ownership from §4 to §8 (F5), and no new component is needed.

## Synthesizer note

There was a single reviewer instance, so no deduplication across instances was needed. Line numbers and code references are the reviewer's own; the synthesizer did not re-check them.

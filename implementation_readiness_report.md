# Goldfish Review Report

**Target:** `/Users/raresspatariu/FrixMCP/docs/PROJECT.md` (`DetailedRange, lines 869-1302`: §11 Roadmap a fasi, §12 Checklist operativa, §13 File suggeriti, §14 Rischi aperti, §15 Riferimenti)

**Stage:** 9 (Implementation Readiness Test). **Mode:** REVIEW.

**Baseline reading for every reviewer:** `GLOSSARY.md`, `docs/adr/0001-out-of-band-confirmation.md`, `docs/adr/0002-hosts-addressed-by-mac.md`. Phases 0 and 1 are already implemented, so the plan was compared with the repo, excluding `target/`.

## Reviewer Status

| Reviewer | Status |
| --- | --- |
| `egm-implementation-readiness` | completed |
| `egm-codebase-details` | completed |
| `egm-style` | completed |

**Verdict: NOT READY.** Two reviewers gave this verdict on their own: `egm-implementation-readiness` and `egm-codebase-details`. `egm-style` found no Blockers but did find several Majors. There are three themes:

1. **The plan is stale against the code.** §11 and §12 present Phases 0 and 1 as future work. §14.2 lists decisions as open that the code has already made. The §13 templates have drifted from the committed files, and some of them would break the build, CI or `cargo deny` if someone copied them.
2. **One Phase 2 acceptance criterion cannot be met** with the registry as built.
3. **Phases 2-4 leave design decisions to the implementer.** These include files, tests, the audit config, CLI flags, the Confirmation dialog, concurrency between the two processes, and code shared with `frix-web`.

**Severity breakdown (after deduplication):** 6 Blocker, 17 Major, 16 Nit.

Where the lenses rated the same issue differently, the consolidated severity is the highest one that the evidence supports. Each finding lists every lens that raised it.

---

## Prioritized Findings (Blockers -> Major -> Nits)

### B1. Phase 2 acceptance contradicts the existing registry (`WLANConfiguration:SetEnable` is Dangerous on every instance) - Severity: Blocker
* **Reviewer Lens:** `egm-implementation-readiness` (F1); reuse note from `egm-codebase-details` (R4)
* **Affected Scope:** Lines 889, 892
* **Discrepancy Description**: The Phase 2 acceptance requires `"spegni la rete ospiti" funziona e un set_guest_wifi su una istanza non-guest è impossibile per costruzione` with `--allow-writes` alone. The registry in `crates/frix-core/src/policy/registry.rs` is keyed by `(service, action)`, and line 89 classifies `("WLANConfiguration", "SetEnable", D)`. The comment explains why: the instance → Wi-Fi Network mapping is not verified yet. `docs/guida-operator-fase1.md` lines 125-128 also asks the Operator to confirm that classification. With today's code, `set_guest_wifi` needs `--allow-dangerous` plus a Confirmation, so the criterion fails. The roadmap does not say how the registry should change. It also does not make Phase 2 depend on the open question at line 1248 (`Quale istanza WLANConfiguration è quale`).
* **Rationale & Recommendation**: Add a Phase 2 step that specifies three things:
  - the registry change: `(service, instance, action)` keys or a tool-level override;
  - how the guest instance is resolved: `X_AVM-DE_GetWLANExtInfo` with `APType == "guest"`, reusing `WifiNetwork::Guest` and the AP-type mapping in `crates/frix-core/src/services/wlan.rs:24-43, 326-341`;
  - what happens when that check fails: deny with `FrixError::Policy`.

  Make line 1248 an explicit prerequisite of Phase 2. Name the test that proves "per costruzione", for example: a fixture where instance 3 has `APType=normal` → refused.

### B2. The §13.1 workspace `Cargo.toml` contradicts the real one - Severity: Blocker
* **Reviewer Lens:** `egm-codebase-details` (B1); `egm-implementation-readiness` (F5); `egm-style` (N4)
* **Affected Scope:** Lines 967-1031 (in particular 970, 977, 982, 991, 994, 997)
* **Discrepancy Description**: The template differs from `/Users/raresspatariu/FrixMCP/Cargo.toml` in these ways:
  - **Members:** they leave out `"xtask"` (real file line 3), yet line 1135 references `cargo xtask record`.
  - **HTTP client:** the template declares `reqwest 0.13`. The repo chose `ureq ~3.5.2` and records why at lines 37-42 (reqwest 83 crates vs ureq 21).
  - **`rmcp`:** `3.9` with default features in the template, `3.10.0` with `default-features = false` and `"macros"` in the repo (line 23).
  - **`tokio` and `rustls`:** the features are different (lines 25, 34).
  - **Missing from the template:** `ring`, `toml`, and the path+version entry for `frix-core`.
  - **In the template but not in the repo:** `tracing`, `tracing-subscriber`, `keyring`, `pbkdf2`, `sha2`, `hmac`, `clap`, `axum`, `tower-http`, `askama`, `anyhow`, `insta`.
  - **Rule violation:** the snippet has no `# perché:` lines, which breaks the document's own rule at line 1129.
  - **Stale notes:** the `# DA VERIFICARE` notes at 977, 982 and 991 are now resolved.
* **Rationale & Recommendation**: §13 reads as copy-paste templates, so anyone who copies this one regresses the workspace. Replace §13.1 with the current file or a pointer to it. Move each future dependency (axum, askama, tower-http, tracing…) into the phase that introduces it, with an exact version, features and a `# perché:` line.

### B3. The §13.3 `deny.toml` would fail `cargo deny check` on the current workspace - Severity: Blocker
* **Reviewer Lens:** `egm-codebase-details` (B2); `egm-implementation-readiness` (F5)
* **Affected Scope:** Lines 1042-1070
* **Discrepancy Description**: The template lacks four settings that the real `/Users/raresspatariu/FrixMCP/deny.toml` has:
  - `allow-wildcard-paths = true` (line 24). Without it, `xtask/Cargo.toml:24` `frix-mcp = { path = "../crates/frix-mcp" }`, which has no version, trips `wildcards = "deny"`.
  - `unused-allowed-license = "allow"` (line 14).
  - `[licenses.private] ignore = true` (lines 16-18).
  - The `openssl-sys` ban (line 27).
* **Rationale & Recommendation**: Sync §13.3 with the real file, and record why `allow-wildcard-paths` is there.

### B4. The live-test command points to the wrong crate - Severity: Blocker
* **Reviewer Lens:** `egm-codebase-details` (B3); `egm-implementation-readiness` (F7, F12)
* **Affected Scope:** Lines 885, 1134
* **Discrepancy Description**: Line 1134 says `cargo test -p frix-core --features live -- --ignored`, but frix-core has no `[features]` section, so the command fails with an unknown-feature error. The `live` feature exists only in `xtask/Cargo.toml:14-17`. The real command, from `xtask/tests/live.rs:10` and `docs/guida-operator-fase1.md:61`, is `cargo test -p xtask --features live --test live -- --ignored --nocapture`. Line 885 (`il test di integrazione --features live`) does not name the package or the test target. The template in §13.6 is meant as rules for coding agents, so they will run this wrong command as written.
* **Rationale & Recommendation**: Use the exact command at both lines.

### B5. The §13.10 CI template does not match `ci.yml` and would not run on `macos-latest` - Severity: Blocker
* **Reviewer Lens:** `egm-codebase-details` (B4); `egm-implementation-readiness` (F6, Major); `egm-style` (N10)
* **Affected Scope:** Lines 1195-1212
* **Discrepancy Description**:
  - **cargo-deny:** `EmbarkStudios/cargo-deny-action@v2` runs in Docker and does not work on macOS. The real `.github/workflows/ci.yml` says so at line 36 and uses `taiki-e/install-action` pinned to a SHA with `cargo-deny@0.20.2` instead (lines 37-40).
  - **Toolchain:** `dtolnay/rust-toolchain@stable` contradicts the `1.99.1` pin. The real workflow runs `rustup toolchain install`, which reads `rust-toolchain.toml`.
  - **Action pinning:** the real workflow pins actions to a SHA (`actions/checkout@3d3c42e… # v7.0.1`) and sets `persist-credentials: false`.
  - **Other missing settings:** the weekly `schedule`, `permissions: contents: read`, and `fail-fast: false`.
  - **Stale note:** the `DA VERIFICARE: azione pinnata a commit` note at line 1206 is resolved in the real file.
* **Rationale & Recommendation**: Replace §13.10 with the real workflow or a pointer to it, and remove the resolved note.

### B6. `CLAUDE.md` does not exist, and the §13.6 rules contradict the code - Severity: Blocker
* **Reviewer Lens:** `egm-codebase-details` (B5); `egm-implementation-readiness` (F7, Major)
* **Affected Scope:** Lines 943, 1112-1140 (and Phase 0 at 875-876)
* **Discrepancy Description**: Phase 0 and §12 step 7 assume `CLAUDE.md` exists, but there is none in the repo. `AGENTS.md` exists, and its content is unrelated (issue tracker, triage labels, domain docs), so the "Regole non negoziabili" are not anywhere an agent will read them. The template also contradicts the code in three places:
  - **Line 1127, `log solo su stderr via tracing`:** tracing is deliberately absent (`crates/frix-mcp/src/main.rs:14-17`).
  - **Line 1134:** the wrong command (see B4).
  - **Line 1135, `scrive in tests/fixtures/raw/, poi oscura`:** the tool actually writes `raw/grezze/` and `raw/oscurate/`, and moving files into `tests/fixtures/` is a separate manual step (`xtask/src/main.rs:12-17`; guida lines 79-80, 121).
* **Rationale & Recommendation**: Decide where the agent rules live (CLAUDE.md, AGENTS.md, or both) and make that a checklist item. Fix the three rules.

---

### M1. The roadmap has no status; Phases 0 and 1 are presented as future work - Severity: Major
* **Reviewer Lens:** `egm-implementation-readiness` (F2); `egm-codebase-details` (M4)
* **Affected Scope:** Lines 873-885, 959
* **Discrepancy Description**: Phase 0 is done (commits 120f1e3 and 073aba7). Phase 1 is mostly done:
  - all 15 tools from §8.1 exist;
  - 35 SCPD are committed;
  - the coverage test `ogni_azione_degli_scpd_del_7590_ha_una_classification` exists.

  Two Phase 1 items remain: the recorded and redacted fixtures, and the live test. Both depend on the Operator steps in `docs/guida-operator-fase1.md` (issues #3, #9, #15), and line 1247 notes that the list fixtures are still hand-written. Lines 876 (`git log ha un commit`) and 959 (`Iniziare la Fase 1`) are stale. A new implementer cannot tell where to start.
* **Rationale & Recommendation**: Give each phase a status (done / in progress / todo) and link the issues. List what remains in Phase 1, and state that Phase 1 closes only after the steps in the Operator guide.

### M2. §12 is an outdated bootstrap script that fails if re-run, and it duplicates the Operator guide - Severity: Major
* **Reviewer Lens:** `egm-implementation-readiness` (F3); `egm-codebase-details` (M4); `egm-style` (M5)
* **Affected Scope:** Lines 914, 940-947, 959
* **Discrepancy Description**:
  - **Stale title:** `Checklist operativa per domani mattina` is tied to the day it was written.
  - **Step 7 would fail:** it runs `git init -b main` and `cargo new` for three crates. Those crates exist now, so `cargo new` fails, and the step leaves out `xtask` and `.cargo/config.toml`.
  - **Two procedures disagree:** the router steps (2-6, 9-10) are repeated in `docs/guida-operator-fase1.md` with different details: `-ext subjectAltName`, which §14.3:1250 needs, `hide_serial_number`, and the 401 probe.
* **Rationale & Recommendation**: Retitle the section (for example, "Checklist operativa iniziale"). Mark steps 1, 7, 8 and 11 as done or historical. Point to the Operator guide as the single current procedure, or merge the two.

### M3. §14.2 and §14.3 list decisions as open that the code has already made - Severity: Major
* **Reviewer Lens:** `egm-implementation-readiness` (F4); `egm-codebase-details` (M2, M3); `egm-style` (N7)
* **Affected Scope:** Lines 1231, 1232, 1233, 1236, 1255
* **Discrepancy Description**:
  - **Line 1231 (reqwest vs ureq):** decided in favour of ureq in `spawn_blocking` (`Cargo.toml:37-42`). The threshold `~80 dipendenze` is also imprecise.
  - **Line 1232 (default TLS mode):** `pin` is already the default (`config.rs:11, 47`). Only the SAN question remains open.
  - **Line 1233 (keyring vs `security`):** decided in favour of `/usr/bin/security` (`secrets.rs:4-10, 22`).
  - **Line 1236 (`Config può già essere una lista per non chiudere la porta`):** the opposite is implemented. `RouterConfig` is a single struct with `deny_unknown_fields` (`config.rs:80-88`).
  - **Line 1255 (rmcp 3.9 MSRV, annotations, `instructions`):** resolved. The repo uses 3.10.0 with MSRV 1.88 (`Cargo.toml:10-11`), and `instructions` is passed through `#[tool_handler]` (`server.rs:54-58`).

  The document calls itself the source of truth (line 1119), so a reader could reopen these decisions or reintroduce reqwest or keyring.
* **Rationale & Recommendation**: Mark each item `deciso il 2026-11-13`, with the outcome and a link to the code, in the same style as items 7 and 10.

### M4. The licence decision gate has passed, and the document and code disagree on it - Severity: Major
* **Reviewer Lens:** `egm-implementation-readiness` (F13); `egm-style` (M5); `egm-codebase-details` (N4)
* **Affected Scope:** Lines 948, 1193, 1227
* **Discrepancy Description**: The text says `Decisione da prendere prima del primo push`, but commits are on `main` and `LICENSE-MIT` and `LICENSE-APACHE` are committed (both match §13.9). `Cargo.toml:8` moves the gate to `prima di rendere pubblico il repo`. Step 8 offers `--public` or `--private` without saying which was chosen.
* **Rationale & Recommendation**: Record the decision (MIT OR Apache-2.0, already applied) with a date, or restate the gate so it matches `Cargo.toml`.

### M5. The §13.4 `.gitignore` would hide `.cargo/config.toml` and break `cargo xtask` - Severity: Major
* **Reviewer Lens:** `egm-codebase-details` (M1); `egm-implementation-readiness` (F5)
* **Affected Scope:** Lines 1072-1094
* **Discrepancy Description**: The bare `config.toml` pattern at line 1079 also ignores `.cargo/config.toml`, which holds the `xtask` alias. The real `.gitignore` has `!/.cargo/config.toml` (lines 6-7) and also ignores `*.crt`, `*.key`, `*.p12` and `*.jsonl`.
* **Rationale & Recommendation**: Sync the template with the real file, and record why the exception is needed.

### M6. The §13.5 `.mcp.json.example` hard-codes a personal path and sets a `RUST_LOG` that does nothing - Severity: Major
* **Reviewer Lens:** `egm-codebase-details` (M9); `egm-implementation-readiness` (F5)
* **Affected Scope:** Lines 1096-1108
* **Discrepancy Description**: The template contains `/Users/raresspatariu/...`. The committed file uses placeholders, which is right for a public repo. `"RUST_LOG": "frix=info"` has no effect because tracing does not exist yet.
* **Rationale & Recommendation**: Use the placeholders. Either note that RUST_LOG only takes effect once tracing is introduced, or remove it.

### M7. The audit configuration is undecided, and `ConfigFile` would reject it - Severity: Major
* **Reviewer Lens:** `egm-implementation-readiness` (F8); `egm-codebase-details` (M7)
* **Affected Scope:** Lines 889, 957
* **Discrepancy Description**: Three parts of the document describe the audit path differently:
  - Line 957: `la sezione [audit] arriverà con le scritture della Fase 2`.
  - §6.1 (line 553): an `--audit-path` flag.
  - §6.2 (line 600): a fixed path.

  Phase 2 does not choose between them or name the keys. `ConfigFile` and `TlsSection` use `#[serde(deny_unknown_fields)]` (`config.rs:28, 45`), so Phase 2 must modify `config.rs`, and the plan does not say so.
* **Rationale & Recommendation**: In Phase 2, state the source of the audit path (flag, `[audit] path`, or fixed default), the exact keys, the precedence, and the file mode 0600. List `config.rs` as a file to modify.

### M8. The CLI flags in the document do not match the hand-written `frix-mcp` parser - Severity: Major
* **Reviewer Lens:** `egm-codebase-details` (M6); `egm-implementation-readiness` (F8, F14)
* **Affected Scope:** Lines 892, 997, 1110, 1156, 1171, 1182
* **Discrepancy Description**: `crates/frix-mcp/src/cli.rs` is hand-written with no clap (lines 1-4) and rejects unknown flags. It accepts only `--config --host --username --http-port --tls-mode --tls-pin --tls-ca-file --help`. `--allow-writes` and `--allow-dangerous` do not exist yet, and Phase 2 does not say how to add them. `--list-tools`, used at line 1182 for the README table, is assigned to no phase, and nothing says which channel it writes to (stdout is the MCP channel). §13.1 and §6.1 still assume clap.
* **Rationale & Recommendation**: In Phase 2, state that the new flags go into `cli::parse` and `USAGE` without clap, or decide to adopt clap. Assign `--list-tools` to a phase, make it write to stderr or a file, or generate the table with an xtask command. If the hand-written parser stays, remove clap from §13.1.

### M9. Phases 2-5 have no file list, no named tests and no dependency choices - Severity: Major
* **Reviewer Lens:** `egm-implementation-readiness` (F9); `egm-codebase-details` (M8)
* **Affected Scope:** Lines 887-910
* **Discrepancy Description**: The phase items are summaries, for example `policy::guard (flag, Lockout check, rate limit, Preview via dry_run)`. No phase has a Create/Modify/Delete list. Expected files include `policy/guard.rs`, `policy/audit.rs`, `validate.rs`, `crates/frix-mcp/src/confirm.rs`, the new `tools/*.rs`, plus changes to `policy/mod.rs`, `config.rs`, `cli.rs` and `server.rs`. The tests are not named either:
  - Lockout tests for each of the three Control Host cases in §6.2 (line 596);
  - a rate-limit boundary test (6/min, one Dangerous every 60 s);
  - a test that `passphrase` is redacted in the audit.

  Several choices are left to the implementer:
  - which time crate to use for Audit Entry timestamps;
  - the rate-limiter design;
  - `frix-web` CSRF, SSE and the port flag;
  - whether Phase 5's `cargo audit settimanale` is still needed, given that CI already runs `cargo deny check` (advisories) every week (`ci.yml:6-9, 40`).
* **Rationale & Recommendation**: For each phase, add a file list and named tests, including edge cases, and record each of these decisions.

### M10. Phase 4 leaves the Confirmation dialog mechanism and the 866/2FA behaviour undefined - Severity: Major
* **Reviewer Lens:** `egm-implementation-readiness` (F10); `egm-style` (N7, N8)
* **Affected Scope:** Lines 903-906
* **Discrepancy Description**:
  - **Confirmation dialog:** `Confirmation fuori banda (ADR 0001)` does not say how `frix-mcp` shows a native macOS dialog (osascript, a Swift helper, or a crate). It also does not say how this is tested on ubuntu-latest CI, or what happens when no GUI session exists.
  - **866/2FA:** `gestione 866 (2FA)` does not say what the Agent gets back, whether there is a retry, or how it interacts with the 120 s timeout. The code `866` is not explained in the text.
  - **Vague criterion:** `se giustificato` (line 904) has no threshold.
  - **Optional items:** the AHA/REST and DECT/Homeplug items have no acceptance criterion.
* **Rationale & Recommendation**: Choose the dialog mechanism in an ADR or §6.3, and define a trait seam so CI can use a fake confirmer. Specify the 866 flow (`FrixError::SecondFactorRequired` already exists in `error.rs:45`). Replace `se giustificato` with a measurable criterion, for example "an endpoint needed by a planned tool that TR-064 does not cover".

### M11. Phase 3 does not design how two processes share the audit and rate-limit state - Severity: Major
* **Reviewer Lens:** `egm-implementation-readiness` (F11)
* **Affected Scope:** Lines 896-899
* **Discrepancy Description**: `stessa policy e stessi flag di frix-mcp, Audit Entry con Initiator operator` means `frix-mcp` and `frix-web` can run at the same time against one JSONL audit file. The plan does not say how concurrent appends are made safe (locking or `O_APPEND`). It also does not say whether the limit of one Dangerous every 60 s is per process; if it is, running both processes doubles it. `Accessibilità e layout mobile` has no testable criterion.
* **Rationale & Recommendation**: Specify how concurrent appends are handled and whether the rate limit is shared, for example derived from the audit file. Make the accessibility check concrete, or drop it from the acceptance criteria.

### M12. The Phase 1 acceptance criteria cannot be checked - Severity: Major
* **Reviewer Lens:** `egm-style` (M1); `egm-implementation-readiness` (F12); `egm-codebase-details` (N6)
* **Affected Scope:** Line 885
* **Discrepancy Description**: `risponde correttamente` has no oracle to compare against. `l'audit è vuoto (nessuna scrittura possibile)` refers to an audit that only arrives in Phase 2 (lines 889, 957), so the clause is vacuous.
* **Rationale & Recommendation**: Use checks that can be run. For example, `list_hosts(active_only=true)` matches the active hosts in the AVM UI, and `get_bandwidth` returns non-zero values during a download. Replace the audit clause with "nessun tool con Level ≠ Read in `tools/list`" (`policy::authorize`, `policy/mod.rs:94-95`).

### M13. The §12 step 9 config example omits keys that `ConfigFile` accepts - Severity: Major
* **Reviewer Lens:** `egm-codebase-details` (M5)
* **Affected Scope:** Lines 925, 949-957
* **Discrepancy Description**: The example shows only `host`, `username`, `[tls] mode` and `pinned_sha256`. `config.rs:29-53` also accepts `http_port`, `hide_serial_number` and `[tls] ca_file`, which is relative to the config file's folder. Step 4 creates `router.der` but never connects it to `mode = "ca"` + `ca_file`.
* **Rationale & Recommendation**: List every accepted key, and connect `router.der` to `ca_file`.

### M14. `frix-web` cannot reuse the config and startup code in `frix-mcp` - Severity: Major
* **Reviewer Lens:** `egm-codebase-details` (R1, R6)
* **Affected Scope:** Lines 896-899
* **Discrepancy Description**: The config/flag merging (`cli::load`), `startup::connect`/`banner` and `sanitize.rs` (which is `pub(crate)`) all live in `frix-mcp`. The test `crates/frix-core/tests/regola_dipendenze.rs:43-48` forbids `frix-web` from depending on `frix-mcp`. Phase 3 says nothing about this, so an implementer will either duplicate the code or break the dependency rule.
* **Rationale & Recommendation**: Add a Phase 3 step that moves these into `frix-core` (for example `frix_core::startup` and the sanitization). Reuse `ConfigFile::parse/overridden_by/resolve` (`config.rs:90-144`) and `secrets::keychain_password`.

### M15. Glossary drift: "banda" used for throughput, and "Dispositivi" used for Host - Severity: Major
* **Reviewer Lens:** `egm-style` (M2)
* **Affected Scope:** Lines 896, 899
* **Discrepancy Description**: In `mostra banda in tempo reale`, "banda" means throughput, but the glossary defines Band as the radio frequency. `Dispositivi` is on the glossary's _Avoid_ list for Host, and it contradicts the §9 page table (line 800), which names the page "Host".
* **Rationale & Recommendation**: Use "traffico" or "throughput" at line 899, and name the page "Host" at line 896.

### M16. Glossary drift: Level names and "istanza" - Severity: Major
* **Reviewer Lens:** `egm-style` (M3)
* **Affected Scope:** Lines 892, 897, 901, 1110, 1119, 1125, 1126, 1156, 1171, 1237
* **Discrepancy Description**: The range mixes `dangerous`, "pericolosi"/"pericolose" and "livello/livelli" for the glossary term Level (Read/Write/Dangerous). `su una istanza non-guest` uses an avoided term, and the elision is also wrong ("un'istanza").
* **Rationale & Recommendation**: In prose, use Level and Read/Write/Dangerous, and keep backticked lowercase only for literal identifiers. At line 892, write "su una Wi-Fi Network diversa da Ospiti".

### M17. The README and SECURITY templates claim more than the design delivers - Severity: Major
* **Reviewer Lens:** `egm-style` (M4)
* **Affected Scope:** Lines 1169, 1171, 1172, 1173
* **Discrepancy Description**:
  - **Line 1169, `senza poter rompere la rete`:** absolute. Dangerous operations such as `reconnect_wan` still disconnect the network after Confirmation.
  - **Line 1171, `conferma esplicita`:** reads like the in-chat confirmation that ADR 0001 rejected.
  - **Line 1172, `nessuna porta esposta`:** `frix-web` listens on 127.0.0.1:7590.
  - **Line 1173, `probabilmente funziona, non garantito`:** an unsupported hedge, and the verb does not agree with its subject.
* **Rationale & Recommendation**:
  - 1169: "senza poter causare un Lockout del Control Host; le operazioni Dangerous richiedono la Confirmation dell'Operator".
  - 1171: "Confirmation fuori banda dell'Operator (dialog macOS)".
  - 1172: "nessuna porta esposta sulla rete (solo 127.0.0.1)".
  - 1173: "Altri modelli: non testati."

---

### N1. Phase 0 commands: syntax error, a tool that is never installed, and an unpinned version - Severity: Nit
* **Reviewer Lens:** `egm-implementation-readiness` (F15); `egm-codebase-details` (N9)
* **Affected Scope:** Lines 876, 918, 948
* **Discrepancy Description**:
  - `cargo clippy -D warnings` is missing `--`.
  - `gitleaks` is part of the acceptance criteria but is never installed.
  - The step 8 grep relies on human judgement and will match `secrets.rs` and `config.rs`.
  - `cargo install cargo-deny` has no version, while CI pins 0.20.2.
* **Rationale & Recommendation**: Use `cargo clippy --all-targets -- -D warnings`. Install `gitleaks` or drop it from the criteria. Narrow the grep. Use `cargo install cargo-deny --version 0.20.2 --locked`.

### N2. §15 has stale facts and a temporary location - Severity: Nit
* **Reviewer Lens:** `egm-implementation-readiness` (F17); `egm-codebase-details` (N7); `egm-style` (M5, N10)
* **Affected Scope:** Lines 1261, 1263, 1272
* **Discrepancy Description**:
  - `Verificati oggi` is tied to the day it was written.
  - `copie nella cartella scratch della sessione; da committare … in fase 1`: the SCPD are already committed in `crates/frix-core/tests/fixtures/scpd/`, and a scratch folder cannot be found later.
  - The versions list says rmcp 3.9.0 (actual 3.10.0), leaves out ureq 3.5.2, rustls 0.23.46, ring 0.17.14, toml 1.2.1 and serde_json 1.0.154, and lists reqwest and keyring, which are unused.
  - `in fase 1` should be capitalised: "in Fase 1".
* **Rationale & Recommendation**: Date-stamp the list as "Verificati il 2026-11-10", update the versions, and replace the scratch reference with the repo path.

### N3. The §13.7 SECURITY.md template has drifted, and its TLS statement is inconsistent - Severity: Nit
* **Reviewer Lens:** `egm-codebase-details` (N2); `egm-style` (N9, N5)
* **Affected Scope:** Lines 1144-1163
* **Discrepancy Description**:
  - The real `SECURITY.md` adds "Nella versione attuale esistono solo i tool `read`." (line 14) and a §7 pointer (line 18).
  - Line 1155, `certificato pinnato; nessuna opzione per disabilitare la verifica`, conflicts with line 1128 (pinning or CA) and with the `ca` mode.
  - `in produzione` (1154) is undefined for a local tool.
  - `Prendiamo sul serio i bug di sicurezza.` (1147) is filler.
* **Rationale & Recommendation**: Align the template with the file. Use "certificato pinnato o CA configurata". Drop the filler and the undefined qualifier.

### N4. The §13.8 README skeleton is outdated - Severity: Nit
* **Reviewer Lens:** `egm-codebase-details` (N3)
* **Affected Scope:** Lines 1167-1189
* **Discrepancy Description**: The real README has sections the template lacks: "Stato", "Struttura" and "Sviluppo". It also says `--list-tools` "quando esisterà" and uses a different licence text.
* **Rationale & Recommendation**: Point to the real file, or align the template with it.

### N5. The Phase 1 service list does not match the module names - Severity: Nit
* **Reviewer Lens:** `egm-codebase-details` (N5)
* **Affected Scope:** Line 881
* **Discrepancy Description**: The real modules in `crates/frix-core/src/services/` are `lan_host_config` (not "lan") and `wan_common`/`wan_connection`/`wan_dsl` (not "wan"). The list also leaves out `device_config`, `homeauto`, `host_filter`, `lan_config_security`, `list_path`, `time` and `user_interface`.
* **Rationale & Recommendation**: Use the real module names.

### N6. The lint rules in §13.6 do not match the lint config - Severity: Nit
* **Reviewer Lens:** `egm-style` (N3); `egm-codebase-details` (N8)
* **Affected Scope:** Lines 1017, 1031, 1122
* **Discrepancy Description**: `i lint sono deny` is not accurate, because `todo = "warn"` (line 1017) fails only through `-D warnings`. The test allow attribute in the repo also includes `clippy::indexing_slicing` (`config.rs:148-153`), which the template leaves out.
* **Rationale & Recommendation**: Set `todo = "deny"`, or reword the rule. Add `indexing_slicing` to the allowed lints for tests.

### N7. Router Account terminology - Severity: Nit
* **Reviewer Lens:** `egm-style` (N1)
* **Affected Scope:** Lines 875, 919, 939, 1176, 1252
* **Discrepancy Description**: The text uses `utente router`, `Utente dedicato`, `Se 401: utente/password` and `utente separato`. The glossary term is Router Account, and bare "utente" is avoided. The FRITZ!OS UI label "Utenti FRITZ!Box" is a UI string and can stay.
* **Rationale & Recommendation**: Use "Router Account".

### N8. "agente" and "persona" are ambiguous in the §13.6 rules - Severity: Nit
* **Reviewer Lens:** `egm-style` (N2)
* **Affected Scope:** Line 1138
* **Discrepancy Description**: "Agent" is a glossary term for the AI that uses FrixMCP over MCP, but here it means the coding AI that works in the repo. `dalla persona` should be "dall'Operator".
* **Rationale & Recommendation**: Write "dall'IA che lavora nel repo" and "dall'Operator".

### N9. Filler and hedging - Severity: Nit
* **Reviewer Lens:** `egm-style` (N5)
* **Affected Scope:** Lines 920, 1130, 1252
* **Discrepancy Description**:
  - Line 920 repeats `(di fabbrica è attivo)` and adds a hedge (`quasi certamente`).
  - `Le idee sì, il codice no.` (1130) repeats the previous sentence.
  - `davvero` (1252) is filler.
* **Rationale & Recommendation**: Drop all three.

### N10. Subjective judgements in the decision tables - Severity: Nit
* **Reviewer Lens:** `egm-style` (N6)
* **Affected Scope:** Lines 1193, 1224, 1227, 1233
* **Discrepancy Description**: The tables use subjective wording instead of criteria: `la massima semplicità`, `protezione da contributori ostili`, the question-and-answer cell at 1224, `AGPL discutibile`, `possibile ma sporco`, and `Il crate è più pulito`.
* **Rationale & Recommendation**: Replace each one with a factual criterion. For example, write that changing the licence later "richiede il consenso di ogni contributore".

### N11. The Phase 5 regression procedure is vague, and some of its items are already done - Severity: Nit
* **Reviewer Lens:** `egm-implementation-readiness` (F16); `egm-style` (N7); `egm-codebase-details` (R7)
* **Affected Scope:** Line 910
* **Discrepancy Description**: `ri-registrare gli SCPD e confrontare` and `un diff negli SCPD è un segnale da leggere` name no command and no action. SECURITY.md and LICENSE already exist.
* **Rationale & Recommendation**: Spell out the procedure: `cargo xtask record` → diff `raw/oscurate/scpd/` against `tests/fixtures/scpd/` → review → `cargo test`, with the coverage test as the gate. Mark SECURITY.md and LICENSE as done.

### N12. Codes and short forms not explained in the text - Severity: Nit
* **Reviewer Lens:** `egm-style` (N8)
* **Affected Scope:** Lines 892, 899
* **Discrepancy Description**: `fatta dalla web` is missing a word ("dalla web UI"). `funziona` (892) names no observable effect.
* **Rationale & Recommendation**: Write "dalla web UI". For 892, give the observable effect, for example "la Wi-Fi Network Ospiti risulta disabilitata in `get_wifi_status`".

### N13. Italian elision and formatting - Severity: Nit
* **Reviewer Lens:** `egm-style` (N10)
* **Affected Scope:** Lines 897, 906, 1205-1206
* **Discrepancy Description**: `una Audit Entry` should be "un'Audit Entry". The note at 1206 is phrased as a check to perform (`DA VERIFICARE`) when it is really a decision, and it covers only one action.
* **Rationale & Recommendation**: Fix the elision. Reword the note as a TODO that covers every action, or delete it after applying B5.

### N14. The 503 retry description could be more precise - Severity: Nit
* **Reviewer Lens:** `egm-codebase-details` (N10)
* **Affected Scope:** Line 1245
* **Discrepancy Description**: The line matches `transport/router.rs:9-15, 38-42, 348-360` but leaves out the retry bound.
* **Rationale & Recommendation**: Write "ritenta i 503 al massimo `MAX_RETRIES` volte, rispettando `Retry-After` fino a 60 s".

### N15. Reuse for Phase 4 SID login and 2FA - Severity: Nit
* **Reviewer Lens:** `egm-codebase-details` (R2)
* **Affected Scope:** Lines 903, 992-994
* **Discrepancy Description**: The plan would add `pbkdf2`, `sha2` and `hmac`, but the workspace already has `ring` (PBKDF2, HMAC, SHA-256) and `md-5`. `FrixError::SecondFactorRequired` and the hints for 866-868 (`crates/frix-mcp/src/response.rs:6-7`) already exist.
* **Rationale & Recommendation**: Reuse `ring` instead of adding new crypto dependencies. This also removes the `DA VERIFICARE` note about versions.

### N16. Reuse for the Phase 2 and Phase 4 policy, validation and tools - Severity: Nit
* **Reviewer Lens:** `egm-codebase-details` (R3, R4, R5, R6)
* **Affected Scope:** Lines 889-892, 906
* **Discrepancy Description**: The plan does not name existing building blocks that later phases should extend rather than rebuild:
  - **Policy:** `Level`, `Classification`, `classification()`, `required_rights()` and `authorize()` in `policy/mod.rs:10-95`; `authorize` is the gate to extend. `FrixError::Policy` and `FrixError::Validation`.
  - **Control Host:** `hosts::find`/`resolve_ip` and the `Mac` type (ADR 0002).
  - **Subnet checks:** `lan_host_config::lan_config`.
  - **Port forwarding and `reconnect_wan`:** `wan_connection::port_mappings`/`active_kind`.
  - **Pre-request rejection tests:** `FixtureTransport` + `RequestLog`.
  - **Tool replies:** `response::from_result` for every new tool.
  - **Audit:** `serde_json` and `secrecy::SecretString` for redaction.
* **Rationale & Recommendation**: Name these in the steps of each phase.

---

## Verified Consistent (no action)

* §13.2 `rust-toolchain.toml` (lines 1035-1040) matches the repo exactly. §13.9 LICENSE matches `LICENSE-MIT`/`LICENSE-APACHE`. (`egm-codebase-details`)
* Line 1247 (the hand-written `wlandevicelist.xml` and the fallback by index) matches the code. (`egm-codebase-details`)
* Lines 903, 906, 1156 and 1237 follow ADR 0001. Line 906 follows ADR 0002. Control Host, Preview, Confirmation, Initiator, Lockout and Reversibility are used in line with the glossary. (`egm-style`)

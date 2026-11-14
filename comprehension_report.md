# Comprehension Test: `docs/PROJECT.md`

**Stage:** 6 (Comprehension Test) · **Reviewer:** `egm-comprehension` (completed) · **FeatureRange:** 1-868 · **DetailedRange:** 869-1302

The reviewer read only the document. No standard and no context files were supplied.

---

### FeatureRange (lines 1-868, §1-§10)

**What this proposes**

FrixMCP is a Rust Cargo workspace made of three crates:
- `frix-core` is a typed TR-064/SOAP client for one specific home FRITZ!Box 7590 on FRITZ!OS 8.25. It holds the safety policy, and an optional SID web-login module.
- `frix-mcp` is a stdio-only MCP server. It exposes router functions to an AI as single-purpose tools.
- `frix-web` is a dashboard bound to loopback only, and it replaces everyday use of the AVM UI.

Every TR-064 action on the router has a static classification in the binary: `read` is always on, `write` needs `--allow-writes` plus an audit entry, `dangerous` needs `--allow-dangerous` plus an out-of-band human confirmation through a native macOS dialog, and `Forbidden` actions are not compiled in at all. The document's argument is that the router's own permission model cannot give real least privilege, because nearly every read needs the admin-level "C" right. So the safety boundary is the policy in `frix-core`. Supporting measures are:
- credentials stored in the macOS Keychain
- TLS with a pinned self-signed certificate
- a check that refuses writes that would lock out the Control Host
- `dry_run` previews
- a JSONL audit log with before and after state
- router strings treated as untrusted data, to resist prompt injection

Undocumented UI endpoints (`data.lua`/`query.lua`) are to be observed with craft-anything or DevTools, and used only for gaps TR-064 cannot fill, behind an unstable feature flag.

**How the existing system works, per this document**

The router answers TR-064 on HTTP port 49000 (for discovery and SCPDs) and HTTPS port 49443 (for authenticated calls), using HTTP Digest auth. Some actions answer with no authentication at all, so anyone on the LAN can already read the host list. Lists come back as `*ListPath` URLs that point at Lua scripts. The web UI uses a separate PBKDF2 challenge/response login that produces a SID. That SID is also needed for AHA-HTTP, the Smart Home REST API and `data.lua`. Two existing FRITZ!Box MCP servers, kambriso and ghbalf, were rejected: kambriso cannot be audited, and ghbalf has plaintext credentials and no gate on writes. fritzconnection (Python) is used only as a reference. Section 2.1 says that today the repo folder is empty apart from this document and that no router user exists yet. Other parts of the document contradict this (see finding 1).

### DetailedRange (lines 869-1302, §11-§15)

**What this proposes**

The roadmap has six phases:
- Phase 0: setup. Create the router user, the Keychain entry, the repo, and get CI green.
- Phase 1: read-only `frix-core` and `frix-mcp`, with every SCPD action classified and checked by a coverage test.
- Phase 2: low-risk writes, together with the guard, audit, Lockout check and rate limit.
- Phase 3: `frix-web`.
- Phase 4: dangerous tools with Confirmation, the UI-analysis session, optional AHA/REST.
- Phase 5: publication.

Each phase has concrete acceptance criteria. An ordered checklist covers creating the dedicated FRITZ!OS user, reading the certificate fingerprint, storing the password in the Keychain, a manual Digest test with curl, scaffolding the repo, local config and wiring into Claude Code. Sections 13-15 give suggested file contents (Cargo.toml with deny-level lints, deny.toml, .gitignore, .mcp.json.example, CLAUDE.md, SECURITY.md, README, a CI workflow), list open decisions (licence, reqwest vs ureq, pin vs CA, keyring vs the `security` CLI) and router facts still to verify, and give source links.

**How the existing system works, per this document**

This range describes the same router facts as §4-§5. However, §14.3 also describes `frix-core` code that already exists (`transport/router.rs`, a hand-written fixture `lists/wlandevicelist.xml`, current retry behaviour), and §12 step 9 describes a config loader that already rejects unknown keys. From the document alone I cannot tell how much has been built.

---

## Findings

### 1. 🟡 Major: the document contradicts itself about what exists today
- **Affected scope:** lines 5, 59, 914 ("per domani mattina"), 957, 1245, 1247; §6.1 layout.
- **Gap:** Line 5 says the document was written "il 2026-11-10 prima della prima riga di codice", and line 59 says the folder is "**vuota** tranne questo documento". Later text assumes code exists:
  - line 1245: "`frix-core` oggi non ritenta dopo un rifiuto e rispetta solo i 503 (`transport/router.rs`)"
  - line 1247: "`frix-core` presume quelli di `GetGenericAssociatedDeviceInfo` … (fixture scritta a mano `lists/wlandevicelist.xml`)"
  - line 957: "Le chiavi sconosciute sono un errore: la sezione `[audit]` arriverà con le scritture della Fase 2 e oggi non va scritta"

  `transport/router.rs` does not appear in the §6.1 layout (which lists only `mod.rs`, `tls.rs`, `digest.rs`), and the decisions table has entries dated 2026-11-12. The §12 title "domani mattina" is a relative date that is now stale.
- **Rationale:** A new reader cannot tell whether this is a plan or a description of a partly built system. They cannot tell which phase is done, or whether §6.1 or the code is the source of truth.

### 2. 🟡 Major: key project vocabulary is delegated to GLOSSARY.md, which is not supplied and has no stated location
- **Affected scope:** lines 88, 143-144, 596, 600, 614, 722-723, 754, 758, 768-769, 1119, 1240.
- **Gap:** Several capitalised terms carry meaning in the safety model but are never defined in this document: Operator, Agent, Control Host, Lockout, Confirmation, Reversibility (values Reversible/Irreversible/"—"), Preview, Audit Entry, Initiator, Wi-Fi Network, Level, Classification. Each points to "vedi `GLOSSARY.md`". That file is missing from the §6.1 workspace layout, and line 1119 refers to it as a bare `GLOSSARY.md`, so its path is unclear. Some terms can be inferred (Control Host from line 792, Lockout partly from line 768, Level and Classification from line 542), but Lockout is the guard that §7.2 T1 and the Phase 2 acceptance criteria depend on, and line 596 gives only examples of what the check rejects, not the rule.
- **Rationale:** This is a self-containment failure on security-critical concepts. A one-line inline definition per term, or a short local glossary, would fix it.

### 3. 🟡 Major: no explanation of how frix-core obtains a Confirmation that is shown by frix-mcp or frix-web
- **Affected scope:** lines 569, 586 step (2), 595, 614, 723; `confirm.rs` (line 556).
- **Gap:** `Tr064Client::call` asks the `Policy` to "verifica flag/conferma" (line 586), and `policy::guard` checks "Confirmation dell'Operator" (line 595). Both live in `frix-core`. The dialog itself is in `frix-mcp/src/confirm.rs`, and in `frix-web` it is "il dialog della UI". The dependency rule (line 569) says `frix-core` depends on neither crate. The document never says how the core gets a yes or no from the front-end: through a callback or trait the front-ends inject, or by the front-ends confirming before they call the core.
- **Rationale:** This is the central safety mechanism for `dangerous` actions, and a reader cannot reconstruct the control flow. The guard could be bypassed if the mechanism is ambiguous.

### 4. 🟢 Nit: the service counts do not add up
- **Affected scope:** lines 62, 128, 146-186.
- **Gap:** Line 62 says "37 servizi (39 voci contando `WLANConfiguration:1/2/3` separatamente)". The table at lines 150-186 already lists the three WLAN instances separately and has exactly 37 rows, and line 128 says 35 SCPD files.
- **Rationale:** Phase 1 acceptance depends on "ogni azione dei 35 SCPD classificata" (line 882).

### 5. 🟢 Nit: the meaning of slash-separated rights is not stated
- **Affected scope:** §4.1 legend (lines 130-137); "Diritto" columns such as "C/A/P".
- **Gap:** It is never said whether "C/A/P" means any one right suffices or all are required. Line 392 implies "any one".
- **Rationale:** This drives the user-rights decision in §4.3 and §7.5.

### 6. 🟢 Nit: phase and screen mismatches between §9 and §11
- **Affected scope:** lines 799-807 vs 896-897; lines 614, 897 vs 903.
- **Gap:** Phase 3 lists "Dispositivi", but §9.2 calls that screen "Host". Phase 3 omits the "Blocchi e profili" and "Smart Home" screens. Phase 3 requires "Confirmation per i `dangerous`" in the web UI, while dangerous tools and out-of-band Confirmation arrive only in Phase 4.
- **Rationale:** A reader cannot tell what Phase 3 delivers.

### 7. 🟢 Nit: assorted undefined references
- Line 82: the rule "Repos never share code" is undefined. Whose rule is it?
- Line 596: `X_AVM-DE_RequestClient` does not appear in the Hosts action table.
- Lines 325-326, 759: "AIN" is never expanded.
- "SCPD" is never expanded.
- Line 1103: `--config` is used in `.mcp.json.example`, but the CLI flags at line 553 list only `--allow-writes --allow-dangerous --audit-path`.

### 8. 🟢 Nit: "utente" is ambiguous
- **Affected scope:** e.g. lines 394, 474, 724, 1150.
- **Gap:** "utente" sometimes means the human (elsewhere "persona" or "Operator") and sometimes the dedicated FRITZ!OS account.
- **Rationale:** In the security sections this makes it unclear who must hold a right or receive a message.

## Overall

No Blockers: both ranges could be paraphrased. Three Major gaps: what exists today versus what is planned (1), reliance on an unsupplied GLOSSARY.md for the safety-model terms (2), and the unexplained Confirmation control flow across crates (3).

Note from the orchestrator: `GLOSSARY.md` exists at the repo root. It was withheld from this stage on purpose, because the test measures whether the document stands alone.

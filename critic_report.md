# Goldfish Review Report

**Target:** `/Users/raresspatariu/FrixMCP/docs/PROJECT.md` (`Lines 1-868`, top through end of §10, before "## 11. Roadmap a fasi")

Stage 7 (Critic Review). Baseline context: `GLOSSARY.md`, `docs/adr/0001-out-of-band-confirmation.md`, `docs/adr/0002-hosts-addressed-by-mac.md`.

## Reviewer Status

| Reviewer | Status |
| --- | --- |
| `egm-design` | completed (2 Blocker, 9 Major, 7 Nit raw) |
| `egm-structural` | completed (0 Blocker, 7 Major, 10 Nit raw; standard: feature-spec-standard) |
| `egm-style` | completed (0 Blocker, 10 Major, 12 Nit raw; no standard) |

After deduplication: **2 Blockers, 20 Majors, 18 Nits**. When reviewers rated the same finding differently, the report uses the highest rating and lists every lens that raised it.

## Prioritized Findings (Blockers -> Major -> Nits)

---

## Blockers

### B1. The Agent can bypass the policy from its own shell on the Control Host - Severity: Blocker
* **Reviewer Lens:** `egm-design`
* **Affected Scope:** Lines 394, 553, 596-600, 631, 664, 670-671, 723; §4.3, §7; ADR 0001
* **Discrepancy Description:** §4.3 says "Il vero confine di sicurezza è quindi **la policy in `frix-core`**". ADR 0001 defines Confirmation as a channel the AI "non può leggere né azionare". But O1 names Claude Code as a target client, and Claude Code gives the Agent a Bash shell on the same Mac, running as the same user. From that shell the Agent could:
  * read the router password with `security find-generic-password -s FrixMCP -w` (line 631 even suggests this command, and the Keychain ACL is not specified);
  * call the router directly with curl;
  * edit `.mcp.json` or `config.toml` to add `--allow-dangerous`;
  * press the native dialog through `osascript` or System Events;
  * rewrite `audit.jsonl`, which is "append-only" only by convention.

  None of T1-T12 covers an "Agent with local code execution". Asset 5 (line 664) names the risk but has no threat row.
* **Rationale & Recommendation:** O3 and the T1/T2 mitigations do not hold until this is fixed.
  * Add a threat row for this case.
  * State which clients the security model holds for (for example, Claude Desktop with no shell, or Claude Code with Bash denied).
  * Restrict the Keychain item's ACL to the signed `frix-mcp`/`frix-web` binaries.
  * Make Confirmation impossible to script (for example, LocalAuthentication or Touch ID).
  * Keep the enabling flags out of files the Agent can edit.
  * Make the audit tamper-evident (hash chain), or drop the "append-only" claim.

### B2. `frix-web` has no authentication, so any local process can act as the Operator - Severity: Blocker
* **Reviewer Lens:** `egm-design`
* **Affected Scope:** Lines 92, 614, 678, 792, 815
* **Discrepancy Description:** Dashboard writes, including Dangerous ones, need only the startup flags and "il dialog della UI". The Audit Entry then records Initiator `operator`. The protections listed are CSRF token, `SameSite=Strict`, `Host`/`Origin` checks and no CORS. They stop a malicious browser page, but not a local `curl` that first GETs the form to obtain the token. Any local process (the Agent through Bash, a malicious package script, another macOS user) can therefore run Dangerous operations with no human Confirmation, and the audit records them as `operator`. That breaks the Initiator model and ADR 0001's statement "lì l'Initiator è già l'Operator".
* **Rationale & Recommendation:**
  * Add real authentication to the dashboard, for example a per-launch secret token in a URL the Operator opens, or a Unix socket with peer-credential checks.
  * Require out-of-band Confirmation for Dangerous operations from the web too.
  * Add a T13 for a non-browser local process.
  * Remove the assumption that loopback alone identifies the Operator.

---

## Major

### M1. Classifications are not one deterministic value, and the registry key cannot express them - Severity: Major
* **Reviewer Lens:** `egm-design`, `egm-structural`, `egm-style`
* **Affected Scope:** Lines 251-256, 388, 404, 590, 776
* **Discrepancy Description:**
  * §4.2.3 classifies `GetSecurityKeys`, `SetSSID` and `SetSecurityKeys` as **D**. Line 251 adds "se mai, solo tool `dangerous` con conferma". Line 776 says these actions on the main networks are "esclusi dal binario".
  * `SetEnable` is "W su istanza 3 (ospiti), **D** su 1 e 2", and `X_AVM-DE_SetWPSEnable`/`SetWPSConfig` is "W/D". Line 590 keys the registry by `(service, action) -> Classification`, which cannot express a per-instance rule. Line 590 also calls §4.2 "la fonte di verità del livello".
  * The completeness test covers only the fixture SCPDs. Nothing says what happens at runtime when the live SCPD, which is re-parsed on every start (line 404), contains an action or argument missing from the registry.
  * Line 388's "se mai, mai esporre" is hard to read.
* **Rationale & Recommendation:**
  * Key the registry by `(service, instance, action)`, with exactly one Classification per key.
  * Resolve each D/X conflict in a single table.
  * State that an action unclassified at runtime is treated as Forbidden.
  * Define the startup behaviour when the live SCPD differs from the fixture (for example, refuse writes and log the mismatch).

### M2. The status header and §2.1 are stale, and dates are relative - Severity: Major
* **Reviewer Lens:** `egm-structural`, `egm-style`
* **Affected Scope:** Lines 5, 20, 57-63, 643
* **Discrepancy Description:**
  * Line 5 reads "scritto il 2026-11-10 prima della prima riga di codice".
  * §2.1 says "Cartella `~/FrixMCP` **vuota** tranne questo documento. Nessun `git init`, nessun `Cargo.toml`."
  * Meanwhile §2.3 has entries dated 2026-11-12, and the repository already has commits for Fase 0 and Fase 1.
  * Relative dates have nothing to anchor them: "scaricata oggi", "scaricati oggi", "Checklist operativa per domani mattina".
* **Rationale & Recommendation:**
  * Add an "Ultimo aggiornamento" date to the status block.
  * Label §2.1 as a dated snapshot ("Stato al 2026-11-10") or update it.
  * Replace "oggi" and "domani mattina" with absolute dates or neutral titles.

### M3. The service count contradicts the controlURL table - Severity: Major
* **Reviewer Lens:** `egm-structural` (Major), `egm-design` (Nit), `egm-style` (Nit)
* **Affected Scope:** Line 62 vs line 128 and lines 148-186
* **Discrepancy Description:** Line 62 reads "37 servizi (39 voci contando `WLANConfiguration:1/2/3` separatamente)". The table has 37 rows that already count the three WLAN instances separately, which gives 35 distinct services. That matches the "35 file SCPD" at line 128.
* **Rationale & Recommendation:** Recount against the `tr64desc.xml` fixture. Then fix line 62, or add the missing rows to the table.

### M4. Objective O3 and threat T1 promise operations the catalogue excludes or does not contain - Severity: Major
* **Reviewer Lens:** `egm-design`, `egm-style`, `egm-structural` (Nit)
* **Affected Scope:** Line 39, line 670, §8.3, line 776
* **Discrepancy Description:** O3 reads "Riavvio, port forwarding, DNS, password Wi-Fi, accesso remoto: mai eseguibili dall'IA senza conferma esplicita della persona (o del tutto esclusi dalla build)." In the catalogue:
  * there is no DNS tool;
  * LAN/DHCP `Set*` and `RemoteAccess:Set*` are X;
  * the only password tool is for the guest Wi-Fi.

  The "o" leaves the reader unable to tell which items are Dangerous and which are Forbidden. T1 mentions "cambia DNS".
* **Rationale & Recommendation:** Give each item in O3 its Classification, or refer to §8.3. Remove DNS and remote access from O3 and T1, or add the tools.

### M5. Glossary drift: "escluso" and the "Livello" column are used for Forbidden - Severity: Major
* **Reviewer Lens:** `egm-structural`, `egm-style`
* **Affected Scope:** Lines 39, 85, 139, 204, 258, 270, 301, 360, 361, 366, 675, 776; "Livello" column in every §4.2 table
* **Discrepancy Description:** The glossary lists "excluded, livello X" under _Avoid_ for Forbidden. The document still uses "escluso dalla build, sempre" and "esclusi dal binario". The "Livello" column holds X values, even though line 144 itself says X is not a Level.
* **Rationale & Recommendation:** Rename the column "Classification" and write "Forbidden" in place of "escluso". For line 776, a heading such as "Forbidden (assenti dal binario)" would work.

### M6. Glossary drift: bare "utente" refers to three different actors - Severity: Major
* **Reviewer Lens:** `egm-style`, `egm-structural`
* **Affected Scope:** Lines 394, 396, 437, 474, 660, 664, 678, 679, 681, 705 (§7.5 heading), 724, 824, 835
* **Discrepancy Description:** The same word stands for different actors:
  * Router Account: "L'utente dedicato", "Utente FRITZ!OS dedicato", "l'utente non ha il diritto X";
  * Operator: "far vedere all'utente", "si informa l'utente";
  * the macOS account: "privilegi dell'utente";
  * unclear: "L'utente ha accesso in sola lettura al repo".

  The glossary lists "utente, user (da solo)" under _Avoid_. Line 724 matters most because it is user-facing error text.
* **Rationale & Recommendation:** Replace each occurrence with Router Account, Operator or "l'account macOS". At line 824, say who has read access to which repository.

### M7. Glossary drift: Host, Control Host, Operator, Agent and Wi-Fi Network have unsanctioned synonyms - Severity: Major
* **Reviewer Lens:** `egm-style`, `egm-structural`
* **Affected Scope:**
  * Host as "dispositivo" or "client": lines 38, 208, 224, 226, 233, 307, 316, 663, 841.
  * Control Host as "il Mac": lines 61, 228, 252, 316, 375, 664, 677, 847.
  * Operator as "persona": lines 3, 31, 39, 350.
  * Agent as "modello": line 31.
  * Wi-Fi Network as "banda": lines 799, 801. "banda" also means bandwidth at lines 41, 614 and 799.
* **Discrepancy Description:** Line 88 states that the document and the code use one name per concept, but each of these terms appears in the glossary's _Avoid_ lists. Lines 252, 316 and 375 describe the Lockout rule without using the term.
* **Rationale & Recommendation:** Do one terminology pass. The tool name `list_wifi_clients` can stay as it is, because it is code. Where the Lockout rule is described, name it. For bandwidth, use "traffico" or "throughput".

### M8. Smart Home read tool placed in Phase 1, but Homeauto is Phase 2 - Severity: Major
* **Reviewer Lens:** `egm-structural`
* **Affected Scope:** Line 743 vs lines 321, 394, 709; line 742
* **Discrepancy Description:** `list_smarthome_devices` is listed under "8.1 Fase 1 — sola lettura". §4.2.8 is labelled "fase 2", and lines 394 and 709 add the Smart Home right only in phase 2. With v1 rights the tool would return 606. `get_access_control` (line 742) has the same problem: it uses HostFilter, which is labelled "fase 2".
* **Rationale & Recommendation:** Move these tools to Phase 2, or mark them as active only when the right is granted.

### M9. No explicit Problem Statement - Severity: Major
* **Reviewer Lens:** `egm-structural` (note: `egm-design` judged the problem to be present, but only implicitly)
* **Affected Scope:** Lines 27-51 (§1)
* **Discrepancy Description:** §1.1 "In una frase" describes the solution. To find the problem, the reader has to piece it together from §2.3 row 1, §3.2 and O1-O6. The feature-spec standard requires a Problem Statement.
* **Rationale & Recommendation:** Add a 3-5 line "Problema" subsection. It should say why existing MCPs and the AVM UI fall short: no permission model, credentials in plain text, no audit.

### M10. Alternatives are scattered, and the most consequential choices have none - Severity: Major
* **Reviewer Lens:** `egm-structural`, `egm-design`
* **Affected Scope:** Lines 76-121, 81, 437, 622, 811-812; ADR 0001
* **Discrepancy Description:** Rejected alternatives are spread over §2.3, §3.2 and lines 437, 622 and 812. Several major choices have no compared option:
  * Rust vs Python with fritzconnection (line 81 only says "niente dipendenze Python/Node");
  * askama/htmx vs an SPA;
  * two independent processes vs a single daemon (this drives M12);
  * confirmation channels other than token vs dialog, such as Touch ID or a Dangerous path only in the dashboard.
* **Rationale & Recommendation:** Add an "Alternative considerate" subsection. List each major decision with its rejected options and a concrete trade-off, and link back to §2.3, §3.2 and the ADRs.

### M11. The summary contradicts the non-goals - Severity: Major
* **Reviewer Lens:** `egm-style`
* **Affected Scope:** Line 3 vs line 50
* **Discrepancy Description:** Line 3 says the dashboard "sostituisce l'interfaccia AVM", but line 50 lists "Sostituire AVM al 100%" as a non-goal. The phrase "in modo autonomo ma sicuro" cannot be measured.
* **Rationale & Recommendation:** Rewrite line 3, for example as "copre i casi d'uso quotidiani dell'interfaccia AVM". Replace "autonomo ma sicuro" with the actual mechanisms: Levels, Confirmation and the Lockout check.

### M12. Rate limit, SID and audit are per process, but several processes run at once - Severity: Major
* **Reviewer Lens:** `egm-design`
* **Affected Scope:** Lines 472, 597, 600, 614, 677
* **Discrepancy Description:** Each Claude session starts its own stdio `frix-mcp`, and `frix-web` is a separate process. Yet the document assumes three things that only one process can guarantee:
  * "Un solo SID per router per programma";
  * a rate limit of "una sola `dangerous` ogni 60 s";
  * an append-only JSONL file.

  As written, N sessions multiply the rate limit, can use up the router's limited SIDs, and append to the audit file concurrently with no guarantee the writes stay intact. T8 relies on "un solo SID", which no process enforces.
* **Rationale & Recommendation:** Say where shared state lives. One option is a `flock`-protected audit file plus a persistent rate-limit counter. The other is a single daemon that owns the router session.

### M13. The `Tr064Client::call` pipeline is out of order, and the layering is underspecified - Severity: Major
* **Reviewer Lens:** `egm-design`
* **Affected Scope:** Lines 545, 556, 586, 590, 595, 720-723
* **Discrepancy Description:**
  * Confirmation (step 2) comes before the "prima" snapshot (step 3), so the Operator confirms without seeing the Preview.
  * `guard.rs` in core "verifica … Confirmation", but the dialogs live in `frix-mcp` and `frix-web`. No trait is defined, and the document's own dependency rule forbids core from depending on either binary.
  * The registry has no fields for the snapshot getter, Reversibility, secret arguments or the Lockout target.
  * No "dopo" snapshot is defined for `Reboot`, `ForceTermination`, `DoUpdate`, `WakeOnLAN` or `AddTicketTime`.
  * The audit outcome when the post-read fails is not specified.
* **Rationale & Recommendation:**
  * Reorder the pipeline: validate → classify → snapshot → Preview → Lockout → Confirmation → execute → post-read (best effort, with a declared outcome) → audit.
  * Define a `ConfirmationProvider` trait in core.
  * Extend the registry entries with the missing fields, or move the policy into the typed `services` layer.

### M14. The Lockout check is incomplete, and the Control Host is not uniquely identified - Severity: Major
* **Reviewer Lens:** `egm-design`, `egm-style` (line 316)
* **Affected Scope:** Lines 229, 316, 596, 754, 758, 769-771, 774
* **Discrepancy Description:**
  * The Control Host is identified by "il suo IP" or by `X_AVM-DE_RequestClient`. Neither handles a Mac with Ethernet and Wi-Fi both up, a VPN, or a rotating private Wi-Fi MAC.
  * Line 316 ("rifiutare se l'IP è quello del Mac") contradicts ADR 0002, which identifies hosts by MAC.
  * Writes that can cause a Lockout but are not covered:
    * `set_wifi_channel` on the Control Host's network (DFS channels);
    * `set_guest_wifi_password` while the Control Host is on the guest network;
    * `HostDoUpdate` on the mesh repeater the Control Host connects through;
    * `apply_firmware_update`, which may change the certificate and cause a `TlsPinMismatch`.
* **Rationale & Recommendation:** Define the Control Host as the set of all its interface MACs, refreshed on every call. List the Lockout rule for each Write or Dangerous tool explicitly.

### M15. MAC→IP resolution is undefined for inactive hosts and duplicate IPs - Severity: Major
* **Reviewer Lens:** `egm-design`
* **Affected Scope:** Lines 721, 754-757, 772; ADR 0002
* **Discrepancy Description:** The host list keeps inactive hosts with their last IP, and that IP may now be leased to another device. This is the same mistaken-target risk ADR 0002 sets out to prevent. Nothing specifies what happens when:
  * the target has `Active=0`;
  * two entries share an IP;
  * the target has no IPv4.

  There is also a race between resolving the IP and writing to it.
* **Rationale & Recommendation:** Require `Active=1` and a unique IP, and refuse otherwise. Re-check `GetSpecificHostEntryByIP(ip).MACAddress == mac` immediately before and after the write, and record both checks in the Audit Entry.

### M16. TLS pinning does not cover port 443, and the pin breaks under Let's Encrypt - Severity: Major
* **Reviewer Lens:** `egm-design`
* **Affected Scope:** Lines 439, 447, 484, 488, 695-703
* **Discrepancy Description:**
  * Only `:49443` is pinned, but the SID login, AHA, REST and `data.lua` all use port 443.
  * With MyFRITZ/Let's Encrypt, the certificate rotates at every renewal, so `TlsPinMismatch` becomes routine. Routine mismatches teach the Operator to re-pin without checking.
  * The first pin is trust-on-first-use, and the document does not state that risk.
* **Rationale & Recommendation:**
  * Pin each port, or verify that both ports serve the same certificate.
  * Under Let's Encrypt, pin the public key or use `ca` mode.
  * Document trust-on-first-use as an accepted risk.
  * Add a re-pin procedure that shows the old and new fingerprints.

### M17. The Confirmation dialog has not been checked for feasibility or limits - Severity: Major
* **Reviewer Lens:** `egm-design`
* **Affected Scope:** Lines 556, 723; ADR 0001
* **Discrepancy Description:**
  * The document does not say how a stdio CLI child process shows a native dialog, and this is not marked [DA VERIFICARE].
  * The 120 s wait may exceed the MCP client's tool-call timeout, and the cancellation semantics are not defined.
  * The Preview shows strings from the router (hostnames, SSIDs) that an attacker controls, but the §7.3 sanitisation applies only to MCP output.
* **Rationale & Recommendation:**
  * Mark the dialog mechanism [DA VERIFICARE] and prototype it in Phase 0.
  * Check client timeouts and specify cancellation.
  * Sanitise the dialog content and render untrusted values as quoted data.

### M18. Running alpha craft-anything with an admin session contradicts the §3 rejection criteria - Severity: Major
* **Reviewer Lens:** `egm-design`
* **Affected Scope:** Lines 80, 112, 824-835, 847
* **Discrepancy Description:** kambriso is rejected as "non auditabile". Yet an alpha third-party tool is to run against the router with a live session of an effectively-admin user. The temporary user is optional ("oppure lo stesso `frixmcp`"), and the SID is cleaned up by hand.
* **Rationale & Recommendation:**
  * Make the temporary analysis user mandatory, and delete it afterwards.
  * Sandbox the tool.
  * Justify why this is acceptable when kambriso was not, or make plan B (DevTools) the default.

### M19. Some objectives are subjective or cannot be measured - Severity: Major
* **Reviewer Lens:** `egm-style`
* **Affected Scope:** Lines 37, 41, 42
* **Discrepancy Description:**
  * O5 is named "Front-end decente", and its criterion "più veloce e più chiara dell'interfaccia AVM" has no metric. Line 789 already gives one: "in meno di 2 secondi".
  * O1's name, "Osservabilità completa", overclaims given the non-goals.
  * O1's phrase "senza mai toccare il router" is wrong, because every read contacts the router.
  * O6's "con coscienza pulita" carries no content.
* **Rationale & Recommendation:**
  * Rename O5 and reuse the 2-second criterion.
  * Drop "completa" from O1, and replace its phrase with "senza eseguire scritture".
  * Drop "con coscienza pulita".

### M20. Some claims go beyond their evidence - Severity: Major
* **Reviewer Lens:** `egm-style`
* **Affected Scope:** Lines 87, 174-176/188 (§4.2.3), 268, 616 (§6.5), 635, 695, 830
* **Discrepancy Description:**
  * Line 87: "chiude l'attacco MITM sulla LAN" and "il difetto più comune degli MCP esistenti". Line 108 admits the existing MCPs were not read in depth.
  * Line 695: "quasi tutti i client".
  * Line 268: "il 'grafico di traffico' della UI AVM è qui", but line 841 still plans to observe that page.
  * Line 635: "il `FixtureTransport` basta per il 90 %".
  * Line 830: "è lecito e normale (lo fanno Home Assistant e decine di progetti)".
  * §4.2.3 and §6.5 headings carry [VERIFICATO], but they contain [DA VERIFICARE] items.
* **Rationale & Recommendation:**
  * Restate each claim to match its evidence, for example "impedisce l'accettazione di un certificato diverso da quello fissato".
  * Drop the unsupported superlatives and percentages, or cite a source.
  * Narrow the headings' verification tags (for example "[VERIFICATO: versioni]").
  * Tag the band labels in rows 174-176 [DA VERIFICARE].

---

## Nits

### N1. `check_firmware_update` declares Reversibility "—" - Severity: Nit
* **Reviewer Lens:** `egm-structural`, `egm-design`
* **Affected Scope:** Line 761 (vs line 722)
* **Discrepancy Description:** Line 722 requires every Write or Dangerous tool to declare its Reversibility, and the glossary defines only Reversible and Irreversible.
* **Rationale & Recommendation:** Pick one of the two values, or reclassify the tool as not a write.

### N2. Variant spellings of Confirmation, Preview, Lockout, Audit Entry and Reversibility - Severity: Nit
* **Reviewer Lens:** `egm-style`, `egm-structural`
* **Affected Scope:** Lines 91, 251, 299, 359, 545, 598, 600, 644, 670, 671, 722, 791
* **Discrepancy Description:** The document uses:
  * "anteprima" and "dry-run" for Preview;
  * lowercase generic "conferma" for Confirmation;
  * "lock-out" for Lockout;
  * "log" for the audit file;
  * "rollback", which is on the _Avoid_ list, although each use denies the feature exists.
* **Rationale & Recommendation:** Use the glossary terms, for example "Preview (parametro `dry_run`)", Lockout, and "file di audit"/Audit Entry.

### N3. `HostsCheckUpdate` (D) vs `UserInterface:X_AVM-DE_CheckUpdate` (W) - Severity: Nit
* **Reviewer Lens:** `egm-design`
* **Affected Scope:** Lines 229, 380
* **Discrepancy Description:** Both actions only check for updates, yet they are classified differently.
* **Rationale & Recommendation:** Align the two classifications, or justify the difference.

### N4. `fritz.box` does not resolve on the Control Host, but the AHA URL uses it - Severity: Nit
* **Reviewer Lens:** `egm-design`
* **Affected Scope:** Lines 61, 484
* **Discrepancy Description:** Line 61 says `fritz.box` does not resolve from the Control Host, but line 484 builds the AHA URL on it.
* **Rationale & Recommendation:** Use the IP address, or note the discrepancy.

### N5. The "fallback env solo per test" is a convention, not compiled out - Severity: Nit
* **Reviewer Lens:** `egm-design`
* **Affected Scope:** Line 521
* **Discrepancy Description:** Nothing stops the test-only environment fallback from shipping in release builds.
* **Rationale & Recommendation:** Gate it with `cfg(test)` or a cargo feature.

### N6. Bootstrap over plain HTTP is not listed as a residual risk - Severity: Nit
* **Reviewer Lens:** `egm-design`
* **Affected Scope:** Line 439; T4
* **Discrepancy Description:** `tr64desc.xml`, the SCPDs and `GetSecurityPort` travel over plain HTTP.
* **Rationale & Recommendation:** Note this in T4 as an accepted residual risk, or re-fetch these files over 49443 after the first bootstrap.

### N7. Operational risks are scattered - Severity: Nit
* **Reviewer Lens:** `egm-structural`
* **Affected Scope:** Lines 656-711 (§7); lines 590, 831
* **Discrepancy Description:** Non-security risks appear only in passing: firmware changes to the SCPD, unstable undocumented endpoints, and no Operator present for the dialog.
* **Rationale & Recommendation:** Add a short list of non-security risks to §7, or point to §14.

### N8. Module path mismatch - Severity: Nit
* **Reviewer Lens:** `egm-structural`
* **Affected Scope:** Line 468 vs line 530
* **Discrepancy Description:** Line 468 names `frix-core::auth::sid`, but the layout tree has `src/sid.rs`.
* **Rationale & Recommendation:** Use one path in both places.

### N9. The layout tree omits GLOSSARY.md - Severity: Nit
* **Reviewer Lens:** `egm-structural`
* **Affected Scope:** Lines 500-567
* **Discrepancy Description:** GLOSSARY.md exists and is cited at lines 88, 144 and 596, but the tree does not list it.
* **Rationale & Recommendation:** Add it to the tree.

### N10. The services-API example selects a network by Band and takes an IP - Severity: Nit
* **Reviewer Lens:** `egm-structural`
* **Affected Scope:** Line 588
* **Discrepancy Description:** `wlan::info(&client, Band)` cannot select the guest network, which shares 2.4 GHz. `host_filter::set_wan_access(&client, Ipv4Addr, Allow)` takes an IP, and the example does not mention MAC resolution.
* **Rationale & Recommendation:** Use a `WifiNetwork` enum, and note that the IP comes from MAC resolution.

### N11. The §4.2.x subsections are not symmetric - Severity: Nit
* **Reviewer Lens:** `egm-structural`
* **Affected Scope:** Lines 194-388
* **Discrepancy Description:**
  * The headings vary: some omit the phase or the priority, and some add free text.
  * The bodies mix tables, prose and bullets.
  * §4.2.8 has no "Scopo:" line.
  * Some cell values are not in the legend: "R (interno)", "W (interno)", "var.", "W/D".
  * Separators are inconsistent: "-/C/C" vs "C/A, C, C".
* **Rationale & Recommendation:** Use one heading template and one table schema for all subsections.

### N12. Verification markers, numbering, empty cells and fences - Severity: Nit
* **Reviewer Lens:** `egm-structural`, `egm-style`
* **Affected Scope:** Lines 348, 402, 476, 490, 681, 760; §8.0; fences at lines 408, 461, 500
* **Discrepancy Description:**
  * Some headings carry non-standard markers: "[VERIFICATO in parte]", "[DA VERIFICARE, tutto]", "[VERIFICATO sul documento First Steps v60]".
  * §8 starts at 8.0, and the catalogue jumps from Fase 2 to Fase 4.
  * Line 760 has an empty cell, and T12 uses "—" in two cells.
  * Three code fences have no language tag.
* **Rationale & Recommendation:**
  * Keep only the two defined markers, and say in the body text what was verified.
  * Renumber §8.
  * Fill the empty cells.
  * Add language tags to the fences.

### N13. No links to the external projects - Severity: Nit
* **Reviewer Lens:** `egm-structural`
* **Affected Scope:** Lines 100-121, 824
* **Discrepancy Description:** ghbalf/mcp-fritzbox, kambriso/fritzbox-mcp-server, fritzctl-py, fritzconnection and craft-anything are named without links.
* **Rationale & Recommendation:** Add inline links, or §15 references.

### N14. Subjective quality judgments - Severity: Nit
* **Reviewer Lens:** `egm-style`
* **Affected Scope:** Lines 80, 81, 105, 114, 377, 437, 488, 624
* **Discrepancy Description:** Examples include "maturo", "il riferimento migliore", "il tool diagnostico perfetto", "È la strada giusta" and "è più robusto". Line 81 calls `rmcp` "maturo", although line 620 has [DA VERIFICARE] items for its API.
* **Rationale & Recommendation:** Replace each judgment with a factual property, or drop it.

### N15. Informal or filler phrasing - Severity: Nit
* **Reviewer Lens:** `egm-style`
* **Affected Scope:** Lines 112, 280, 350, 492, 614, 826, 865
* **Discrepancy Description:** Examples include "simpatico e libero", "Limiti onesti:", "senza autenticazione!", "è esattamente il tipo di cosa che non vogliamo".
* **Rationale & Recommendation:** Drop the filler, and use "Limiti" as the heading.

### N16. Unquantified intensifiers - Severity: Nit
* **Reviewer Lens:** `egm-style`
* **Affected Scope:** Lines 375, 392
* **Discrepancy Description:** The text uses "rischio massimo" and "**moltissime azioni**".
* **Rationale & Recommendation:** Give a count, or a list.

### N17. Speculative asides with no decision attached - Severity: Nit
* **Reviewer Lens:** `egm-style`
* **Affected Scope:** Lines 361, 470, 828
* **Discrepancy Description:** Examples: "Possibile tool 'backup locale' in futuro"; "non dovrebbe servire ma costa poco implementarlo" (the MD5 fallback); "che è comunque sufficiente".
* **Rationale & Recommendation:** Move the aside at line 361 to §14. At line 470, state the decision on the MD5 fallback (yes or no). Drop "comunque sufficiente".

### N18. "innocuo" used as a safety label - Severity: Nit
* **Reviewer Lens:** `egm-style`
* **Affected Scope:** Line 752 (`wake_host`)
* **Discrepancy Description:** "innocuo" is a judgment, not a description of the effect.
* **Rationale & Recommendation:** State the effect, for example "invia un magic packet; nessuna modifica di configurazione".

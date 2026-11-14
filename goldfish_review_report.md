Status: resolved

# Goldfish Annotated Review Report

**Target:** `docs/PROJECT.md` (full file; findings from `consolidated_review_report.md`, actions A1-A16)

L'Operator ha chiesto di sistemare tutti i punti. Le correzioni marcate PROPOSE FIX sono state applicate. Due parti sono contestate, con evidenza nel codice, e i nit stilistici sono in gran parte rifiutati.

**Esito:** 14 applicate (alcune in parte), 2 contestate in parte (A8, A14), 0 da sottoporre all'Operator, nit in gran parte rifiutati (A15, A16).

File toccati: `docs/PROJECT.md`, `docs/adr/0001-out-of-band-confirmation.md`, `SECURITY.md`, `README.md`, `.mcp.json.example`. Nessun file di codice modificato.

---

### A1. Confine di fiducia [`zoom-out`, `design`, `inventive-catalyst`] - Severity: Blocker
* ***AI Suggestion:*** PROPOSE FIX (applicata)
* ***Justification:*** l'Agent con una shell aggira qualunque controllo interno al processo. La CLI `security` (`secrets.rs`) rende inutile una ACL del Keychain.
* ***Edit:***
  - nuova §7.0 con i termini chiave, la garanzia, i limiti e i deployment supportati;
  - minaccia T13 aggiunta;
  - O3 e §4.3 riformulati;
  - §14.2.3 marcata come decisa, con la conseguenza;
  - `.mcp.json.example` punta a `~/.local/bin/frix-mcp`;
  - aggiunti a `SECURITY.md` "Deployment supportati" e il fuori ambito esteso agli agent con shell;
  - `README.md` allineato.

### A2. `frix-web` in sola lettura [`design`, `zoom-out`] - Severity: Blocker
* ***AI Suggestion:*** PROPOSE FIX (applicata)
* ***Edit:***
  - §6.4 e §9 riscritte: nessun form, CSRF o Confirmation web; schermate senza azioni; "Blocchi e profili" tolta;
  - T9 ridotta a XSS e lettura;
  - non-obiettivo aggiunto in §1.3;
  - nell'ADR 0001 la conseguenza 3 è sospesa ("Aggiornamento 2026-11-13").

### A3. Level al tool [`codebase-details`, `design`, `zoom-out`] - Severity: Blocker
* ***AI Suggestion:*** PROPOSE FIX (applicata nel documento; il codice si cambia nella Fase 2)
* ***Edit:***
  - legenda della §4.1: la colonna "Livello" diventa "Rischio", indicativa;
  - §6.2: il registro decide "raggiungibile / Forbidden" per le scritture e il Level sta nel tool;
  - §8.0 dichiara i tool come fonte di verità;
  - l'accettazione della Fase 2 ora è coerente (`set_guest_wifi` è Write);
  - §14.3: la verifica dell'istanza Ospiti è marcata come bloccante per la Fase 2.
* ***Residuo:*** `registry.rs` assegna ancora D a `SetEnable` su tutte le istanze; il cambio è nella lista file della Fase 2 (§11).

### A4. Allineamento al codice [`codebase-details`, `implementation-readiness`, `zoom-out`, `comprehension`] - Severity: Blocker
* ***AI Suggestion:*** PROPOSE FIX (applicata)
* ***Edit:***
  - intestazione con data e stato;
  - §2.1 riscritta con lo stato reale e il conteggio dei servizi spiegato (37 servizi, 35 SCPD);
  - §6.1 con il layout reale, le voci future marcate e la regola di dipendenza verificata dal test;
  - §6.5 ridotta alle regole di scelta;
  - §12 sostituita con un link alla guida operativa;
  - §13 sostituita dai requisiti per file, con la numerazione 13.1-13.10 mantenuta perché i commenti nel codice la citano;
  - §13.6: le regole per gli agent senza `CLAUDE.md` e senza `tracing`;
  - §14.1 e §14.2 marcate come decise;
  - §15 aggiornata.

### A5. Comando del test live [`implementation-readiness`] - Severity: Blocker
* ***AI Suggestion:*** PROPOSE FIX (applicata)
* ***Edit:*** in §6.6, §11 e §13.6 il comando ora è `cargo test -p xtask --features live --test live -- --ignored --nocapture`, lo stesso della guida operativa.

### A6. Esecutore delle scritture + `Confirm` [`comprehension`, `design`, `zoom-out`] - Severity: Major
* ***AI Suggestion:*** PROPOSE FIX (applicata)
* ***Edit:***
  - §6.2: `call` resta una RPC semplice, come nel codice;
  - esecutore in 13 passi: audit `intent` prima della chiamata, seconda lettura dopo la Confirmation, risoluzione MAC → IP con `Active` e un solo IP;
  - trait `Confirm` definito in `frix-core`;
  - chiave `audit_path` prevista in `ConfigFile`.

### A7. Stato per processo [`design`, `zoom-out`] - Severity: Major
* ***AI Suggestion:*** PROPOSE FIX (applicata)
* ***Edit:***
  - rate limit per processo, solo per l'Initiator `agent`, senza il limite extra sulle D;
  - audit con una sola `write(2)` in `O_APPEND`;
  - daemon scartato (§2.3, §14.2.11);
  - in §2.3 tolto "un solo punto di controllo".

### A8. Interfacce fuori dalla v1 [`zoom-out`, `design`] - Severity: Major
* ***AI Suggestion:*** PROPOSE FIX in parte, CONTEST in parte
* ***Applicato:***
  - §5.2-§5.6 raccolte in una "§5.2 Interfacce fuori dalla v1", con i fatti e i link;
  - §10 ridotta a un piano con DevTools;
  - §8.4 (resources e prompt) tolta;
  - clausola multi-router tolta;
  - pin della porta 443 rinviato al SID (§7.4, punto 6).
* ***Contestato:*** togliere `tls.mode = "ca"`. È già implementata e testata (`crates/frix-core/src/config.rs`, `TlsMode::Ca`; `crates/frix-mcp/src/cli.rs`, `--tls-mode pin|ca`). Toglierla sarebbe una modifica al codice, non una semplificazione del design. Il documento ora la descrive come implementata.

### A9. Catalogo D e spike [`design`, `zoom-out`, `implementation-readiness`] - Severity: Major
* ***AI Suggestion:*** PROPOSE FIX (applicata)
* ***Edit:***
  - §8.3 ridotta a 5 tool, che esistono solo se lo spike riesce;
  - port forwarding, WPS e `DoUpdate` diventano Forbidden con il motivo;
  - Fase 4 con lo spike e il criterio di uscita, più la gestione di 866;
  - nell'ADR 0001 il meccanismo è marcato [DA VERIFICARE].

### A10. Lockout e Control Host [`design`, `inventive-catalyst`, `zoom-out`] - Severity: Major
* ***AI Suggestion:*** PROPOSE FIX (applicata)
* ***Edit:***
  - §6.2 definisce la garanzia e il Control Host come insieme di Host;
  - le tabelle §8.2 e §8.3 hanno una colonna "Lockout";
  - tolto il dettaglio su `X_AVM-DE_RequestClient`;
  - la nota HostFilter della §4.2.7 è per MAC.

### A11. Obiettivi e affermazioni [`structural`, `design`, `style`] - Severity: Major
* ***AI Suggestion:*** PROPOSE FIX (applicata)
* ***Edit:***
  - problem statement nell'intestazione;
  - O3 elenca i tool e i Forbidden;
  - O5 misurabile (2 s);
  - §1.1 e §1.3 coerenti;
  - "append-only" sostituito con "tracciabilità, non prova di manomissione";
  - alternative collegate alle decisioni in §2.3 e §14.2;
  - claim del README attenuati.

### A12. Pianificabilità delle fasi [`implementation-readiness`] - Severity: Major
* ***AI Suggestion:*** PROPOSE FIX (applicata)
* ***Edit:***
  - §11: tabella di stato; Fase 1 con accettazione verificabile;
  - Fase 2 con lista file e test nominati;
  - Fasi 3 e 4 con test e accettazione;
  - spostamento in `frix-core` di config e avvio prima della Fase 3 (§6.1);
  - flag reali documentati (§6.3).

### A13. Leggibilità autonoma [`comprehension`] - Severity: Major
* ***AI Suggestion:*** PROPOSE FIX (applicata)
* ***Edit:*** link a `GLOSSARY.md` con il percorso nell'intestazione; definizioni brevi in §7.0; SCPD e AIN espansi; legenda "basta un diritto qualunque" in §4.1; GLOSSARY nel layout.

### A14. Tool Smart Home in Fase 1 [`structural`, `zoom-out`] - Severity: Major
* ***AI Suggestion:*** CONTEST (con chiarimento nel testo)
* ***Justification:***
  - `GetGenericDeviceInfos` richiede C oppure H, e "C/H" significa che ne basta uno (`policy::required_rights`, "I diritti che bastano, uno qualunque");
  - il Router Account ha C;
  - il tool è già implementato e testato in Fase 1 (`crates/frix-mcp/src/tools/smarthome.rs`, `tests/smarthome.rs`).

  Il difetto reale era la legenda ambigua, ora corretta: §4.1, titolo della §4.2.8, §4.3 e §7.5 (il diritto "Smart Home" non serve).

### A15. Pulizie puntuali - Severity: Nit
* ***AI Suggestion:*** PROPOSE FIX per alcune, DECLINE per le altre
* ***Applicate:***
  - Reversibility di `check_firmware_update`;
  - rischio residuo del bootstrap HTTP (T14);
  - §15 aggiornata;
  - T8 descrive il comportamento reale di `router.rs`.
* ***Rifiutate:*** asimmetria delle §4.2.x, marcatori di verifica, link ai progetti (già presenti in §15), fallback env (non esiste più: il Keychain usa la CLI).

### A16. Terminologia - Severity: Nit
* ***AI Suggestion:*** PROPOSE FIX solo nelle sezioni riscritte, DECLINE altrove
* ***Applicate:*** "Router Account" al posto di "utente dedicato" (§4.3, §7.1, §7.5, T10), "Host" invece di "Dispositivi", "traffico" invece di "banda" per il throughput, "Control Host" invece di "il Mac".
* ***Rifiutate:*** "escluso" nelle note delle tabelle §4.2 (la colonna dice già X), frasi riempitive e giudizi soggettivi nelle sezioni non toccate.

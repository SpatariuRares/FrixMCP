# Review consolidata: `docs/PROJECT.md`

**Data:** 2026-11-13 · **Modalità:** report consolidato, tutti gli stage (5 → 9) · **FeatureRange:** 1-868 · **DetailedRange:** 869-1302
**Contesto dato ai reviewer:** `GLOSSARY.md`, ADR 0001, ADR 0002 (lo Stage 6 ha letto solo il documento, di proposito).

## Verdetto

**Non pronto per le Fasi 2-4.** La forma a tre crate regge e il codice delle Fasi 0-1 è già vicino a un design più snello. Il documento però:

1. promette una garanzia di sicurezza che il design non può mantenere (l'Agent ha una shell come lo stesso utente macOS);
2. dà a `frix-web` le scritture senza autenticazione;
3. assegna i Level in due posti che già si contraddicono, tanto che l'accettazione della Fase 2 non è raggiungibile con il registry attuale;
4. è rimasto indietro rispetto al codice: §12-§14 descrivono un repo vuoto e file che oggi sono diversi.

La strada indicata dallo Stage 8 è **tagliare prima di aggiungere**. Tre tagli (confine di fiducia come vincolo di deployment, `frix-web` in sola lettura nella v1, Level ai tool e non alle azioni) eliminano gran parte dell'irrobustimento chiesto dagli stage 5 e 7. Dopo i tagli restano tre aggiunte vere: un esecutore delle scritture in `frix-core`, un trait `Confirm` iniettato dai front-end e una Audit Entry scritta prima della chiamata.

## Stato degli stage

| Stage | Report | Reviewer | Stato | 🔴 | 🟡 | 🟢 |
| --- | --- | --- | --- | --- | --- | --- |
| 5 Zoom Out iniziale | `zoom_out_report.md` | zoom-out, inventive-catalyst | completed | 1 | 14 | 8 |
| 6 Comprehension | `comprehension_report.md` | comprehension | completed | 0 | 3 | 5 |
| 7 Critic | `critic_report.md` | design, structural, style | completed | 2 | 20 | 18 |
| 8 Zoom Out intermedio | `intermediate_zoom_out_report.md` | zoom-out | completed | 1 | 8 | 3 |
| 9 Implementation Readiness | `implementation_readiness_report.md` | implementation-readiness, codebase-details, style | completed | 6 | 17 | 16 |

Nessun reviewer è fallito o ha richiesto un retry. Totale grezzo: 10 🔴, 62 🟡, 50 🟢, ridotti qui sotto a 17 azioni.

**Verifica:** i numeri di riga sono quelli dei reviewer, non ricontrollati uno per uno. Ho verificato sul codice tre affermazioni dei blocker dello Stage 9 e sono vere:
- `registry.rs:89` classifica `WLANConfiguration:SetEnable` come D su tutte le istanze;
- la feature `live` sta in `xtask/Cargo.toml`, non in `frix-core`;
- `CLAUDE.md` non esiste.

---

## Azioni

Ordine consigliato: prima le decisioni di taglio (A1-A3), poi l'allineamento al codice (A4-A5), poi il design delle scritture (A6-A10). La revisione dei termini (A16) va fatta per ultima, perché molte righe interessate spariranno.

### 🔴 Blocker

#### A1. Dichiarare il confine di fiducia come vincolo di deployment
- **Problema:** con Claude Code l'Agent gira come lo stesso utente macOS e ha una shell. Può leggere la password dal Keychain, chiamare il router con `curl`, aggiungere `--allow-dangerous` a `.mcp.json`, cliccare il dialog con `osascript`, riscrivere `audit.jsonl` o ricompilare `frix-mcp` senza guard, visto che `.mcp.json` punta a `target/release`. Nessuna minaccia T1-T12 copre questo caso e `SECURITY.md` lo dichiara fuori ambito, mentre O3 e §4.3 L394 promettono il contrario.
- **Azione:**
  - riformulare la garanzia: la policy limita gli errori dell'Agent e le chiamate indotte da prompt injection (T1, T2), non un Agent che esegue codice arbitrario;
  - in §7 elencare i deployment supportati (Claude Desktop, oppure Claude Code con Bash e scritture fuori dal progetto negate o in sandbox);
  - aggiungere T13 "fuori ambito per costruzione, mitigato dalla configurazione del client";
  - far puntare `.mcp.json.example` a un binario installato fuori dal repo;
  - allineare O3, L394, `SECURITY.md` e la voce "Confirmation" del GLOSSARY.
  - Non adottare la ACL del Keychain con `-T`: con la CLI `security` (`secrets.rs`) il programma che legge è lo stesso per `frix-mcp` e per l'Agent. Marcare §14.2.3 come deciso e scriverne la conseguenza.
- **Fonti:** S5 "The Agent is a same-user process"; S7 B1, M4 (parte); S8 F1, F2.

#### A2. `frix-web` in sola lettura nella v1
- **Problema:** `frix-web` non ha autenticazione. Qualunque processo locale può eseguire scritture e azioni D dalla dashboard, e l'audit le registra come `operator`. Questo rompe il modello di Initiator dell'ADR 0001. Lo scope della v1 è anche ampio: layout mobile, nonostante l'accesso da mobile sia un non-obiettivo, e Confirmation per i `dangerous` nella Fase 3, che però arrivano solo nella Fase 4.
- **Azione:**
  - ridurre la v1 alle pagine in sola lettura Panoramica, Host e Audit, togliendo form, CSRF e Confirmation web;
  - legare la conseguenza 3 dell'ADR 0001 a un futuro ADR sulle scritture web;
  - togliere "layout mobile" e lasciare a T9 solo XSS e CSP.
- **Fonti:** S7 B2; S8 F3; S5 "Contradiction about mobile access", "frix-web v1 scope is broad"; S6 #6; S9 M14 (parte).

#### A3. Il Level appartiene al tool, il registry decide solo consentito o Forbidden
- **Problema:** i Level stanno in §4.2 e in `policy::registry`, e in §8 anche per tool. Le due fonti già divergono: `registry.rs` mette D su `SetEnable` per tutte le istanze, mentre la Fase 2 chiede che `set_guest_wifi` funzioni con il solo `--allow-writes`. **Per questo l'accettazione della Fase 2 non è raggiungibile.** Molte azioni D non sono usate da nessun tool.
- **Azione:**
  - rendere il registry un'allowlist: un'azione è usata da un tool della §8 oppure è Forbidden, e il test di copertura resta;
  - spostare il Level sul tool, che fissa da sé la propria istanza (`set_guest_wifi` usa sempre l'istanza ospiti);
  - in §4.2 sostituire le colonne Livello con "usato da: <tool>" oppure "Forbidden".
  - **Prerequisito:** verificare sul router reale quale istanza `WLANConfiguration:N` corrisponde alla rete ospiti (`registry.rs` la segna come non verificata).
- **Fonti:** S9 B1; S7 M1, N3; S5 "Two sources of truth for action Levels", "Many actions marked Dangerous"; S8 F5.

#### A4. Riallineare il documento al codice e togliere le copie dei file
- **Problema:** l'intestazione e §2.1 dicono "prima della prima riga di codice" e "cartella vuota", mentre le Fasi 0-1 sono state committate. Le copie in §13 sono sbagliate rispetto ai file reali:
  - `Cargo.toml` senza `xtask`, con reqwest invece di ureq e la versione di rmcp sbagliata;
  - `deny.toml` senza `allow-wildcard-paths`, quindi `cargo deny check` fallirebbe;
  - la CI usa un'action Docker su `macos-latest`;
  - `CLAUDE.md` non esiste e le regole di §13.6 non corrispondono al codice;
  - `.gitignore` nasconderebbe `.cargo/config.toml`;
  - `.mcp.json.example` contiene un percorso personale.

  §12 duplica `docs/guida-operator-fase1.md` e non si può rieseguire. §14.2 e §14.3 elencano come aperte decisioni già prese (ureq, CLI `security`, modalità TLS, router singolo, rmcp 3.10, licenza). Il conteggio dei servizi non torna (35, 37 o 39).
- **Azione:**
  - aggiungere un blocco "Ultimo aggiornamento / stato" con lo stato di ogni fase in §11 e datare §2.1 come istantanea;
  - sostituire §12 con un link alla guida operativa;
  - sostituire ogni copia di §13 con una riga sola su cosa quel file deve garantire;
  - ridurre §6.5 alle regole di scelta;
  - marcare come decise le voci di §14.2;
  - correggere il conteggio dei servizi;
  - decidere se le regole per l'IA stanno in `AGENTS.md` (che esiste) o in `CLAUDE.md`, e aggiornare il riferimento.
- **Fonti:** S9 B2, B3, B5, B6, M1-M6, M13, N2-N4, N6; S5 "Document claims the repo is empty", "§13 copies whole repo files"; S6 #1, #4; S7 M2, M3, N9; S8 F11.

#### A5. Correggere il comando del test live
- **Problema:** L885 e L1134 usano `cargo test -p frix-core --features live`, ma la feature sta in `xtask`, quindi il comando fallisce.
- **Azione:** usare il comando reale (`cargo xtask` oppure `-p xtask --features live`) in entrambi i punti, con lo stesso comando della guida operativa.
- **Fonti:** S9 B4.

### 🟡 Major

#### A6. Un esecutore delle scritture in `frix-core` con un trait `Confirm` iniettato
- **Problema:** `Tr064Client::call` dovrebbe fare tutto (validazione, conferma, snapshot, esecuzione, audit) in un ordine sbagliato. Il documento non spiega come `frix-core` ottenga la Confirmation, visto che non dipende dai front-end. L'audit viene scritto dopo l'effetto, quindi una scrittura può restare non registrata. La finestra di Confirmation di 120 s riapre il TOCTOU che l'ADR 0002 chiude. La risoluzione da MAC a IP non è definita per Host inattivi o con IP duplicati. La configurazione `[audit]` verrebbe rifiutata da `deny_unknown_fields`.
- **Azione:**
  - lasciare `call` come RPC semplice con il solo gate di lettura, come già fa il codice;
  - le scritture passano da un esecutore che, in ordine: valida; risolve MAC → IP (`Active=1` e un solo IP, altrimenti rifiuta); legge lo stato; produce la Preview; esegue il Lockout check; chiama `Confirm`, solo per D; rilegge lo stato e annulla se è cambiato; scrive l'audit `intent` e rifiuta la scrittura se l'audit fallisce; esegue la chiamata SOAP; rilegge lo stato; scrive l'audit `result`;
  - definire la chiave `[audit]` nel `ConfigFile`.
- **Fonti:** S6 #3; S7 M13, M15; S5 "Policy, snapshots and audit placed inside the TR-064 client", "120 s Confirmation window", "Audit is written after the effect"; S8 F4; S9 M7, N16.

#### A7. Niente stato condiviso tra processi: dichiarare cosa garantisce ogni processo
- **Problema:** rate limit, SID e handle dell'audit sono per processo, ma il testo li descrive come "un solo punto di controllo". Le proposte di daemon o `flock` aggiungono parti da proteggere senza un bisogno dimostrato.
- **Azione:**
  - L92 diventa "una sola Policy (in `frix-core`)";
  - un solo limite di scrittura per processo per l'Initiator `agent`;
  - eliminare i limiti specifici sulle D, che richiedono già un clic;
  - audit con una sola `write(2)` per riga su un descrittore `O_APPEND`;
  - registrare in §14.2 il daemon come considerato e rifiutato.
- **Fonti:** S5 "Per-process policy state", "Rate limits layered", "Write flags fixed per process"; S7 M12; S8 F6; S9 M11.

#### A8. Togliere dalla v1 i percorsi di ricerca speculativi
- **Problema:** login SID/PBKDF2/MD5, AHA, REST, `data.lua`, craft-anything (alpha, con sessione admin), risorse e prompt MCP, la clausola multi-router e la modalità TLS `ca` non hanno nessun tool v1 che li usi. In più generano problemi propri: pinning sulla porta 443, `fritz.box` che non si risolve, percorsi dei moduli incoerenti.
- **Azione:** raccogliere §5.2-§5.6 e §10 in un'appendice "Rinviato" con i link, tenendo il formato di `ENDPOINTS.md` e la regola "TR-064 vince". Usare DevTools come piano. Eliminare la clausola multi-router e `tls.mode = "ca"`.
- **Fonti:** S5 "SID login, PBKDF2", "Multi-router", "Craft-anything plan", "Two TLS modes", "Optional MCP resources"; S7 M16, M18, N4, N8; S8 F7; S9 N15.

#### A9. Ridurre il catalogo D e aprire la Fase 4 con uno spike sulla Confirmation
- **Problema:** non è verificato che un processo figlio su stdio possa mostrare il dialog nativo, che l'Agent non possa automatizzarlo, né che 120 s stiano dentro il timeout del client MCP. Il comportamento con 866/2FA non è definito. Tre tool hanno rischi propri: `add_port_forwarding` (il router salva un IP, che con il tempo può passare a un altro dispositivo), `apply_firmware_update` (il certificato cambia e il pin si rompe), `start_wps` (finestra di pairing aperta).
- **Azione:**
  - la Fase 4 parte con uno spike che ha un criterio di uscita esplicito;
  - l'ADR 0001 descrive la proprietà richiesta (un canale che l'Agent non legge né aziona) e segna il meccanismo come [DA VERIFICARE];
  - nella v1 al massimo `reboot_router` e `reconnect_wan`;
  - port forwarding, WPS e applicazione del firmware diventano Forbidden.
- **Fonti:** S7 M17; S8 F8; S9 M10; S5 "Port forwarding stores an IP".

#### A10. Lockout e Control Host: una garanzia definita e una regola per tool
- **Problema:** il Lockout è definito solo nel GLOSSARY e nel documento compare solo come esempi. Il Control Host è indicato per IP (contro l'ADR 0002) o con un solo MAC, mentre un Mac ne ha diversi.
- **Azione:** definire il Control Host come l'insieme degli Host del router il cui IP è uno degli indirizzi locali del Control Host. Aggiungere una colonna "Lockout" alle tabelle della §8 al posto delle note in testo libero. Togliere il dettaglio su `X_AVM-DE_RequestClient`.
- **Fonti:** S7 M14; S5 "Control Host identified by a single MAC"; S8 F9; S6 #2 (parte), #7 (parte).

#### A11. Obiettivi, problem statement, alternative e affermazioni senza prove
- **Problema:**
  - manca un problem statement esplicito (lo structural lo considera assente, il design implicito);
  - O3 e T1 promettono operazioni assenti dal catalogo o Forbidden (DNS, accesso remoto);
  - la sintesi contraddice i non-obiettivi;
  - O5 non è misurabile;
  - alcune affermazioni vanno oltre le prove ("append-only", README e template SECURITY);
  - le alternative sono sparse.
- **Azione:**
  - un paragrafo con il problem statement;
  - O3 elenca esattamente i tool della §8.3 e aggiunge "il resto è Forbidden";
  - L3 diventa "copre i casi d'uso quotidiani";
  - O5 usa il criterio dei 2 secondi di L789;
  - sostituire "append-only" con "tracciabilità, non prova di manomissione";
  - linkare gli ADR e §14.2 invece di creare una sezione di alternative, e aggiungere ADR per le decisioni A2 e A7.
- **Fonti:** S7 M4, M9, M10, M11, M19, M20; S8 F10; S9 M17.

#### A12. Rendere pianificabili le Fasi 2-5
- **Problema:**
  - mancano liste di file, test nominati e scelte di dipendenze;
  - i nuovi flag non sono collegati al parser CLI scritto a mano di `frix-mcp` (`--config` è usato ma non documentato);
  - `frix-web` non può riusare config e avvio che oggi stanno in `frix-mcp` senza violare la regola sulle dipendenze;
  - l'accettazione della Fase 1 non è verificabile così com'è scritta.
- **Azione:**
  - per ogni fase indicare file toccati, test nominati e criterio verificabile;
  - spostare il caricamento della config in `frix-core` oppure in un modulo condiviso;
  - documentare i flag reali.
- **Fonti:** S9 M8, M9, M12, M14, N1, N5, N11; S6 #7 (parte).

#### A13. Rendere il documento leggibile da solo
- **Problema:** termini critici per la sicurezza (Operator, Agent, Control Host, Lockout, Confirmation, Preview, Audit Entry, Initiator, Reversibility) rimandano a `GLOSSARY.md`, che non compare nel layout di §6.1 e ha un percorso ambiguo. AIN, SCPD e la notazione "C/A/P" (basta un diritto qualsiasi, o servono tutti?) non sono spiegati.
- **Azione:** inserire `GLOSSARY.md` nel layout con il percorso completo, aggiungere una definizione di una riga per ciascun termine critico alla prima occorrenza ed espandere AIN, SCPD e la legenda dei diritti.
- **Fonti:** S6 #2, #5, #7; S7 N9; S9 N12.

#### A14. Spostare il tool Smart Home di lettura alla fase giusta
- **Problema:** un tool Smart Home di lettura sta nella Fase 1, ma Homeauto è nella Fase 2 e i diritti della Fase 1 non lo coprono.
- **Azione:** spostarlo nella Fase 2, oppure dichiarare il diritto richiesto nella Fase 1.
- **Fonti:** S7 M8; S5 "Phase 1 Smart Home tool versus Phase 1 rights".

### 🟢 Nit

#### A15. Pulizie puntuali
Reversibility "—" su `check_firmware_update` (S7 N1); fallback env solo per i test non escluso in compilazione (S7 N5); bootstrap su HTTP non elencato come rischio residuo (S7 N6); rischi operativi sparsi (S7 N7); l'esempio dell'API sceglie la rete per Band (S7 N10); sottosezioni §4.2.x asimmetriche, marcatori e numerazione (S7 N11, N12); nessun link ai progetti esterni (S7 N13); §15 con fatti superati (S9 N2); descrizione del retry 503 (S9 N14); avvio che dipende dal router raggiungibile (S5); conteggi dei servizi se non risolti in A4.

#### A16. Revisione dei termini, per ultima
"escluso" e la colonna "Livello" usati per Forbidden; "utente" con tre significati; sinonimi non ammessi per Host, Control Host, Operator, Agent, Wi-Fi Network; "banda" usato per il throughput; "Dispositivi" al posto di Host; nomi dei Level e "istanza"; Router Account; "agente" e "persona" in §13.6; frasi riempitive, giudizi soggettivi, intensificatori, "innocuo" usato come etichetta di sicurezza, elisioni.
**Fonti:** S6 #8; S7 M5, M6, M7, N2, N14-N18; S8 F12; S9 M15, M16, N7, N8, N9, N10, N13.

---

## Tensioni tra i reviewer

- **Zoom-out contro inventive-catalyst (S5).** Il primo vuole meno meccanismi, il secondo di più (lock condiviso, audit prima della scrittura, ricontrolli). Lo Stage 8 decide: si adottano solo `Confirm`, l'audit prima della scrittura, la rilettura dopo la Confirmation e il Control Host con più MAC. Si rifiutano ACL Keychain, flag non modificabili, audit con hash a catena, daemon/`flock`, binding a digest e finestre di scrittura temporizzate.
- **Problem statement (S7).** Lo structural lo considera assente, il design implicito. In entrambi i casi la soluzione è un paragrafo esplicito (A11).
- **Severità (S5).** L'inventive-catalyst non può superare Major per vincolo della persona, quindi ha segnalato F1 come Major raccomandandolo come Blocker. Il synthesizer l'ha promosso, e lo Stage 7 e lo Stage 8 l'hanno confermato in modo indipendente.

## Matrice di tracciabilità

| Azione | S5 Zoom Out | S6 Comprehension | S7 Critic | S8 Zoom Out int. | S9 Readiness |
| --- | --- | --- | --- | --- | --- |
| A1 Confine di fiducia | Blocker (IC F1) | — | B1, M4 | F1, F2 | — |
| A2 `frix-web` read-only | mobile, scope web | #6 | B2 | F3 | M14 (parte) |
| A3 Level ai tool | due fonti, troppe D | — | M1, N3 | F5 | B1 |
| A4 Allineamento al codice | repo vuoto, copie §13 | #1, #4 | M2, M3, N9 | F11 | B2, B3, B5, B6, M1-M6, M13, N2-N4, N6 |
| A5 Comando test live | — | — | — | — | B4 |
| A6 Esecutore + `Confirm` | policy nel client, TOCTOU, audit dopo | #3 | M13, M15 | F4 | M7, N16 |
| A7 Stato per processo | stato per processo, rate limit, flag | — | M12 | F6 | M11 |
| A8 Fuori dalla v1 | SID, multi-router, craft, TLS, risorse | — | M16, M18, N4, N8 | F7 | N15 |
| A9 Catalogo D + spike | port forward su IP | — | M17 | F8 | M10 |
| A10 Lockout / Control Host | MAC singolo | #2 (parte), #7 (parte) | M14 | F9 | — |
| A11 Obiettivi e affermazioni | — | — | M4, M9, M10, M11, M19, M20 | F10 | M17 |
| A12 Pianificabilità fasi | — | #7 (parte) | — | — | M8, M9, M12, M14, N1, N5, N11 |
| A13 Leggibilità autonoma | — | #2, #5, #7 | N9 | — | N12 |
| A14 Smart Home in Fase 1 | nit Smart Home | — | M8 | — | — |
| A15 Pulizie | avvio legato al router | — | N1, N5-N7, N10-N13 | — | N2, N14 |
| A16 Termini | — | #8 | M5-M7, N2, N14-N18 | F12 | M15, M16, N7-N10, N13 |

Nota: le voci di S5 senza numero sono i titoli dei finding in `zoom_out_report.md`; IC indica l'inventive-catalyst.

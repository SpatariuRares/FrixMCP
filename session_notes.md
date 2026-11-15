# Note di sessione: supporto Windows per FrixMCP

* **Data:** 2026-11-15
* **Branch:** `test/windows-support`
* **Documento collegato:** [`concepts_brief.md`](concepts_brief.md) (brief in
  inglese con tutte le decisioni, le alternative e le milestone)

Queste note riassumono la conversazione, così che la si possa riprendere da un
altro PC. Il contesto della chat non viaggia con git, queste note sì.

## 1. Domanda di partenza: FrixMCP funziona su Windows?

No. Compila, ma all'avvio fallisce.

* **Bloccante:** `crates/frix-core/src/secrets.rs:26` usa
  `#[cfg(not(target_os = "linux"))]`, quindi su Windows prova a lanciare
  `/usr/bin/security`, che non esiste. Il server si ferma con
  `KeyringUnavailable`.
* **Config di default:** `crates/frix-mcp/src/cli.rs:111` deriva il percorso
  da `HOME`, che su Windows di solito non è impostata. Con `--config` il
  problema non si presenta.
* **Fine riga:** `core.autocrlf=true` e nessun `.gitattributes`, quindi le
  fixture escono in CRLF. Dalla lettura del codice il rischio sembra basso.
* **CI:** gira solo su `macos-latest` e `ubuntu-latest`.
* **README:** è scritto solo per macOS e Linux (`security`, `secret-tool`,
  `openssl`, sintassi POSIX).
* **Cosa va già bene:** `rustls` con `ring` (senza OpenSSL), `ureq`, MCP su
  stdio e i nomi dei file delle fixture sono portabili.

## 2. Obiettivo deciso

**Gli stessi comandi nel README** per gli utenti della repo pubblica, su macOS,
Linux e Windows. L'obiettivo **non** è avere un'unica implementazione.

## 3. Misura delle dipendenze (`cargo tree`)

Metodo: `cargo tree --target <triple> -e normal,build`, sul grafo di
`frix-mcp`, con lo stesso `Cargo.lock` della repo e confrontando i nomi dei
crate. Il grafo attuale è di circa 94-95 crate.

| Target | Store | Crate nuovi | Librerie C |
|---|---|---|---|
| macOS | `apple-native-keyring-store` | 7 | Security.framework |
| Windows | `windows-native-keyring-store` | **3** (byteorder, keyring-core, store) | nessuna |
| Linux | `zbus-secret-service-keyring-store` | 60 (porta anche async-io, nonostante la feature tokio) | nessuna |
| Linux | `dbus-secret-service-keyring-store` | 22 | `libdbus` |

Correzione emersa in revisione: la §14.2.1 di `docs/PROJECT.md` conta i crate
**totali** con un tetto di circa 80 (reqwest 83, ureq 21). I numeri "63/16
nuovi" vengono dal commento in `Cargo.toml`.

## 4. Decisioni prese

* macOS resta su `/usr/bin/security`. Così si evitano i dialoghi del Keychain
  per un binario non firmato dopo ogni `cargo install`.
* Linux resta su `/usr/bin/secret-tool`.
* Windows legge dal Credential Manager con `keyring-core` +
  `windows-native-keyring-store`.
* `frix-mcp set-password` esiste su tutti e tre i sistemi. FrixMCP non vede
  mai la password: lancia il comando del sistema e gli passa il terminale.
  * macOS: `security add-generic-password -U ... -w`
  * Linux: `secret-tool store ...`
  * Windows: `cmdkey /generic:<target> /user:<user> /pass`
* `frix-mcp fingerprint` usa un verificatore TLS separato che registra il
  certificato e interrompe sempre la connessione. Non si può raggiungere da
  `TlsTrust`, quindi resta vera la regola "nessuna modalità salta la verifica".
* `--help` stampa i percorsi giusti per il sistema. Su Windows la config è in
  `%APPDATA%\frixmcp\config.toml`, mentre macOS e Linux restano su
  `~/.config`.
* Tutto l'output dei sottocomandi va su **stderr**, perché stdout resta il
  canale MCP (`print_stdout = "deny"`, senza eccezioni).
* Nel README la generazione della password diventa una frase neutra ("genera
  24+ caratteri con il tuo password manager"), senza `--generate`.
* Per chi aggiorna sono accettabili passaggi manuali. In pratica non ne
  servono, perché su macOS e Linux non cambia nulla.
* **Criterio di successo riformulato:** un utente Windows con la toolchain
  Rust e gli MSVC Build Tools segue il README fino a `frix` connesso, senza
  openssl, Git Bash o WSL. Inoltre la CI è verde anche su `windows-latest` e
  passa una prova manuale su una macchina Windows reale con il 7590 (15 tool
  in `/mcp`, `get_system_info` funziona).

## 5. Alternative scartate

* **keyring su tutti i sistemi:** dialoghi del Keychain su macOS, e su Linux
  60 crate (zbus) oppure 22 più `libdbus` (dbus).
* **Modulo PowerShell CredentialManager:** è uno strumento in più da
  installare.
* **Password in una variabile d'ambiente o in un file (anche cifrato con
  DPAPI):** viola "no secrets in files".
* **WSL:** scartato perché:
  * `secret-tool` dentro WSL non ha un Secret Service attivo;
  * non chiude la falla della PowerShell;
  * è uno strumento extra, che richiede amministratore e riavvio;
  * il client MCP dovrebbe lanciare `wsl.exe`, una configurazione fragile.

## 6. I problemi principali emersi nella revisione

La revisione Goldfish (7 revisori) ha trovato 5 bloccanti, 13 problemi
importanti e 6 minori. I dettagli sono nel brief.

1. **Su Windows il README lascia una shell all'AI.** La lista dei tool negati
   non comprende la PowerShell, e il Credential Manager non distingue tra
   programmi (minaccia T13). Va risolto insieme alle istruzioni per Windows.
2. **"Senza strumenti extra" non era raggiungibile:** `openssl rand`, la
   modalità `ca`, gli MSVC Build Tools, la sintassi POSIX. Da qui il criterio
   riformulato.
3. **`fingerprint`** deve rispettare la regola sulla verifica TLS, scoprire
   la porta con `GetSecurityPort` e stampare una riga
   `pinned_sha256 = "..."` pronta da incollare.
4. **`set-password`** non può nascondere l'input su Windows senza codice
   `unsafe` o un crate in più, da cui la scelta di `cmdkey`. C'è anche una
   trappola: il README salva la password al passo 2 ma scrive la config al
   passo 4.
5. **`secrets.rs`** va diviso in un modulo per sistema, con un errore che non
   presupponga un programma esterno. Senza questo, `clippy -D warnings`
   fallisce su Windows per il codice morto.

Inoltre:

* Serve l'ADR 0003, perché la §14.2.3 viene ribaltata per Windows.
* Circa 30 punti della documentazione parlano solo di macOS o Unix.
* Le Fasi 2 e 4 restano escluse su Windows.

## 7. Da verificare prima di implementare

* `cmdkey /pass` nasconde l'input? Scrive una voce che
  `windows-native-keyring-store` rilegge, con lo stesso nome della voce e la
  stessa codifica? **Se no, la scelta su `set-password` va riaperta.**
* Per i due crate di Windows: convenzione sul nome della voce, API, MSRV
  (≤ 1.88), licenze, versione di `windows-sys` che tirano dentro.
* I nomi dei tool shell di Claude Code su Windows, da aggiungere alla lista
  dei tool negati.
* Se i runner Windows di GitHub permettono di scrivere nel Credential
  Manager.
* Se FRITZ!OS usa lo stesso certificato sulle porte 443 e 49443.

## 8. Milestone proposte

Ognuna deve lasciare la CI verde.

* **M0:** `.gitattributes`, un `cfg` esplicito per ogni sistema, Windows in
  CI.
* **M1:** percorso di default per sistema e `--help` dinamico.
* **M2:** parser dei sottocomandi e `fingerprint`.
* **M3:** lettura dello store Windows, nuovi errori, ADR 0003.
* **M4:** `set-password`.
* **M5:** revisione della documentazione e prova manuale end-to-end.

## 9. Stato dell'ambiente su questo PC (Windows 11)

* Rust installato via winget (`Rustlang.Rustup`). La toolchain 1.99.1 è
  installata da `rust-toolchain.toml`.
* Gli MSVC Build Tools non sono stati verificati: probabilmente mancano, e
  senza di loro non si compila né si lanciano i test.
* WSL non è installato.
* Le misure di `cargo tree` erano in crate di prova nello scratchpad della
  sessione, che non viene salvato. Per rifarle basta il metodo della sezione 3.

## 10. Dove eravamo rimasti

* Brainstorm EGM: completato, brief approvato.
* `/egm:egm-draft`: avviato, ma non è stata scritta nessuna sezione di
  `design_draft.md`.
* `/mattpocock-skills:implement`: avviato e interrotto prima di qualsiasi
  modifica al codice.

**Per riprendere dall'altro PC:**

```sh
git fetch
git switch test/windows-support
```

Poi, a scelta:

* `/egm:egm-draft concepts_brief.md` per scrivere il design formale;
* `/mattpocock-skills:implement` con spec `concepts_brief.md`, partendo da M0.

# FrixMCP

Server MCP e dashboard locale per FRITZ!Box, scritti in Rust, con un modello di permessi a tre livelli che limita cosa un'IA può fare sul router: letture libere, scritture dietro flag, operazioni pericolose dietro conferma della persona.

- Sola lettura di default. Scritture con `--allow-writes`, operazioni pericolose con `--allow-dangerous` e conferma esplicita (fasi successive: oggi esistono solo letture).
- Credenziali nel Keychain, TLS pinnato, nessuna porta esposta, audit di ogni modifica.
- Testato su FRITZ!Box 7590 / FRITZ!OS 8.25. Altri modelli: probabilmente funziona, non garantito.

Stato: in sviluppo (Fase 0/1). Progetto, glossario e decisioni: `docs/PROJECT.md`, `GLOSSARY.md`, `docs/adr/`.

## Struttura

- `crates/frix-core`: client TR-064, Policy, tipi di dominio (libreria).
- `crates/frix-mcp`: server MCP su stdio.
- `crates/frix-web`: dashboard su 127.0.0.1 (vuota fino alla Fase 3).

`frix-mcp` e `frix-web` dipendono da `frix-core`; `frix-core` non dipende da nessuno dei due; i due binari non si conoscono.

## Installazione

`cargo install --path crates/frix-mcp --root ~/.local` → `~/.local/bin/frix-mcp`, fuori dal repo (vedi sotto). Per creare il Router Account, salvare la password nel Keychain e scrivere la config: `docs/guida-operator-fase1.md`.

## Sviluppo

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo deny check
```

## Uso con Claude Code

Copia `.mcp.json.example` in `.mcp.json` (ignorato da git) e adatta i percorsi. Nessuna password nel file: il binario la prende dal Keychain.

Le garanzie valgono solo se l'IA non ha una shell sul tuo Mac: usa Claude Desktop, oppure Claude Code con Bash negato o in sandbox. Dettagli in `SECURITY.md` e `docs/PROJECT.md` §7.0.

## Tool disponibili

(tabella generata da `frix-mcp --list-tools`, quando esisterà)

## Sicurezza

Vedi `SECURITY.md`.

## Licenza

A scelta, MIT (`LICENSE-MIT`) oppure Apache-2.0 (`LICENSE-APACHE`).

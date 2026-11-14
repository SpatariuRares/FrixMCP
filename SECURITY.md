# Security policy

FrixMCP controlla un router domestico. Prendiamo sul serio i bug di sicurezza.

## Segnalazioni

Apri una "security advisory" privata su GitHub (Security → Report a vulnerability) oppure scrivi all'indirizzo indicato nel profilo del maintainer. Non aprire issue pubbliche per vulnerabilità. Risposta entro 7 giorni.

## Modello di sicurezza (riassunto)

- Trasporto MCP solo stdio; dashboard solo su 127.0.0.1 e in sola lettura.
- Credenziali nel Keychain macOS; mai in file di configurazione o variabili d'ambiente in produzione.
- TLS verso il router con certificato pinnato; nessuna opzione per disabilitare la verifica.
- Tre livelli di tool: `read` (default), `write` (`--allow-writes`), `dangerous` (`--allow-dangerous` + Confirmation fuori banda dell'Operator). Nella versione attuale esistono solo i tool `read`.
- Audit di ogni scrittura, con dati sensibili redatti: dà tracciabilità, non prova di manomissione (un processo dello stesso utente può riscriverlo).
- L'output del router è trattato come dato non fidato (prompt injection).

Dettagli in `docs/PROJECT.md` §7.

## Deployment supportati

Le garanzie sopra limitano gli errori dell'IA e la prompt injection. Valgono solo se l'IA non può eseguire codice come il tuo utente macOS: Claude Desktop, oppure Claude Code con Bash negato o in sandbox e scritture limitate al progetto, con `frix-mcp` installato fuori dal repo. Dettagli in `docs/PROJECT.md` §7.0.

## Cosa è fuori ambito

- Vulnerabilità del FRITZ!OS stesso (segnalarle ad AVM).
- Attacchi da un processo già in esecuzione come lo stesso utente macOS, incluso un agent con accesso alla shell (`docs/PROJECT.md` §7.0, minaccia T13).

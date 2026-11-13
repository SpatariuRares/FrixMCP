# Guida per l'Operator: chiudere la Fase 1 sul router reale

Passi da fare a mano, collegati al FRITZ!Box, per chiudere i ticket #3 e #9 e sbloccare #15.
Tutti i comandi si lanciano dalla root del repo. Riferimenti: `docs/PROJECT.md` §12 e §14.3.

## Parte 1: Router Account e config (#3)

**1. Router Account.** Dall'interfaccia web: Sistema → Utenti FRITZ!Box → Aggiungi utente.

- Nome: `frixmcp`.
- Password: generala con `openssl rand -base64 24` e non salvarla altrove, va solo nel Keychain (passo 3).
- Diritti: **solo** "Impostazioni FRITZ!Box".
- Deseleziona "Accesso anche da Internet consentito".

**2. Impronta e SAN del certificato** (non servono credenziali):

```sh
openssl s_client -connect 192.168.178.1:49443 </dev/null 2>/dev/null \
  | openssl x509 -noout -fingerprint -sha256 -ext subjectAltName
```

Annota l'impronta (`AA:BB:…`) e se nel SAN compare l'IP `192.168.178.1`.

**3. Password nel Keychain.** Il comando la chiede in modo interattivo:

```sh
security add-generic-password -s FrixMCP -a frixmcp -w
```

**4. Config** in `~/.config/frixmcp/config.toml`, fuori dal repo:

```sh
mkdir -p ~/.config/frixmcp && nano ~/.config/frixmcp/config.toml
```

```toml
host = "192.168.178.1"
username = "frixmcp"
# hide_serial_number = true

[tls]
mode = "pin"
pinned_sha256 = "AA:BB:…"   # l'impronta del passo 2
```

Le chiavi sconosciute sono un errore: non aggiungere `[audit]` o altre sezioni.

**5. Parametri Digest** dalla risposta 401, senza credenziali:

```sh
curl -sk -i -X POST https://192.168.178.1:49443/upnp/control/deviceinfo | grep -i www-authenticate
```

Annota `qop`, `algorithm` e se c'è `opaque`. Il `-k` serve solo a questo test manuale; il codice usa sempre il pinning.

## Parte 2: test live e registrazione (#9)

**6. Test live**, che fa solo letture:

```sh
cargo test -p xtask --features live --test live -- --ignored --nocapture
```

Se passa, all'avvio stampa su stderr Router Account e diritti: controlla che il Router Account sia `frixmcp` e che non abbia accesso da Internet.

| Errore | Causa |
|---|---|
| `tls_pin_mismatch` | impronta nel config sbagliata |
| 401 | utente o password sbagliati |
| 606 | diritti mancanti |
| `file di config non trovato` | manca il passo 4 |

**7. Registrazione delle fixture:**

```sh
cargo xtask record
```

- Le risposte grezze finiscono in `crates/frix-core/tests/fixtures/raw/grezze/`, che è ignorata da git e non va mai committata.
- Le copie oscurate finiscono in `crates/frix-core/tests/fixtures/raw/oscurate/`.

**8. Controllo a mano delle copie oscurate.** L'oscuramento parte da una lista di campi scritta a mano, quindi un campo imprevisto potrebbe sfuggire. Cerca i tuoi dati reali:

```sh
D=crates/frix-core/tests/fixtures/raw/oscurate
grep -rni '<hostname del router>' $D
grep -rni '<MAC del router, con e senza i due punti>' $D
grep -rni '<il tuo SSID>' $D
grep -rni '<un nome di un tuo dispositivo>' $D
grep -rnE '([0-9]{1,3}\.){3}[0-9]{1,3}' $D | grep -v '192\.168\.' | head   # IP pubblici
```

Tutti i grep devono restituire nulla, o solo valori sostituiti.

**9. Dati per il commento su #3.** Stanno già nelle copie oscurate:

```sh
grep -rh -A1 'APType\|X_AVM-DE_FrequencyBand' $D/responses/WLANConfiguration-*/X_AVM-DE_GetWLANExtInfo.xml
grep -rh 'NewDefaultConnectionService' $D/responses/Layer3Forwarding-1/
grep -rh 'NewWANAccessType' $D/responses/WANCommonInterfaceConfig-1/
```

- Il primo ti dice quale istanza 1/2/3 è `main_2g`, `main_5g` o `guest`. Controlla anche che i valori siano proprio `normal` e `guest`: il codice li presume.
- Il secondo ti dice se la connessione è PPP o IP.
- Il terzo ti dà il `WANAccessType`.

**10. Commento su #3 e chiusura delle due issue.** Il commento non deve contenere password né l'impronta completa:

```sh
gh issue comment 3 --body "Digest: qop=…, algorithm=…, opaque=sì/no. SAN: IP presente sì/no. WLAN: 1=…, 2=…, 3=…. Connessione: PPP/IP. WANAccessType: …"
gh issue close 3
gh issue close 9 --comment "Test live passato in sola lettura; fixture oscurate registrate e controllate a mano."
```

Non serve committare le fixture adesso: restano nella cartella ignorata e le sposta #15.

## Parte 3: allineamento alle fixture reali (#15)

**11.** Chiedi a Claude Code di eseguire `/implement-spec 1`: con #9 chiusa, #15 si sblocca. In #15 un agente:

- sposta le fixture oscurate in `tests/fixtures/`, rinominando `devicehostlist.lua` in `.xml` e `meshlist.lua` in `.json`;
- corregge il codice dove le risposte reali differiscono da quelle scritte a mano;
- aggiorna la §14.3 di `docs/PROJECT.md` con quello che hai verificato.

Nello stesso giro vanno confermate le scelte di Classification che si discostano dalla §4.2:

- `X_AVM-DE_GetWLANHybridMode` è Dangerous perché restituisce la passphrase;
- `WLANConfiguration:SetEnable` è Dangerous su tutte e tre le istanze;
- le scritture "fuori v1" e AppSetup, WANMobileConnection e `X_AVM-DE_DoPrepareCGI` sono Forbidden.

## Parte 4: usarlo da Claude Code (facoltativo)

**12.** Compila e configura il server MCP:

```sh
cargo build --release
cp .mcp.json.example .mcp.json      # .mcp.json è ignorato da git
```

In `.mcp.json` sostituisci i due percorsi segnaposto con quello del binario (`<repo>/target/release/frix-mcp`) e quello del config (`~/.config/frixmcp/config.toml`, scritto per esteso). Poi apri `claude` e verifica con `/mcp` che `frix` risulti connesso.

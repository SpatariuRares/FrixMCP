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
  | openssl x509 -noout -fingerprint -sha256
openssl s_client -connect 192.168.178.1:49443 </dev/null 2>/dev/null \
  | openssl x509 -noout -text | grep -A1 'Subject Alternative Name'
```

(L'openssl di macOS è LibreSSL e non conosce `-ext`: per questo il SAN si legge da `-text`.)

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
curl -sk -i -X POST https://192.168.178.1:49443/upnp/control/deviceinfo \
  -H 'Content-Type: text/xml; charset="utf-8"' \
  -H 'SOAPACTION: "urn:dslforum-org:service:DeviceInfo:1#GetInfo"' \
  -d '<?xml version="1.0"?><s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/"><s:Body><u:GetInfo xmlns:u="urn:dslforum-org:service:DeviceInfo:1"/></s:Body></s:Envelope>' \
  | grep -i www-authenticate
```

Senza l'header `SOAPACTION` il router non risponde con la sfida Digest. Annota `qop`, `algorithm` e se c'è `opaque`. Il `-k` serve solo a questo test manuale; il codice usa sempre il pinning.

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
grep -rnE '([0-9]{1,3}\.){3}[0-9]{1,3}' $D | grep -v '192\.168\.' | head
```

Sostituisci ogni `<…>` con il tuo valore reale prima di lanciare il comando. L'ultimo comando cerca IP pubblici (in zsh un `#` a fine riga non è un commento, quindi non aggiungerlo).

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
- `WLANConfiguration:SetEnable` è oggi Dangerous su tutte e tre le istanze nel registro; dalla Fase 2 il Level passa ai tool (`docs/PROJECT.md` §6.2) e `set_guest_wifi` sarà Write sull'istanza Ospiti verificata al passo 9;
- le scritture "fuori v1" e AppSetup, WANMobileConnection e `X_AVM-DE_DoPrepareCGI` sono Forbidden.

## Parte 4: usarlo da Claude Code (facoltativo)

**12.** Installa il server MCP **fuori dal repo**, così modificare il codice non cambia il programma che parla col router (`docs/PROJECT.md` §7.0):

```sh
cargo install --path crates/frix-mcp --root ~/.local     # → ~/.local/bin/frix-mcp
cp .mcp.json.example .mcp.json                           # .mcp.json è ignorato da git
```

In `.mcp.json` sostituisci `<utente>` nei due percorsi (binario e config, scritti per esteso). Dopo ogni modifica al codice che vuoi usare, ripeti `cargo install`.

Le garanzie di FrixMCP valgono solo se l'IA non ha una shell sul Mac: usalo da Claude Desktop, oppure da Claude Code con Bash negato o in sandbox. Oggi esistono solo letture, quindi il rischio pratico è basso, ma la regola conta dalla Fase 2. Poi apri il client e verifica che `frix` risulti connesso (in Claude Code: `/mcp`).

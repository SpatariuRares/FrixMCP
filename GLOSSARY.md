# FrixMCP

Server MCP e dashboard locale che permettono a un'IA e a una persona di osservare e governare un FRITZ!Box di casa con lo stesso modello di permessi.

## Persone e identità

**Operator**:
La persona umana che usa FrixMCP; è l'unica che può confermare un'operazione Dangerous.
_Avoid_: utente, user (da solo)

**Agent**:
L'IA che usa FrixMCP tramite MCP; può proporre operazioni Dangerous ma mai confermarle.
_Avoid_: Claude, il modello, mcp (come nome di attore)

**Initiator**:
Chi ha chiesto una scrittura: l'Agent oppure l'Operator.
_Avoid_: origine, origin, mcp/web

**Router Account**:
L'utente FRITZ!OS con cui FrixMCP si autentica sul router.
_Avoid_: utente, user (da solo), admin

**Control Host**:
L'Host su cui girano FrixMCP e la sua dashboard; FrixMCP non deve mai tagliarlo fuori dalla rete.
_Avoid_: il Mac, macchina locale, localhost

## Rete

**Host**:
Qualunque apparecchio che il router conosce nella sua lista host, inclusi i nodi della mesh; la sua identità è il MAC, mai l'IP.
_Avoid_: dispositivo, device, client (da soli)

**SmartHome Device**:
Un apparecchio domotico gestito dal router e identificato da un AIN (presa, termostato, sensore).
_Avoid_: dispositivo, device (da soli)

**Wi-Fi Network**:
Una delle tre reti Wi-Fi del router: Principale 2,4 GHz, Principale 5 GHz, Ospiti.
_Avoid_: banda (per indicare una rete), WLAN, istanza

**Band**:
La frequenza radio su cui trasmette una Wi-Fi Network (2,4 GHz o 5 GHz).
_Avoid_: rete (per indicare una frequenza)

## Permessi

**Level**:
Il grado di rischio di un'operazione esposta: Read, Write o Dangerous.
_Avoid_: livello X, excluded (non è un Level)

**Classification**:
La decisione presa per ogni azione del router: esposta con un Level, oppure Forbidden.

**Forbidden**:
Classification di un'azione del router che FrixMCP non può eseguire in nessun caso, perché assente dal programma.
_Avoid_: excluded, livello X, disabilitata

**Confirmation**:
Il consenso dato dall'Operator a una singola operazione Dangerous, su un canale che l'IA non può leggere né azionare.
_Avoid_: confirm token, conferma in chat

**Preview**:
La descrizione dell'effetto di una scrittura, calcolata senza eseguirla; è ciò che l'Operator vede prima di una Confirmation.
_Avoid_: dry run (è il nome del parametro, non del concetto), anteprima, riassunto

**Lockout**:
La perdita di accesso del Control Host al router che non si risolve da sola; FrixMCP la impedisce sempre, a qualunque Level.
_Avoid_: disconnessione (un riavvio disconnette ma non è un Lockout)

## Tracciabilità

**Audit Entry**:
La registrazione di una scrittura eseguita: chi l'ha chiesta, cosa, lo stato prima e lo stato dopo.
_Avoid_: log (il log è quello del router)

**Reversibility**:
Proprietà dichiarata di ogni operazione di scrittura: Reversible se riapplicare lo stato precedente ne annulla l'effetto, Irreversible altrimenti.
_Avoid_: rollback, undo

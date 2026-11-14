# Confirmation fuori banda, non token in-band

Le operazioni Dangerous richieste dall'Agent si eseguono solo dopo una Confirmation dell'Operator data in un dialog nativo di macOS mostrato da `frix-mcp`, con la Preview dell'effetto; il dialog scade dopo 120 s e la scadenza o la chiusura valgono come rifiuto. Abbiamo scartato il `confirm_token` monouso restituito nella conversazione (proposto in origine in `docs/PROJECT.md` §8.0) perché l'Agent riceve il token e può rimandarlo da solo, anche sotto prompt injection: sembrava una conferma umana senza esserlo.

## Consequences

- `frix-mcp` deve poter mostrare un dialog sul Control Host, quindi le operazioni Dangerous da MCP funzionano solo con un Operator davanti al Mac.
- L'Agent riceve soltanto "Confirmation negata o scaduta", mai il motivo.
- Dalla dashboard la Confirmation è il dialog della UI, perché lì l'Initiator è già l'Operator.

## Aggiornamento 2026-11-13

- La proprietà richiesta è il canale: la Confirmation passa da un canale che l'Agent non può leggere né azionare. Il dialog nativo è il meccanismo candidato, **[DA VERIFICARE]** con lo spike della Fase 4 (`docs/PROJECT.md` §11): un processo dello stesso utente potrebbe premere il pulsante, e 120 s potrebbero superare il timeout del client MCP. Se lo spike fallisce, i tool Dangerous restano Forbidden.
- La terza conseguenza è sospesa: in v1 `frix-web` è in sola lettura (`docs/PROJECT.md` §6.4). Le scritture dalla dashboard richiederanno un ADR su come la dashboard autentica l'Operator, perché oggi qualunque processo locale può aprire `127.0.0.1`.
- La garanzia vale nei deployment supportati di `docs/PROJECT.md` §7.0: un Agent con una shell sul Control Host può aggirare qualunque dialog.

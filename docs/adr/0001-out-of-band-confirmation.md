# Confirmation fuori banda, non token in-band

Le operazioni Dangerous richieste dall'Agent si eseguono solo dopo una Confirmation dell'Operator data in un dialog nativo di macOS mostrato da `frix-mcp`, con la Preview dell'effetto; il dialog scade dopo 120 s e la scadenza o la chiusura valgono come rifiuto. Abbiamo scartato il `confirm_token` monouso restituito nella conversazione (proposto in origine in `docs/PROJECT.md` §8.0) perché l'Agent riceve il token e può rimandarlo da solo, anche sotto prompt injection: sembrava una conferma umana senza esserlo.

## Consequences

- `frix-mcp` deve poter mostrare un dialog sul Control Host, quindi le operazioni Dangerous da MCP funzionano solo con un Operator davanti al Mac.
- L'Agent riceve soltanto "Confirmation negata o scaduta", mai il motivo.
- Dalla dashboard la Confirmation è il dialog della UI, perché lì l'Initiator è già l'Operator.

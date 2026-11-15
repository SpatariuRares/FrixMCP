# Out-of-band Confirmation, not an in-band token

Dangerous operations requested by the Agent run only after a Confirmation from the Operator, given in a native macOS dialog shown by `frix-mcp` with the Preview of the effect; the dialog expires after 120 s, and expiry or closing count as a refusal. We rejected the single-use `confirm_token` returned in the conversation (originally proposed in `docs/PROJECT.md` §8.0) because the Agent receives the token and can send it back by itself, even under prompt injection: it looked like a human confirmation without being one.

## Consequences

- `frix-mcp` must be able to show a dialog on the Control Host, so Dangerous operations from MCP work only with an Operator in front of the Mac.
- The Agent only receives "Confirmation denied or expired", never the reason.
- From the dashboard the Confirmation is the UI dialog, because there the Initiator is already the Operator.

## Update 2026-11-13

- The required property is the channel: the Confirmation goes through a channel the Agent can neither read nor operate. The native dialog is the candidate mechanism, **[TO VERIFY]** with the Phase 4 spike (`docs/PROJECT.md` §11): a process running as the same user could press the button, and 120 s could exceed the MCP client's timeout. If the spike fails, the Dangerous tools stay Forbidden.
- The third consequence is suspended: in v1 `frix-web` is read-only (`docs/PROJECT.md` §6.4). Writes from the dashboard will need an ADR on how the dashboard authenticates the Operator, because today any local process can open `127.0.0.1`.
- The guarantee holds in the supported deployments of `docs/PROJECT.md` §7.0: an Agent with a shell on the Control Host can bypass any dialog.

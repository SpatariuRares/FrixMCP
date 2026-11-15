# Security policy

FrixMCP controls a home router. We take security bugs seriously.

## Reporting

Open a private security advisory on GitHub (Security → Report a vulnerability) or write to the address listed on the maintainer's profile. Do not open public issues for vulnerabilities. Response within 7 days.

## Security model (summary)

- MCP transport over stdio only; dashboard on 127.0.0.1 only, and read-only.
- Credentials in the system keyring (macOS Keychain, Secret Service on Linux); never in configuration files or environment variables in production.
- TLS to the router with a pinned certificate; no option to disable verification.
- Three tool levels: `read` (default), `write` (`--allow-writes`), `dangerous` (`--allow-dangerous` + out-of-band Confirmation by the Operator). The current version has only `read` tools.
- Audit of every write, with sensitive data redacted: it gives traceability, not tamper evidence (a process running as the same user can rewrite it).
- Router output is treated as untrusted data (prompt injection).

Details in `docs/PROJECT.md` §7.

## Supported deployments

The guarantees above limit AI mistakes and prompt injection. They hold only if the AI cannot run code as your macOS user: Claude Desktop, or Claude Code with Bash denied or sandboxed and writes limited to the project, with `frix-mcp` installed outside the repo. Details in `docs/PROJECT.md` §7.0.

## Out of scope

- Vulnerabilities in FRITZ!OS itself (report them to AVM).
- Attacks from a process already running as the same macOS user, including an agent with shell access (`docs/PROJECT.md` §7.0, threat T13).

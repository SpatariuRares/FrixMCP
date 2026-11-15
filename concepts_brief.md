# Windows support with one set of setup instructions

* **Author:** Rares.Spatariu.extcefla
* **Date:** 2026-11-15

## Overview

FrixMCP runs today on macOS and Linux only. On Windows it compiles, but it
fails at startup: `secrets.rs:26` sends every non-Linux target to
`/usr/bin/security`, which does not exist on Windows.

The goal is that a user of the public repository can follow **the same README
steps** on macOS, Linux and Windows. One implementation for all three is not a
goal. Each OS keeps its own password store:

| OS | Password store | How FrixMCP reads it |
|----|----------------|----------------------|
| macOS | Keychain | `/usr/bin/security` (unchanged) |
| Linux | Secret Service | `/usr/bin/secret-tool` (unchanged) |
| Windows | Credential Manager | `keyring-core` 1.0.1 + `windows-native-keyring-store` 1.1.0 |

The plan also adds three things that make the README the same on every OS:

* `frix-mcp set-password` stores the router account password.
* `frix-mcp fingerprint` prints the router's certificate fingerprint, so
  `openssl` is no longer needed.
* `--help` prints the right config path for the OS. On Windows that is
  `%APPDATA%\frixmcp\config.toml`.

Users can be asked to do manual steps when they upgrade.

**Success (restated after review):**

* A Windows user who has the Rust toolchain and its documented prerequisites
  (on Windows, the MSVC Build Tools) follows the README from install to `frix`
  connected in Claude Code or Claude Desktop.
* They need no other tool: no openssl, no Git Bash, no WSL.
* CI passes on `windows-latest` as well as on macOS and Linux.
* A manual end-to-end check on a real Windows machine against the 7590 passes:
  `/mcp` shows 15 tools, and `get_system_info` succeeds.

The first version said "without installing extra tools". That cannot hold,
because `cargo install` on Windows needs the MSVC Build Tools.

## Analysis

Seven independent reviewers looked at the plan. After merging overlapping
points, they found 5 blockers, 13 major issues and 6 minor ones.

### Correction to the measured facts

* `docs/PROJECT.md` §14.2.1 (line 987) counts **total** crates against a limit
  of about 80: reqwest 83, ureq 21. The "63 new / 16 new" numbers come from
  the `# why:` comment in `Cargo.toml:37-41`.
* The Windows store adds 3 crates, so the total goes from about 95 to 98. That
  stays under either reading of the rule.
* The "+3" holds only if the store crate uses a `windows-sys` version that is
  already in `Cargo.lock` (0.52.0 or 0.61.2).

### Blockers

1. **"No extra tools" cannot be met as written.**
   * README step 1 makes the password with `openssl rand`.
   * `tls.mode = "ca"` needs openssl to export `router.der`.
   * `cargo install` on Windows needs the MSVC Build Tools. `ring` compiles C
     and assembly.
   * Steps 2 and 3 use POSIX shell syntax (`</dev/null`, `\` line breaks),
     which PowerShell does not accept.

   Restate the goal as "nothing beyond the Rust toolchain and its documented
   prerequisites", and list MSVC Build Tools under Requirements.
2. **The README's deny list does not remove the shell on Windows.**
   * README.md:115-122 denies `Bash` and the file tools. On Windows, Claude
     Code can also offer a PowerShell tool.
   * Any process of the same user can read a Credential Manager entry, so a
     model with PowerShell can take the password and call the router
     directly. That is threat T13, and every guarantee in §7.0 depends on
     blocking it.
   * The deny list needs the Windows shell tool names. "same macOS user" in
     PROJECT.md and SECURITY.md must become "same OS user".
3. **`fingerprint` clashes with the rule that no TLS mode skips verification**
   (tls.rs:1-3, §7.4.5).
   * Two ways to build it:
     * Reuse `PinVerifier` with a pin that can never match, and print the
       `found` value from `TlsPinMismatch`. This adds no new TLS code.
     * Write a separate verifier that only records the certificate and always
       aborts.
   * Either way, it must be a small public function in frix-core, and it must
     send no credentials.
   * The port must come from `GetSecurityPort`, as startup does
     (startup.rs:31-33), not from a hardcoded 49443.
   * The output must be a line ready to paste as `pinned_sha256 = "..."`.
   * This is trust-on-first-use, the same as the openssl step today. Write
     that down as a known risk.
4. **`set-password` cannot hide the typed password on Windows without new
   code or a new crate.**
   * `forbid(unsafe_code)` rules out console API calls in our own code.
     `rpassword` would add crates that were not counted.
   * Proposed fix: FrixMCP never handles the password. It runs the OS tool
     with the terminal passed through:
     * macOS: `security add-generic-password -U ... -w`. Without `-U`, a
       second run fails.
     * Linux: `secret-tool store ...`.
     * Windows: `cmdkey /generic:<target> /user:<user> /pass`.
   * Not verified: that `cmdkey` hides the input, and that it writes the
     target name and encoding the crate reads.
   * Step-order trap: the README stores the password at step 2 but writes the
     config at step 4. A `set-password` that loads the full config fails with
     "missing `host`".
5. **The secrets code and its errors assume a command-line backend.**
   * `cfg(not(target_os = "linux"))` puts Windows on the macOS path.
   * `KeyringUnavailable { program }` assumes an external program.
   * On Windows the parsing helpers become dead code, so
     `clippy -D warnings` fails.
   * Fix: one module per OS, each with the same `read`/`store` interface, and
     a `compile_error!` for any other OS. Add a backend-neutral error with
     en/it texts. Wrap the crate's `String` result in `SecretString` right
     away. Do not register a process-wide default store.

### Major issues

* **The Credential Manager entry is not defined.** It has a target name, not
  a service/account pair. Fix the target name and credential type explicitly,
  test them, and put the real name in error messages and docs.
* **"Same README" is not defined.** Paths in the MCP JSON need `\\` or `/` on
  Windows. Windows paths inside TOML basic strings are escape errors. Claude
  Desktop keeps its config in a different place. The install docs already
  disagree on `~/.cargo/bin` versus `~/.local/bin`. Say which steps are
  identical, and let the binary print the paths that differ by OS. A
  `print-mcp-config` command could print the JSON already correctly escaped.
* **The CLI parser only understands flags** (cli.rs:50-77). Subcommands need
  their own dispatch, an "unknown command" error, a flag list per subcommand,
  and a check that `cargo xtask record` still works, since it reuses the
  parser.
* **stdout is the MCP channel** (`print_stdout = "deny"`,
  `reserved_stdout.rs`). Subcommand output goes to stderr, or through one
  audited exception. Server mode must never prompt for input.
* **Default config path.** Today only `HOME` is read, and HOME is usually
  unset on Windows. `usage()` is a fixed string. The tests isolate themselves
  by overriding `HOME` alone. Pass the environment in as a parameter,
  override `APPDATA` in tests too, and keep `~/.config` on macOS.
* **Decision record.** §14.2.3 rejects the keyring crate. It is now reversed
  for Windows, so write ADR 0003 and mark §14.2.3 as superseded.
* **Documentation sweep.** Around 30 places in PROJECT.md, README.md,
  SECURITY.md, the operator guide and doc comments are written for macOS or
  Unix only.
* **Scope.** Phases 2 and 4 (native confirmation dialog, audit file with
  0600 and `O_APPEND`) are designed for macOS only. Windows support covers
  Phase 1 (read-only).
* **Dependencies.** Put the crates under `[target.'cfg(windows)']` in
  frix-core. Check their MSRV against 1.88, their licenses, and that the
  default-off features still include the store.
* **What CI proves.** A green `windows-latest` run proves that the code
  compiles and that the unit and fixture tests pass, nothing more. Add a
  manual end-to-end check on Windows against the 7590: `/mcp` shows 15
  tools and `get_system_info` succeeds.
* **Error messages should give the fix**: "run `frix-mcp set-password`",
  "run `frix-mcp fingerprint`".
* **Migration.** Nothing changes on macOS or Linux, and no Windows users
  exist yet. Write this down explicitly.
* **Rollout.** Ship as v0.2.0 with a CHANGELOG entry, and update the README
  tag.

### Minor issues

* Add a `.gitattributes` with `* text=auto eol=lf` and `*.der binary`. The
  code probably handles CRLF, but this removes the risk.
* Name one domain term, **Password Store**, and stop using "Keychain" in code
  that runs on every OS.
* Reword "read-only" as "read-only towards the router".
* Windows Notepad can save the file as `config.toml.txt` or add a BOM. Say
  how to handle both.
* Optional extra commands:
  * `frix-mcp check` connects, prints the banner and exits.
  * `frix-mcp init` walks through the whole setup.
  * `set-password --generate` creates the password.
  * `fingerprint --save-der` saves the certificate for `ca` mode.

### Ideal final result

The README becomes: install, run one command, paste the printed block. The
binary knows the per-OS facts (paths, store name, fingerprint), so the docs
do not have to repeat them.

## Goals vs. Non-Goals

* **In-Scope Goals:**
  * FrixMCP starts and reads the router on Windows (x86_64, MSVC target).
  * The same README commands on all three OSes, with a clear list of the
    steps that can only differ (prerequisites, paths, client config).
  * `set-password` and `fingerprint` work the same way on every OS.
  * `--help` prints the paths for the current OS.
  * CI runs on `windows-latest`, plus a manual end-to-end check on Windows.
  * ADR 0003 and the documentation sweep.
* **Out-of-Scope Non-Goals:**
  * Phases 2 and 4 on Windows (write tools, confirmation dialog, audit file).
  * Prebuilt Windows binaries, an MSI installer, code signing.
  * WSL, Windows on ARM, BSD.
  * Automatic migration of existing items or config.
  * Changing the macOS or Linux backend.
  * Support for `XDG_CONFIG_HOME`.

## Alternatives Weighed

* **keyring-core on all three OSes.** Rejected by author: on macOS it risks
  Keychain prompts for an unsigned binary after every update. Linux costs 60
  extra crates with zbus, or 22 plus the system `libdbus` with dbus.
* **zbus Secret Service store on Linux.** Rejected: 60 new crates and a
  second async runtime next to tokio.
* **dbus Secret Service store on Linux.** Rejected by author: it adds a C
  library, and `secret-tool` already works.
* **A system command on Windows.** Not possible: no built-in tool can print a
  stored generic credential (`cmdkey` cannot).
* **PowerShell CredentialManager module.** Rejected: it is an extra tool to
  install.
* **Password in an environment variable or a file (including a DPAPI
  encrypted file).** Rejected: it breaks "no secrets in files" and goal O4.
* **Windows support through WSL (the Linux build running inside WSL).**
  Rejected by author:
  * `secret-tool` needs a Secret Service (gnome-keyring over D-Bus), which WSL
    does not start by default.
  * A Windows-side shell can still run `wsl secret-tool lookup`, so the T13
    gap stays open.
  * WSL is an extra tool that needs admin rights and a reboot, which breaks
    the success criterion.
  * The MCP client would have to launch `wsl.exe -e ...`, which fails
    silently when WSL is stopped or the distribution name differs.
  * Not verified on this machine, because WSL is not installed.
* **A `set-password` that only wraps `security` and `secret-tool`, with no
  Windows support.** Rejected by author: Windows is required for public
  users.

## Key Decisions & Staging for Next Steps

**Decided:**

* The goal is the same instructions, not the same implementation.
* macOS stays on `security`, and Linux stays on `secret-tool`.
* Windows uses keyring-core + windows-native-keyring-store.
* Manual upgrade steps are acceptable.
* `--help` prints the OS-specific paths.
* `set-password` stays on all three OSes (resolves O2). FrixMCP never handles
  the password: it runs the OS tool and passes the terminal through.
  * macOS: `security add-generic-password -U ... -w`.
  * Linux: `secret-tool store`.
  * Windows: `cmdkey /generic:<target> /user:<user> /pass`.
* The success criterion is restated as in the Overview (resolves O6).

**Decisions taken during review (all resolved):**

* **O1.** Resolved: a separate verifier that only records the certificate and
  always aborts. It cannot be reached from `TlsTrust`, and a test checks that
  `TlsTrust` still has only Pin and Ca. Reusing `PinVerifier` with an
  impossible pin was rejected: it would tie `fingerprint` to how an error is
  formatted.
* **O2.** Resolved: keep `set-password`, with `cmdkey` on Windows.
* **O7.** Resolved: no WSL path (see Alternatives Weighed).
* **O3.** Resolved: `%APPDATA%\frixmcp\config.toml` on Windows, and macOS and
  Linux stay on `~/.config`. Tests override both `HOME` and `APPDATA`.
* **O4.** Resolved: all subcommand output goes to stderr, and
  `print_stdout = "deny"` keeps no exceptions.
* **O5.** Resolved: neutral wording in the README ("generate 24+ characters
  with your password manager"), with no `--generate`. That keeps the secret
  off the screen, and `set-password` stays a pass-through to the OS tool.
* **O6.** Resolved: success goal restated in the Overview.

**Verify first:**

* The crates: target-name convention, encoding, store API, MSRV, licenses,
  and which `windows-sys` version they pull in.
* Whether `cmdkey /pass` hides the input, and whether its entry is
  compatible with what the crate reads.
* The names of Claude Code's shell tools on Windows.
* Whether hosted Windows runners allow writes to Credential Manager.
* Whether FRITZ!OS serves the same certificate on ports 443 and 49443.

**Proposed milestones**, each keeping CI green:

* **M0:** `.gitattributes`, explicit per-OS cfgs, Windows in CI.
* **M1:** OS default path and dynamic `--help`.
* **M2:** subcommand parser and `fingerprint`.
* **M3:** reading the Windows store, new error variants, ADR 0003.
* **M4:** `set-password`.
* **M5:** documentation sweep and manual end-to-end check.

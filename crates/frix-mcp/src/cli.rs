//! Flag di `frix-mcp` (docs/PROJECT.md §6.3, §13.5). Analisi a mano: pochi flag, e
//! nessun testo di aiuto o versione stampato su stdout, che è il canale MCP.
//!
//! Non esiste alcun flag per disattivare la verifica TLS: un flag sconosciuto è un errore.

use std::path::{Path, PathBuf};

use frix_core::FrixError;
use frix_core::config::{ConfigFile, RouterConfig, TlsMode};

/// Testo di aiuto, per stderr.
pub const USAGE: &str = "\
uso: frix-mcp [--config <file>] [--host <ip>] [--username <utente>] [--http-port <porta>]
              [--tls-mode pin|ca] [--tls-pin <sha256>] [--tls-ca-file <file.der>]

Server MCP su stdio. Config di default: ~/.config/frixmcp/config.toml.
I flag sovrascrivono il file. La password si legge dal Keychain
(service FrixMCP, account = username).";

/// Argomenti già interpretati.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cli {
    /// `--help`: stampare l'aiuto su stderr e uscire.
    pub help: bool,
    /// `--config`: file di configurazione esplicito (deve esistere).
    pub config: Option<PathBuf>,
    /// Valori dei flag, che vincono sul file.
    pub overrides: ConfigFile,
}

/// Interpreta gli argomenti (senza il nome del programma).
pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<Cli, FrixError> {
    let mut cli = Cli::default();
    let mut args = args.into_iter();
    while let Some(flag) = args.next() {
        if flag == "--help" || flag == "-h" {
            cli.help = true;
            continue;
        }
        let (nome, valore) = if let Some((nome, valore)) = flag.split_once('=') {
            (nome.to_owned(), valore.to_owned())
        } else {
            let valore = args
                .next()
                .ok_or_else(|| FrixError::Config(format!("il flag {flag} richiede un valore")))?;
            (flag, valore)
        };
        let o = &mut cli.overrides;
        match nome.as_str() {
            "--config" => cli.config = Some(PathBuf::from(valore)),
            "--host" => o.host = Some(valore),
            "--username" => o.username = Some(valore),
            "--http-port" => {
                o.http_port =
                    Some(valore.parse().map_err(|_| {
                        FrixError::Config(format!("--http-port non valido: {valore}"))
                    })?);
            }
            "--tls-mode" => o.tls.mode = Some(valore.parse::<TlsMode>()?),
            "--tls-pin" => o.tls.pinned_sha256 = Some(valore),
            "--tls-ca-file" => o.tls.ca_file = Some(PathBuf::from(valore)),
            altro => {
                return Err(FrixError::Config(format!(
                    "flag sconosciuto: {altro}\n{USAGE}"
                )));
            }
        }
    }
    Ok(cli)
}

/// Config di default: `~/.config/frixmcp/config.toml`.
#[must_use]
pub fn default_config_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| {
        PathBuf::from(home)
            .join(".config")
            .join("frixmcp")
            .join("config.toml")
    })
}

/// File di config (esplicito o di default) più flag, validati.
///
/// Un `--config` esplicito deve esistere; quello di default può mancare se i flag
/// bastano. Un `ca_file` relativo nel file si risolve dalla cartella del file;
/// uno passato come flag, dalla cartella corrente.
pub fn load(cli: &Cli) -> Result<RouterConfig, FrixError> {
    let (percorso, obbligatorio) = match &cli.config {
        Some(p) => (Some(p.clone()), true),
        None => (default_config_path(), false),
    };
    let mut file = ConfigFile::default();
    if let Some(percorso) = percorso {
        match std::fs::read_to_string(&percorso) {
            Ok(testo) => {
                file = ConfigFile::parse(&testo)?;
                let base = percorso.parent().unwrap_or_else(|| Path::new("."));
                if let Some(ca) = &file.tls.ca_file {
                    file.tls.ca_file = Some(base.join(ca));
                }
            }
            Err(e) if obbligatorio || e.kind() != std::io::ErrorKind::NotFound => {
                return Err(FrixError::Config(format!("{}: {e}", percorso.display())));
            }
            Err(_) => {}
        }
    }
    file.overridden_by(cli.overrides.clone())
        .resolve(Path::new("."))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn args(testo: &str) -> Vec<String> {
        testo.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn flag_riconosciuti() {
        let cli = parse(args(
            "--config /tmp/c.toml --host 10.0.0.1 --username frix --http-port=49000 --tls-mode ca --tls-ca-file r.der",
        ))
        .unwrap();

        assert_eq!(cli.config, Some(PathBuf::from("/tmp/c.toml")));
        assert_eq!(cli.overrides.host.as_deref(), Some("10.0.0.1"));
        assert_eq!(cli.overrides.username.as_deref(), Some("frix"));
        assert_eq!(cli.overrides.http_port, Some(49000));
        assert_eq!(cli.overrides.tls.mode, Some(TlsMode::Ca));
        assert_eq!(cli.overrides.tls.ca_file, Some(PathBuf::from("r.der")));
    }

    #[test]
    fn nessun_flag_per_disattivare_la_verifica_tls() {
        for riga in [
            "--insecure",
            "--insecure-http",
            "--no-verify",
            "--tls-insecure",
            "--danger-accept-invalid-certs",
            "--tls-mode none",
            "--tls-mode insecure",
            "--tls-mode off",
        ] {
            let errore = parse(args(riga)).unwrap_err();
            assert_eq!(errore.kind(), "config", "{riga}");
        }
    }

    #[test]
    fn config_esplicito_mancante_e_un_errore() {
        let cli = parse(args("--config /percorso/che/non/esiste.toml")).unwrap();
        let errore = load(&cli).unwrap_err();
        assert!(errore.to_string().contains("non/esiste.toml"), "{errore}");
    }
}

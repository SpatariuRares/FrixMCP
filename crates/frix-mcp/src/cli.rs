//! `frix-mcp` flags (docs/PROJECT.md §6.3, §13.5). Parsed by hand: there are few flags,
//! and no help or version text is printed on stdout, which is the MCP channel.
//!
//! There is no flag to disable TLS verification: an unknown flag is an error.

use std::path::{Path, PathBuf};

use frix_core::FrixError;
use frix_core::config::{ConfigFile, RouterConfig, TlsMode};
use frix_core::error::ConfigError;
use frix_core::language::Language;

/// Help text, for stderr, in `language`.
#[must_use]
pub fn usage(language: Language) -> &'static str {
    match language {
        Language::En => {
            "\
usage: frix-mcp [--config <file>] [--host <ip>] [--username <user>] [--http-port <port>]
                [--tls-mode pin|ca] [--tls-pin <sha256>] [--tls-ca-file <file.der>]

MCP server on stdio. Default config: ~/.config/frixmcp/config.toml.
Flags override the file. The password is read from the keyring
(service FrixMCP, account = username)."
        }
        Language::It => {
            "\
uso: frix-mcp [--config <file>] [--host <ip>] [--username <utente>] [--http-port <porta>]
              [--tls-mode pin|ca] [--tls-pin <sha256>] [--tls-ca-file <file.der>]

Server MCP su stdio. Config di default: ~/.config/frixmcp/config.toml.
I flag sovrascrivono il file. La password si legge dal portachiavi
(service FrixMCP, account = username)."
        }
    }
}

/// Parsed arguments.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cli {
    /// `--help`: print the help on stderr and exit.
    pub help: bool,
    /// `--config`: explicit configuration file (must exist).
    pub config: Option<PathBuf>,
    /// Flag values, which take precedence over the file.
    pub overrides: ConfigFile,
}

/// Flags that take a value: the same ones matched in [`parse`].
const FLAGS: [&str; 7] = [
    "--config",
    "--host",
    "--username",
    "--http-port",
    "--tls-mode",
    "--tls-pin",
    "--tls-ca-file",
];

/// Parses the arguments (without the program name).
pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<Cli, FrixError> {
    let mut cli = Cli::default();
    let mut args = args.into_iter();
    while let Some(flag) = args.next() {
        if flag == "--help" || flag == "-h" {
            cli.help = true;
            continue;
        }
        let (name, inline) = match flag.split_once('=') {
            Some((name, value)) => (name.to_owned(), Some(value.to_owned())),
            None => (flag, None),
        };
        // An unknown flag is rejected before reading its value: otherwise it would
        // swallow the next argument, or end with "requires a value".
        if !FLAGS.contains(&name.as_str()) {
            return Err(FrixError::Config(ConfigError::UnknownFlag(name)));
        }
        let value = match inline {
            Some(value) => value,
            None => args
                .next()
                .ok_or_else(|| FrixError::Config(ConfigError::FlagNeedsValue(name.clone())))?,
        };
        let o = &mut cli.overrides;
        match name.as_str() {
            "--config" => cli.config = Some(PathBuf::from(value)),
            "--host" => o.host = Some(value),
            "--username" => o.username = Some(value),
            "--http-port" => {
                o.http_port =
                    Some(value.parse().map_err(|_| {
                        FrixError::Config(ConfigError::InvalidHttpPort(value.clone()))
                    })?);
            }
            "--tls-mode" => o.tls.mode = Some(value.parse::<TlsMode>()?),
            "--tls-pin" => o.tls.pinned_sha256 = Some(value),
            "--tls-ca-file" => o.tls.ca_file = Some(PathBuf::from(value)),
            other => {
                return Err(FrixError::Config(ConfigError::UnknownFlag(
                    other.to_owned(),
                )));
            }
        }
    }
    Ok(cli)
}

/// Default config: `~/.config/frixmcp/config.toml`.
#[must_use]
pub fn default_config_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| {
        PathBuf::from(home)
            .join(".config")
            .join("frixmcp")
            .join("config.toml")
    })
}

/// The `--config` value among raw arguments, also when other flags are invalid: the
/// language of the error about an invalid flag comes from that file.
#[must_use]
pub fn config_flag(args: &[String]) -> Option<PathBuf> {
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if arg == "--config" {
            return args.next().map(PathBuf::from);
        }
        if let Some(value) = arg.strip_prefix("--config=") {
            return Some(PathBuf::from(value));
        }
    }
    None
}

/// Language of the config file (explicit or default), for messages printed before the
/// configuration is valid: help and startup errors. English if the file is missing or
/// unreadable.
#[must_use]
pub fn file_language(config: Option<&Path>) -> Language {
    config
        .map(Path::to_path_buf)
        .or_else(default_config_path)
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| ConfigFile::parse(&text).ok())
        .and_then(|file| file.language)
        .unwrap_or_default()
}

/// Config file (explicit or default) plus flags, validated.
///
/// An explicit `--config` must exist; the default one may be missing if the flags
/// are enough. A relative `ca_file` in the file resolves from the file's directory;
/// one passed as a flag, from the current directory.
pub fn load(cli: &Cli) -> Result<RouterConfig, FrixError> {
    load_with_default(cli, default_config_path())
}

/// [`load`] with the default path passed in from outside, for tests.
fn load_with_default(cli: &Cli, default: Option<PathBuf>) -> Result<RouterConfig, FrixError> {
    let (path, required) = match &cli.config {
        Some(p) => (Some(p.clone()), true),
        None => (default, false),
    };
    let mut file = ConfigFile::default();
    // A missing default is not an error if the flags are enough; if they are not, the
    // error says where the file was looked for, not just the name of the missing key.
    let mut missing_default = None;
    if let Some(path) = path {
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                file = ConfigFile::parse(&text)?;
                let base = path.parent().unwrap_or_else(|| Path::new("."));
                if let Some(ca) = &file.tls.ca_file {
                    file.tls.ca_file = Some(base.join(ca));
                }
            }
            Err(e) if required || e.kind() != std::io::ErrorKind::NotFound => {
                return Err(FrixError::Config(ConfigError::ReadFile {
                    path: path.display().to_string(),
                    error: e.to_string(),
                }));
            }
            Err(_) => missing_default = Some(path),
        }
    }
    file.overridden_by(cli.overrides.clone())
        .resolve(Path::new("."))
        .map_err(|error| match (error, missing_default) {
            (FrixError::Config(cause), Some(path)) => {
                FrixError::Config(ConfigError::DefaultFileMissing {
                    cause: Box::new(cause),
                    path: path.display().to_string(),
                })
            }
            (error, _) => error,
        })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn args(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn recognized_flags() {
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
    fn no_flag_disables_tls_verification() {
        for line in [
            "--insecure",
            "--insecure-http",
            "--no-verify",
            "--tls-insecure",
            "--danger-accept-invalid-certs",
            "--tls-mode none",
            "--tls-mode insecure",
            "--tls-mode off",
        ] {
            let error = parse(args(line)).unwrap_err();
            assert_eq!(error.kind(), "config", "{line}");
        }
    }

    #[test]
    fn missing_explicit_config_is_an_error() {
        let cli = parse(args("--config /path/that/does/not/exist.toml")).unwrap();
        let error = load(&cli).unwrap_err();
        assert!(error.to_string().contains("not/exist.toml"), "{error}");
    }

    #[test]
    fn missing_default_config_says_where_it_was_looked_for() {
        let cli = parse(args("")).unwrap();
        let default = PathBuf::from("/path/that/does/not/exist/config.toml");
        let error = load_with_default(&cli, Some(default)).unwrap_err();
        let text = error.to_string();
        assert!(text.contains("missing `host`"), "{text}");
        assert!(
            text.contains("config file not found: /path/that/does/not/exist/config.toml"),
            "{text}"
        );
    }

    #[test]
    fn unknown_flag_is_named_before_its_value_is_read() {
        for line in ["--insecure", "--insecure --host h", "--host h --insecure"] {
            assert_eq!(
                parse(args(line)).unwrap_err(),
                FrixError::Config(ConfigError::UnknownFlag("--insecure".into())),
                "{line}"
            );
        }
        assert_eq!(
            parse(args("--host")).unwrap_err(),
            FrixError::Config(ConfigError::FlagNeedsValue("--host".into()))
        );
    }

    #[test]
    fn config_flag_found_in_both_forms() {
        assert_eq!(
            config_flag(&args("--insecure --config /a.toml")),
            Some(PathBuf::from("/a.toml"))
        );
        assert_eq!(
            config_flag(&args("--config=/b.toml --x")),
            Some(PathBuf::from("/b.toml"))
        );
        assert_eq!(config_flag(&args("--host h")), None);
    }

    #[test]
    fn file_language_reads_the_config_and_falls_back_to_english() {
        let dir = std::env::temp_dir().join(format!("frix-cli-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("config.toml");
        std::fs::write(&file, "language = \"it\"\n").unwrap();

        assert_eq!(file_language(Some(&file)), Language::It);
        assert_eq!(
            file_language(Some(Path::new("/path/that/does/not/exist.toml"))),
            Language::En
        );
        std::fs::write(&file, "language = [not toml").unwrap();
        assert_eq!(file_language(Some(&file)), Language::En);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn missing_default_config_is_fine_if_flags_are_enough() {
        let cli = parse(args(&format!(
            "--host 10.0.0.1 --username frix --tls-pin {}",
            ["AB"; 32].join(":")
        )))
        .unwrap();
        let default = PathBuf::from("/path/that/does/not/exist/config.toml");
        assert!(load_with_default(&cli, Some(default)).is_ok());
    }
}

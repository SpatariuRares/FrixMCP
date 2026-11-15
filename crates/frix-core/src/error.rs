//! `FrixError`: the domain error of `frix-core` (docs/PROJECT.md §5.1, §6.2).
//!
//! Texts coming from the router (for example `errorDescription`) are untrusted
//! data: whoever shows them to the Agent must sanitize them (docs/PROJECT.md §7.3).
//!
//! `Display` is the English text for the Agent. [`FrixError::localized`] is the text for
//! the Operator in their [`Language`]: it translates the message, not the details that
//! come from the router or from libraries (`errorDescription`, TOML, TLS, I/O errors).

use crate::language::Language;
use crate::policy::Right;
use crate::secrets::KEYCHAIN_SERVICE;

/// Domain error returned by every operation against the router.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FrixError {
    /// HTTP 401 or `UPnP` fault 401: credentials rejected, or action unknown to the router.
    #[error("not authenticated on the router (401)")]
    Unauthenticated,
    /// Fault 402: wrong number or names of arguments.
    #[error("invalid arguments for the router (402): {description}")]
    InvalidArgs {
        /// The router's `errorDescription`.
        description: String,
    },
    /// Fault 606: the Router Account is authenticated but lacks the required right.
    #[error("action {action} not authorized for the Router Account (606): {description}")]
    NotAuthorized {
        /// The `Service#Action` called, for example `DeviceInfo:1#GetInfo`.
        action: String,
        /// The rights that suffice, any one of them (docs/PROJECT.md §4.2, rights column);
        /// `None` if §4.2 does not document the action's right.
        rights: Option<&'static [Right]>,
        /// The router's `errorDescription`.
        description: String,
    },
    /// Fault 714: the requested entry does not exist (for example unknown MAC).
    #[error("no such entry (714): {description}")]
    NoSuchEntry {
        /// The router's `errorDescription`.
        description: String,
    },
    /// Fault 820: internal router error.
    #[error("internal router error (820): {description}")]
    RouterInternal {
        /// The router's `errorDescription`.
        description: String,
    },
    /// Fault 866, 867 or 868: second factor required, blocked or busy.
    #[error("the router requires the second factor ({code})")]
    SecondFactorRequired {
        /// Fault code: 866, 867 or 868.
        code: u16,
    },
    /// Any other `UPnP` fault.
    #[error("fault SOAP {code}: {description}")]
    Soap {
        /// The router's `errorCode`.
        code: u16,
        /// The router's `errorDescription`.
        description: String,
    },
    /// HTTP transport error (connection, unexpected HTTP status, missing fixture).
    #[error("transport: {0}")]
    Transport(String),
    /// TLS error other than a wrong fingerprint (handshake, configuration, CA certificate).
    #[error("TLS: {0}")]
    Tls(String),
    /// The router's certificate does not have the configured SHA-256 fingerprint: possible
    /// MITM or regenerated certificate (docs/PROJECT.md §7.4). No fallback to HTTP.
    #[error(
        "router TLS fingerprint differs from the configured one: expected {expected}, found {}. \
         If the certificate was regenerated, read the fingerprint again (docs/PROJECT.md §7.4) and update tls.pinned_sha256",
        .found.as_deref().unwrap_or("no certificate")
    )]
    TlsPinMismatch {
        /// Configured fingerprint (`AB:CD:…`).
        expected: String,
        /// Fingerprint of the presented certificate; `None` if there was none.
        found: Option<String>,
    },
    /// Configuration or keyring unusable at startup.
    #[error("configuration: {0}")]
    Config(ConfigError),
    /// The Policy denies the action: it never reaches the transport.
    #[error("denied by the Policy: {0}")]
    Policy(String),
    /// Service, action or arguments not in the SCPD, or invalid value.
    #[error("validation: {0}")]
    Validation(String),
    /// Router response that cannot be interpreted.
    #[error("unparsable router response: {0}")]
    Parse(String),
}

impl FrixError {
    /// Stable name of the error kind, to show to the Agent.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Unauthenticated => "unauthenticated",
            Self::InvalidArgs { .. } => "invalid_args",
            Self::NotAuthorized { .. } => "not_authorized",
            Self::NoSuchEntry { .. } => "no_such_entry",
            Self::RouterInternal { .. } => "router_internal",
            Self::SecondFactorRequired { .. } => "second_factor_required",
            Self::Soap { .. } => "soap_fault",
            Self::Transport(_) => "transport",
            Self::Tls(_) => "tls",
            Self::TlsPinMismatch { .. } => "tls_pin_mismatch",
            Self::Config(_) => "config",
            Self::Policy(_) => "policy",
            Self::Validation(_) => "validation",
            Self::Parse(_) => "parse",
        }
    }

    /// Text for the Operator in `language`; in English it is the same as `Display`.
    #[must_use]
    pub fn localized(&self, language: Language) -> String {
        if language == Language::En {
            return self.to_string();
        }
        match self {
            Self::Unauthenticated => "non autenticato sul router (401)".to_owned(),
            Self::InvalidArgs { description } => {
                format!("argomenti non validi per il router (402): {description}")
            }
            Self::NotAuthorized {
                action,
                description,
                ..
            } => format!(
                "azione {action} non autorizzata per il Router Account (606): {description}"
            ),
            Self::NoSuchEntry { description } => format!("voce inesistente (714): {description}"),
            Self::RouterInternal { description } => {
                format!("errore interno del router (820): {description}")
            }
            Self::SecondFactorRequired { code } => {
                format!("il router richiede il secondo fattore ({code})")
            }
            Self::Soap { code, description } => format!("fault SOAP {code}: {description}"),
            Self::Transport(detail) => format!("trasporto: {detail}"),
            Self::Tls(detail) => format!("TLS: {detail}"),
            Self::TlsPinMismatch { expected, found } => format!(
                "impronta TLS del router diversa da quella configurata: attesa {expected}, \
                 trovata {}. Se il certificato è stato rigenerato, rileggi l'impronta \
                 (docs/PROJECT.md §7.4) e aggiorna tls.pinned_sha256",
                found.as_deref().unwrap_or("nessun certificato")
            ),
            Self::Config(error) => format!("configurazione: {}", error.text(language)),
            Self::Policy(detail) => format!("negato dalla Policy: {detail}"),
            Self::Validation(detail) => format!("validazione: {detail}"),
            Self::Parse(detail) => format!("risposta del router non interpretabile: {detail}"),
        }
    }
}

/// Why the configuration, the flags or the keyring cannot be used at startup.
///
/// Typed rather than a string so that the Operator reads it in their [`Language`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// A required key is neither in the file nor among the flags.
    Missing(&'static str),
    /// `tls.mode` (or `--tls-mode`) is neither `pin` nor `ca`.
    UnknownTlsMode(String),
    /// `tls.pinned_sha256` is not 32 hex bytes.
    InvalidPin,
    /// `config.toml` is not valid TOML or has an unknown key; the TOML parser's text.
    Toml(String),
    /// The config file cannot be read.
    ReadFile {
        /// The file.
        path: String,
        /// The I/O error.
        error: String,
    },
    /// `tls.ca_file` cannot be read.
    CaFile {
        /// The file.
        path: String,
        /// The I/O error.
        error: String,
    },
    /// The configuration is incomplete and the default config file does not exist.
    DefaultFileMissing {
        /// What is missing.
        cause: Box<ConfigError>,
        /// Where the file was looked for.
        path: String,
    },
    /// A flag without its value.
    FlagNeedsValue(String),
    /// `--http-port` is not a port number.
    InvalidHttpPort(String),
    /// A flag that does not exist.
    UnknownFlag(String),
    /// The program that reads the keyring cannot be run.
    KeyringUnavailable {
        /// The program, for example `/usr/bin/secret-tool`.
        program: &'static str,
        /// The I/O error.
        error: String,
    },
    /// No password for the Router Account in the keyring.
    PasswordNotFound {
        /// The Router Account.
        account: String,
    },
    /// The keyring item is not UTF-8 text.
    PasswordNotUtf8 {
        /// The Router Account.
        account: String,
    },
}

impl ConfigError {
    /// The message in `language`.
    #[must_use]
    pub fn text(&self, language: Language) -> String {
        let s = KEYCHAIN_SERVICE;
        match (self, language) {
            (Self::Missing(key), Language::En) => {
                format!("missing `{key}` (in the config file or as a flag)")
            }
            (Self::Missing(key), Language::It) => {
                format!("manca `{key}` (nel file di config o come flag)")
            }
            (Self::UnknownTlsMode(mode), Language::En) => {
                format!("unknown TLS mode: {mode:?} (allowed: pin, ca)")
            }
            (Self::UnknownTlsMode(mode), Language::It) => {
                format!("modalità TLS sconosciuta: {mode:?} (ammesse: pin, ca)")
            }
            (Self::InvalidPin, Language::En) => {
                "tls.pinned_sha256: invalid SHA-256 fingerprint: 32 hex bytes required (AB:CD:…)"
                    .to_owned()
            }
            (Self::InvalidPin, Language::It) => {
                "tls.pinned_sha256: impronta SHA-256 non valida: servono 32 byte esadecimali \
                 (AB:CD:…)"
                    .to_owned()
            }
            (Self::Toml(error), _) => format!("config.toml: {error}"),
            (Self::ReadFile { path, error }, _) => format!("{path}: {error}"),
            (Self::CaFile { path, error }, _) => format!("tls.ca_file {path}: {error}"),
            (Self::DefaultFileMissing { cause, path }, Language::En) => format!(
                "{}; config file not found: {path} (create it or use --config)",
                cause.text(language)
            ),
            (Self::DefaultFileMissing { cause, path }, Language::It) => format!(
                "{}; file di config non trovato: {path} (crealo o usa --config)",
                cause.text(language)
            ),
            (Self::FlagNeedsValue(flag), Language::En) => format!("flag {flag} requires a value"),
            (Self::FlagNeedsValue(flag), Language::It) => {
                format!("il flag {flag} richiede un valore")
            }
            (Self::InvalidHttpPort(value), Language::En) => format!("invalid --http-port: {value}"),
            (Self::InvalidHttpPort(value), Language::It) => {
                format!("--http-port non valido: {value}")
            }
            (Self::UnknownFlag(flag), Language::En) => format!("unknown flag: {flag}"),
            (Self::UnknownFlag(flag), Language::It) => format!("flag sconosciuto: {flag}"),
            (Self::KeyringUnavailable { program, error }, Language::En) => {
                format!("cannot run {program}: {error}")
            }
            (Self::KeyringUnavailable { program, error }, Language::It) => {
                format!("impossibile eseguire {program}: {error}")
            }
            (Self::PasswordNotFound { account }, Language::En) => format!(
                "Router Account password for `{account}` not found in the keyring \
                 (service `{s}`, account `{account}`)"
            ),
            (Self::PasswordNotFound { account }, Language::It) => format!(
                "password del Router Account `{account}` non trovata nel portachiavi \
                 (service `{s}`, account `{account}`)"
            ),
            (Self::PasswordNotUtf8 { account }, Language::En) => {
                format!("the keyring item `{s}`/`{account}` is not UTF-8 text")
            }
            (Self::PasswordNotUtf8 { account }, Language::It) => {
                format!("la voce del portachiavi `{s}`/`{account}` non è testo UTF-8")
            }
        }
    }
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text(Language::En))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples() -> Vec<FrixError> {
        let config = [
            ConfigError::Missing("host"),
            ConfigError::UnknownTlsMode("off".into()),
            ConfigError::InvalidPin,
            ConfigError::DefaultFileMissing {
                cause: Box::new(ConfigError::Missing("host")),
                path: "/p/config.toml".into(),
            },
            ConfigError::FlagNeedsValue("--host".into()),
            ConfigError::InvalidHttpPort("x".into()),
            ConfigError::UnknownFlag("--insecure".into()),
            ConfigError::KeyringUnavailable {
                program: "/usr/bin/secret-tool",
                error: "No such file".into(),
            },
            ConfigError::PasswordNotFound {
                account: "frixmcp".into(),
            },
            ConfigError::PasswordNotUtf8 {
                account: "frixmcp".into(),
            },
        ];
        let mut errors: Vec<FrixError> = config.into_iter().map(FrixError::Config).collect();
        errors.extend([
            FrixError::Unauthenticated,
            FrixError::NotAuthorized {
                action: "DeviceInfo:1#GetInfo".into(),
                rights: None,
                description: "Action Not Authorized".into(),
            },
            FrixError::SecondFactorRequired { code: 866 },
            FrixError::TlsPinMismatch {
                expected: "AA".into(),
                found: None,
            },
            FrixError::Transport("connection refused".into()),
        ]);
        errors
    }

    #[test]
    fn english_is_the_display_text_for_the_agent() {
        for error in samples() {
            assert_eq!(error.localized(Language::En), error.to_string());
        }
    }

    #[test]
    fn italian_differs_from_english_and_keeps_the_details() {
        for error in samples() {
            let italian = error.localized(Language::It);
            assert_ne!(italian, error.to_string(), "{italian}");
        }
        let error = FrixError::Config(ConfigError::PasswordNotFound {
            account: "frixmcp".into(),
        });
        let italian = error.localized(Language::It);
        assert!(
            italian.contains("portachiavi") && italian.contains("frixmcp"),
            "{italian}"
        );
    }

    #[test]
    fn missing_certificate_is_named_in_both_languages() {
        let error = FrixError::TlsPinMismatch {
            expected: "AA".into(),
            found: None,
        };
        assert!(error.to_string().contains("found no certificate"));
        assert!(
            error
                .localized(Language::It)
                .contains("trovata nessun certificato")
        );
    }
}

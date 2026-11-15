//! Router connection configuration (docs/PROJECT.md §7.4, §13.5): `config.toml`
//! outside the repo, overridable by flags. No password: that lives in the Keychain.
//!
//! ```toml
//! host = "192.168.178.1"
//! username = "frixmcp"
//! # http_port = 49000
//! # hide_serial_number = true       # get_system_info hides the serial (§8.1)
//! # language = "it"                 # Operator messages: "en" (default) or "it"
//!
//! [tls]
//! mode = "pin"                       # default; or "ca"
//! pinned_sha256 = "AB:CD:…"          # for mode = "pin"
//! # ca_file = "router.der"           # for mode = "ca", relative to the config file
//! ```
//!
//! Unknown keys are an error: there is no key to disable TLS verification, and one
//! written by mistake is not silently ignored.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{ConfigError, FrixError};
use crate::language::Language;
use crate::transport::{DEFAULT_HTTP_PORT, Fingerprint, RouterSettings, TlsTrust};

/// Partial configuration, from `config.toml` or flags: absent fields stay `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigFile {
    /// IP or name of the router.
    pub host: Option<String>,
    /// Router Account.
    pub username: Option<String>,
    /// Plain HTTP port (default 49000).
    pub http_port: Option<u16>,
    /// Redacts the router's serial number in responses to the Agent (default `false`).
    pub hide_serial_number: Option<bool>,
    /// Language of the messages for the Operator (default English).
    pub language: Option<Language>,
    /// `[tls]` section.
    #[serde(default)]
    pub tls: TlsSection,
}

/// `[tls]` section.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TlsSection {
    /// `pin` (default) or `ca`.
    pub mode: Option<TlsMode>,
    /// SHA-256 fingerprint of the router's certificate, for `pin`.
    pub pinned_sha256: Option<String>,
    /// The router's DER certificate, for `ca`; relative to the config file's directory.
    pub ca_file: Option<PathBuf>,
}

/// The only two TLS modes: there is none that skips verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TlsMode {
    /// SHA-256 fingerprint of the certificate.
    Pin,
    /// The router's certificate as the only root.
    Ca,
}

impl std::str::FromStr for TlsMode {
    type Err = FrixError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "pin" => Ok(Self::Pin),
            "ca" => Ok(Self::Ca),
            other => Err(FrixError::Config(ConfigError::UnknownTlsMode(
                other.to_owned(),
            ))),
        }
    }
}

/// Complete, validated configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouterConfig {
    /// Host, port and TLS verification.
    pub settings: RouterSettings,
    /// Router Account; the password is read from the Keychain under this name.
    pub username: String,
    /// The router's serial number does not reach the Agent.
    pub hide_serial_number: bool,
    /// Language of the messages for the Operator.
    pub language: Language,
}

impl ConfigFile {
    /// Parses the text of `config.toml`.
    pub fn parse(text: &str) -> Result<Self, FrixError> {
        toml::from_str(text).map_err(|e| FrixError::Config(ConfigError::Toml(e.to_string())))
    }

    /// Fields present in `other` (for example the flags) win over those of `self`.
    #[must_use]
    pub fn overridden_by(self, other: Self) -> Self {
        Self {
            host: other.host.or(self.host),
            username: other.username.or(self.username),
            http_port: other.http_port.or(self.http_port),
            hide_serial_number: other.hide_serial_number.or(self.hide_serial_number),
            language: other.language.or(self.language),
            tls: TlsSection {
                mode: other.tls.mode.or(self.tls.mode),
                pinned_sha256: other.tls.pinned_sha256.or(self.tls.pinned_sha256),
                ca_file: other.tls.ca_file.or(self.tls.ca_file),
            },
        }
    }

    /// Validates and completes; `base` is the directory a relative `ca_file` resolves against.
    pub fn resolve(self, base: &Path) -> Result<RouterConfig, FrixError> {
        let missing = |key: &'static str| FrixError::Config(ConfigError::Missing(key));
        let host = self.host.ok_or_else(|| missing("host"))?;
        let username = self.username.ok_or_else(|| missing("username"))?;
        let tls = match self.tls.mode.unwrap_or(TlsMode::Pin) {
            TlsMode::Pin => TlsTrust::Pin(
                self.tls
                    .pinned_sha256
                    .ok_or_else(|| missing("tls.pinned_sha256"))?
                    .parse::<Fingerprint>()
                    .map_err(|_| FrixError::Config(ConfigError::InvalidPin))?,
            ),
            TlsMode::Ca => {
                let file = base.join(self.tls.ca_file.ok_or_else(|| missing("tls.ca_file"))?);
                TlsTrust::Ca(std::fs::read(&file).map_err(|e| {
                    FrixError::Config(ConfigError::CaFile {
                        path: file.display().to_string(),
                        error: e.to_string(),
                    })
                })?)
            }
        };
        Ok(RouterConfig {
            settings: RouterSettings {
                host,
                http_port: self.http_port.unwrap_or(DEFAULT_HTTP_PORT),
                tls,
            },
            username,
            hide_serial_number: self.hide_serial_number.unwrap_or(false),
            language: self.language.unwrap_or_default(),
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use std::path::Path;

    use super::*;
    use crate::transport::TlsTrust;

    const FINGERPRINT: &str = "55:98:B9:D3:18:60:95:02:02:CF:CB:DD:29:07:C3:20:7C:D1:C8:CD:33:EC:4F:70:88:65:05:C5:4A:BD:E0:66";

    fn complete() -> String {
        format!(
            "host = \"192.168.178.1\"\nusername = \"frixmcp\"\n[tls]\npinned_sha256 = \"{FINGERPRINT}\"\n"
        )
    }

    #[test]
    fn complete_config_defaults_to_pin_mode() {
        let config = ConfigFile::parse(&complete())
            .unwrap()
            .resolve(Path::new("/nowhere"))
            .unwrap();

        assert_eq!(config.username, "frixmcp");
        assert_eq!(config.settings.host, "192.168.178.1");
        assert_eq!(config.settings.http_port, 49000);
        assert_eq!(
            config.settings.tls,
            TlsTrust::Pin(FINGERPRINT.parse().unwrap())
        );
    }

    #[test]
    fn no_key_disables_tls_verification() {
        for extra in [
            "insecure = true\n",
            "[tls]\nverify = false\n",
            "[tls]\ninsecure = true\n",
            "[tls]\nmode = \"none\"\n",
            "[tls]\nmode = \"insecure\"\n",
            "[tls]\nmode = \"off\"\n",
        ] {
            let text = format!("host = \"h\"\nusername = \"u\"\n{extra}");
            let error = ConfigFile::parse(&text).unwrap_err();
            assert_eq!(error.kind(), "config", "{extra}: {error}");
        }
    }

    #[test]
    fn pin_without_fingerprint_rejected() {
        let error = ConfigFile::parse("host = \"h\"\nusername = \"u\"\n[tls]\nmode = \"pin\"\n")
            .unwrap()
            .resolve(Path::new("/nowhere"))
            .unwrap_err();

        assert!(error.to_string().contains("pinned_sha256"), "{error}");
    }

    #[test]
    fn missing_host_or_username() {
        let error = ConfigFile::parse("username = \"u\"")
            .unwrap()
            .resolve(Path::new("/nowhere"))
            .unwrap_err();
        assert!(error.to_string().contains("host"), "{error}");
    }

    #[test]
    fn serial_is_visible_unless_hide_serial_number() {
        let visible = ConfigFile::parse(&complete())
            .unwrap()
            .resolve(Path::new("/nowhere"))
            .unwrap();
        let redacted = ConfigFile::parse(&format!("hide_serial_number = true\n{}", complete()))
            .unwrap()
            .resolve(Path::new("/nowhere"))
            .unwrap();

        assert!(!visible.hide_serial_number);
        assert!(redacted.hide_serial_number);
    }

    #[test]
    fn language_is_english_unless_set() {
        let default = ConfigFile::parse(&complete())
            .unwrap()
            .resolve(Path::new("/nowhere"))
            .unwrap();
        let italian = ConfigFile::parse(&format!("language = \"it\"\n{}", complete()))
            .unwrap()
            .resolve(Path::new("/nowhere"))
            .unwrap();

        assert_eq!(default.language, Language::En);
        assert_eq!(italian.language, Language::It);
        let error = ConfigFile::parse("language = \"fr\"").unwrap_err();
        assert_eq!(error.kind(), "config", "{error}");
    }

    #[test]
    fn flags_override_the_file() {
        let flags = ConfigFile {
            host: Some("10.0.0.1".into()),
            http_port: Some(8080),
            ..ConfigFile::default()
        };
        let config = ConfigFile::parse(&complete())
            .unwrap()
            .overridden_by(flags)
            .resolve(Path::new("/nowhere"))
            .unwrap();

        assert_eq!(config.settings.host, "10.0.0.1");
        assert_eq!(config.settings.http_port, 8080);
        assert_eq!(config.username, "frixmcp");
    }

    #[test]
    fn ca_mode_reads_certificate_relative_to_config_file() {
        let dir = std::env::temp_dir().join(format!("frix-config-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("router.cer"), b"fake DER").unwrap();
        let text =
            "host = \"h\"\nusername = \"u\"\n[tls]\nmode = \"ca\"\nca_file = \"router.cer\"\n";

        let config = ConfigFile::parse(text).unwrap().resolve(&dir).unwrap();

        assert_eq!(config.settings.tls, TlsTrust::Ca(b"fake DER".to_vec()));
        let missing = ConfigFile::parse("host = \"h\"\nusername = \"u\"\n[tls]\nmode = \"ca\"\n")
            .unwrap()
            .resolve(&dir)
            .unwrap_err();
        assert!(missing.to_string().contains("ca_file"), "{missing}");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

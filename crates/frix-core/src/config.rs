//! Configurazione del collegamento al router (docs/PROJECT.md §7.4, §13.5): `config.toml`
//! fuori dal repo, sovrascrivibile da flag. Nessuna password: quella sta nel Keychain.
//!
//! ```toml
//! host = "192.168.178.1"
//! username = "frixmcp"
//! # http_port = 49000
//! # hide_serial_number = true       # get_system_info non mostra il seriale (§8.1)
//!
//! [tls]
//! mode = "pin"                       # default; oppure "ca"
//! pinned_sha256 = "AB:CD:…"          # per mode = "pin"
//! # ca_file = "router.der"           # per mode = "ca", relativo al file di config
//! ```
//!
//! Le chiavi sconosciute sono un errore: non esiste una chiave per disattivare la
//! verifica TLS, e una scritta per sbaglio non viene ignorata in silenzio.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::FrixError;
use crate::transport::{DEFAULT_HTTP_PORT, Fingerprint, RouterSettings, TlsTrust};

/// Configurazione parziale, da `config.toml` o dai flag: i campi assenti restano `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigFile {
    /// IP o nome del router.
    pub host: Option<String>,
    /// Router Account.
    pub username: Option<String>,
    /// Porta HTTP in chiaro (default 49000).
    pub http_port: Option<u16>,
    /// Oscura il numero di serie del router nelle risposte all'Agent (default `false`).
    pub hide_serial_number: Option<bool>,
    /// Sezione `[tls]`.
    #[serde(default)]
    pub tls: TlsSection,
}

/// Sezione `[tls]`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TlsSection {
    /// `pin` (default) o `ca`.
    pub mode: Option<TlsMode>,
    /// Impronta SHA-256 del certificato del router, per `pin`.
    pub pinned_sha256: Option<String>,
    /// Certificato DER del router, per `ca`; relativo alla cartella del file di config.
    pub ca_file: Option<PathBuf>,
}

/// Le sole due modalità TLS: non ne esiste una che non verifichi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TlsMode {
    /// Impronta SHA-256 del certificato.
    Pin,
    /// Certificato del router come unica root.
    Ca,
}

impl std::str::FromStr for TlsMode {
    type Err = FrixError;

    fn from_str(testo: &str) -> Result<Self, Self::Err> {
        match testo {
            "pin" => Ok(Self::Pin),
            "ca" => Ok(Self::Ca),
            altro => Err(FrixError::Config(format!(
                "modalità TLS sconosciuta: {altro:?} (ammesse: pin, ca)"
            ))),
        }
    }
}

/// Configurazione completa e validata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouterConfig {
    /// Host, porta e verifica TLS.
    pub settings: RouterSettings,
    /// Router Account; la password si legge dal Keychain con questo nome.
    pub username: String,
    /// Il numero di serie del router non arriva all'Agent.
    pub hide_serial_number: bool,
}

impl ConfigFile {
    /// Interpreta il testo di `config.toml`.
    pub fn parse(testo: &str) -> Result<Self, FrixError> {
        toml::from_str(testo).map_err(|e| FrixError::Config(format!("config.toml: {e}")))
    }

    /// I campi presenti in `altro` (per esempio i flag) vincono su quelli di `self`.
    #[must_use]
    pub fn overridden_by(self, altro: Self) -> Self {
        Self {
            host: altro.host.or(self.host),
            username: altro.username.or(self.username),
            http_port: altro.http_port.or(self.http_port),
            hide_serial_number: altro.hide_serial_number.or(self.hide_serial_number),
            tls: TlsSection {
                mode: altro.tls.mode.or(self.tls.mode),
                pinned_sha256: altro.tls.pinned_sha256.or(self.tls.pinned_sha256),
                ca_file: altro.tls.ca_file.or(self.tls.ca_file),
            },
        }
    }

    /// Valida e completa; `base` è la cartella contro cui si risolve un `ca_file` relativo.
    pub fn resolve(self, base: &Path) -> Result<RouterConfig, FrixError> {
        let manca = |chiave: &str| {
            FrixError::Config(format!("manca `{chiave}` (nel file di config o come flag)"))
        };
        let host = self.host.ok_or_else(|| manca("host"))?;
        let username = self.username.ok_or_else(|| manca("username"))?;
        let tls = match self.tls.mode.unwrap_or(TlsMode::Pin) {
            TlsMode::Pin => TlsTrust::Pin(
                self.tls
                    .pinned_sha256
                    .ok_or_else(|| manca("tls.pinned_sha256"))?
                    .parse::<Fingerprint>()
                    .map_err(|e| FrixError::Config(format!("tls.pinned_sha256: {e}")))?,
            ),
            TlsMode::Ca => {
                let file = base.join(self.tls.ca_file.ok_or_else(|| manca("tls.ca_file"))?);
                TlsTrust::Ca(std::fs::read(&file).map_err(|e| {
                    FrixError::Config(format!("tls.ca_file {}: {e}", file.display()))
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

    const IMPRONTA: &str = "55:98:B9:D3:18:60:95:02:02:CF:CB:DD:29:07:C3:20:7C:D1:C8:CD:33:EC:4F:70:88:65:05:C5:4A:BD:E0:66";

    fn completa() -> String {
        format!(
            "host = \"192.168.178.1\"\nusername = \"frixmcp\"\n[tls]\npinned_sha256 = \"{IMPRONTA}\"\n"
        )
    }

    #[test]
    fn config_completa_in_modalita_pin_di_default() {
        let config = ConfigFile::parse(&completa())
            .unwrap()
            .resolve(Path::new("/nessuna"))
            .unwrap();

        assert_eq!(config.username, "frixmcp");
        assert_eq!(config.settings.host, "192.168.178.1");
        assert_eq!(config.settings.http_port, 49000);
        assert_eq!(
            config.settings.tls,
            TlsTrust::Pin(IMPRONTA.parse().unwrap())
        );
    }

    #[test]
    fn nessuna_chiave_per_disattivare_la_verifica_tls() {
        for extra in [
            "insecure = true\n",
            "[tls]\nverify = false\n",
            "[tls]\ninsecure = true\n",
            "[tls]\nmode = \"none\"\n",
            "[tls]\nmode = \"insecure\"\n",
            "[tls]\nmode = \"off\"\n",
        ] {
            let testo = format!("host = \"h\"\nusername = \"u\"\n{extra}");
            let errore = ConfigFile::parse(&testo).unwrap_err();
            assert_eq!(errore.kind(), "config", "{extra}: {errore}");
        }
    }

    #[test]
    fn pin_senza_impronta_rifiutato() {
        let errore = ConfigFile::parse("host = \"h\"\nusername = \"u\"\n[tls]\nmode = \"pin\"\n")
            .unwrap()
            .resolve(Path::new("/nessuna"))
            .unwrap_err();

        assert!(errore.to_string().contains("pinned_sha256"), "{errore}");
    }

    #[test]
    fn mancano_host_o_username() {
        let errore = ConfigFile::parse("username = \"u\"")
            .unwrap()
            .resolve(Path::new("/nessuna"))
            .unwrap_err();
        assert!(errore.to_string().contains("host"), "{errore}");
    }

    #[test]
    fn il_seriale_e_visibile_salvo_hide_serial_number() {
        let visibile = ConfigFile::parse(&completa())
            .unwrap()
            .resolve(Path::new("/nessuna"))
            .unwrap();
        let oscurato = ConfigFile::parse(&format!("hide_serial_number = true\n{}", completa()))
            .unwrap()
            .resolve(Path::new("/nessuna"))
            .unwrap();

        assert!(!visibile.hide_serial_number);
        assert!(oscurato.hide_serial_number);
    }

    #[test]
    fn i_flag_sovrascrivono_il_file() {
        let flag = ConfigFile {
            host: Some("10.0.0.1".into()),
            http_port: Some(8080),
            ..ConfigFile::default()
        };
        let config = ConfigFile::parse(&completa())
            .unwrap()
            .overridden_by(flag)
            .resolve(Path::new("/nessuna"))
            .unwrap();

        assert_eq!(config.settings.host, "10.0.0.1");
        assert_eq!(config.settings.http_port, 8080);
        assert_eq!(config.username, "frixmcp");
    }

    #[test]
    fn modalita_ca_legge_il_certificato_relativo_al_file_di_config() {
        let dir = std::env::temp_dir().join(format!("frix-config-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("router.cer"), b"DER finto").unwrap();
        let testo =
            "host = \"h\"\nusername = \"u\"\n[tls]\nmode = \"ca\"\nca_file = \"router.cer\"\n";

        let config = ConfigFile::parse(testo).unwrap().resolve(&dir).unwrap();

        assert_eq!(config.settings.tls, TlsTrust::Ca(b"DER finto".to_vec()));
        let mancante = ConfigFile::parse("host = \"h\"\nusername = \"u\"\n[tls]\nmode = \"ca\"\n")
            .unwrap()
            .resolve(&dir)
            .unwrap_err();
        assert!(mancante.to_string().contains("ca_file"), "{mancante}");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

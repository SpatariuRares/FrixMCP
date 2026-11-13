//! Password del Router Account dal Keychain di macOS (docs/PROJECT.md §7.5, §13.5):
//! voce "generic password" con service `FrixMCP` e account = `username` del config.
//!
//! Scelta (docs/PROJECT.md §14.2.3): la CLI `/usr/bin/security` invece del crate
//! `keyring`. Il crate 4.x non ha più un backend implicito: va scelto lo store
//! `apple-native-keyring-store` (feature `keychain`, che porta `security-framework`
//! e FFI) e registrato a runtime, ed esiste solo su macOS. La CLI non aggiunge
//! dipendenze, compila ovunque (sugli altri sistemi la lettura fallisce con un
//! errore chiaro) e legge la stessa voce del Keychain. Non c'è alcun fallback a file
//! o variabili d'ambiente.

use std::process::{Command, Stdio};

use secrecy::SecretString;

use crate::error::FrixError;

/// `service` della voce del Keychain.
pub const KEYCHAIN_SERVICE: &str = "FrixMCP";

/// Path assoluto: niente `PATH` da cui farsi sostituire il programma.
const SECURITY: &str = "/usr/bin/security";

/// Legge la password di `account` dal Keychain. Gli errori non contengono mai l'output.
pub fn keychain_password(account: &str) -> Result<SecretString, FrixError> {
    let uscita = Command::new(SECURITY)
        .args([
            "find-generic-password",
            "-s",
            KEYCHAIN_SERVICE,
            "-a",
            account,
            "-w",
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| FrixError::Config(format!("impossibile eseguire {SECURITY}: {e}")))?;
    password_from_output(uscita.status.success(), uscita.stdout, account)
}

/// Interpreta l'output di `security find-generic-password -w`: la password seguita da `\n`.
fn password_from_output(
    riuscito: bool,
    stdout: Vec<u8>,
    account: &str,
) -> Result<SecretString, FrixError> {
    let non_trovata = || {
        FrixError::Config(format!(
            "password del Router Account `{account}` non trovata nel Keychain \
             (service `{KEYCHAIN_SERVICE}`, account `{account}`)"
        ))
    };
    if !riuscito {
        return Err(non_trovata());
    }
    let mut password = String::from_utf8(stdout).map_err(|_| {
        FrixError::Config(format!(
            "la voce del Keychain `{KEYCHAIN_SERVICE}`/`{account}` non è testo UTF-8"
        ))
    })?;
    if password.ends_with('\n') {
        password.pop();
    }
    if password.is_empty() {
        return Err(non_trovata());
    }
    Ok(SecretString::from(password))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use secrecy::ExposeSecret as _;

    use super::*;

    const SEGRETO: &str = "Zx9-segreto-di-prova";

    #[test]
    fn toglie_solo_l_a_capo_finale() {
        let password =
            password_from_output(true, format!("{SEGRETO} \n").into_bytes(), "frixmcp").unwrap();
        assert_eq!(password.expose_secret(), format!("{SEGRETO} "));
        assert!(!format!("{password:?}").contains(SEGRETO));
    }

    #[test]
    fn voce_assente_o_vuota_e_un_errore_senza_output() {
        for (riuscito, stdout) in [(false, format!("{SEGRETO}\n")), (true, "\n".to_owned())] {
            let errore =
                password_from_output(riuscito, stdout.into_bytes(), "frixmcp").unwrap_err();
            let messaggio = errore.to_string();
            assert_eq!(errore.kind(), "config");
            assert!(messaggio.contains("frixmcp") && messaggio.contains("FrixMCP"));
            assert!(!messaggio.contains(SEGRETO));
            assert!(!format!("{errore:?}").contains(SEGRETO));
        }
    }

    #[test]
    fn output_non_utf8_rifiutato_senza_mostrarlo() {
        let errore = password_from_output(true, vec![0xff, 0xfe, b'Q'], "frixmcp").unwrap_err();
        assert_eq!(errore.kind(), "config");
        assert!(!format!("{errore} {errore:?}").contains('Q'));
    }
}

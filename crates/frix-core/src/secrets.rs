//! Router Account password from the system keyring (docs/PROJECT.md §7.5, §13.5):
//! on macOS the Keychain ("generic password" item), on Linux the Secret Service
//! (GNOME Keyring or `KWallet`) via `secret-tool`. In both cases the item has
//! service `FrixMCP` and account = the config `username`.
//!
//! Choice (docs/PROJECT.md §14.2.3): the `/usr/bin/security` CLI instead of the
//! `keyring` crate. Crate 4.x no longer has an implicit backend: the
//! `apple-native-keyring-store` store must be chosen (feature `keychain`, which brings
//! `security-framework` and FFI) and registered at runtime, and it exists only on macOS.
//! The CLI adds no dependencies, compiles everywhere (on other systems the read fails
//! with a clear error) and reads the same Keychain item. On Linux the same choice
//! applies with `/usr/bin/secret-tool` (libsecret). There is no fallback to files or
//! environment variables.

use std::process::{Command, Stdio};

use secrecy::SecretString;

use crate::error::{ConfigError, FrixError};

/// `service` of the Keychain item.
pub const KEYCHAIN_SERVICE: &str = "FrixMCP";

/// Program that reads the item. Absolute path: no `PATH` through which the program
/// could be replaced.
#[cfg(not(target_os = "linux"))]
const PROGRAM: &str = "/usr/bin/security";
#[cfg(target_os = "linux")]
const PROGRAM: &str = "/usr/bin/secret-tool";

/// Arguments of [`PROGRAM`] to read the password of `account`.
#[cfg(not(target_os = "linux"))]
fn args(account: &str) -> [&str; 6] {
    [
        "find-generic-password",
        "-s",
        KEYCHAIN_SERVICE,
        "-a",
        account,
        "-w",
    ]
}
#[cfg(target_os = "linux")]
fn args(account: &str) -> [&str; 5] {
    ["lookup", "service", KEYCHAIN_SERVICE, "account", account]
}

/// Reads the password of `account` from the keyring. Errors never contain the output.
pub fn keychain_password(account: &str) -> Result<SecretString, FrixError> {
    let output = Command::new(PROGRAM)
        .args(args(account))
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| {
            FrixError::Config(ConfigError::KeyringUnavailable {
                program: PROGRAM,
                error: e.to_string(),
            })
        })?;
    password_from_output(output.status.success(), output.stdout, account)
}

/// Parses the output of `security find-generic-password -w` or `secret-tool lookup`:
/// the password, followed by `\n` if present.
fn password_from_output(
    succeeded: bool,
    stdout: Vec<u8>,
    account: &str,
) -> Result<SecretString, FrixError> {
    let not_found = || {
        FrixError::Config(ConfigError::PasswordNotFound {
            account: account.to_owned(),
        })
    };
    if !succeeded {
        return Err(not_found());
    }
    let mut password = String::from_utf8(stdout).map_err(|_| {
        FrixError::Config(ConfigError::PasswordNotUtf8 {
            account: account.to_owned(),
        })
    })?;
    if password.ends_with('\n') {
        password.pop();
    }
    if password.is_empty() {
        return Err(not_found());
    }
    Ok(SecretString::from(password))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use secrecy::ExposeSecret as _;

    use super::*;

    const SECRET: &str = "Zx9-test-secret";

    #[test]
    fn strips_only_the_trailing_newline() {
        let password =
            password_from_output(true, format!("{SECRET} \n").into_bytes(), "frixmcp").unwrap();
        assert_eq!(password.expose_secret(), format!("{SECRET} "));
        assert!(!format!("{password:?}").contains(SECRET));
    }

    #[test]
    fn missing_or_empty_item_is_an_error_without_output() {
        for (succeeded, stdout) in [(false, format!("{SECRET}\n")), (true, "\n".to_owned())] {
            let error =
                password_from_output(succeeded, stdout.into_bytes(), "frixmcp").unwrap_err();
            let message = error.to_string();
            assert_eq!(error.kind(), "config");
            assert!(message.contains("frixmcp") && message.contains("FrixMCP"));
            assert!(!message.contains(SECRET));
            assert!(!format!("{error:?}").contains(SECRET));
        }
    }

    #[test]
    fn non_utf8_output_rejected_without_showing_it() {
        let error = password_from_output(true, vec![0xff, 0xfe, b'Q'], "frixmcp").unwrap_err();
        assert_eq!(error.kind(), "config");
        assert!(!format!("{error} {error:?}").contains('Q'));
    }
}

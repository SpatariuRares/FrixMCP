//! `Mac`: the identity of a Host (ADR 0002).

use std::fmt;
use std::str::FromStr;

use crate::error::FrixError;

/// A 6-byte MAC address. Always written `AA:BB:CC:DD:EE:FF` (uppercase, colons),
/// the format of the FRITZ!Box host list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Mac([u8; 6]);

impl Mac {
    /// The 6 bytes of the address.
    #[must_use]
    pub fn octets(self) -> [u8; 6] {
        self.0
    }
}

impl FromStr for Mac {
    type Err = FrixError;

    /// Accepts six hex pairs separated all by `:` or all by `-`, in upper or
    /// lower case. Rejects any other form.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let invalid = || FrixError::Validation(format!("invalid MAC: {text:?}"));
        let separator = if text.contains('-') { '-' } else { ':' };
        let mut bytes = [0_u8; 6];
        let mut parts = text.split(separator);
        for b in &mut bytes {
            let pair = parts.next().ok_or_else(invalid)?;
            if pair.len() != 2 || !pair.bytes().all(|c| c.is_ascii_hexdigit()) {
                return Err(invalid());
            }
            *b = u8::from_str_radix(pair, 16).map_err(|_| invalid())?;
        }
        if parts.next().is_some() {
            return Err(invalid());
        }
        Ok(Self(bytes))
    }
}

impl fmt::Display for Mac {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, byte) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(":")?;
            }
            write!(f, "{byte:02X}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::Mac;

    #[test]
    fn accepts_upper_lower_and_dashes_and_normalizes() {
        for text in [
            "02:00:00:00:10:0A",
            "02:00:00:00:10:0a",
            "02-00-00-00-10-0a",
        ] {
            assert_eq!(
                text.parse::<Mac>().unwrap().to_string(),
                "02:00:00:00:10:0A"
            );
        }
    }

    #[test]
    fn rejects_invalid_forms() {
        for text in [
            "",
            "02:00:00:00:10",
            "02:00:00:00:10:0A:FF",
            "02:00:00:00:10:0G",
            "02:00:00:00:10:0",
            "020000000010",
            "02:00-00:00:10:0A",
            "02:00:00:00:10:+A",
            " 02:00:00:00:10:0A",
            "192.168.178.20",
        ] {
            let error = text.parse::<Mac>().unwrap_err();
            assert_eq!(error.kind(), "validation", "{text:?}");
        }
    }
}

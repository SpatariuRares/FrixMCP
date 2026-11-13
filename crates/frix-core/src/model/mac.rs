//! `Mac`: l'identità di un Host (ADR 0002).

use std::fmt;
use std::str::FromStr;

use crate::error::FrixError;

/// Indirizzo MAC di 6 byte. Si scrive sempre `AA:BB:CC:DD:EE:FF` (maiuscolo, due punti),
/// il formato della lista host del FRITZ!Box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Mac([u8; 6]);

impl Mac {
    /// I 6 byte dell'indirizzo.
    #[must_use]
    pub fn octets(self) -> [u8; 6] {
        self.0
    }
}

impl FromStr for Mac {
    type Err = FrixError;

    /// Accetta sei coppie esadecimali separate tutte da `:` o tutte da `-`, in
    /// maiuscolo o minuscolo. Rifiuta ogni altra forma.
    fn from_str(testo: &str) -> Result<Self, Self::Err> {
        let non_valido = || FrixError::Validation(format!("MAC non valido: {testo:?}"));
        let separatore = if testo.contains('-') { '-' } else { ':' };
        let mut byte = [0_u8; 6];
        let mut parti = testo.split(separatore);
        for b in &mut byte {
            let coppia = parti.next().ok_or_else(non_valido)?;
            if coppia.len() != 2 || !coppia.bytes().all(|c| c.is_ascii_hexdigit()) {
                return Err(non_valido());
            }
            *b = u8::from_str_radix(coppia, 16).map_err(|_| non_valido())?;
        }
        if parti.next().is_some() {
            return Err(non_valido());
        }
        Ok(Self(byte))
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
    fn accetta_maiuscolo_minuscolo_e_trattini_e_normalizza() {
        for testo in [
            "02:00:00:00:10:0A",
            "02:00:00:00:10:0a",
            "02-00-00-00-10-0a",
        ] {
            assert_eq!(
                testo.parse::<Mac>().unwrap().to_string(),
                "02:00:00:00:10:0A"
            );
        }
    }

    #[test]
    fn rifiuta_le_forme_non_valide() {
        for testo in [
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
            let errore = testo.parse::<Mac>().unwrap_err();
            assert_eq!(errore.kind(), "validation", "{testo:?}");
        }
    }
}

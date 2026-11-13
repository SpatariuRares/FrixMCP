//! Sanitizzazione dei testi che arrivano dal router (docs/PROJECT.md §7.3).
//!
//! Le funzioni qui sono pure; le applica `response.rs` a ogni risposta di tool,
//! quindi un tool nuovo non le chiama mai direttamente.

use serde_json::Value;

/// Lunghezza massima, in byte, di una stringa nei dati di un tool.
pub(crate) const MAX_CAMPO: usize = 128;

/// Lunghezza massima, in byte, di una stringa nei dati di un tool che restituisce
/// righe di testo libero (il log di sistema): una riga di log supera spesso [`MAX_CAMPO`].
pub(crate) const MAX_RIGA: usize = 512;

/// Marcatore aggiunto a una stringa accorciata.
pub(crate) const MARCATORE_TRONCATO: &str = "…[troncato]";

const ESC: char = '\u{1b}';
const BEL: char = '\u{7}';
/// CSI a un solo carattere (C1).
const CSI_C1: char = '\u{9b}';
/// OSC a un solo carattere (C1).
const OSC_C1: char = '\u{9d}';

/// Sanifica ricorsivamente ogni stringa (valori e chiavi) di un valore JSON.
pub(crate) fn json(valore: Value, limite: usize) -> Value {
    match valore {
        Value::String(s) => Value::String(testo(&s, limite)),
        Value::Array(v) => Value::Array(v.into_iter().map(|e| json(e, limite)).collect()),
        Value::Object(mappa) => Value::Object(
            mappa
                .into_iter()
                .map(|(k, v)| (testo(&k, limite), json(v, limite)))
                .collect(),
        ),
        altro => altro,
    }
}

/// Toglie sequenze ANSI, caratteri di controllo e caratteri invisibili di
/// formattazione (bidi, larghezza zero); poi limita a `limite` byte, dichiarando il taglio.
pub(crate) fn testo(s: &str, limite: usize) -> String {
    let pulito = senza_controlli(&senza_ansi(s));
    accorcia(pulito, limite)
}

/// Rimuove le sequenze di escape ANSI/ECMA-48: CSI, stringhe OSC/DCS/APC/PM/SOS
/// e gli escape a due caratteri.
fn senza_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            ESC => match chars.next() {
                Some('[') => salta_csi(&mut chars),
                Some(']' | 'P' | 'X' | '^' | '_') => salta_stringa(&mut chars),
                // escape con byte intermedi (0x20–0x2F) e un byte finale
                Some(' '..='/') => {
                    while chars.next_if(|c| matches!(c, ' '..='/')).is_some() {}
                    chars.next();
                }
                // escape a due caratteri (o ESC finale): si scartano entrambi
                _ => {}
            },
            CSI_C1 => salta_csi(&mut chars),
            OSC_C1 => salta_stringa(&mut chars),
            _ => out.push(c),
        }
    }
    out
}

/// Parametri (0x30–0x3F), intermedi (0x20–0x2F) e byte finale (0x40–0x7E) di una CSI.
fn salta_csi(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    while chars.next_if(|c| matches!(c, '0'..='?')).is_some() {}
    while chars.next_if(|c| matches!(c, ' '..='/')).is_some() {}
    chars.next_if(|c| matches!(c, '@'..='~'));
}

/// Una stringa di controllo termina con BEL o con ST (`ESC \`).
fn salta_stringa(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    while let Some(c) = chars.next() {
        if c == BEL {
            return;
        }
        if c == ESC {
            chars.next_if_eq(&'\\');
            return;
        }
    }
}

/// Spazi bianchi di controllo diventano uno spazio (le parole restano separate);
/// gli altri caratteri di controllo e quelli invisibili si tolgono.
fn senza_controlli(s: &str) -> String {
    s.chars()
        .filter_map(|c| match c {
            '\t' | '\n' | '\r' => Some(' '),
            c if c.is_control() || invisibile(c) => None,
            c => Some(c),
        })
        .collect()
}

/// Controlli bidirezionali e caratteri a larghezza zero: possono nascondere o
/// riordinare il testo che l'Operator vede.
fn invisibile(c: char) -> bool {
    matches!(
        c,
        '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{2069}' | '\u{feff}'
    )
}

/// Limita `s` a `limite` byte su un confine di carattere; se taglia, il risultato
/// termina con [`MARCATORE_TRONCATO`] ed è comunque lungo al più `limite` byte.
fn accorcia(s: String, limite: usize) -> String {
    if s.len() <= limite {
        return s;
    }
    let mut fine = limite.saturating_sub(MARCATORE_TRONCATO.len());
    while !s.is_char_boundary(fine) {
        fine -= 1;
    }
    let mut out = s.get(..fine).unwrap_or_default().to_owned();
    out.push_str(MARCATORE_TRONCATO);
    out
}

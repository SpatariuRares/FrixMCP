//! Sanitization of the text that comes from the router (docs/PROJECT.md §7.3).
//!
//! The functions here are pure; `response.rs` applies them to every tool response,
//! so a new tool never calls them directly.

use serde_json::Value;

/// Maximum length, in bytes, of a string in a tool's data.
pub(crate) const MAX_FIELD: usize = 128;

/// Maximum length, in bytes, of a string in the data of a tool that returns lines of
/// free text (the system log): a log line often exceeds [`MAX_FIELD`].
pub(crate) const MAX_LINE: usize = 512;

/// Marker appended to a shortened string.
pub(crate) const TRUNCATION_MARKER: &str = "…[truncated]";

const ESC: char = '\u{1b}';
const BEL: char = '\u{7}';
/// Single-character CSI (C1).
const CSI_C1: char = '\u{9b}';
/// Single-character OSC (C1).
const OSC_C1: char = '\u{9d}';

/// Recursively sanitizes every string (values and keys) of a JSON value.
pub(crate) fn json(value: Value, limit: usize) -> Value {
    match value {
        Value::String(s) => Value::String(text(&s, limit)),
        Value::Array(v) => Value::Array(v.into_iter().map(|e| json(e, limit)).collect()),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(k, v)| (text(&k, limit), json(v, limit)))
                .collect(),
        ),
        other => other,
    }
}

/// Removes ANSI sequences, control characters and invisible formatting characters
/// (bidi, zero width); then caps at `limit` bytes, declaring the cut.
pub(crate) fn text(s: &str, limit: usize) -> String {
    let clean = without_controls(&without_ansi(s));
    shorten(clean, limit)
}

/// Removes ANSI/ECMA-48 escape sequences: CSI, OSC/DCS/APC/PM/SOS strings
/// and two-character escapes.
fn without_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            ESC => match chars.next() {
                Some('[') => skip_csi(&mut chars),
                Some(']' | 'P' | 'X' | '^' | '_') => skip_string(&mut chars),
                // escape with intermediate bytes (0x20–0x2F) and a final byte
                Some(' '..='/') => {
                    while chars.next_if(|c| matches!(c, ' '..='/')).is_some() {}
                    chars.next();
                }
                // two-character escape (or trailing ESC): both are discarded
                _ => {}
            },
            CSI_C1 => skip_csi(&mut chars),
            OSC_C1 => skip_string(&mut chars),
            _ => out.push(c),
        }
    }
    out
}

/// Parameter (0x30–0x3F), intermediate (0x20–0x2F) and final (0x40–0x7E) bytes of a CSI.
fn skip_csi(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    while chars.next_if(|c| matches!(c, '0'..='?')).is_some() {}
    while chars.next_if(|c| matches!(c, ' '..='/')).is_some() {}
    chars.next_if(|c| matches!(c, '@'..='~'));
}

/// A control string ends with BEL or with ST (`ESC \`).
fn skip_string(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
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

/// Whitespace control characters become a space (words stay separated);
/// the other control characters and the invisible ones are removed.
fn without_controls(s: &str) -> String {
    s.chars()
        .filter_map(|c| match c {
            '\t' | '\n' | '\r' => Some(' '),
            c if c.is_control() || invisible(c) => None,
            c => Some(c),
        })
        .collect()
}

/// Bidirectional controls and zero-width characters: they can hide or
/// reorder the text the Operator sees.
fn invisible(c: char) -> bool {
    matches!(
        c,
        '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{2069}' | '\u{feff}'
    )
}

/// Caps `s` at `limit` bytes on a character boundary; if it cuts, the result
/// ends with [`TRUNCATION_MARKER`] and is still at most `limit` bytes long.
fn shorten(s: String, limit: usize) -> String {
    if s.len() <= limit {
        return s;
    }
    let mut end = limit.saturating_sub(TRUNCATION_MARKER.len());
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    let mut out = s.get(..end).unwrap_or_default().to_owned();
    out.push_str(TRUNCATION_MARKER);
    out
}

//! Regola di dipendenza fra crate (docs/PROJECT.md §6.1):
//! `frix-mcp` e `frix-web` dipendono da `frix-core`; `frix-core` non dipende
//! da nessuno dei due; `frix-mcp` e `frix-web` non si conoscono.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

/// Nomi dei crate del workspace che compaiono come dipendenza nel manifest
/// di `crate_dir`. Considera solo le righe di tabelle `*dependencies*`.
fn dipendenze_interne(crate_dir: &str) -> Vec<String> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(crate_dir)
        .join("Cargo.toml");
    let testo = std::fs::read_to_string(&manifest)
        .unwrap_or_else(|e| panic!("manifest {} non leggibile: {e}", manifest.display()));

    let mut in_dipendenze = false;
    let mut trovate = Vec::new();
    for riga in testo.lines().map(str::trim) {
        if riga.starts_with('[') {
            in_dipendenze = riga.contains("dependencies");
            continue;
        }
        if !in_dipendenze {
            continue;
        }
        let nome = riga.split(['=', '.']).next().unwrap_or("").trim();
        if ["frix-core", "frix-mcp", "frix-web"].contains(&nome) {
            trovate.push(nome.to_owned());
        }
    }
    trovate
}

#[test]
fn frix_core_non_dipende_dai_binari() {
    assert_eq!(dipendenze_interne("frix-core"), Vec::<String>::new());
}

#[test]
fn frix_mcp_dipende_solo_da_frix_core() {
    assert_eq!(dipendenze_interne("frix-mcp"), vec!["frix-core"]);
}

#[test]
fn frix_web_dipende_solo_da_frix_core() {
    assert_eq!(dipendenze_interne("frix-web"), vec!["frix-core"]);
}

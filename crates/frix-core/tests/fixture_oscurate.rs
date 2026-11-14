//! Le fixture nel repo sono oscurate: niente hostname né MAC reali del router
//! (spec #1, "Testing Decisions").
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

/// Hostname reale del router prima dell'oscuramento (sostituito da `FRITZ-Box-7590`).
const HOSTNAME_REALE: &str = "rares7590";

fn file_fixture(dir: &Path, trovati: &mut Vec<PathBuf>) {
    for voce in std::fs::read_dir(dir).expect("cartella fixture") {
        let path = voce.expect("voce").path();
        // `raw/` è l'uscita locale di `cargo xtask record`, esclusa da git: contiene
        // apposta le risposte non oscurate.
        if path.is_dir() && path.file_name().is_some_and(|nome| nome == "raw") {
            continue;
        }
        if path.is_dir() {
            file_fixture(&path, trovati);
        } else {
            trovati.push(path);
        }
    }
}

#[test]
fn nessuna_fixture_contiene_l_hostname_reale_del_router() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut file = Vec::new();
    file_fixture(&dir, &mut file);

    assert!(
        file.iter().any(|f| f.ends_with("scpd/tr64desc.xml")),
        "mancano gli SCPD fixture"
    );
    for path in file {
        let contenuto = std::fs::read_to_string(&path).expect("fixture testuale");
        assert!(
            !contenuto.to_lowercase().contains(HOSTNAME_REALE),
            "{} contiene l'hostname reale",
            path.display()
        );
    }
}

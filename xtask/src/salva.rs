//! Scrittura di una registrazione: copie grezze e copie oscurate con il layout delle
//! fixture di `crates/frix-core/tests/fixtures/` (docs/PROJECT.md §6.1):
//!
//! - `scpd/<file>`: descrizioni (`tr64desc.xml`, SCPD), `GET` in chiaro;
//! - `responses/<Servizio>-<istanza>/<Azione>[@<Arg>=<valore>&…].xml`: risposte SOAP;
//! - `lists/<file>`: liste `*ListPath`, `GET` autenticati (senza la query con il SID).
//!
//! Le copie grezze vanno in `<cartella>/grezze/`, quelle oscurate in `<cartella>/oscurate/`.
//! Con la cartella di default (`tests/fixtures/raw/`, in `.gitignore`) nessuna delle due
//! entra nel repo: le copie oscurate si rileggono e si spostano a mano fra le fixture.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use frix_core::tr064::soap_request_arguments;
use frix_core::transport::{Channel, HttpRequest, Method};

use crate::oscura::{Errore, oscura};
use crate::registra::Scambio;

/// Che cosa è stato scritto.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Salvataggio {
    /// Percorsi relativi delle fixture scritte (uguali in `grezze/` e `oscurate/`).
    pub file: Vec<String>,
    /// Richieste non salvate, con il motivo (risposta non 200, path non riconosciuto).
    pub scartati: Vec<String>,
}

/// Scrive gli scambi in `cartella/grezze/` e, oscurati tutti insieme, in `cartella/oscurate/`.
/// Se l'oscuramento fallisce non scrive nessuna copia oscurata.
pub fn salva(scambi: &[Scambio], cartella: &Path) -> Result<Salvataggio, Errore> {
    let mut fixture: BTreeMap<String, String> = BTreeMap::new();
    let mut salvataggio = Salvataggio::default();
    for scambio in scambi {
        let Some(relativo) = percorso_fixture(&scambio.richiesta) else {
            salvataggio
                .scartati
                .push(format!("path non riconosciuto: {}", scambio.richiesta.path));
            continue;
        };
        if scambio.risposta.status != 200 {
            salvataggio
                .scartati
                .push(format!("{relativo}: HTTP {}", scambio.risposta.status));
            continue;
        }
        // la stessa lettura ripetuta da più tool: vale l'ultima risposta
        fixture.insert(relativo, scambio.risposta.body.clone());
    }

    for (relativo, corpo) in &fixture {
        scrivi(&cartella.join("grezze").join(relativo), corpo)?;
    }

    // i nomi dei file passano dall'oscuramento come testo (un argomento può essere un MAC)
    let documenti: Vec<String> = fixture
        .keys()
        .cloned()
        .chain(fixture.values().cloned())
        .collect();
    let oscurati = oscura(&documenti)?;
    let (nomi, corpi) = oscurati.split_at(fixture.len());
    for (relativo, corpo) in nomi.iter().zip(corpi) {
        scrivi(&cartella.join("oscurate").join(relativo), corpo)?;
    }
    salvataggio.file = fixture.into_keys().collect();
    Ok(salvataggio)
}

fn scrivi(percorso: &Path, contenuto: &str) -> Result<(), Errore> {
    let errore = |e: std::io::Error| Errore(format!("{}: {e}", percorso.display()));
    if let Some(cartella) = percorso.parent() {
        std::fs::create_dir_all(cartella).map_err(errore)?;
    }
    std::fs::write(percorso, contenuto).map_err(errore)
}

/// Percorso relativo della fixture per una richiesta, o `None` se non ha una forma nota.
fn percorso_fixture(richiesta: &HttpRequest) -> Option<String> {
    match (richiesta.method, richiesta.channel) {
        (Method::Get, Channel::Plain) => Some(format!("scpd/{}", nome_file(&richiesta.path)?)),
        (Method::Get, Channel::Secure) => {
            let senza_query = richiesta.path.split('?').next()?;
            Some(format!("lists/{}", nome_file(senza_query)?))
        }
        (Method::Post, _) => {
            let (service, action) = richiesta.soap_service_action()?;
            let (tipo, istanza) = service.split_once(':')?;
            if ![tipo, istanza, action].iter().all(|s| sicuro(s)) {
                return None;
            }
            let argomenti = argomenti_soap(&richiesta.body, action)?;
            let suffisso = if argomenti.is_empty() {
                String::new()
            } else {
                let coppie: Vec<String> = argomenti
                    .iter()
                    .map(|(nome, valore)| format!("{nome}={valore}"))
                    .collect();
                format!("@{}", coppie.join("&"))
            };
            Some(format!("responses/{tipo}-{istanza}/{action}{suffisso}.xml"))
        }
    }
}

/// Ultimo segmento del path, se è un nome di file sicuro.
fn nome_file(path: &str) -> Option<&str> {
    let nome = path.rsplit('/').next()?;
    (sicuro(nome) && nome != "." && nome != "..").then_some(nome)
}

/// Caratteri ammessi in un nome di file di fixture.
fn sicuro(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ':'))
}

/// Argomenti in ingresso dell'envelope SOAP inviato, se hanno nomi e valori sicuri.
fn argomenti_soap(envelope: &str, action: &str) -> Option<Vec<(String, String)>> {
    let argomenti = soap_request_arguments(envelope, action).ok()?;
    argomenti
        .iter()
        .all(|(nome, valore)| sicuro(nome) && (valore.is_empty() || sicuro(valore)))
        .then_some(argomenti)
}

/// Cartella di default: `crates/frix-core/tests/fixtures/raw/`, esclusa da git.
#[must_use]
pub fn cartella_predefinita() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../crates/frix-core/tests/fixtures/raw")
}

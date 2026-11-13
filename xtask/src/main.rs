//! `cargo xtask record`: registra le risposte del router reale e ne produce copie oscurate
//! (docs/PROJECT.md §6.6). Esegue solo letture: i tool della §8.1 attraverso la Policy,
//! più un registratore che rifiuta da sé ogni azione che non sia di Level Read.
//! Tutto l'output va su stderr.

use std::path::PathBuf;
use std::process::ExitCode;

use xtask::registra::registra_dal_router;
use xtask::salva::{cartella_predefinita, salva};

const USAGE: &str = "uso: cargo xtask record [--out <cartella>] [flag di frix-mcp: --config, --host, --username, --tls-mode, --tls-pin, --tls-ca-file, --http-port]

Esegue le letture dei tool della §8.1 sul router reale (password dal Keychain) e scrive:
  <cartella>/grezze/    risposte così come arrivano (dati personali: non pubblicarle)
  <cartella>/oscurate/  copie oscurate, con il layout di crates/frix-core/tests/fixtures/
Cartella di default: crates/frix-core/tests/fixtures/raw/ (esclusa da git).";

type Errore = Box<dyn std::error::Error + Send + Sync>;

// perché: stderr è l'unico canale del comando.
#[allow(clippy::print_stderr)]
#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(codice) => codice,
        Err(errore) => {
            eprintln!("xtask: {errore}");
            ExitCode::FAILURE
        }
    }
}

#[allow(clippy::print_stderr)]
async fn run() -> Result<ExitCode, Errore> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("record") => {}
        Some("--help" | "-h") | None => {
            eprintln!("{USAGE}");
            return Ok(ExitCode::SUCCESS);
        }
        Some(altro) => return Err(format!("comando sconosciuto: {altro}\n{USAGE}").into()),
    }
    let mut cartella = cartella_predefinita();
    let mut flag_frix = Vec::new();
    while let Some(arg) = args.next() {
        if arg == "--help" || arg == "-h" {
            eprintln!("{USAGE}");
            return Ok(ExitCode::SUCCESS);
        } else if arg == "--out" {
            cartella = PathBuf::from(args.next().ok_or("--out richiede una cartella")?);
        } else if let Some(valore) = arg.strip_prefix("--out=") {
            cartella = PathBuf::from(valore);
        } else {
            flag_frix.push(arg);
        }
    }

    let registrazione = registra_dal_router(flag_frix).await?;
    let mut fallite = 0;
    for esito in &registrazione.esiti {
        match &esito.errore {
            None => eprintln!("ok      {}", esito.tool),
            Some(errore) => {
                fallite += 1;
                eprintln!("errore  {}: {errore}", esito.tool);
            }
        }
    }
    let salvataggio = salva(&registrazione.scambi, &cartella)?;
    for scartato in &salvataggio.scartati {
        eprintln!("scartato {scartato}");
    }
    eprintln!(
        "{} fixture in {}/grezze e {}/oscurate. Rileggi le copie oscurate prima di spostarle in tests/fixtures/.",
        salvataggio.file.len(),
        cartella.display(),
        cartella.display()
    );
    Ok(if fallite == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

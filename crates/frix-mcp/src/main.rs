//! `frix-mcp`: server MCP su stdio per il FRITZ!Box.
//!
//! stdout è riservato al protocollo MCP: aiuto, riepilogo d'avvio ed errori vanno su
//! stderr. Avvio: flag e config, password dal Keychain, discovery, diritti su stderr,
//! poi MCP su stdio (docs/PROJECT.md §6.3).

use std::process::ExitCode;

use frix_core::secrets;
use frix_core::transport::Credentials;
use frix_mcp::{cli, startup};

// perché: stderr è l'unico canale per l'Operator; stdout è del protocollo MCP.
// Niente `tracing` (§13.6) per ora: queste tre righe (errore, aiuto, riepilogo d'avvio)
// sono messaggi per l'Operator, non log diagnostici, e `tracing-subscriber` sarebbe una
// dipendenza nuova solo per loro. Si introduce con il primo log vero.
#[allow(clippy::print_stderr)]
#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(errore) => {
            eprintln!("frix-mcp: {errore}");
            ExitCode::FAILURE
        }
    }
}

#[allow(clippy::print_stderr)]
async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cli = cli::parse(std::env::args().skip(1))?;
    if cli.help {
        eprintln!("{}", cli::USAGE);
        return Ok(());
    }
    let config = cli::load(&cli)?;
    let password = secrets::keychain_password(&config.username)?;
    let hide_serial_number = config.hide_serial_number;
    let started = startup::connect(
        config.settings,
        Credentials {
            username: config.username,
            password,
        },
    )
    .await?;
    eprintln!("{}", startup::banner(&started.user));
    started
        .server
        .with_hidden_serial_number(hide_serial_number)
        .serve_stdio()
        .await
}

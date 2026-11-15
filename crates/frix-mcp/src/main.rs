//! `frix-mcp`: MCP server on stdio for the FRITZ!Box.
//!
//! stdout is reserved for the MCP protocol: help, startup summary and errors go to
//! stderr, in the Operator's language (`language` in `config.toml`). Startup: flags and
//! config, password from the keyring, discovery, rights on stderr, then MCP on stdio
//! (docs/PROJECT.md §6.3).

use std::process::ExitCode;

use frix_core::FrixError;
use frix_core::config::RouterConfig;
use frix_core::error::ConfigError;
use frix_core::language::Language;
use frix_core::secrets;
use frix_core::transport::Credentials;
use frix_mcp::{cli, startup};

// why: stderr is the only channel to the Operator; stdout belongs to the MCP protocol.
// No `tracing` (§13.6) for now: these lines (error, help, startup summary) are
// messages for the Operator, not diagnostic logs, and `tracing-subscriber` would be a
// new dependency just for them. It comes in with the first real log.
#[allow(clippy::print_stderr)]
#[tokio::main]
async fn main() -> ExitCode {
    // Until the config is valid, the language comes from the config file, if readable.
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cli = match cli::parse(args.clone()) {
        Ok(cli) => cli,
        Err(error) => {
            let language = cli::file_language(cli::config_flag(&args).as_deref());
            return fail(&error, language);
        }
    };
    if cli.help {
        eprintln!("{}", cli::usage(cli::file_language(cli.config.as_deref())));
        return ExitCode::SUCCESS;
    }
    let config = match cli::load(&cli) {
        Ok(config) => config,
        Err(error) => return fail(&error, cli::file_language(cli.config.as_deref())),
    };
    let language = config.language;
    let Err(error) = serve(config).await else {
        return ExitCode::SUCCESS;
    };
    if let Some(error) = error.downcast_ref::<FrixError>() {
        return fail(error, language);
    }
    eprintln!("frix-mcp: {error}");
    ExitCode::FAILURE
}

/// Writes `error` on stderr in `language`; after an unknown flag, also the help.
#[allow(clippy::print_stderr)]
fn fail(error: &FrixError, language: Language) -> ExitCode {
    eprintln!("frix-mcp: {}", error.localized(language));
    if matches!(error, FrixError::Config(ConfigError::UnknownFlag(_))) {
        eprintln!("{}", cli::usage(language));
    }
    ExitCode::FAILURE
}

#[allow(clippy::print_stderr)]
async fn serve(config: RouterConfig) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let password = secrets::keychain_password(&config.username)?;
    let started = startup::connect(
        config.settings,
        Credentials {
            username: config.username,
            password,
        },
    )
    .await?;
    eprintln!("{}", startup::banner(&started.user, config.language));
    started
        .server
        .with_hidden_serial_number(config.hide_serial_number)
        .serve_stdio()
        .await
}

//! `FrixError`: l'errore di dominio di `frix-core` (docs/PROJECT.md §5.1, §6.2).
//!
//! I testi che arrivano dal router (per esempio `errorDescription`) sono dati
//! non fidati: chi li mostra all'Agent li deve sanificare (docs/PROJECT.md §7.3).

use crate::policy::Right;

/// Errore di dominio restituito da ogni operazione verso il router.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FrixError {
    /// HTTP 401 o fault `UPnP` 401: credenziali rifiutate, o azione sconosciuta al router.
    #[error("non autenticato sul router (401)")]
    Unauthenticated,
    /// Fault 402: numero o nome degli argomenti errato.
    #[error("argomenti non validi per il router (402): {description}")]
    InvalidArgs {
        /// `errorDescription` del router.
        description: String,
    },
    /// Fault 606: il Router Account è autenticato ma non ha il diritto necessario.
    #[error("azione {action} non autorizzata per il Router Account (606): {description}")]
    NotAuthorized {
        /// `Servizio#Azione` chiamata, per esempio `DeviceInfo:1#GetInfo`.
        action: String,
        /// I diritti che bastano, uno qualunque (docs/PROJECT.md §4.2, colonna "Diritto");
        /// `None` se la §4.2 non documenta il diritto dell'azione.
        rights: Option<&'static [Right]>,
        /// `errorDescription` del router.
        description: String,
    },
    /// Fault 714: la voce richiesta non esiste (per esempio MAC sconosciuto).
    #[error("voce inesistente (714): {description}")]
    NoSuchEntry {
        /// `errorDescription` del router.
        description: String,
    },
    /// Fault 820: errore interno del router.
    #[error("errore interno del router (820): {description}")]
    RouterInternal {
        /// `errorDescription` del router.
        description: String,
    },
    /// Fault 866, 867 o 868: serve il secondo fattore, è bloccato o è occupato.
    #[error("il router richiede il secondo fattore ({code})")]
    SecondFactorRequired {
        /// Codice del fault: 866, 867 o 868.
        code: u16,
    },
    /// Qualunque altro fault `UPnP`.
    #[error("fault SOAP {code}: {description}")]
    Soap {
        /// `errorCode` del router.
        code: u16,
        /// `errorDescription` del router.
        description: String,
    },
    /// Errore del trasporto HTTP (connessione, stato HTTP inatteso, fixture mancante).
    #[error("trasporto: {0}")]
    Transport(String),
    /// Errore TLS diverso da un'impronta sbagliata (handshake, configurazione, certificato CA).
    #[error("TLS: {0}")]
    Tls(String),
    /// Il certificato del router non ha l'impronta SHA-256 configurata: possibile MITM
    /// o certificato rigenerato (docs/PROJECT.md §7.4). Nessun fallback a HTTP.
    #[error(
        "impronta TLS del router diversa da quella configurata: attesa {expected}, trovata {found}. \
         Se il certificato è stato rigenerato, rileggi l'impronta (docs/PROJECT.md §7.4) e aggiorna tls.pinned_sha256"
    )]
    TlsPinMismatch {
        /// Impronta configurata (`AB:CD:…`).
        expected: String,
        /// Impronta del certificato presentato, o `nessun certificato`.
        found: String,
    },
    /// Configurazione o Keychain non utilizzabili all'avvio.
    #[error("configurazione: {0}")]
    Config(String),
    /// La Policy nega l'azione: non arriva mai al transport.
    #[error("negato dalla Policy: {0}")]
    Policy(String),
    /// Servizio, azione o argomenti non presenti nello SCPD, o valore non valido.
    #[error("validazione: {0}")]
    Validation(String),
    /// Risposta del router non interpretabile.
    #[error("risposta del router non interpretabile: {0}")]
    Parse(String),
}

impl FrixError {
    /// Nome stabile del tipo di errore, da mostrare all'Agent.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Unauthenticated => "unauthenticated",
            Self::InvalidArgs { .. } => "invalid_args",
            Self::NotAuthorized { .. } => "not_authorized",
            Self::NoSuchEntry { .. } => "no_such_entry",
            Self::RouterInternal { .. } => "router_internal",
            Self::SecondFactorRequired { .. } => "second_factor_required",
            Self::Soap { .. } => "soap_fault",
            Self::Transport(_) => "transport",
            Self::Tls(_) => "tls",
            Self::TlsPinMismatch { .. } => "tls_pin_mismatch",
            Self::Config(_) => "config",
            Self::Policy(_) => "policy",
            Self::Validation(_) => "validation",
            Self::Parse(_) => "parse",
        }
    }
}

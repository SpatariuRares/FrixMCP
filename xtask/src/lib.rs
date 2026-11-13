//! Comandi di sviluppo di `FrixMCP` (docs/PROJECT.md §6.1, §6.6).
//!
//! - [`oscura`]: sostituisce MAC, seriali, SSID, nomi, hostname e IP pubblico nelle
//!   risposte registrate, lasciando valido l'XML o il JSON.
//! - [`registra`]: esegue le [`letture`] dei tool attraverso un trasporto che registra
//!   ogni scambio e lascia passare solo azioni di Level Read.

pub mod letture;
pub mod oscura;
pub mod registra;
pub mod salva;

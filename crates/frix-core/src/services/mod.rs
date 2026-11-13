//! Un modulo per servizio TR-064, con funzioni tipizzate sopra [`Tr064Client`](crate::tr064::Tr064Client).
//!
//! Fuori da questi moduli non circolano stringhe grezze di servizi e azioni.
//! Un'azione nuova si aggiunge qui e nel registro della Policy (`policy::registry`).

use crate::error::FrixError;

mod device_config;
pub mod device_info;
pub mod homeauto;
pub mod host_filter;
pub mod hosts;
pub mod lan_config_security;
pub mod lan_host_config;
mod list_path;
pub mod time;
pub mod user_interface;
pub mod wan_common;
pub mod wan_connection;
pub mod wan_dsl;
pub mod wlan;

/// Una lista letta dal router voce per voce (un'azione `Generic…` per indice), con un
/// tetto esplicito di voci: un router anomalo non può causare migliaia di chiamate.
#[derive(Debug, Clone, PartialEq)]
pub struct EntryList<T> {
    /// Le voci lette e interpretate, in ordine di indice.
    pub entries: Vec<T>,
    /// Le voci saltate perché non interpretabili: le altre restano valide.
    pub skipped: Vec<SkippedEntry>,
    /// `Some` se il tetto ha fermato la lettura prima della fine della lista.
    pub truncation: Option<EntryTruncation>,
}

impl<T> Default for EntryList<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            skipped: Vec::new(),
            truncation: None,
        }
    }
}

impl<T> EntryList<T> {
    /// Aggiunge la voce `index`, oppure la segna come saltata se non è interpretabile
    /// ([`FrixError::Parse`]). Ogni altro errore (trasporto, diritti, …) ferma la lettura.
    pub(crate) fn push(&mut self, index: u16, voce: Result<T, FrixError>) -> Result<(), FrixError> {
        match voce {
            Ok(voce) => self.entries.push(voce),
            Err(FrixError::Parse(reason)) => self.skipped.push(SkippedEntry { index, reason }),
            Err(errore) => return Err(errore),
        }
        Ok(())
    }
}

/// Una voce saltata: il suo indice nel router e perché non è interpretabile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedEntry {
    /// Indice della voce (l'argomento `New…Index`).
    pub index: u16,
    /// Il motivo, testo prodotto da `frix-core`.
    pub reason: String,
}

/// Il tetto di voci ha fermato la lettura.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryTruncation {
    /// Il tetto: voci lette al massimo.
    pub limit: usize,
    /// Voci non lette, se il router ne dichiara il totale; `None` se non si sa.
    pub omitted: Option<usize>,
}

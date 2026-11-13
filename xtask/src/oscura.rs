//! Oscuramento delle risposte registrate (docs/PROJECT.md §6.6, ticket #8).
//!
//! Due passate su tutti i documenti di una registrazione:
//! 1. **raccolta**: i valori dei campi sensibili ([`CAMPI`]) e ogni MAC trovato nel testo;
//! 2. **sostituzione**: ogni valore raccolto viene sostituito ovunque compaia (anche dentro
//!    il log o in un altro documento) con un sostituto della sua categoria.
//!
//! La struttura non cambia: nell'XML si riscrivono solo testo e valori degli attributi
//! (mai i nomi degli elementi), nel JSON solo le stringhe (mai le chiavi). Lo stesso MAC
//! ha sempre lo stesso sostituto, in qualunque documento e con qualunque maiuscolo/separatore.
//! Alla fine ogni documento viene ricontrollato: se un valore raccolto sopravvive,
//! l'oscuramento fallisce e nessuna fixture va scritta.
//!
//! Limite: un valore di meno di [`MIN_SOTTOSTRINGA`] caratteri si sostituisce solo
//! quando è l'intero valore di un campo, non come sottostringa di un testo libero.

use std::borrow::Cow;
use std::collections::BTreeMap;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::Value;

/// Errore dell'oscuramento: il documento non si può pubblicare come fixture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Errore(pub String);

impl std::fmt::Display for Errore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Errore {}

/// Che cosa rappresenta un valore sensibile; decide la forma del sostituto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Categoria {
    Mac,
    Seriale,
    Ssid,
    Nome,
    IpPubblico,
    Sid,
}

/// Campi sensibili per nome locale (elemento o attributo XML, chiave JSON).
/// I MAC non servono qui: si riconoscono dalla forma in qualunque campo.
const CAMPI: &[(&str, Categoria)] = &[
    // seriali e identificativi di dispositivo
    ("NewSerialNumber", Categoria::Seriale),
    ("serialNumber", Categoria::Seriale),
    ("NewProvisioningCode", Categoria::Seriale),
    ("NewAIN", Categoria::Seriale),
    ("AIN", Categoria::Seriale),
    ("ain", Categoria::Seriale),
    ("identifier", Categoria::Seriale),
    // SSID
    ("NewSSID", Categoria::Ssid),
    ("ssid", Categoria::Ssid),
    // hostname, nomi di Host e di SmartHome Device, account del provider
    ("friendlyName", Categoria::Nome),
    ("HostName", Categoria::Nome),
    ("NewHostName", Categoria::Nome),
    ("X_AVM-DE_FriendlyName", Categoria::Nome),
    ("NewX_AVM-DE_FriendlyName", Categoria::Nome),
    ("device_name", Categoria::Nome),
    ("NewDeviceName", Categoria::Nome),
    ("NewUserName", Categoria::Nome),
    ("NewPortMappingDescription", Categoria::Nome),
    // IP pubblico
    ("NewExternalIPAddress", Categoria::IpPubblico),
    ("NewExternalIPv6Address", Categoria::IpPubblico),
    ("NewIPv6Prefix", Categoria::IpPubblico),
    ("NewPrefix", Categoria::IpPubblico),
];

/// Lunghezza minima (in caratteri) per sostituire un valore anche dentro un testo libero.
pub const MIN_SOTTOSTRINGA: usize = 4;

/// Sostituto fisso del SID nei path delle liste (stesso valore delle fixture scritte a mano).
const SID_FISSO: &str = "0123456789abcdef";

/// Formato di un documento registrato.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Formato {
    /// Envelope SOAP, descrizioni, liste XML.
    Xml,
    /// Topologia mesh.
    Json,
    /// Qualunque altro testo (per esempio un log scaricato da un path).
    Testo,
}

impl Formato {
    /// Riconosce il formato dal primo carattere significativo.
    #[must_use]
    pub fn riconosci(documento: &str) -> Self {
        match documento.trim_start().chars().next() {
            Some('<') => Self::Xml,
            Some('{' | '[') => Self::Json,
            _ => Self::Testo,
        }
    }
}

/// Oscura insieme tutti i documenti di una registrazione e restituisce le copie
/// oscurate, nello stesso ordine. Fallisce se un documento non è XML/JSON valido
/// o se un valore raccolto sopravvive; l'errore indica il documento (da 0) senza
/// ripetere il valore.
pub fn oscura(documenti: &[String]) -> Result<Vec<String>, Errore> {
    let nel_documento = |i: usize| move |e: Errore| Errore(format!("documento {i}: {e}"));
    let mut oscuratore = Oscuratore::default();
    for (i, documento) in documenti.iter().enumerate() {
        percorri(documento, &mut |campo, testo| {
            oscuratore.raccogli(campo, testo);
            testo.to_owned()
        })
        .map_err(nel_documento(i))?;
    }
    let oscuratore = oscuratore.pronto();
    documenti
        .iter()
        .enumerate()
        .map(|(i, documento)| {
            let oscurato = percorri(documento, &mut |campo, testo| {
                oscuratore.sostituisci(campo, testo)
            })
            .map_err(nel_documento(i))?;
            oscuratore.verifica(&oscurato).map_err(nel_documento(i))?;
            Ok(oscurato)
        })
        .collect()
}

/// Valori raccolti e loro sostituti.
#[derive(Debug, Default)]
struct Oscuratore {
    /// originale → (categoria, sostituto)
    valori: BTreeMap<String, (Categoria, String)>,
    /// MAC normalizzato (`AA:BB:…`) → sostituto
    mac: BTreeMap<String, String>,
    contatori: BTreeMap<Categoria, u32>,
    /// Sostituzioni da applicare come sottostringa, dalla più lunga.
    in_ordine: Vec<(String, String)>,
}

impl Oscuratore {
    fn prossimo(&mut self, categoria: Categoria) -> u32 {
        let n = self.contatori.entry(categoria).or_insert(0);
        *n += 1;
        *n
    }

    fn raccogli(&mut self, campo: &str, testo: &str) {
        for mac in trova_mac(testo) {
            self.registra_mac(&mac.normalizzato);
        }
        for sid in trova_sid(testo) {
            self.valori
                .entry(sid.to_owned())
                .or_insert((Categoria::Sid, SID_FISSO.to_owned()));
        }
        let Some(categoria) = categoria_del_campo(campo) else {
            return;
        };
        let valore = testo.trim();
        if valore.is_empty() || self.valori.contains_key(valore) {
            return;
        }
        // il seriale del 7590 è il MAC senza separatori: stesso sostituto del MAC
        if categoria == Categoria::Seriale
            && let Some(mac) = mac_da_esadecimale(valore)
        {
            self.registra_mac(&mac);
            return;
        }
        let n = self.prossimo(categoria);
        let sostituto = match categoria {
            Categoria::Seriale => format!("SERIALE{n:05}"),
            Categoria::Ssid => format!("WLAN-{n:02}"),
            Categoria::IpPubblico if valore.contains(':') => format!("2001:db8::{n:x}"),
            Categoria::IpPubblico => format!("203.0.113.{}", n % 254 + 1),
            Categoria::Nome | Categoria::Mac | Categoria::Sid => format!("nome-{n:03}"),
        };
        self.valori
            .insert(valore.to_owned(), (categoria, sostituto));
    }

    fn registra_mac(&mut self, normalizzato: &str) {
        if self.mac.contains_key(normalizzato) {
            return;
        }
        let n = self.prossimo(Categoria::Mac) + 0x0f;
        let sostituto = format!("02:00:00:00:{:02X}:{:02X}", (n >> 8) & 0xff, n & 0xff);
        let senza_separatori = normalizzato.replace(':', "");
        let sostituto_senza = sostituto.replace(':', "");
        for forma in [senza_separatori.clone(), senza_separatori.to_lowercase()] {
            self.valori
                .insert(forma, (Categoria::Mac, sostituto_senza.clone()));
        }
        self.mac.insert(normalizzato.to_owned(), sostituto);
    }

    fn pronto(mut self) -> Self {
        let mut in_ordine: Vec<(String, String)> = self
            .valori
            .iter()
            .filter(|(originale, _)| originale.chars().count() >= MIN_SOTTOSTRINGA)
            .map(|(originale, (_, sostituto))| (originale.clone(), sostituto.clone()))
            .collect();
        in_ordine.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.0.cmp(&b.0)));
        self.in_ordine = in_ordine;
        self
    }

    fn sostituisci(&self, campo: &str, testo: &str) -> String {
        // campo sensibile: si sostituisce l'intero valore, anche se corto
        if categoria_del_campo(campo).is_some()
            && let Some((_, sostituto)) = self.valori.get(testo.trim())
        {
            return testo.replace(testo.trim(), sostituto);
        }
        let mut risultato = self.sostituisci_mac(testo);
        for (originale, sostituto) in &self.in_ordine {
            if risultato.contains(originale.as_str()) {
                risultato = risultato.replace(originale.as_str(), sostituto);
            }
        }
        risultato
    }

    fn sostituisci_mac(&self, testo: &str) -> String {
        let mut risultato = String::with_capacity(testo.len());
        let mut ultimo = 0;
        for mac in trova_mac(testo) {
            let Some(sostituto) = self.mac.get(&mac.normalizzato) else {
                continue;
            };
            let sostituto = if mac.separatore == '-' {
                sostituto.replace(':', "-")
            } else {
                sostituto.clone()
            };
            risultato.push_str(testo.get(ultimo..mac.inizio).unwrap_or_default());
            if mac.minuscolo {
                risultato.push_str(&sostituto.to_lowercase());
            } else {
                risultato.push_str(&sostituto);
            }
            ultimo = mac.fine;
        }
        risultato.push_str(testo.get(ultimo..).unwrap_or_default());
        risultato
    }

    /// Nessun valore raccolto (in chiaro, con escape XML o JSON) e nessun MAC non
    /// oscurato deve restare nel documento.
    fn verifica(&self, documento: &str) -> Result<(), Errore> {
        for (originale, (categoria, _)) in &self.valori {
            if originale.chars().count() < MIN_SOTTOSTRINGA || *categoria == Categoria::Sid {
                continue;
            }
            let escape_xml = quick_xml::escape::escape(originale.as_str());
            let escape_json = serde_json::to_string(originale).unwrap_or_default();
            let escape_json = escape_json.trim_matches('"');
            if documento.contains(originale.as_str())
                || documento.contains(escape_xml.as_ref())
                || documento.contains(escape_json)
            {
                return Err(Errore(format!(
                    "un valore di categoria {categoria:?} è sopravvissuto all'oscuramento"
                )));
            }
        }
        let sostituti: Vec<&String> = self.mac.values().collect();
        if let Some(mac) = trova_mac(documento)
            .into_iter()
            .find(|m| !sostituti.contains(&&m.normalizzato))
        {
            return Err(Errore(format!(
                "un MAC non oscurato è rimasto nel documento (posizione {})",
                mac.inizio
            )));
        }
        Ok(())
    }
}

fn categoria_del_campo(campo: &str) -> Option<Categoria> {
    CAMPI
        .iter()
        .find(|(nome, _)| *nome == campo)
        .map(|(_, categoria)| *categoria)
}

/// Un MAC trovato in un testo.
#[derive(Debug)]
struct MacTrovato {
    inizio: usize,
    fine: usize,
    /// Forma `AA:BB:CC:DD:EE:FF`.
    normalizzato: String,
    separatore: char,
    minuscolo: bool,
}

/// MAC con separatore `:` o `-` (sei coppie esadecimali), non attaccati ad altre cifre esadecimali.
fn trova_mac(testo: &str) -> Vec<MacTrovato> {
    const LUNGHEZZA: usize = 17;
    let byte = testo.as_bytes();
    let mut trovati = Vec::new();
    let mut i = 0;
    while i + LUNGHEZZA <= byte.len() {
        let finestra = byte.get(i..i + LUNGHEZZA).unwrap_or_default();
        let separatore = finestra.get(2).copied().unwrap_or_default();
        let forma_mac = (separatore == b':' || separatore == b'-')
            && finestra.iter().enumerate().all(|(j, c)| {
                if j % 3 == 2 {
                    *c == separatore
                } else {
                    c.is_ascii_hexdigit()
                }
            });
        let prima_libera = i == 0
            || byte
                .get(i - 1)
                .is_none_or(|c| !c.is_ascii_hexdigit() && *c != separatore);
        let dopo_libera = byte
            .get(i + LUNGHEZZA)
            .is_none_or(|c| !c.is_ascii_hexdigit() && *c != separatore);
        if forma_mac && prima_libera && dopo_libera {
            let originale = testo.get(i..i + LUNGHEZZA).unwrap_or_default();
            trovati.push(MacTrovato {
                inizio: i,
                fine: i + LUNGHEZZA,
                normalizzato: originale.to_uppercase().replace('-', ":"),
                separatore: char::from(separatore),
                minuscolo: originale.chars().any(|c| c.is_ascii_lowercase()),
            });
            i += LUNGHEZZA;
        } else {
            i += 1;
        }
    }
    trovati
}

/// MAC normalizzato da 12 cifre esadecimali senza separatori, se il valore ha quella forma.
fn mac_da_esadecimale(valore: &str) -> Option<String> {
    if valore.len() != 12 || !valore.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let maiuscolo = valore.to_uppercase();
    let coppie: Vec<&str> = (0..6)
        .filter_map(|k| maiuscolo.get(k * 2..k * 2 + 2))
        .collect();
    Some(coppie.join(":"))
}

/// Valori di `sid=` nei path delle liste.
fn trova_sid(testo: &str) -> Vec<&str> {
    testo
        .match_indices("sid=")
        .filter_map(|(i, chiave)| {
            let resto = testo.get(i + chiave.len()..)?;
            let fine = resto
                .find(|c: char| !c.is_ascii_hexdigit())
                .unwrap_or(resto.len());
            resto.get(..fine).filter(|s| !s.is_empty())
        })
        .collect()
}

/// Applica `f(campo, testo)` a ogni valore testuale del documento e restituisce il
/// documento riscritto con la stessa struttura.
fn percorri(documento: &str, f: &mut dyn FnMut(&str, &str) -> String) -> Result<String, Errore> {
    match Formato::riconosci(documento) {
        Formato::Xml => percorri_xml(documento, f),
        Formato::Json => percorri_json(documento, f),
        Formato::Testo => Ok(f("", documento)),
    }
}

fn percorri_json(
    documento: &str,
    f: &mut dyn FnMut(&str, &str) -> String,
) -> Result<String, Errore> {
    fn visita(valore: &mut Value, campo: &str, f: &mut dyn FnMut(&str, &str) -> String) {
        match valore {
            Value::String(s) => *s = f(campo, s),
            Value::Array(voci) => {
                for voce in voci {
                    visita(voce, campo, f);
                }
            }
            Value::Object(mappa) => {
                for (chiave, voce) in mappa.iter_mut() {
                    visita(voce, chiave, f);
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) => {}
        }
    }
    let mut valore: Value =
        serde_json::from_str(documento).map_err(|e| Errore(format!("JSON non valido: {e}")))?;
    visita(&mut valore, "", f);
    let mut testo = serde_json::to_string_pretty(&valore)
        .map_err(|e| Errore(format!("JSON non serializzabile: {e}")))?;
    testo.push('\n');
    Ok(testo)
}

fn percorri_xml(
    documento: &str,
    f: &mut dyn FnMut(&str, &str) -> String,
) -> Result<String, Errore> {
    let errore = |e: &dyn std::fmt::Display| Errore(format!("XML non valido: {e}"));
    let mut reader = Reader::from_str(documento);
    let mut uscita = String::with_capacity(documento.len());
    let mut elementi: Vec<String> = Vec::new();
    let mut testo = String::new();
    loop {
        let inizio = posizione(reader.buffer_position())?;
        let evento = reader.read_event().map_err(|e| errore(&e))?;
        let fine = posizione(reader.buffer_position())?;
        let grezzo = documento.get(inizio..fine).unwrap_or_default();
        match evento {
            Event::Text(t) => {
                testo.push_str(&t.xml10_content());
                continue;
            }
            Event::CData(c) => {
                testo.push_str(&c.into_inner());
                continue;
            }
            Event::GeneralRef(r) => {
                let riferimento = format!("&{};", r.xml10_content());
                let risolto = quick_xml::escape::unescape(&riferimento).map_err(|e| errore(&e))?;
                testo.push_str(&risolto);
                continue;
            }
            _ => {}
        }
        if !testo.is_empty() {
            let campo = elementi.last().map_or("", String::as_str);
            uscita.push_str(&quick_xml::escape::escape(f(campo, &testo)));
            testo.clear();
        }
        match evento {
            Event::Start(e) => {
                uscita.push_str(&riscrivi_tag(&e, false, f)?);
                elementi.push(nome_locale(&e));
            }
            Event::Empty(e) => uscita.push_str(&riscrivi_tag(&e, true, f)?),
            Event::End(_) => {
                elementi.pop();
                uscita.push_str(grezzo);
            }
            Event::Comment(c) => {
                uscita.push_str("<!--");
                uscita.push_str(&f("", &c.xml10_content()));
                uscita.push_str("-->");
            }
            Event::Eof => break,
            _ => uscita.push_str(grezzo),
        }
    }
    Ok(uscita)
}

fn posizione(p: u64) -> Result<usize, Errore> {
    usize::try_from(p).map_err(|_| Errore("documento troppo grande".into()))
}

fn nome_locale(e: &BytesStart<'_>) -> String {
    e.local_name().as_ref().to_owned()
}

/// Riscrive `<nome attr="…">` applicando `f` ai soli valori degli attributi.
fn riscrivi_tag(
    e: &BytesStart<'_>,
    vuoto: bool,
    f: &mut dyn FnMut(&str, &str) -> String,
) -> Result<String, Errore> {
    let mut tag = format!("<{}", e.name().as_ref());
    for attributo in e.attributes() {
        let attributo = attributo.map_err(|e| Errore(format!("attributo XML non valido: {e}")))?;
        let valore: Cow<'_, str> = attributo
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .map_err(|e| Errore(format!("attributo XML non valido: {e}")))?;
        let campo = attributo.key.local_name().as_ref().to_owned();
        tag.push(' ');
        tag.push_str(attributo.key.as_ref());
        tag.push_str("=\"");
        tag.push_str(&quick_xml::escape::escape(f(&campo, &valore)));
        tag.push('"');
    }
    tag.push_str(if vuoto { "/>" } else { ">" });
    Ok(tag)
}

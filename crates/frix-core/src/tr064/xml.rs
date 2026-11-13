//! Albero XML minimo sopra `quick-xml`: nomi locali (senza prefisso di namespace)
//! e testo già decodificato. Basta per `tr64desc.xml`, SCPD, envelope SOAP e
//! documenti annidati come i diritti di `X_AVM-DE_GetCurrentUser`.

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::error::FrixError;

/// Elemento XML con nome locale, testo concatenato e figli.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Element {
    pub(crate) name: String,
    pub(crate) text: String,
    pub(crate) children: Vec<Element>,
}

impl Element {
    /// Primo figlio diretto con questo nome locale.
    pub(crate) fn child(&self, name: &str) -> Option<&Element> {
        self.children.iter().find(|c| c.name == name)
    }

    /// Testo del primo figlio diretto con questo nome locale.
    pub(crate) fn child_text(&self, name: &str) -> Option<&str> {
        self.child(name).map(|c| c.text.as_str())
    }

    /// Tutti i discendenti (a qualunque profondità) con questo nome locale, in ordine di documento.
    pub(crate) fn descendants<'a>(&'a self, name: &'a str) -> Vec<&'a Element> {
        let mut trovati = Vec::new();
        self.collect(name, &mut trovati);
        trovati
    }

    fn collect<'a>(&'a self, name: &str, trovati: &mut Vec<&'a Element>) {
        for figlio in &self.children {
            if figlio.name == name {
                trovati.push(figlio);
            }
            figlio.collect(name, trovati);
        }
    }
}

/// Analizza un documento e restituisce l'elemento radice.
pub(crate) fn parse(documento: &str) -> Result<Element, FrixError> {
    let mut reader = Reader::from_str(documento);
    // la radice fittizia raccoglie l'elemento del documento
    let mut pila: Vec<Element> = vec![Element::default()];
    loop {
        let evento = reader
            .read_event()
            .map_err(|e| FrixError::Parse(format!("XML non valido: {e}")))?;
        match evento {
            Event::Start(e) => pila.push(Element {
                name: e.local_name().as_ref().to_owned(),
                ..Element::default()
            }),
            Event::Empty(e) => {
                let vuoto = Element {
                    name: e.local_name().as_ref().to_owned(),
                    ..Element::default()
                };
                aggancia(&mut pila, vuoto)?;
            }
            Event::End(_) => {
                let chiuso = pila
                    .pop()
                    .ok_or_else(|| FrixError::Parse("chiusura senza apertura".into()))?;
                aggancia(&mut pila, chiuso)?;
            }
            Event::Text(t) => testo_corrente(&mut pila)?.push_str(&t.xml10_content()),
            Event::CData(c) => {
                let dati = c.into_inner();
                testo_corrente(&mut pila)?.push_str(&dati);
            }
            Event::GeneralRef(r) => {
                let riferimento = format!("&{};", r.xml10_content());
                let risolto = quick_xml::escape::unescape(&riferimento)
                    .map_err(|e| FrixError::Parse(format!("entità non valida: {e}")))?;
                testo_corrente(&mut pila)?.push_str(&risolto);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let mut radice_fittizia = pila
        .pop()
        .ok_or_else(|| FrixError::Parse("documento vuoto".into()))?;
    if !pila.is_empty() {
        return Err(FrixError::Parse("elementi non chiusi".into()));
    }
    radice_fittizia
        .children
        .pop()
        .ok_or_else(|| FrixError::Parse("documento senza elemento radice".into()))
}

fn aggancia(pila: &mut [Element], figlio: Element) -> Result<(), FrixError> {
    pila.last_mut()
        .ok_or_else(|| FrixError::Parse("chiusura senza apertura".into()))?
        .children
        .push(figlio);
    Ok(())
}

fn testo_corrente(pila: &mut [Element]) -> Result<&mut String, FrixError> {
    pila.last_mut()
        .map(|e| &mut e.text)
        .ok_or_else(|| FrixError::Parse("testo fuori dal documento".into()))
}

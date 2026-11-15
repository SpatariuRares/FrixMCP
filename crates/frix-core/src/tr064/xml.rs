//! Minimal XML tree on top of `quick-xml`: local names (no namespace prefix) and
//! already decoded text. Enough for `tr64desc.xml`, SCPDs, SOAP envelopes and
//! nested documents such as the rights of `X_AVM-DE_GetCurrentUser`.

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::error::FrixError;

/// XML element with local name, concatenated text and children.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Element {
    pub(crate) name: String,
    pub(crate) text: String,
    pub(crate) children: Vec<Element>,
}

impl Element {
    /// First direct child with this local name.
    pub(crate) fn child(&self, name: &str) -> Option<&Element> {
        self.children.iter().find(|c| c.name == name)
    }

    /// Text of the first direct child with this local name.
    pub(crate) fn child_text(&self, name: &str) -> Option<&str> {
        self.child(name).map(|c| c.text.as_str())
    }

    /// All descendants (at any depth) with this local name, in document order.
    pub(crate) fn descendants<'a>(&'a self, name: &'a str) -> Vec<&'a Element> {
        let mut found = Vec::new();
        self.collect(name, &mut found);
        found
    }

    fn collect<'a>(&'a self, name: &str, found: &mut Vec<&'a Element>) {
        for child in &self.children {
            if child.name == name {
                found.push(child);
            }
            child.collect(name, found);
        }
    }
}

/// Parses a document and returns the root element.
pub(crate) fn parse(document: &str) -> Result<Element, FrixError> {
    let mut reader = Reader::from_str(document);
    // the dummy root collects the document element
    let mut stack: Vec<Element> = vec![Element::default()];
    loop {
        let event = reader
            .read_event()
            .map_err(|e| FrixError::Parse(format!("invalid XML: {e}")))?;
        match event {
            Event::Start(e) => stack.push(Element {
                name: e.local_name().as_ref().to_owned(),
                ..Element::default()
            }),
            Event::Empty(e) => {
                let empty = Element {
                    name: e.local_name().as_ref().to_owned(),
                    ..Element::default()
                };
                attach(&mut stack, empty)?;
            }
            Event::End(_) => {
                let closed = stack
                    .pop()
                    .ok_or_else(|| FrixError::Parse("closing tag without opening".into()))?;
                attach(&mut stack, closed)?;
            }
            Event::Text(t) => current_text(&mut stack)?.push_str(&t.xml10_content()),
            Event::CData(c) => {
                let data = c.into_inner();
                current_text(&mut stack)?.push_str(&data);
            }
            Event::GeneralRef(r) => {
                let reference = format!("&{};", r.xml10_content());
                let resolved = quick_xml::escape::unescape(&reference)
                    .map_err(|e| FrixError::Parse(format!("invalid entity: {e}")))?;
                current_text(&mut stack)?.push_str(&resolved);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let mut dummy_root = stack
        .pop()
        .ok_or_else(|| FrixError::Parse("empty document".into()))?;
    if !stack.is_empty() {
        return Err(FrixError::Parse("unclosed elements".into()));
    }
    dummy_root
        .children
        .pop()
        .ok_or_else(|| FrixError::Parse("document without root element".into()))
}

fn attach(stack: &mut [Element], child: Element) -> Result<(), FrixError> {
    stack
        .last_mut()
        .ok_or_else(|| FrixError::Parse("closing tag without opening".into()))?
        .children
        .push(child);
    Ok(())
}

fn current_text(stack: &mut [Element]) -> Result<&mut String, FrixError> {
    stack
        .last_mut()
        .map(|e| &mut e.text)
        .ok_or_else(|| FrixError::Parse("text outside the document".into()))
}

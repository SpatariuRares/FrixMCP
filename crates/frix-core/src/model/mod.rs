//! Tipi di dominio con validazione nel costruttore (docs/PROJECT.md §6.1, `model/`).

mod host;
mod mac;
mod mesh;

pub use host::Host;
pub use mac::Mac;
pub use mesh::{MeshEnd, MeshLink, MeshNode, MeshTopology};

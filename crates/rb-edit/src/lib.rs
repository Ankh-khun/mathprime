//! rb-edit: de formule-editor van MathPrime, met het gedrag van Mathcad Prime.
//!
//! * [`tree`]: de bewerkbare formule (blokken, placeholders, cursor) en wat elke toets doet;
//! * [`convert`]: van formule naar de tekst van een `.rkn`-regel en terug;
//! * [`layout`]: opmaak tijdens het bewerken, met cursorplekken;
//! * [`keys`]: de sneltoetsen van Prime.
//!
//! Zonder grafische afhankelijkheden, zodat het typen zelf getest kan worden: zie `tests.rs`.

pub mod complete;
pub mod convert;
pub mod keys;
pub mod layout;
pub mod tree;

pub use convert::{from_stmt, from_text, region_text};
pub use keys::{mathcad_key, Action, Key, KeyPress};
pub use layout::{layout, Caret, EditLayout};
pub use tree::{Atom, Block, Cmd, Cursor, Editor, Effect, Template};

/// Typ een reeks tekens in een editor (voor tests en voorbeelden). Een spatie is de spatiebalk
/// (selectie vergroten), zoals in Mathcad.
pub fn type_str(ed: &mut Editor, s: &str) -> Effect {
    let mut last = Effect::None;
    for c in s.chars() {
        last = ed.apply(Cmd::Char(c));
    }
    last
}

#[cfg(test)]
mod tests;

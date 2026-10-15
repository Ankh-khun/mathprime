//! rb-export: weergave en export van een MathPrime-werkblad.
//!
//! * [`view`]: wat elke regio laat zien (formule, tekst, tabel, grafiek), voor de interface en de export;
//! * [`doc`]: paginalayout en het tekenen van grafieken;
//! * [`svg`] en [`pdf`]: de uitvoerformaten;
//! * [`font`]: de lettertypen (dezelfde als in de interface) en hun maten.

pub mod doc;
pub mod font;
pub mod pdf;
pub mod png;
pub mod svg;
pub mod view;

pub use doc::{layout_document, layout_with_regions, RegionRect, Setup};
pub use font::FontMetrics;
pub use view::{region_view, Content, View};

use rb_sheet::Sheet;

/// Het werkblad als SVG (één doorlopende pagina).
pub fn export_svg(sheet: &Sheet, strict: bool) -> String {
    let setup = Setup { strict, ..Setup::continuous() };
    svg::to_svg(&layout_document(sheet, &setup))
}

/// Het werkblad als PDF (A4).
pub fn export_pdf(sheet: &Sheet, title: &str, strict: bool) -> Vec<u8> {
    let setup = Setup { strict, ..Setup::a4() };
    pdf::to_pdf(&layout_document(sheet, &setup), title)
}

//! rb-layout: formuleweergave voor MathPrime.
//!
//! Zet een geparseerde regel ([`rb_core::Stmt`]) en zijn resultaat om in een [`MBox`]: een vak met
//! breedte, hoogte boven en onder de basislijn, en een lijst tekenopdrachten ([`Item`]): tekst op een
//! positie, lijnen en polylijnen. Breuken, wortels, haken, matrixhaken en de tekens Σ, ∏ en ∫ worden
//! als lijnen getekend, zodat het resultaat niet afhangt van welke symbolen een lettertype heeft.
//! De interface, SVG en PDF tekenen dezelfde vakken, dus ze zien er overal hetzelfde uit.
//!
//! Coördinaten: x naar rechts, y naar **beneden**; de oorsprong van een vak ligt links op de
//! basislijn. De eenheid is die van de lettergrootte (pixels in de interface, punten in PDF).

mod expr;
pub mod geom;

pub use expr::{expr_box, stmt_box, value_box, LayoutOptions};
pub use geom::{Item, MBox, Metrics, Style};

/// Eenvoudige maten voor tests en terugval: elk teken is 0.55 em breed.
pub struct FixedMetrics;

impl Metrics for FixedMetrics {
    fn width(&self, text: &str, size: f32) -> f32 {
        0.55 * size * text.chars().count() as f32
    }
}

#[cfg(test)]
mod tests;

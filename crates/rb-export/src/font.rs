//! Lettertypen en lettermaten.
//!
//! De gewone letter is Ubuntu Light, de standaardletter van egui; tekens die daarin ontbreken
//! (zoals ∈, ∧, ∨, →) komen uit Hack. De interface laadt dezelfde twee lettertypen in dezelfde
//! volgorde, dus de maten kloppen overal.

use rb_layout::Metrics;
use std::sync::OnceLock;

/// Welke van de twee lettertypen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontId {
    /// Ubuntu Light (gewone tekst en formules).
    Main,
    /// Hack (terugval voor ontbrekende tekens, en tabellen).
    Mono,
}

pub struct Font {
    pub name: &'static str,
    pub data: &'static [u8],
    pub face: ttf_parser::Face<'static>,
}

impl Font {
    fn load(name: &'static str, data: &'static [u8]) -> Font {
        let face = ttf_parser::Face::parse(data, 0).expect("ingebouwd lettertype is geldig");
        Font { name, data, face }
    }

    pub fn units(&self) -> f32 {
        self.face.units_per_em() as f32
    }

    pub fn glyph(&self, c: char) -> Option<u16> {
        self.face.glyph_index(c).map(|g| g.0)
    }

    /// Breedte van een glyph in fracties van de lettergrootte.
    pub fn advance(&self, gid: u16) -> f32 {
        self.face.glyph_hor_advance(ttf_parser::GlyphId(gid)).unwrap_or(0) as f32 / self.units()
    }
}

pub struct Fonts {
    pub main: Font,
    pub mono: Font,
}

impl Fonts {
    pub fn get(&self, id: FontId) -> &Font {
        match id {
            FontId::Main => &self.main,
            FontId::Mono => &self.mono,
        }
    }

    /// Splits tekst in stukken per lettertype: `prefer` waar mogelijk, anders het andere.
    pub fn runs(&self, text: &str, prefer: FontId) -> Vec<(FontId, String)> {
        let other = if prefer == FontId::Main { FontId::Mono } else { FontId::Main };
        let mut out: Vec<(FontId, String)> = Vec::new();
        for c in text.chars() {
            let id = if self.get(prefer).glyph(c).is_some() || self.get(other).glyph(c).is_none() { prefer } else { other };
            match out.last_mut() {
                Some((last, s)) if *last == id => s.push(c),
                _ => out.push((id, c.to_string())),
            }
        }
        out
    }

    /// Breedte van tekst bij lettergrootte `size`, met terugval per teken.
    pub fn width(&self, text: &str, size: f32, prefer: FontId) -> f32 {
        self.runs(text, prefer)
            .iter()
            .map(|(id, s)| {
                let f = self.get(*id);
                s.chars().map(|c| f.glyph(c).map_or(0.5, |g| f.advance(g))).sum::<f32>()
            })
            .sum::<f32>()
            * size
    }
}

/// De twee ingebouwde lettertypen (één keer ingelezen).
pub fn fonts() -> &'static Fonts {
    static FONTS: OnceLock<Fonts> = OnceLock::new();
    FONTS.get_or_init(|| Fonts {
        main: Font::load("Ubuntu-Light", epaint_default_fonts::UBUNTU_LIGHT),
        mono: Font::load("Hack-Regular", epaint_default_fonts::HACK_REGULAR),
    })
}

/// Lettermaten voor de formulelayout, op basis van Ubuntu Light met Hack als terugval.
pub struct FontMetrics;

impl Metrics for FontMetrics {
    fn width(&self, text: &str, size: f32) -> f32 {
        fonts().width(text, size, FontId::Main)
    }

    fn ascent(&self) -> f32 {
        // hoogte van hoofdletters plus wat ruimte, niet de (ruime) ascender van het lettertype
        let f = &fonts().main;
        f.face.capital_height().map_or(0.7, |h| h as f32 / f.units()) + 0.08
    }

    fn descent(&self) -> f32 {
        let f = &fonts().main;
        (-f.face.descender() as f32 / f.units()).min(0.25)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terugval_per_teken() {
        let f = fonts();
        let runs = f.runs("k ∈ v", FontId::Main);
        assert_eq!(runs.len(), 3, "{runs:?}");
        assert_eq!(runs[1], (FontId::Mono, "∈".to_string()));
        assert!(f.width("MMM", 10.0, FontId::Main) > f.width("iii", 10.0, FontId::Main));
        assert!(FontMetrics.ascent() > 0.6 && FontMetrics.descent() > 0.1);
    }
}

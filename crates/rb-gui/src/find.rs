//! Zoeken en vervangen (Ctrl+F, Ctrl+H) en een variabele hernoemen.
//!
//! Gewoon zoeken kijkt in de tekst van elke regio (hoofdletters maken niet uit). "Hele namen"
//! werkt op de tokens van de lexer: `L` vindt dan niet `Lmax` en niet `L` in een tekstregio, en
//! hernoemen raakt alleen de regio's die de variabele echt gebruiken (de definitie en wat ervan
//! afhangt volgens de graaf), zodat de eenheid `L` (liter) erboven blijft staan.

use crate::app::App;
use eframe::egui::{self, RichText};

#[derive(Debug, Clone, Default)]
pub struct FindState {
    pub query: String,
    pub replace: String,
    /// Alleen hele namen (variabelen), niet stukjes tekst.
    pub whole: bool,
    /// De vervangregel tonen (Ctrl+H).
    pub show_replace: bool,
    /// Bij welke treffer we zijn (index in de lijst van treffers).
    pub current: usize,
    /// Het zoekveld krijgt de focus (net geopend).
    pub focus: bool,
}

/// Komt `name` als heel token (naam) voor in `src`? Geeft de byteposities.
pub fn name_spans(src: &str, name: &str) -> Vec<(usize, usize)> {
    let Ok(tokens) = rb_core::lex(src) else { return Vec::new() };
    tokens.iter().filter(|t| matches!(&t.tok, rb_core::Tok::Ident(n) if n == name)).map(|t| (t.start, t.end)).collect()
}

/// Zit `query` (zonder op hoofdletters te letten) in `src`?
pub fn contains_text(src: &str, query: &str) -> bool {
    !query.is_empty() && src.to_lowercase().contains(&query.to_lowercase())
}

/// Vervang elk heel token `old` in `src` door `new`.
pub fn rename_in(src: &str, old: &str, new: &str) -> String {
    let mut out = src.to_string();
    for (a, b) in name_spans(src, old).into_iter().rev() {
        out.replace_range(a..b, new);
    }
    out
}

/// Vervang elke plek van `query` (zonder op hoofdletters te letten) door `new`.
pub fn replace_text(src: &str, query: &str, new: &str) -> String {
    if query.is_empty() {
        return src.to_string();
    }
    let lower = src.to_lowercase();
    let q = query.to_lowercase();
    let mut out = String::new();
    let mut i = 0;
    // to_lowercase kan de lengte veranderen (zelden); dan gewoon letterlijk vervangen
    if lower.len() != src.len() {
        return src.replace(query, new);
    }
    while let Some(k) = lower[i..].find(&q) {
        out.push_str(&src[i..i + k]);
        out.push_str(new);
        i += k + q.len();
    }
    out.push_str(&src[i..]);
    out
}

/// Is dit een geldige naam voor een variabele?
fn valid_name(s: &str) -> bool {
    // de lexer sluit af met een Eof-token
    matches!(rb_core::lex(s).as_deref(), Ok([t, end]) if matches!(t.tok, rb_core::Tok::Ident(_)) && matches!(end.tok, rb_core::Tok::Eof))
}

impl App {
    /// De regio's met een treffer, op volgorde.
    fn find_matches(&self, f: &FindState) -> Vec<usize> {
        if f.query.is_empty() {
            return Vec::new();
        }
        (0..self.sheet.regions().len()).filter(|&j| self.region_matches(j, f)).collect()
    }

    fn region_matches(&self, j: usize, f: &FindState) -> bool {
        let r = &self.sheet.regions()[j];
        if f.whole {
            matches!(r.kind, rb_sheet::Kind::Math(_) | rb_sheet::Kind::Table(_) | rb_sheet::Kind::Formula(_) | rb_sheet::Kind::Constraint(_))
                && !name_spans(&r.source, &f.query).is_empty()
        } else {
            contains_text(&r.source, &f.query)
        }
    }

    /// De regio's waar hernoemen mag: de definitie(s) van de naam en alles wat daarvan afhangt.
    fn rename_targets(&self, name: &str) -> Vec<usize> {
        let g = self.sheet.graph();
        let defs: Vec<usize> = (0..self.sheet.regions().len())
            .filter(|&j| g.defines[j].as_deref() == Some(name) || g.also_defines[j].iter().any(|n| n == name))
            .collect();
        if defs.is_empty() {
            // geen definitie (bijvoorbeeld een naam in een solve block): overal waar hij staat
            return (0..self.sheet.regions().len()).filter(|&j| !name_spans(&self.sheet.regions()[j].source, name).is_empty()).collect();
        }
        let mut out: Vec<usize> = defs.clone();
        for &d in &defs {
            out.extend(g.dependents[d].iter().copied().filter(|&j| !name_spans(&self.sheet.regions()[j].source, name).is_empty()));
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Open de zoekbalk (Ctrl+F) of de vervangbalk (Ctrl+H). Een geselecteerde naam in de formule
    /// waar je in staat, wordt de zoekterm.
    pub(crate) fn open_find(&mut self, replace: bool) {
        let mut f = self.find.take().unwrap_or_default();
        f.show_replace = replace || f.show_replace;
        f.focus = true;
        self.find = Some(f);
    }

    /// Ga naar treffer `k` (rondgaand).
    fn goto_match(&mut self, matches: &[usize], k: usize) {
        if matches.is_empty() {
            return;
        }
        let k = k % matches.len();
        if let Some(f) = &mut self.find {
            f.current = k;
        }
        self.goto_region(matches[k]);
    }

    /// De balk boven het werkblad.
    pub(crate) fn find_bar(&mut self, ui: &mut egui::Ui) {
        let Some(mut f) = self.find.clone() else { return };
        let matches = self.find_matches(&f);
        let mut close = false;
        let (mut next, mut prev, mut replace_all) = (false, false, false);
        ui.horizontal(|ui| {
            ui.label(RichText::new(crate::icons::SEARCH.to_string()).size(15.0));
            let r = ui.add(egui::TextEdit::singleline(&mut f.query).hint_text("Zoeken").desired_width(200.0));
            if std::mem::take(&mut f.focus) {
                r.request_focus();
            }
            if r.changed() {
                f.current = 0;
            }
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                if ui.input(|i| i.modifiers.shift) { prev = true } else { next = true }
                r.request_focus();
            }
            let count = if f.query.is_empty() {
                String::new()
            } else if matches.is_empty() {
                "geen treffers".into()
            } else {
                format!("{} van {}", f.current.min(matches.len() - 1) + 1, matches.len())
            };
            ui.label(RichText::new(count).weak());
            if ui.small_button("▲").on_hover_text("Vorige (Shift+Enter)").clicked() {
                prev = true;
            }
            if ui.small_button("▼").on_hover_text("Volgende (Enter)").clicked() {
                next = true;
            }
            ui.checkbox(&mut f.whole, "Hele namen").on_hover_text("Alleen variabelen en functies met precies deze naam, niet stukjes tekst");
            ui.toggle_value(&mut f.show_replace, "Vervangen");
            if f.show_replace {
                ui.add(egui::TextEdit::singleline(&mut f.replace).hint_text(if f.whole { "Nieuwe naam" } else { "Vervangen door" }).desired_width(160.0));
                let label = if f.whole { "Hernoem overal" } else { "Vervang alles" };
                if ui.add_enabled(!matches.is_empty(), egui::Button::new(label)).clicked() {
                    replace_all = true;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button(crate::icons::CLOSE.to_string()).on_hover_text("Sluiten (Esc)").clicked() {
                    close = true;
                }
            });
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                close = true;
            }
        });
        if close {
            self.find = None;
            return;
        }
        let cur = f.current;
        self.find = Some(f.clone());
        if next {
            self.goto_match(&matches, cur + if matches.is_empty() { 0 } else { 1 });
        } else if prev {
            self.goto_match(&matches, cur + matches.len().saturating_sub(1));
        }
        if replace_all {
            self.replace_all(&f);
        }
    }

    fn replace_all(&mut self, f: &FindState) {
        if f.whole && !valid_name(&f.replace) {
            self.status = format!("'{}' is geen geldige naam", f.replace);
            return;
        }
        let targets = if f.whole { self.rename_targets(&f.query) } else { self.find_matches(f) };
        let n = targets.len();
        let (q, r, whole) = (f.query.clone(), f.replace.clone(), f.whole);
        self.rewrite(move |items| {
            for j in targets {
                if let Some(it) = items.get_mut(j) {
                    it.text = if whole { rename_in(&it.text, &q, &r) } else { replace_text(&it.text, &q, &r) };
                }
            }
        });
        self.status = if whole { format!("'{}' hernoemd naar '{}' in {n} regio('s)", f.query, f.replace) } else { format!("{n} regio('s) aangepast") };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hele_namen() {
        assert_eq!(name_spans("L := 2 m", "L"), vec![(0, 1)]);
        assert!(name_spans("Lmax := L * 2", "L").len() == 1);
        assert_eq!(rename_in("Lmax := L * 2 + f(L)", "L", "lengte"), "Lmax := lengte * 2 + f(lengte)");
        assert_eq!(rename_in("M.max := q * L^2 / 8", "M.max", "Mmax"), "Mmax := q * L^2 / 8");
        assert!(valid_name("lengte") && !valid_name("2a") && !valid_name("a b"));
    }

    #[test]
    fn gewone_tekst() {
        assert!(contains_text("# Doorbuiging", "doorbuig"));
        assert_eq!(replace_text("Balk en BALK", "balk", "ligger"), "ligger en ligger");
        assert_eq!(replace_text("abc", "", "x"), "abc");
    }
}

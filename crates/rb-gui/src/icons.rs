//! Pictogrammen (Tabler Icons, outline; MIT-licentie, zie `assets/tabler-icons-LICENSE.txt`) en de
//! bouwstenen van het lint: een knop met een pictogram of symbool en een kort opschrift eronder,
//! en een groep met een bijschrift, zoals in Mathcad Prime.
//!
//! Het lettertype staat als terugval achter de gewone lettertypen, zodat een pictogram ook midden
//! in een tekst kan staan (`format!("{} Opslaan", icons::SAVE)`).

use eframe::egui::{self, text::LayoutJob, Align, Color32, FontFamily, FontId, Response, RichText, Sense, Stroke, TextFormat, Ui, Vec2};

pub const FUNCTION: char = '\u{eeb2}';
pub const TEXT: char = '\u{ebc5}';
pub const CHART: char = '\u{ea5c}';
pub const TARGET: char = '\u{eb35}';
pub const TABLE: char = '\u{eba1}';
pub const PHOTO: char = '\u{eb0a}';
pub const UNDO: char = '\u{eb77}';
pub const REDO: char = '\u{eb78}';
pub const CUT: char = '\u{eb1b}';
pub const COPY: char = '\u{ea7a}';
pub const BOOK: char = '\u{ea39}';
pub const SETTINGS: char = '\u{eb20}';
pub const FILE: char = '\u{eaa4}';
pub const CLOSE: char = '\u{eb55}';
pub const PLUS: char = '\u{eb0b}';
pub const OK: char = '\u{ea67}';
pub const ALERT: char = '\u{ea05}';
pub const ZOOM_IN: char = '\u{eb56}';
pub const ZOOM_OUT: char = '\u{eb57}';
pub const PDF: char = '\u{fb10}';
pub const VECTOR: char = '\u{eca9}';
pub const EXPORT: char = '\u{ede9}';
pub const CODE: char = '\u{ea77}';
pub const AREA: char = '\u{f28d}';
pub const PAGE_BREAK: char = '\u{ec81}';
pub const HEADER: char = '\u{ead7}';
pub const FOOTER: char = '\u{ead3}';
pub const SUN: char = '\u{eb30}';
pub const MOON: char = '\u{eaf8}';
pub const DESKTOP: char = '\u{ea89}';
pub const DOTS: char = '\u{eaba}';
pub const GRID: char = '\u{fca5}';
pub const LINES: char = '\u{ea08}';
pub const EMPTY: char = '\u{eb2c}';
pub const REFRESH: char = '\u{eb13}';
pub const HELP: char = '\u{eabf}';
pub const KEYBOARD: char = '\u{ebd6}';
pub const NEW: char = '\u{eaa0}';
pub const OPEN: char = '\u{faf7}';
pub const SAVE: char = '\u{eb62}';

/// De accentkleur van het programma (tabbladen, selectie, actieve knoppen).
pub const ACCENT: Color32 = Color32::from_rgb(37, 99, 235);

/// De achtergrond rond de pagina's (ook de kleur van het actieve tabblad).
pub fn canvas_bg(dark: bool) -> Color32 {
    if dark { Color32::from_rgb(18, 19, 22) } else { Color32::from_rgb(235, 236, 240) }
}

/// Is dit één pictogram (privégebied van Unicode)?
fn is_icon(s: &str) -> bool {
    let mut it = s.chars();
    matches!((it.next(), it.next()), (Some(c), None) if ('\u{e000}'..='\u{f8ff}').contains(&c) || ('\u{f0000}'..).contains(&c))
}

/// Voeg het pictogramlettertype toe als terugval voor de gewone lettertypen.
pub fn add_font(fonts: &mut egui::FontDefinitions) {
    fonts.font_data.insert(
        "tabler".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../assets/tabler-icons-outline.ttf"))),
    );
    for fam in [FontFamily::Proportional, FontFamily::Monospace] {
        if let Some(list) = fonts.families.get_mut(&fam) {
            list.push("tabler".to_owned());
        }
    }
}

/// Een knop op het lint: groot pictogram of symbool, klein opschrift eronder. `on`: actief (zoals
/// een gekozen thema). Zonder rand, met een vlak bij aanwijzen.
pub fn ribbon_button(ui: &mut Ui, big: &str, caption: &str, on: bool) -> Response {
    let color = ui.visuals().text_color();
    let weak = ui.visuals().weak_text_color();
    let mut job = LayoutJob::default();
    job.halign = Align::Center;
    let big_size = if is_icon(big) { 20.0 } else { 16.0 };
    job.append(big, 0.0, TextFormat { font_id: FontId::proportional(big_size), color, ..Default::default() });
    if !caption.is_empty() {
        job.append("\n", 0.0, TextFormat { font_id: FontId::proportional(11.0), ..Default::default() });
        job.append(caption, 0.0, TextFormat { font_id: FontId::proportional(11.0), color: weak, ..Default::default() });
    }
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let pad = Vec2::new(7.0, 3.0);
    let size = Vec2::new((galley.size().x + 2.0 * pad.x).max(40.0), (galley.size().y + 2.0 * pad.y).max(44.0));
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let v = ui.visuals();
        let fill = if on {
            Some(ACCENT.gamma_multiply(0.16))
        } else if resp.is_pointer_button_down_on() {
            Some(v.widgets.active.weak_bg_fill)
        } else if resp.hovered() {
            Some(v.widgets.hovered.weak_bg_fill)
        } else {
            None
        };
        if let Some(f) = fill {
            ui.painter().rect_filled(rect, 6.0, f);
        }
        // de galley is gecentreerd op x (halign), dus de ankerplek is het midden
        let top = rect.center().y - galley.size().y / 2.0;
        ui.painter().galley(egui::pos2(rect.center().x, top), galley, color);
    }
    resp
}

/// Een groep knoppen op het lint, met een klein bijschrift eronder en een scheidingslijn rechts.
pub fn ribbon_group<R>(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    let out = ui.vertical(|ui| {
        let r = ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 1.0;
            add(ui)
        });
        let w = r.response.rect.width();
        ui.allocate_ui(Vec2::new(w, 14.0), |ui| ui.vertical_centered(|ui| ui.label(RichText::new(title).size(11.0).weak())));
        r.inner
    });
    let rect = out.response.rect;
    ui.add_space(5.0);
    let x = ui.cursor().left();
    let line = ui.visuals().widgets.noninteractive.bg_stroke.color;
    ui.painter().vline(x, rect.top() + 4.0..=rect.bottom() - 4.0, Stroke::new(1.0, line));
    ui.add_space(6.0);
    out.inner
}

/// Een tabblad van het lint: tekst met een streep eronder als het actief is.
pub fn underline_tab(ui: &mut Ui, label: &str, on: bool) -> Response {
    let text = if on { RichText::new(label).strong() } else { RichText::new(label).color(ui.visuals().weak_text_color()) };
    let resp = ui.add(egui::Label::new(text.size(14.0)).sense(Sense::click()));
    let y = resp.rect.bottom() + 4.0;
    if on {
        ui.painter().hline(resp.rect.left()..=resp.rect.right(), y, Stroke::new(2.0, ACCENT));
    } else if resp.hovered() {
        ui.painter().hline(resp.rect.left()..=resp.rect.right(), y, Stroke::new(1.0, ui.visuals().weak_text_color()));
    }
    resp
}

#[cfg(test)]
mod tests {
    #[test]
    fn pictogrammen_bestaan_in_het_lettertype() {
        let font = include_bytes!("../assets/tabler-icons-outline.ttf");
        // de cmap-tabel bevat de codes; zoek de twee bytes van een paar pictogrammen (grof, maar
        // vangt een verkeerd of leeg lettertype)
        assert!(font.len() > 1_000_000, "lettertype ontbreekt of is te klein");
        assert!(super::is_icon(&super::FUNCTION.to_string()) && !super::is_icon("x/y") && !super::is_icon("a"));
    }
}

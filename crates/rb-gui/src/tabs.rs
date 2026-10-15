//! Tabbladen: meerdere werkbladen tegelijk open, zoals in Mathcad Prime (Ctrl+Tab wisselt).
//!
//! Het actieve blad staat in de velden van [`App`] (zoals voorheen, zodat de rest van de code
//! niet verandert); de andere bladen staan in `App::tabs`. Op de plek van het actieve blad staat
//! daar een leeg blad als plaatshouder. Wisselen ruilt de velden om ([`App::swap_doc`]).

use crate::app::App;
use crate::canvas::CanvasState;
use eframe::egui::{self, RichText};
use rb_sheet::Sheet;
use std::path::PathBuf;

/// Alles wat bij één open werkblad hoort.
pub(crate) struct DocState {
    pub source: String,
    pub base_dir: PathBuf,
    pub calculated: String,
    pub sheet: Sheet,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub import_name: Option<String>,
    pub import_notes: Vec<String>,
    pub cursor: Option<usize>,
    pub jump_to: Option<usize>,
    pub cv: CanvasState,
    pub generation: u64,
    /// Vast kenmerk van het blad (voor de schuifpositie van het canvas).
    pub id: u64,
}

impl DocState {
    pub fn empty(base_dir: PathBuf, id: u64) -> DocState {
        DocState {
            source: String::new(),
            base_dir,
            calculated: String::new(),
            sheet: Sheet::load(""),
            path: None,
            dirty: false,
            import_name: None,
            import_notes: Vec::new(),
            cursor: None,
            jump_to: None,
            cv: CanvasState::default(),
            generation: 0,
            id,
        }
    }

    /// De naam op het tabblad.
    pub fn name(&self) -> String {
        if let Some(n) = &self.import_name {
            return n.clone();
        }
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map_or("naamloos.rkn".to_string(), |n| n.to_string_lossy().into_owned())
    }
}

impl App {
    /// Ruil het actieve blad met `d`. Wat voor het hele programma geldt (instellingen, thema,
    /// open vensters) blijft staan.
    fn swap_doc(&mut self, d: &mut DocState) {
        use std::mem::swap;
        swap(&mut self.source, &mut d.source);
        swap(&mut self.base_dir, &mut d.base_dir);
        swap(&mut self.calculated, &mut d.calculated);
        swap(&mut self.sheet, &mut d.sheet);
        swap(&mut self.path, &mut d.path);
        swap(&mut self.dirty, &mut d.dirty);
        swap(&mut self.import_name, &mut d.import_name);
        swap(&mut self.import_notes, &mut d.import_notes);
        swap(&mut self.cursor, &mut d.cursor);
        swap(&mut self.jump_to, &mut d.jump_to);
        swap(&mut self.generation, &mut d.generation);
        swap(&mut self.doc_id, &mut d.id);
        let keep = (self.cv.set.clone(), self.cv.dark, self.cv.show_settings, self.cv.show_functions);
        swap(&mut self.cv, &mut d.cv);
        (self.cv.set, self.cv.dark, self.cv.show_settings, self.cv.show_functions) = keep;
    }

    fn placeholder(&mut self) -> DocState {
        self.next_id += 1;
        DocState::empty(self.base_dir.clone(), self.next_id)
    }

    /// Ga naar tabblad `to`. Een formule die bewerkt wordt, wordt eerst afgerond.
    pub(crate) fn switch_tab(&mut self, to: usize) {
        if to == self.active || to >= self.tabs.len() {
            return;
        }
        self.commit(false);
        let dummy = self.placeholder();
        // het nieuwe blad uit zijn plek halen (daar komt de plaatshouder), actief maken, en het
        // oude blad op zijn eigen plek terugzetten (in plaats van de oude plaatshouder)
        let mut doc = std::mem::replace(&mut self.tabs[to], dummy);
        self.swap_doc(&mut doc);
        self.tabs[self.active] = doc;
        self.active = to;
        self.title.clear();
    }

    /// Volgend (of vorig) tabblad, rondgaand.
    pub(crate) fn cycle_tab(&mut self, back: bool) {
        let n = self.tabs.len();
        if n > 1 {
            let to = if back { (self.active + n - 1) % n } else { (self.active + 1) % n };
            self.switch_tab(to);
        }
    }

    /// Is het actieve blad nog leeg en onaangeroerd (dan opent een bestand hier, niet in een nieuw tabblad)?
    fn pristine(&self) -> bool {
        self.source.trim().is_empty() && self.path.is_none() && !self.dirty && self.import_name.is_none()
    }

    /// Een nieuw tabblad (leeg) en daarheen.
    pub(crate) fn new_tab(&mut self) {
        let d = self.placeholder();
        self.tabs.push(d);
        self.switch_tab(self.tabs.len() - 1);
    }

    /// Open een bestand in een eigen tabblad. Is het al open, dan gaat MathPrime daarheen.
    pub(crate) fn open_in_tab(&mut self, p: PathBuf) {
        let same = |q: &Option<PathBuf>| q.as_ref().is_some_and(|q| q == &p);
        if same(&self.path) {
            return;
        }
        if let Some(i) = (0..self.tabs.len()).find(|&i| i != self.active && same(&self.tabs[i].path)) {
            self.switch_tab(i);
            return;
        }
        if !self.pristine() {
            self.new_tab();
        }
        self.open_path(p);
    }

    /// Een blad met deze tekst in een (nieuw) tabblad, zoals het welkomstblad.
    pub(crate) fn text_in_tab(&mut self, text: &str) {
        if !self.pristine() {
            self.new_tab();
        }
        self.set_source(text.to_string(), None);
    }

    /// Sluit het actieve tabblad (na de vraag over niet-opgeslagen wijzigingen). Het laatste
    /// tabblad wordt een leeg blad.
    pub(crate) fn close_tab(&mut self) {
        if self.tabs.len() == 1 {
            self.set_source(String::new(), None);
            self.import_name = None;
            return;
        }
        let a = self.active;
        self.tabs.remove(a);
        let to = a.min(self.tabs.len() - 1);
        let dummy = self.placeholder();
        let mut next = std::mem::replace(&mut self.tabs[to], dummy);
        self.swap_doc(&mut next); // het gesloten blad zit nu in `next` en verdwijnt
        self.active = to;
        self.title.clear();
    }

    /// Het eerste tabblad met niet-opgeslagen wijzigingen.
    pub(crate) fn first_dirty_tab(&self) -> Option<usize> {
        if self.dirty {
            return Some(self.active);
        }
        (0..self.tabs.len()).find(|&i| i != self.active && self.tabs[i].dirty)
    }

    /// De naam van tabblad `i`, met • als er iets niet is opgeslagen.
    fn tab_label(&self, i: usize) -> (String, bool) {
        if i == self.active {
            (self.file_name(), self.dirty)
        } else {
            (self.tabs[i].name(), self.tabs[i].dirty)
        }
    }

    /// De balk met tabbladen onder het lint. Het actieve tabblad heeft de kleur van het werkblad
    /// eronder, zodat het daarin overloopt; × verschijnt bij aanwijzen.
    pub(crate) fn tab_bar(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let mut go = None;
        let mut close = None;
        let canvas = crate::icons::canvas_bg(self.cv.dark);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.add_space(6.0);
            for i in 0..self.tabs.len() {
                let (name, dirty) = self.tab_label(i);
                // lange namen inkorten
                let name = if name.chars().count() > 28 { format!("{}…", name.chars().take(26).collect::<String>()) } else { name };
                let on = i == self.active;
                let text_color = if on { ui.visuals().strong_text_color() } else { ui.visuals().weak_text_color() };
                let font = egui::FontId::proportional(13.0);
                let label = format!("{}  {name}", crate::icons::FILE);
                let galley = ui.fonts_mut(|f| f.layout_no_wrap(label, font.clone(), text_color));
                let dot = if dirty { 12.0 } else { 0.0 };
                let size = egui::vec2(galley.size().x + dot + 44.0, 30.0);
                let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
                let hovered = ui.rect_contains_pointer(rect);
                let p = ui.painter();
                let round = egui::CornerRadius { nw: 7, ne: 7, sw: 0, se: 0 };
                if on {
                    p.rect_filled(rect, round, canvas);
                    p.hline(rect.left() + 6.0..=rect.right() - 6.0, rect.top() + 1.0, egui::Stroke::new(2.0, crate::icons::ACCENT));
                } else if hovered {
                    p.rect_filled(rect, round, ui.visuals().widgets.hovered.weak_bg_fill);
                }
                let mut x = rect.left() + 12.0;
                if dirty {
                    p.circle_filled(egui::pos2(x + 3.0, rect.center().y), 3.5, egui::Color32::from_rgb(217, 119, 6));
                    x += dot;
                }
                p.galley(egui::pos2(x, rect.center().y - galley.size().y / 2.0), galley, text_color);
                // sluitknopje rechts, alleen bij aanwijzen of op het actieve tabblad
                let xr = egui::Rect::from_center_size(egui::pos2(rect.right() - 14.0, rect.center().y), egui::vec2(18.0, 18.0));
                let over_x = ui.rect_contains_pointer(xr);
                if on || hovered {
                    if over_x {
                        p.rect_filled(xr, 4.0, ui.visuals().widgets.active.weak_bg_fill);
                    }
                    p.text(xr.center(), egui::Align2::CENTER_CENTER, crate::icons::CLOSE, egui::FontId::proportional(13.0), text_color);
                }
                if resp.clicked() {
                    if over_x {
                        close = Some(i);
                    } else {
                        go = Some(i);
                    }
                }
                // middelste muisknop sluit, zoals in een browser
                if resp.middle_clicked() {
                    close = Some(i);
                }
                resp.on_hover_text(self.tab_path(i));
            }
            let plus = ui.add(egui::Button::new(RichText::new(crate::icons::PLUS.to_string()).size(15.0)).frame_when_inactive(false));
            if plus.on_hover_text("Nieuw werkblad (Ctrl+N)").clicked() {
                self.new_tab();
            }
        });
        if let Some(i) = go {
            self.switch_tab(i);
        }
        if let Some(i) = close {
            self.switch_tab(i);
            self.guarded(ctx, crate::app::Pending::TabSluiten);
        }
    }

    /// Het volledige pad van tabblad `i` (voor de tooltip).
    fn tab_path(&self, i: usize) -> String {
        let p = if i == self.active { self.path.as_ref() } else { self.tabs[i].path.as_ref() };
        p.map_or("Nog niet opgeslagen".to_string(), |p| p.display().to_string())
    }
}

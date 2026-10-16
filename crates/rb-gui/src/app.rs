//! De toepassing: bron links, opgemaakt werkblad rechts, en de menu's.

use crate::keymap::{toggle_greek, Action, Keymap};
use eframe::egui::{self, text::CCursor, text::CCursorRange, Color32, RichText, ScrollArea, TextEdit, ViewportCommand};
use rb_sheet::Sheet;
use std::path::PathBuf;
use std::time::Instant;

const WELKOM: &str = include_str!("welkom.rkn");

/// Wat er moet gebeuren nadat de gebruiker over niet-opgeslagen wijzigingen heeft beslist.
#[derive(Clone)]
pub(crate) enum Pending {
    /// Het actieve tabblad sluiten (Ctrl+W).
    TabSluiten,
    /// Het programma sluiten (na alle tabbladen met wijzigingen).
    Sluiten,
}

pub struct App {
    pub(crate) source: String,
    /// De map van het blad: bestandspaden (READCSV, ...) zijn daar relatief aan.
    pub(crate) base_dir: PathBuf,
    /// De bron bij de laatste herberekening (om te zien welke regel veranderde).
    pub(crate) calculated: String,
    pub(crate) sheet: Sheet,
    pub(crate) path: Option<PathBuf>,
    pub(crate) dirty: bool,
    show_source: bool,
    pub(crate) zoom: f32,
    pub(crate) status: String,
    pub(crate) keymap: Keymap,
    pub(crate) tab: crate::ribbon::Tab,
    /// De formulebibliotheek.
    pub(crate) lib: crate::library::LibraryState,
    show_keys: bool,
    pub(crate) show_help: bool,
    /// Een uit Mathcad ingelezen blad: voorgestelde naam bij opslaan, en wat niet overkwam.
    pub(crate) import_name: Option<String>,
    pub(crate) import_notes: Vec<String>,
    show_import: bool,
    pending: Option<Pending>,
    allow_close: bool,
    /// Cursor in de editor (tekenindex) bij het laatste frame.
    pub(crate) cursor: Option<usize>,
    /// Zet de cursor in de editor op deze tekenindex (na een klik in het werkblad).
    pub(crate) jump_to: Option<usize>,
    /// Het werkblad als canvas (zie canvas.rs).
    pub(crate) cv: crate::canvas::CanvasState,
    /// Telt op bij elke herberekening; de layout van het canvas hangt eraan.
    pub(crate) generation: u64,
    pub(crate) title: String,
    /// De open werkbladen (tabbladen); op plek `active` staat een plaatshouder, het actieve blad
    /// staat in de velden hierboven (zie tabs.rs).
    pub(crate) tabs: Vec<crate::tabs::DocState>,
    pub(crate) active: usize,
    /// Kenmerk van het actieve blad (schuifpositie), en de teller voor nieuwe.
    pub(crate) doc_id: u64,
    pub(crate) next_id: u64,
    /// Recent geopende bestanden (nieuwste eerst), voor het lege blad; zie settings.rs.
    pub(crate) recent: Vec<PathBuf>,
    /// Bestanden van latere starts (een dubbelklik in de Verkenner); zie instance.rs.
    inbox: Option<crate::instance::Inbox>,
    /// Kopieën van een vorige keer (na een crash), nog niet beantwoord; zie recovery.rs.
    pub(crate) recovery: Option<Vec<crate::recovery::Found>>,
    /// De zoekbalk (Ctrl+F); zie find.rs.
    pub(crate) find: Option<crate::find::FindState>,
    pub(crate) last_autosave: Instant,
    /// Schermafdruk voor tests (MATHPRIME_SCREENSHOT).
    shot: Option<crate::shot::Shot>,
}

fn editor_id() -> egui::Id {
    egui::Id::new("rekenblad-bron")
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, file: Option<PathBuf>, inbox: Option<crate::instance::Inbox>) -> App {
        setup_fonts(&cc.egui_ctx);
        cc.egui_ctx.set_theme(egui::Theme::Light);
        let mut app = App {
            source: String::new(),
            base_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            calculated: String::new(),
            sheet: Sheet::load(""),
            path: None,
            dirty: false,
            show_source: false,
            zoom: 1.5,
            status: String::new(),
            keymap: Keymap::load(),
            tab: Default::default(),
            lib: Default::default(),
            show_keys: false,
            show_help: false,
            import_name: None,
            import_notes: Vec::new(),
            show_import: false,
            pending: None,
            allow_close: false,
            cursor: None,
            jump_to: None,
            cv: Default::default(),
            generation: 0,
            title: String::new(),
            tabs: Vec::new(),
            active: 0,
            doc_id: 0,
            next_id: 0,
            recent: Vec::new(),
            inbox,
            recovery: None,
            find: None,
            last_autosave: Instant::now(),
            shot: crate::shot::Shot::from_env(),
        };
        app.load_settings();
        app.load_recent();
        app.check_recovery();
        if let Some(i) = &app.inbox {
            i.set_ctx(&cc.egui_ctx);
        }
        // de plaatshouder van het eerste (actieve) tabblad
        app.tabs.push(crate::tabs::DocState::empty(app.base_dir.clone(), u64::MAX));
        app.load_library();
        app.apply_theme(&cc.egui_ctx);
        app.zoom = app.cv.set.start_zoom;
        match file {
            Some(p) => app.open_path(p),
            None if app.cv.set.start_welcome => app.set_source(WELKOM.to_string(), None),
            // een leeg blad, zoals Mathcad; het welkomstblad staat onder Help
            None => app.set_source(String::new(), None),
        }
        if !app.keymap.problems.is_empty() {
            app.status = format!("Problemen in keymap.txt: {}", app.keymap.problems.join("; "));
        }
        app
    }

    pub(crate) fn set_source(&mut self, text: String, path: Option<PathBuf>) {
        let start = Instant::now();
        self.source = text;
        self.calculated = self.source.clone();
        self.sheet = Sheet::load_in(&self.source, &self.base_dir);
        self.path = path;
        let name = self.file_name();
        self.sheet.set_name(&name);
        self.dirty = false;
        self.generation += 1;
        self.cv.reset();
        self.status = format!("Doorgerekend in {:.1} ms", start.elapsed().as_secs_f64() * 1000.0);
    }

    /// Reken opnieuw na een wijziging in de bron: als precies één regel veranderde, alleen wat
    /// daarvan afhangt (via de afhankelijkheidsgraaf), anders het hele blad.
    pub(crate) fn recalc(&mut self, force_full: bool) {
        let start = Instant::now();
        let ms = || start.elapsed().as_secs_f64() * 1000.0;
        let n = |s: &Sheet| s.regions().len();
        self.status = match recalc_sheet(&mut self.sheet, &self.calculated, &self.source, force_full) {
            Some(redone) => format!("{redone} van {} regio's herberekend in {:.1} ms", n(&self.sheet), ms()),
            None => format!("Hele blad ({} regio's) doorgerekend in {:.1} ms", n(&self.sheet), ms()),
        };
        self.calculated = self.source.clone();
        self.generation += 1;
    }

    pub(crate) fn file_name(&self) -> String {
        if let Some(n) = &self.import_name {
            return n.clone();
        }
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map_or("naamloos.rkn".to_string(), |n| n.to_string_lossy().into_owned())
    }

    pub(crate) fn open_path(&mut self, p: PathBuf) {
        let is_mcdx = p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("mcdx"));
        if is_mcdx {
            return self.import_mcdx(p);
        }
        match std::fs::read_to_string(&p) {
            Ok(text) => {
                if let Some(d) = p.parent() {
                    self.base_dir = d.to_path_buf();
                }
                self.remember_recent(&p);
                self.set_source(text, Some(p));
                self.import_name = None;
            }
            Err(e) => self.status = format!("Kan {} niet openen: {e}", p.display()),
        }
    }

    /// Een Mathcad Prime-bestand inlezen. Het wordt een nieuw, nog niet opgeslagen blad: opslaan
    /// maakt een .rkn en laat het .mcdx-bestand ongemoeid.
    fn import_mcdx(&mut self, p: PathBuf) {
        let grid = rb_export::doc::Grid::new(&crate::canvas::setup());
        let result = std::fs::read(&p)
            .map_err(|e| e.to_string())
            .and_then(|b| rb_mcdx::import(&b, rb_mcdx::Grid { col_w: grid.col_w, row_h: rb_export::doc::ROW_H }));
        match result {
            Ok(imp) => {
                if let Some(d) = p.parent() {
                    self.base_dir = d.to_path_buf();
                }
                self.remember_recent(&p);
                self.set_source(imp.source, None);
                let stem = p.file_stem().map_or("mathcad".into(), |s| s.to_string_lossy().into_owned());
                self.import_name = Some(format!("{stem}.rkn"));
                self.dirty = true;
                let errors = self.sheet.error_count();
                self.status = format!(
                    "Uit Mathcad ingelezen: {} regio's, {} niet overgenomen (staan als grijze notitie op hun plek), {errors} met een fout",
                    imp.regions,
                    imp.notes.len()
                );
                self.import_notes = imp.notes;
                self.show_import = !self.import_notes.is_empty();
            }
            Err(e) => self.status = format!("Kan {} niet inlezen: {e}", p.display()),
        }
    }

    fn save_to(&mut self, p: PathBuf) -> bool {
        match std::fs::write(&p, &self.source) {
            Ok(()) => {
                self.status = format!("Opgeslagen: {}", p.display());
                if let Some(d) = p.parent() {
                    self.base_dir = d.to_path_buf();
                }
                self.remember_recent(&p);
                self.path = Some(p);
                self.import_name = None;
                self.dirty = false;
                let name = self.file_name();
                self.sheet.set_name(&name);
                true
            }
            Err(e) => {
                self.status = format!("Opslaan mislukt: {e}");
                false
            }
        }
    }

    fn save(&mut self) -> bool {
        match self.path.clone() {
            Some(p) => self.save_to(p),
            None => self.save_as(),
        }
    }

    fn save_as(&mut self) -> bool {
        let dialog = rfd::FileDialog::new().add_filter("MathPrime", &["rkn"]).set_file_name(self.file_name());
        match dialog.save_file() {
            Some(p) => self.save_to(p),
            None => false,
        }
    }

    fn export(&mut self, pdf: bool) {
        let stem = self.file_name().trim_end_matches(".rkn").to_string();
        let (ext, label) = if pdf { ("pdf", "PDF") } else { ("svg", "SVG") };
        let Some(p) = rfd::FileDialog::new().add_filter(label, &[ext]).set_file_name(format!("{stem}.{ext}")).save_file() else {
            return;
        };
        let bytes = if pdf {
            rb_export::export_pdf(&self.sheet, &stem, false)
        } else {
            rb_export::export_svg(&self.sheet, false).into_bytes()
        };
        self.status = match std::fs::write(&p, bytes) {
            Ok(()) => format!("Geëxporteerd: {}", p.display()),
            Err(e) => format!("Exporteren mislukt: {e}"),
        };
    }

    /// Opslaan als Mathcad Prime-bestand (.mcdx). Het eigen bestand (.rkn) blijft het werkbestand.
    pub(crate) fn export_mcdx(&mut self) {
        self.commit(false);
        let stem = self.file_name().trim_end_matches(".rkn").trim_end_matches(".mcdx").to_string();
        let Some(p) = rfd::FileDialog::new().add_filter("Mathcad Prime", &["mcdx"]).set_file_name(format!("{stem}.mcdx")).save_file() else {
            return;
        };
        let grid = rb_export::doc::Grid::new(&crate::canvas::setup());
        let out = rb_mcdx::export(&self.source, rb_mcdx::Grid { col_w: grid.col_w, row_h: rb_export::doc::ROW_H });
        self.status = match std::fs::write(&p, &out.bytes) {
            Ok(()) if out.notes.is_empty() => format!("Opgeslagen als Mathcad: {}", p.display()),
            Ok(()) => {
                let n = out.notes.len();
                self.import_notes = out.notes;
                self.show_import = true;
                format!("Opgeslagen als Mathcad: {} ({n} regio's als tekst, zie het lijstje)", p.display())
            }
            Err(e) => format!("Opslaan als Mathcad mislukt: {e}"),
        };
    }

    /// Voer een actie uit, eventueel na de vraag of wijzigingen bewaard moeten worden.
    pub(crate) fn run(&mut self, ctx: &egui::Context, action: Action) {
        match action {
            Action::Nieuw => self.new_tab(),
            Action::Openen => {
                if let Some(p) = rfd::FileDialog::new().add_filter("MathPrime of Mathcad Prime", &["rkn", "mcdx"]).add_filter("MathPrime", &["rkn"]).add_filter("Mathcad Prime", &["mcdx"]).pick_file() {
                    self.open_in_tab(p);
                }
            }
            Action::Opslaan => {
                self.save();
            }
            Action::OpslaanAls => {
                self.save_as();
            }
            Action::ExportPdf => self.export(true),
            Action::ExportSvg => self.export(false),
            Action::BronTonen => self.show_source = !self.show_source,
            Action::Groter => self.zoom = (self.zoom * 1.15).min(5.0),
            Action::Kleiner => self.zoom = (self.zoom / 1.15).max(0.6),
            Action::Herberekenen => self.recalc(true),
            Action::Grieks => self.greek(ctx),
            Action::Sneltoetsen => self.show_keys = true,
            Action::Help => self.show_help = true,
            Action::Welkomstblad => self.text_in_tab(WELKOM),
            Action::Instellingen => self.cv.show_settings = true,
            Action::Zoeken => self.open_find(false),
            Action::Vervangen => self.open_find(true),
            Action::Afdrukvoorbeeld => {
                self.commit(false);
                self.cv.preview = !self.cv.preview;
            }
        }
    }

    pub(crate) fn guarded(&mut self, ctx: &egui::Context, p: Pending) {
        if self.dirty {
            self.pending = Some(p);
        } else {
            self.finish(ctx, p);
        }
    }

    fn finish(&mut self, ctx: &egui::Context, p: Pending) {
        match p {
            Pending::TabSluiten => self.close_tab(),
            // nog een tabblad met wijzigingen: daarheen, en opnieuw vragen
            Pending::Sluiten => match self.first_dirty_tab() {
                Some(i) => {
                    self.switch_tab(i);
                    self.pending = Some(Pending::Sluiten);
                }
                None => {
                    self.allow_close = true;
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
            },
        }
    }

    /// Zet de letter voor de cursor om in een Griekse letter (en terug).
    fn greek(&mut self, ctx: &egui::Context) {
        let Some(idx) = self.cursor.filter(|i| *i > 0) else { return };
        let mut chars: Vec<char> = self.source.chars().collect();
        if let Some(g) = chars.get(idx - 1).and_then(|c| toggle_greek(*c)) {
            chars[idx - 1] = g;
            self.source = chars.into_iter().collect();
            self.dirty = true;
            self.recalc(false);
            self.jump_to = Some(idx);
            ctx.memory_mut(|m| m.request_focus(editor_id()));
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let mut fired = Vec::new();
        // eerst de sneltoetsen met de meeste modifiers: egui laat Ctrl+P ook bij Ctrl+Alt+P afgaan
        let mut binds: Vec<_> = self.keymap.binds.iter().collect();
        binds.sort_by_key(|(_, sc)| {
            let m = sc.modifiers;
            std::cmp::Reverse(m.alt as u8 + m.shift as u8 + m.ctrl as u8 + m.command as u8)
        });
        for (action, sc) in binds {
            if ctx.input_mut(|i| i.consume_shortcut(sc)) {
                fired.push(*action);
            }
        }
        for a in fired {
            self.run(ctx, a);
        }
    }

    fn menu(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let mut chosen = None;
        let item = |ui: &mut egui::Ui, km: &Keymap, a: Action, chosen: &mut Option<Action>| {
            use crate::icons as i;
            let icon = match a {
                Action::Nieuw => i::NEW,
                Action::Openen => i::OPEN,
                Action::Opslaan | Action::OpslaanAls => i::SAVE,
                Action::ExportPdf => i::PDF,
                Action::ExportSvg => i::VECTOR,
                Action::BronTonen => i::CODE,
                Action::Groter => i::ZOOM_IN,
                Action::Kleiner => i::ZOOM_OUT,
                Action::Herberekenen => i::REFRESH,
                Action::Grieks => i::TEXT,
                Action::Sneltoetsen => i::KEYBOARD,
                Action::Help => i::HELP,
                Action::Welkomstblad => i::BOOK,
                Action::Instellingen => i::SETTINGS,
                Action::Zoeken | Action::Vervangen => i::SEARCH,
                Action::Afdrukvoorbeeld => i::PDF,
            };
            let mut b = egui::Button::new(format!("{icon}   {}", a.label()));
            if let Some(sc) = km.shortcut(a) {
                b = b.shortcut_text(ui.ctx().format_shortcut(sc));
            }
            if ui.add(b).clicked() {
                *chosen = Some(a);
                ui.close();
            }
        };
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("Bestand", |ui| {
                for a in [Action::Nieuw, Action::Openen, Action::Opslaan, Action::OpslaanAls] {
                    item(ui, &self.keymap, a, &mut chosen);
                }
                if ui.button("Opslaan als Mathcad (.mcdx)…").clicked() {
                    ui.close();
                    self.export_mcdx();
                }
                ui.separator();
                for a in [Action::Afdrukvoorbeeld, Action::ExportPdf, Action::ExportSvg] {
                    item(ui, &self.keymap, a, &mut chosen);
                }
                ui.separator();
                item(ui, &self.keymap, Action::Instellingen, &mut chosen);
            });
            ui.menu_button("Beeld", |ui| {
                for a in [Action::Zoeken, Action::Vervangen, Action::BronTonen, Action::Groter, Action::Kleiner, Action::Herberekenen] {
                    item(ui, &self.keymap, a, &mut chosen);
                }
                ui.separator();
                let mut dark = self.cv.dark;
                if ui.checkbox(&mut dark, "Donkere modus").clicked() {
                    self.cv.set.thema = if dark { crate::settings::Thema::Donker } else { crate::settings::Thema::Licht };
                    self.apply_theme(ctx);
                    self.save_settings();
                    ui.close();
                }
            });
            ui.menu_button("Invoegen", |ui| {
                item(ui, &self.keymap, Action::Grieks, &mut chosen);
                if ui.button("Afbeelding…").clicked() {
                    ui.close();
                    self.insert(ctx, crate::canvas::Ins::Image);
                }
            });
            ui.menu_button("Help", |ui| {
                for a in [Action::Help, Action::Sneltoetsen, Action::Welkomstblad] {
                    item(ui, &self.keymap, a, &mut chosen);
                }
            });
        });
        if let Some(a) = chosen {
            self.run(ctx, a);
        }
    }

    fn editor(&mut self, ui: &mut egui::Ui) {
        ScrollArea::vertical().id_salt("bron").show(ui, |ui| {
            let out = TextEdit::multiline(&mut self.source)
                .id(editor_id())
                .code_editor()
                .desired_width(f32::INFINITY)
                .desired_rows(40)
                .lock_focus(true)
                .show(ui);
            if out.response.changed() {
                self.dirty = true;
                self.cv.source_changed();
                self.recalc(false);
            }
            self.cursor = out.cursor_range.map(|r| r.primary.index.0);
        });
        if let Some(idx) = self.jump_to.take() {
            if let Some(mut state) = TextEdit::load_state(ui.ctx(), editor_id()) {
                state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(idx))));
                state.store(ui.ctx(), editor_id());
            }
            ui.ctx().memory_mut(|m| m.request_focus(editor_id()));
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(p) = self.pending.clone() {
            // (opslaan gelukt of niet opslaan, annuleren)
            let mut choice: Option<bool> = None;
            let mut cancel = false;
            egui::Modal::new(egui::Id::new("niet-opgeslagen")).show(ctx, |ui| {
                ui.heading("Niet-opgeslagen wijzigingen");
                ui.label(format!("{} heeft wijzigingen die nog niet zijn opgeslagen.", self.file_name()));
                ui.horizontal(|ui| {
                    if ui.button("Opslaan").clicked() && self.save() {
                        choice = Some(true);
                    }
                    if ui.button("Niet opslaan").clicked() {
                        choice = Some(false);
                    }
                    if ui.button("Annuleren").clicked() {
                        cancel = true;
                    }
                });
            });
            // eerst de vraag weg, dan de actie (die kan een nieuwe vraag stellen: het volgende tabblad)
            if cancel || choice.is_some() {
                self.pending = None;
            }
            if let Some(saved) = choice {
                if !saved {
                    self.dirty = false;
                }
                self.finish(ctx, p);
            }
        }
        let mut open = self.show_keys;
        egui::Window::new("Sneltoetsen").open(&mut open).collapsible(false).show(ctx, |ui| {
            egui::Grid::new("toetsen").striped(true).show(ui, |ui| {
                for (a, sc) in &self.keymap.binds {
                    ui.label(a.label());
                    ui.monospace(ctx.format_shortcut(sc));
                    ui.monospace(a.key());
                    ui.end_row();
                }
            });
            ui.separator();
            match &self.keymap.path {
                Some(p) => {
                    ui.label(format!("Aanpassen in {} (regels 'actie = toets').", p.display()));
                    if !p.exists() && ui.button("Bestand met de standaardtoetsen aanmaken").clicked() {
                        let ok = p.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|_| std::fs::write(p, Keymap::default_file()));
                        self.status = match ok {
                            Ok(()) => format!("Aangemaakt: {}", p.display()),
                            Err(e) => format!("Kan {} niet aanmaken: {e}", p.display()),
                        };
                    }
                    if ui.button("Opnieuw inlezen").clicked() {
                        self.keymap = Keymap::load();
                    }
                }
                None => {
                    ui.label("Geen instellingenmap gevonden; de standaardtoetsen gelden.");
                }
            }
            for prob in &self.keymap.problems {
                ui.colored_label(Color32::from_rgb(190, 30, 30), prob);
            }
        });
        self.show_keys = open;
        let mut open = self.show_help;
        egui::Window::new("Syntax en hulp").open(&mut open).default_width(560.0).show(ctx, |ui| {
            ScrollArea::vertical().max_height(500.0).show(ui, |ui| {
                ui.monospace(HELP);
            });
        });
        self.show_help = open;
        let mut open = self.show_import;
        egui::Window::new("Mathcad: wat niet helemaal overkwam").open(&mut open).default_width(480.0).show(ctx, |ui| {
            ui.label("Bij inlezen staat op die plek een grijze notitie; bij opslaan staat de regio in Mathcad als tekst.");
            ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                for n in &self.import_notes {
                    ui.label(format!("• {n}"));
                }
            });
        });
        self.show_import = open;
    }
}

/// Breng `sheet` van bron `old` naar bron `new`. Als precies één gewone regel veranderde, wordt
/// alleen die regio vervangen en herberekend wat ervan afhangt (geeft het aantal herberekende
/// regio's); anders wordt het hele blad opnieuw ingelezen (`None`).
pub(crate) fn recalc_sheet(sheet: &mut Sheet, old: &str, new: &str, force_full: bool) -> Option<usize> {
    let o: Vec<&str> = old.lines().collect();
    let n: Vec<&str> = new.lines().collect();
    if !force_full && o.len() == n.len() {
        let changed: Vec<usize> = (0..n.len()).filter(|&k| o[k] != n[k]).collect();
        if let [k] = changed[..] {
            // alleen een regel die zelf een regio is (niet in een programmablok, niet leeg)
            let region = sheet.regions().iter().position(|r| r.line == k + 1 && !r.source.contains('\n'));
            let simple = |s: &str| !s.trim().is_empty() && !s.trim_end().ends_with('{') && s.trim() != "}";
            if let (Some(idx), true, true) = (region, simple(o[k]), simple(n[k])) {
                return Some(sheet.update_region(idx, n[k]).len());
            }
        }
    }
    let name = sheet.name().to_string();
    *sheet = Sheet::load_in(new, &sheet.dir().to_path_buf());
    sheet.set_name(&name);
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn een_regel_wijzigen_rekent_alleen_de_afhankelijken_opnieuw() {
        let old = "a := 2 m\nb := 3 m\nc := a * 2\nc =\nb =\n";
        let mut sheet = Sheet::load(old);
        let new = old.replace("a := 2 m", "a := 5 m");
        assert_eq!(recalc_sheet(&mut sheet, old, &new, false), Some(3));
        // hetzelfde resultaat als opnieuw inlezen
        assert_eq!(sheet.render(), Sheet::load(&new).render());
        // een regel erbij: alles opnieuw
        let more = format!("{new}d := 1\n");
        assert_eq!(recalc_sheet(&mut sheet, &new, &more, false), None);
        assert_eq!(sheet.regions().len(), 6);
    }

    #[test]
    fn regels_in_een_blok_of_lege_regels_geven_een_volledige_herberekening() {
        let old = "f(n) := {\n  s := 0\n  s + n\n}\nf(2) =\n";
        let mut sheet = Sheet::load(old);
        let new = old.replace("s := 0", "s := 10");
        assert_eq!(recalc_sheet(&mut sheet, old, &new, false), None);
        assert_eq!(sheet.render(), Sheet::load(&new).render());
        assert!(sheet.render().contains("f(2) = 12"));
        let old2 = "a := 1\n\nb := 2\n";
        let mut sheet = Sheet::load(old2);
        assert_eq!(recalc_sheet(&mut sheet, old2, "a := 1\nc := 3\nb := 2\n", false), None);
    }

    #[test]
    fn welkomstblad_rekent_foutloos() {
        let sheet = Sheet::load(WELKOM);
        assert_eq!(sheet.error_count(), 0, "{}", sheet.render());
        // M = q L²/8 = 3 kN/m · 16 m² / 8; w(L/2) = 5 q L⁴ / (384 E I)
        assert!(sheet.render().contains("M.max = 6 kN·m"), "{}", sheet.render());
        assert!(sheet.render().contains("w(L / 2) = 5.95238 mm"), "{}", sheet.render());
    }
}

const HELP: &str = r#"Het werkblad werkt zoals Mathcad Prime. Klik op een lege plek: daar staat het rode kruisje,
en wat je typt wordt een formule. Klik op een formule om hem te wijzigen, sleep om hem te
verplaatsen, sleep vanaf een lege plek om regio's te selecteren (Delete, Ctrl+X/C/V).

  :  definitie (:=)        =  uitrekenen; typ daarna een eenheid      /  breuk
  ^  macht                 [  index                                   \  wortel
  |  absolute waarde       !  faculteit      Spatie  selectie vergroten
  Ctrl+G Griekse letter    Ctrl+M matrix (Shift+Enter rij, Shift+Spatie kolom)
  Ctrl+Shift+D afgeleide   Ctrl+Shift+I integraal   Ctrl+Shift+4 som   Ctrl+Shift+3 product
  ]  programma   {  lokale toekenning   }  if   Ctrl+Shift+] else   Ctrl+; else if
  Ctrl+] while   Ctrl+Shift+' for   Ctrl+Shift+[ break   Ctrl+Shift+; continue   Ctrl+\ return
  Ctrl+= gelijk (==)   Ctrl+9 ≤   Ctrl+0 ≥   Ctrl+Shift+7 and   Ctrl+Shift+2 or   Ctrl+Shift+1 not
  "…" in een formule: tekst als waarde (concat, strlen, substr, search, num2str)
  "  of Ctrl+T tekst       Ctrl+2 grafiek          Ctrl+7 solve block (Given)
  Ctrl+Z/Y ongedaan/opnieuw   F5 herberekenen   F2 functies   Ctrl+P PDF   Ctrl+wiel zoomen
  Esc of klikken ernaast: formule klaar     Enter in een programma: regel erbij

Onder Beeld staat de bron (gewone tekst, .rkn), voor wie liever typt:
a := 5 m                 definitie            f(x) := x^2      functie
a * 2 =                  resultaat            a * 2 = cm       in een gekozen eenheid
" tekst                  tekstregel           # kop            kop
i := 1..10               bereik               v[i] := i^2      element van een vector
[1, 2; 3, 4]             matrix               M[0, 1]          index (ORIGIN := 1 voor 1-gebaseerd)
if(x < 0, -x, x)         voorwaarde           x <= 5 and y != 2   vergelijken (1 of 0)
sum(i^2, i, 1, 10)       som                  integral(x^2, x, 0, 1)   integraal
deriv(x^3, x, 2)         afgeleide            3 + 4i           complex getal
20 °C                    temperatuur          T = °F           tonen in °F
plot x = 0 ; 10 : sin(x), cos(x)              grafiek (200 punten)
plot t = 0 s ; 2 s ; 500 : u(t) = mm          500 punten, y-as in mm
plot vx : vy                                  meetpunten

f(n) := {                programmablok (inspringen; for, while, if/else,
    s := 0               break, continue, return), afgesloten met }
    for k in 1..n
        s := s + k
    s
}

Given                    solve block: vergelijkingen met ==, dan
x + y == 10              Find(x, y), Minerr(x, y), Minimize(f, x, y) of Maximize(f, x, y)
Find(x, y) =

Zie README.md en docs/PLAN.md voor alles."#;

impl eframe::App for App {
    /// Netjes afgesloten: de herstelkopieën zijn niet meer nodig.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.clear_recovery();
    }

    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        if let Some(s) = &mut self.shot {
            s.inject(raw);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = &ui.ctx().clone();

        // bestanden van een tweede start (dubbelklik in de Verkenner): een tabblad, en naar voren
        let requests = self.inbox.as_ref().map(|i| i.take()).unwrap_or_default();
        if !requests.is_empty() {
            for p in requests.into_iter().flatten() {
                self.open_in_tab(p);
            }
            // zonder dit schuift egui bij het terugkrijgen van de focus het werkblad (de widget met
            // toetsenbordfocus) naar beneden; een klik op het blad geeft het de focus weer
            if let Some(id) = ctx.memory(|m| m.focused()) {
                ctx.memory_mut(|m| m.surrender_focus(id));
            }
            ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(ViewportCommand::Focus);
        }
        // Ctrl+Tab en Ctrl+Shift+Tab: volgend en vorig tabblad (vóór de toetsen van het werkblad)
        let back = ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::Tab));
        let fwd = ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Tab));
        if back || fwd {
            self.cycle_tab(back);
        }
        self.canvas_keys(ctx);
        self.shortcuts(ctx);
        // bestanden naar het venster gesleept: .rkn/.mcdx openen, PNG invoegen
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect());
        for p in dropped {
            match p.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
                Some("png") => self.insert_image_file(&p),
                Some("rkn" | "mcdx") => self.open_in_tab(p),
                _ => self.status = format!("{} kan niet geopend worden (wel: .rkn, .mcdx, .png)", p.display()),
            }
        }
        // bij een schermafdruk voor tests niet om opslaan vragen
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close && self.shot.is_none() {
            // een tabblad met wijzigingen: daarheen en vragen (daarna de volgende)
            if let Some(i) = self.first_dirty_tab() {
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
                self.switch_tab(i);
                self.pending = Some(Pending::Sluiten);
            }
        }
        let title = format!("{}{} — MathPrime", if self.dirty { "• " } else { "" }, self.file_name());
        if title != self.title {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.title = title;
        }
        egui::Panel::top("menu").show(ui, |ui| self.menu(ctx, ui));
        egui::Panel::top("lint").show(ui, |ui| self.ribbon(ctx, ui));
        let tab_fill = ui.visuals().panel_fill;
        egui::Panel::top("tabbladen")
            .frame(egui::Frame::new().fill(tab_fill).inner_margin(egui::Margin { left: 0, right: 0, top: 5, bottom: 0 }))
            .show_separator_line(false)
            .show(ui, |ui| self.tab_bar(ctx, ui));
        if self.find.is_some() {
            egui::Panel::top("zoeken").show(ui, |ui| self.find_bar(ui));
        }
        if self.cv.preview {
            egui::Panel::top("afdrukvoorbeeld").show(ui, |ui| {
                ui.horizontal(|ui| {
                    let pages = self.page_count();
                    ui.label(RichText::new(format!("{}  Afdrukvoorbeeld", crate::icons::PDF)).strong());
                    ui.label(RichText::new(format!("{pages} {} · zo komt het in de PDF", if pages == 1 { "pagina" } else { "pagina's" })).weak());
                    if ui.button(format!("{}  PDF maken…", crate::icons::PDF)).clicked() {
                        self.run(ctx, Action::ExportPdf);
                    }
                    if ui.button("Terug naar bewerken (Esc)").clicked() {
                        self.cv.preview = false;
                    }
                });
            });
        }
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                let errors = self.sheet.error_count();
                let n = self.sheet.regions().len();
                let (green, red) = if self.cv.dark {
                    (Color32::from_rgb(110, 200, 140), Color32::from_rgb(240, 120, 120))
                } else {
                    (Color32::from_rgb(0, 120, 60), Color32::from_rgb(190, 30, 30))
                };
                if errors > 0 {
                    let text = format!("{} {errors} {}", crate::icons::ALERT, if errors == 1 { "fout" } else { "fouten" });
                    let r = ui.add(egui::Label::new(RichText::new(text).color(red)).sense(egui::Sense::click()));
                    if r.on_hover_text("Naar de eerste fout").clicked() {
                        self.goto_first_error();
                    }
                } else {
                    ui.label(RichText::new(format!("{} Geen fouten", crate::icons::OK)).color(green));
                }
                ui.add_space(8.0);
                ui.label(RichText::new(format!("{n} regio's  ·  {}", self.status)).weak());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let plain = |s: String| egui::Button::new(RichText::new(s).size(14.0)).frame_when_inactive(false);
                    if ui.add(plain(crate::icons::ZOOM_IN.to_string())).on_hover_text("Groter (Ctrl+wiel)").clicked() {
                        self.run(ctx, Action::Groter);
                    }
                    if ui.add(egui::Label::new(format!("{:.0}%", self.zoom / 1.5 * 100.0)).sense(egui::Sense::click())).on_hover_text("Terug naar 100%").clicked() {
                        self.zoom = 1.5;
                    }
                    if ui.add(plain(crate::icons::ZOOM_OUT.to_string())).on_hover_text("Kleiner (Ctrl+wiel)").clicked() {
                        self.run(ctx, Action::Kleiner);
                    }
                });
            });
        });
        if self.show_source {
            let w = ctx.content_rect().width() * 0.4;
            egui::Panel::left("bronpaneel")
                .resizable(true)
                .default_size(w)
                .show(ui, |ui| self.editor(ui));
        }
        if self.lib.show {
            egui::Panel::right("formules").resizable(true).default_size(340.0).show(ui, |ui| self.library_panel(ctx, ui));
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(crate::icons::canvas_bg(self.cv.dark)).inner_margin(8.0))
            .show(ui, |ui| {
                // elk tabblad zijn eigen schuifpositie
                ScrollArea::both().id_salt(("werkblad", self.doc_id)).show(ui, |ui| self.canvas(ui));
            });
        self.dialogs(ctx);
        self.functions_window(ctx);
        self.settings_window(ctx);
        self.recovery_window(ctx);
        self.autosave(ctx);
        if self.cv.set.thema == crate::settings::Thema::Systeem {
            self.apply_theme(ctx);
        }
        self.library_form(ctx);
        if let Some(s) = &mut self.shot {
            s.tick(ctx);
        }
    }
}

/// Dezelfde lettertypen als de export: Ubuntu Light met Hack als terugval voor ontbrekende tekens.
fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    crate::icons::add_font(&mut fonts);
    if let Some(fam) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        if !fam.iter().any(|f| f == "Hack") {
            fam.insert(1, "Hack".to_owned());
        }
    }
    ctx.set_fonts(fonts);
}


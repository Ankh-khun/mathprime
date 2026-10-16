//! Het werkblad als canvas, zoals in Mathcad Prime.
//!
//! Regio's staan op hun plek in het raster van de pagina (`@kolom,rij` in de bron). Klik op een lege
//! plek: daar komt het rode kruisje, en wat je typt wordt een nieuwe wiskunderegio. Klik op een regio
//! om hem te bewerken; de toetsen zijn die van Mathcad Prime (zie `rb_edit::keys`). Slepen verplaatst
//! regio's, slepen vanaf een lege plek selecteert ze (Delete, Ctrl+X/C/V).
//!
//! Elke wijziging schrijft de bron opnieuw, met een positie voor elke regio, en rekent opnieuw.

use crate::app::{App, Pending};
use crate::keymap::Action as MenuAction;
use crate::paint;
use eframe::egui::{self, Color32, Event, EventFilter, FontFamily, FontId, Key as EKey, Pos2, Rect, Sense, Stroke, StrokeKind, TextEdit, Vec2};
use rb_edit::keys::Sheet as SheetAction;
use rb_edit::{mathcad_key, Action, Cmd, Editor, Effect, Key, KeyPress};
use rb_export::doc::{box_ops, layout_full, Grid, Page, RegionRect, Setup, ROW_H};
use rb_export::FontMetrics;
use rb_layout::LayoutOptions;
use rb_sheet::{parse_sheet, Kind, Output, Pos};
use crate::taal::t;

/// Ruimte rond en tussen de pagina's op het scherm (pixels).
const PAGE_GAP: f32 = 18.0;
const BLUE: Color32 = Color32::from_rgb(30, 90, 200);
const SELECT: Color32 = Color32::from_rgb(214, 228, 250);

/// Het paginaformaat van het werkblad: A4, grafieken interactief.
pub fn setup() -> Setup {
    Setup {
        static_plots: false,
        ..Setup::a4()
    }
}

fn canvas_id() -> egui::Id {
    egui::Id::new("werkblad-canvas")
}

fn text_id() -> egui::Id {
    egui::Id::new("werkblad-tekstregio")
}

/// Een regio als regel van de bron.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub col: usize,
    pub row: usize,
    pub text: String,
}

/// De bron bij een lijst regio's: van boven naar beneden, elke regio met zijn positie.
pub fn serialize(items: &mut [Item]) -> String {
    items.sort_by_key(|i| (i.row, i.col));
    items.iter().map(|i| format!("@{},{} {}\n", i.col, i.row, i.text)).collect()
}

pub struct Layout {
    generation: u64,
    pages: Vec<Page>,
    rects: Vec<RegionRect>,
    grid: Grid,
    /// Gemaakt voor het afdrukvoorbeeld (andere opmaak: vaste grafieken, geen schermtekens).
    preview: bool,
}

pub enum Mode {
    Math(Editor),
    /// Tekst (`prefix` is `" `, `# ` enz.), of de bron van een regio die geen formule is (`mono`).
    Text { prefix: String, buf: String, mono: bool },
    /// Een tabel (Ctrl+6): kolomnamen, eenheden en cellen, elk een tekstvak.
    Table(TableEdit),
}

pub struct TableEdit {
    names: Vec<String>,
    units: Vec<String>,
    rows: Vec<Vec<String>>,
    /// Dit vak krijgt de focus (rij, kolom); rij 0 en 1 zijn de namen en eenheden.
    focus: Option<(usize, usize)>,
}

impl TableEdit {
    fn new() -> TableEdit {
        TableEdit {
            names: vec!["x".into(), "y".into()],
            units: vec![String::new(), String::new()],
            rows: vec![vec![String::new(); 2]; 3],
            focus: Some((2, 0)),
        }
    }

    fn from_spec(t: &rb_sheet::TableSpec) -> TableEdit {
        TableEdit {
            names: t.cols.iter().map(|c| c.name.clone()).collect(),
            units: t.cols.iter().map(|c| c.unit.clone()).collect(),
            rows: t.rows.clone(),
            focus: Some((2, 0)),
        }
    }

    /// De bron (`tabel x [mm], F [kN] { ... }`); lege kolommen en rijen vallen weg.
    fn source(&self) -> String {
        let keep: Vec<usize> = (0..self.names.len()).filter(|&k| !self.names[k].trim().is_empty()).collect();
        if keep.is_empty() {
            return String::new();
        }
        let head: Vec<String> = keep
            .iter()
            .map(|&k| match self.units[k].trim() {
                "" => self.names[k].trim().to_string(),
                u => format!("{} [{u}]", self.names[k].trim()),
            })
            .collect();
        let mut rows: Vec<String> = self
            .rows
            .iter()
            .filter(|r| keep.iter().any(|&k| !r[k].trim().is_empty()))
            .map(|r| keep.iter().map(|&k| r[k].trim()).collect::<Vec<_>>().join(", "))
            .collect();
        if rows.is_empty() {
            rows.push(vec!["0"; keep.len()].join(", "));
        }
        format!("tabel {} {{\n{}\n}}", head.join(", "), rows.iter().map(|r| format!("    {r}")).collect::<Vec<_>>().join("\n"))
    }

    fn cell_id(r: usize, c: usize) -> egui::Id {
        egui::Id::new(("tabelcel", r, c))
    }
}

pub struct Edit {
    col: usize,
    row: usize,
    /// Linksboven, in doorlopende coördinaten van het blad.
    at: (f32, f32),
    /// De regio die bewerkt wordt (index in het blad); `None` voor een nieuwe.
    region: Option<usize>,
    mode: Mode,
    /// Commentaar achter de formule (` // ...`); blijft bewaard.
    comment: String,
    changed: bool,
    focus: bool,
}

enum Drag {
    Move { start: Pos2, regions: Vec<usize> },
    Lasso { start: Pos2 },
}

#[derive(Default)]
pub struct CanvasState {
    edit: Option<Edit>,
    /// Het rode kruisje: hier komt wat je typt.
    cross: Option<(usize, usize)>,
    selected: Vec<usize>,
    drag: Option<Drag>,
    undo: Vec<String>,
    redo: Vec<String>,
    layout: Option<Layout>,
    pub show_functions: bool,
    /// De cursor of het kruisje in beeld schuiven (na typen).
    reveal: bool,
    /// De instellingen (thema, achtergrond, start, aanvullen, cijfers, zoom); zie settings.rs.
    pub set: crate::settings::Settings,
    /// Het venster Instellingen is open.
    pub show_settings: bool,
    /// Donkere modus.
    pub dark: bool,
    /// Afdrukvoorbeeld: het blad zoals in de PDF, zonder bewerken (Bestand → Afdrukvoorbeeld).
    pub preview: bool,
    /// De lijst met namen tijdens het typen (automatisch aanvullen).
    complete: Option<Completion>,
}

struct Completion {
    items: Vec<rb_edit::complete::Suggestion>,
    sel: usize,
    /// Met ↑/↓ gekozen: dan neemt Enter het voorstel over (anders is Enter gewoon Enter).
    moved: bool,
}

/// Achtergrond van de pagina's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Background {
    Leeg,
    /// Puntjes op het raster van de regio's, zoals in Mathcad.
    #[default]
    Raster,
    /// Ruitjespapier (5 mm).
    Ruitjes,
    /// Gelinieerd, zoals een schrift.
    Lijnen,
}

impl Background {
    pub const ALL: [Background; 4] = [Background::Leeg, Background::Raster, Background::Ruitjes, Background::Lijnen];

    pub fn label(self) -> &'static str {
        match self {
            Background::Leeg => "Leeg",
            Background::Raster => "Raster",
            Background::Ruitjes => "Ruitjes",
            Background::Lijnen => "Lijnen",
        }
    }

    pub(crate) fn key(self) -> &'static str {
        match self {
            Background::Leeg => "leeg",
            Background::Raster => "raster",
            Background::Ruitjes => "ruitjes",
            Background::Lijnen => "lijnen",
        }
    }
}

/// Iets dat het lint invoegt.
#[derive(Debug, Clone)]
pub enum Ins {
    Cmd(Cmd),
    /// Tekens typen, alsof ze op het toetsenbord werden getypt.
    Type(&'static str),
    Sheet(SheetAction),
    /// Een afbeelding kiezen en invoegen.
    Image,
    /// De formule die je bewerkt (of die geselecteerd is) omwerken naar deze grootheid.
    Solve(String),
    /// Het paneel met formules tonen of verbergen.
    Library,
}

impl CanvasState {
    /// Na het openen van een ander blad.
    pub fn reset(&mut self) {
        let (set, dark) = (self.set.clone(), self.dark);
        *self = CanvasState { set, dark, ..Default::default() };
    }

    /// Na een wijziging in de bron buiten het canvas.
    pub fn source_changed(&mut self) {
        self.edit = None;
        self.selected.clear();
        self.drag = None;
    }
}

/// Plaats op het scherm.
#[derive(Clone)]
struct Geo {
    px: f32,
    py: f32,
    zoom: f32,
    page_w: f32,
    page_h: f32,
    grid: Grid,
}

impl Geo {
    fn stride(&self) -> f32 {
        self.page_h * self.zoom + PAGE_GAP
    }
    fn page_origin(&self, k: usize) -> Pos2 {
        Pos2::new(self.px, self.py + k as f32 * self.stride())
    }
    fn local(&self, k: usize, x: f32, y: f32) -> Pos2 {
        self.page_origin(k) + Vec2::new(x, y) * self.zoom
    }
    /// Doorlopende coördinaten → scherm.
    fn screen(&self, x: f32, y: f32) -> Pos2 {
        let k = self.grid.page_of(y);
        self.local(k, x, y - self.grid.page_shift(k))
    }
    /// Scherm → doorlopende coördinaten (binnen de marges van de pagina).
    fn sheet(&self, p: Pos2) -> (f32, f32) {
        let k = ((p.y - self.py) / self.stride()).floor().max(0.0) as usize;
        let o = self.page_origin(k);
        let m = self.grid.margin;
        let x = ((p.x - o.x) / self.zoom).clamp(m, self.page_w - m);
        let y = ((p.y - o.y) / self.zoom).clamp(m, self.page_h - m - ROW_H);
        (x, y + self.grid.page_shift(k))
    }
    fn rect(&self, r: &RegionRect) -> Rect {
        Rect::from_min_size(self.local(r.page, r.rect[0], r.rect[1]), Vec2::new(r.rect[2].max(4.0), r.rect[3].max(4.0)) * self.zoom)
    }
}

/// Een toets van egui als toets van de Prime-tabel (letters als hoofdletter, zoals op de toets).
fn key_of(k: EKey) -> Option<Key> {
    use EKey::*;
    Some(match k {
        ArrowLeft => Key::Left,
        ArrowRight => Key::Right,
        ArrowUp => Key::Up,
        ArrowDown => Key::Down,
        Home => Key::Home,
        End => Key::End,
        Backspace => Key::Backspace,
        Delete => Key::Delete,
        Enter => Key::Enter,
        Space => Key::Space,
        Escape => Key::Escape,
        Tab => Key::Tab,
        F1 => Key::F(1),
        F2 => Key::F(2),
        F3 => Key::F(3),
        F4 => Key::F(4),
        F5 => Key::F(5),
        F6 => Key::F(6),
        F7 => Key::F(7),
        F8 => Key::F(8),
        F9 => Key::F(9),
        F10 => Key::F(10),
        F11 => Key::F(11),
        F12 => Key::F(12),
        Quote => Key::Char('\''),
        Minus => Key::Char('-'),
        Equals => Key::Char('='),
        Semicolon => Key::Char(';'),
        Backslash => Key::Char('\\'),
        OpenBracket => Key::Char('['),
        CloseBracket => Key::Char(']'),
        Slash => Key::Char('/'),
        Period => Key::Char('.'),
        Comma => Key::Char(','),
        Backtick => Key::Char('`'),
        _ => {
            let s = k.symbol_or_name();
            let mut cs = s.chars();
            match (cs.next(), cs.next()) {
                (Some(c), None) => Key::Char(c.to_ascii_uppercase()),
                _ => return None,
            }
        }
    })
}

/// `" tekst` → (`" `, `tekst`); ook voor `# kop` en `// commentaar`.
fn text_prefix(s: &str) -> (String, String) {
    let n = s.find(|c: char| !matches!(c, '"' | '#' | '/')).unwrap_or(s.len());
    let rest = &s[n..];
    (format!("{} ", &s[..n]), rest.strip_prefix(' ').unwrap_or(rest).to_string())
}

/// `x := 5 // toelichting` → (`x := 5`, ` // toelichting`).
fn split_comment(s: &str) -> (&str, String) {
    match s.find("//") {
        Some(k) if !s.contains('\n') => (s[..k].trim_end(), format!(" {}", &s[k..])),
        _ => (s, String::new()),
    }
}

impl App {
    pub(crate) fn ensure_layout(&mut self) {
        let preview = self.cv.preview;
        if self.cv.layout.as_ref().map(|l| (l.generation, l.preview)) != Some((self.generation, preview)) {
            // in het afdrukvoorbeeld dezelfde opmaak als de PDF (vaste grafieken, geen schermtekens)
            let s = if preview { Setup { static_plots: true, ..Setup::a4() } } else { setup() };
            let (pages, rects, grid) = layout_full(&self.sheet, &s);
            self.cv.layout = Some(Layout { generation: self.generation, pages, rects, grid, preview });
        }
    }

    /// Het aantal pagina's van het blad (zoals nu opgemaakt).
    pub(crate) fn page_count(&mut self) -> usize {
        self.ensure_layout();
        self.lay().pages.len()
    }

    fn lay(&self) -> &Layout {
        self.cv.layout.as_ref().expect("layout")
    }

    /// Linksboven (doorlopend) en grootte van regio `j`.
    fn region_box(&self, j: usize) -> Option<(f32, f32, f32, f32)> {
        let l = self.lay();
        l.rects.iter().find(|r| r.region == j).map(|r| (r.rect[0], r.rect[1] + l.grid.page_shift(r.page), r.rect[2], r.rect[3]))
    }

    /// Naar de eerste regio met een fout: het kruisje erop en in beeld (vanuit de statusbalk).
    pub(crate) fn goto_first_error(&mut self) {
        if let Some(j) = (0..self.sheet.regions().len()).find(|&j| matches!(self.sheet.output(j), Output::Error { .. })) {
            self.goto_region(j);
        }
    }

    /// Naar regio `j`: geselecteerd, het kruisje erop en in beeld (fouten, zoeken).
    pub(crate) fn goto_region(&mut self, j: usize) {
        self.commit(false);
        let items = self.items();
        if let Some(it) = items.get(j) {
            self.cv.cross = Some((it.col, it.row));
            self.cv.selected = vec![j];
            self.cv.reveal = true;
        }
    }

    /// De regio's met hun plek zoals ze nu op het blad staan.
    fn items(&mut self) -> Vec<Item> {
        self.ensure_layout();
        let l = self.lay();
        self.sheet
            .regions()
            .iter()
            .enumerate()
            .map(|(j, r)| {
                let (col, row) = match l.rects.iter().find(|q| q.region == j) {
                    Some(q) => l.grid.cell(q.rect[0], q.rect[1] + l.grid.page_shift(q.page)),
                    None => (r.pos.col, r.pos.row),
                };
                Item { col, row, text: r.source.clone() }
            })
            .collect()
    }

    /// Pas de regio's aan, schrijf de bron opnieuw en reken door.
    pub(crate) fn rewrite(&mut self, f: impl FnOnce(&mut Vec<Item>)) {
        let mut items = self.items();
        f(&mut items);
        let text = serialize(&mut items);
        if text == self.source {
            return;
        }
        self.cv.undo.push(std::mem::replace(&mut self.source, text));
        self.cv.redo.clear();
        self.dirty = true;
        self.reload();
    }

    pub(crate) fn reload(&mut self) {
        let start = std::time::Instant::now();
        let name = self.sheet.name().to_string();
        self.sheet = rb_sheet::Sheet::load_in(&self.source, &self.base_dir);
        self.sheet.set_name(&name);
        self.calculated = self.source.clone();
        self.generation += 1;
        self.cv.selected.clear();
        self.status = format!("Doorgerekend in {:.1} ms", start.elapsed().as_secs_f64() * 1000.0);
        self.ensure_layout();
    }

    fn undo(&mut self) {
        if let Some(e) = self.cv.edit.take() {
            if e.changed {
                // eerst de bewerking zelf terugdraaien
                self.cv.cross = Some((e.col, e.row));
                return;
            }
        }
        match self.cv.undo.pop() {
            Some(prev) => {
                self.cv.redo.push(std::mem::replace(&mut self.source, prev));
                self.dirty = true;
                self.reload();
            }
            None => self.status = "Niets meer ongedaan te maken".into(),
        }
    }

    fn redo(&mut self) {
        self.cv.edit = None;
        if let Some(next) = self.cv.redo.pop() {
            self.cv.undo.push(std::mem::replace(&mut self.source, next));
            self.dirty = true;
            self.reload();
        }
    }

    /// Rasterplek onder alle regio's.
    fn below_all(&mut self) -> (usize, usize) {
        self.ensure_layout();
        let l = self.lay();
        let bottom = l.rects.iter().map(|r| r.rect[1] + r.rect[3] + l.grid.page_shift(r.page)).fold(0.0, f32::max);
        if bottom <= 0.0 {
            (0, 1)
        } else {
            (0, row_below(&l.grid, bottom))
        }
    }

    fn cross_or_default(&mut self) -> (usize, usize) {
        match self.cv.cross {
            Some(c) => c,
            None => self.below_all(),
        }
    }

    fn start_new(&mut self, col: usize, row: usize, mode: Mode) {
        self.ensure_layout();
        let at = self.lay().grid.point(col, row);
        self.cv.edit = Some(Edit { col, row, at, region: None, mode, comment: String::new(), changed: false, focus: true });
        self.cv.cross = None;
        self.cv.selected.clear();
    }

    /// Begin met het bewerken van regio `j`; `click` (doorlopend) zet de cursor.
    fn start_region(&mut self, j: usize, click: Option<(f32, f32)>) {
        // een afbeelding bewerk je niet: selecteren (verslepen, wissen, kopiëren)
        if matches!(self.sheet.regions().get(j).map(|r| &r.kind), Some(Kind::Image(_))) {
            self.cv.selected = vec![j];
            self.cv.cross = None;
            return;
        }
        let items = self.items();
        let Some(item) = items.get(j) else { return };
        let (col, row) = (item.col, item.row);
        // een gebied: klik op ▸/▾ klapt in of uit
        if let Some(Kind::Area { title, collapsed }) = self.sheet.regions().get(j).map(|r| &r.kind) {
            let on_mark = match (click, self.region_box(j)) {
                (Some((x, _)), Some((bx, ..))) => x < bx + 22.0,
                _ => false,
            };
            if on_mark {
                let text = if *collapsed { format!("area {title}") } else { format!("area ingeklapt {title}") };
                let text = text.trim_end().to_string();
                self.rewrite(move |items| items[j].text = text);
                self.cv.cross = None;
                return;
            }
        }
        let at = self.region_box(j).map_or(self.lay().grid.point(col, row), |b| (b.0, b.1));
        let r = &self.sheet.regions()[j];
        let raw = || Mode::Text { prefix: String::new(), buf: r.source.clone(), mono: true };
        let mut comment = String::new();
        let mode = match &r.kind {
            Kind::Text(_) | Kind::Heading(_) | Kind::Comment(_) => {
                let (prefix, buf) = text_prefix(&r.source);
                Mode::Text { prefix, buf, mono: false }
            }
            Kind::Table(t) => Mode::Table(TableEdit::from_spec(t)),
            Kind::Math(_) | Kind::Given | Kind::Constraint(_) | Kind::Formula(_) => {
                let (code, c) = split_comment(&r.source);
                match rb_edit::from_text(code) {
                    Some(b) => {
                        comment = c;
                        Mode::Math(Editor::with_root(b))
                    }
                    None => raw(),
                }
            }
            _ => raw(),
        };
        self.cv.edit = Some(Edit { col, row, at, region: Some(j), mode, comment, changed: false, focus: true });
        self.cv.cross = None;
        self.cv.selected.clear();
        if let Some(p) = click {
            self.click_in_edit(p);
        }
    }

    /// Zet de cursor in de formule die bewerkt wordt op het punt `p` (doorlopend).
    fn click_in_edit(&mut self, p: (f32, f32)) {
        let Some(e) = &self.cv.edit else { return };
        let Mode::Math(ed) = &e.mode else { return };
        let el = self.edit_layout(e, ed);
        let hit = el.hit(p.0 - e.at.0, p.1 - (e.at.1 + el.mbox.ascent));
        if let (Some(c), Some(Edit { mode: Mode::Math(ed), .. })) = (hit, &mut self.cv.edit) {
            ed.cursor = c;
            ed.selection = None;
        }
    }

    fn edit_layout(&self, e: &Edit, ed: &Editor) -> rb_edit::EditLayout {
        let s = setup();
        let opts = LayoutOptions { size: s.formula_size };
        // het oude resultaat tonen zolang de formule niet veranderd is
        let result = e
            .region
            .filter(|_| !e.changed)
            .and_then(|j| self.sheet.result(j))
            .map(|(v, d)| rb_layout::value_box(&FontMetrics, v, d, opts));
        rb_edit::layout(ed, &FontMetrics, s.formula_size, result)
    }

    /// Rond de bewerking af: de regio in de bron zetten en doorrekenen. `keep`: blijven bewerken
    /// (na `=`, met de cursor in de eenheidsplaats).
    pub(crate) fn commit(&mut self, keep: bool) {
        let Some(mut e) = self.cv.edit.take() else { return };
        let text = match &e.mode {
            Mode::Math(ed) => {
                let t = rb_edit::region_text(&ed.root);
                if t.trim().is_empty() {
                    String::new()
                } else {
                    format!("{t}{}", e.comment)
                }
            }
            Mode::Text { prefix, buf, .. } => {
                let b = buf.trim_end();
                if b.trim().is_empty() {
                    String::new()
                } else {
                    format!("{prefix}{}", b.trim_start())
                }
            }
            Mode::Table(t) => t.source(),
        };
        if e.changed {
            let (col, row, region, t) = (e.col, e.row, e.region, text.clone());
            self.rewrite(move |items| {
                if let Some(j) = region.filter(|j| *j < items.len()) {
                    items.remove(j);
                }
                if !t.is_empty() {
                    items.push(Item { col, row, text: t });
                }
            });
        }
        let pos = Pos { row: e.row, col: e.col };
        let found = self.sheet.regions().iter().position(|r| r.pos == pos && r.source == text.trim());
        if keep && found.is_some() {
            e.region = found;
            e.changed = false;
            if let Some((x, y, _, _)) = found.and_then(|j| self.region_box(j)) {
                e.at = (x, y);
            }
            self.cv.edit = Some(e);
            return;
        }
        // het kruisje onder de regio, zoals na Enter in Prime
        let j = found.or(e.region.filter(|_| !e.changed));
        let row = match j.and_then(|j| self.region_box(j)) {
            Some((_, y, _, h)) => row_below(&self.lay().grid, y + h),
            None => e.row,
        };
        self.cv.cross = Some(self.free_cell(e.col, row));
    }

    /// De eerste rasterplek vanaf (col, ow) naar beneden die niet in een regio valt.
    fn free_cell(&self, col: usize, mut row: usize) -> (usize, usize) {
        let l = self.lay();
        for _ in 0..1000 {
            let (x, y) = l.grid.point(col, row);
            let hit = l.rects.iter().find(|r| {
                let top = r.rect[1] + l.grid.page_shift(r.page);
                x >= r.rect[0] - 2.0 && x <= r.rect[0] + r.rect[2] + 2.0 && y + ROW_H * 0.5 >= top && y <= top + r.rect[3]
            });
            match hit {
                Some(r) => row = row_below(&l.grid, r.rect[1] + l.grid.page_shift(r.page) + r.rect[3]).max(row + 1),
                None => break,
            }
        }
        (col, row)
    }

    fn delete_selected(&mut self) {
        let mut sel = std::mem::take(&mut self.cv.selected);
        sel.sort_unstable();
        sel.dedup();
        self.rewrite(|items| {
            for j in sel.into_iter().rev() {
                if j < items.len() {
                    items.remove(j);
                }
            }
        });
    }

    fn copy(&mut self, ctx: &egui::Context, cut: bool) {
        if let Some(Edit { mode: Mode::Math(ed), .. }) = &self.cv.edit {
            ctx.copy_text(rb_edit::region_text(&ed.root));
            return;
        }
        if self.cv.selected.is_empty() {
            return;
        }
        let items = self.items();
        let sel: Vec<&Item> = self.cv.selected.iter().filter_map(|j| items.get(*j)).collect();
        let c0 = sel.iter().map(|i| i.col).min().unwrap_or(0);
        let r0 = sel.iter().map(|i| i.row).min().unwrap_or(1);
        let text: String = sel.iter().map(|i| format!("@{},{} {}\n", i.col - c0, i.row - r0 + 1, i.text)).collect();
        ctx.copy_text(text);
        if cut {
            self.delete_selected();
        }
    }

    fn paste(&mut self, s: &str) {
        if let Some(Edit { mode: Mode::Math(ed), changed, .. }) = &mut self.cv.edit {
            if ed.root.is_empty() {
                if let Some(b) = rb_edit::from_text(s.trim()) {
                    *ed = Editor::with_root(b);
                    *changed = true;
                    return;
                }
            }
            for c in s.chars().filter(|c| !c.is_whitespace()) {
                ed.apply(Cmd::Char(c));
            }
            *changed = true;
            return;
        }
        if self.cv.edit.is_some() {
            return;
        }
        let regs = parse_sheet(s);
        if regs.is_empty() {
            return;
        }
        let (c0, r0) = self.cross_or_default();
        let min_c = regs.iter().map(|r| r.pos.col).min().unwrap_or(0);
        let min_r = regs.iter().map(|r| r.pos.row).min().unwrap_or(1);
        self.rewrite(|items| {
            for r in regs {
                items.push(Item { col: c0 + r.pos.col - min_c, row: r0 + r.pos.row - min_r, text: r.source });
            }
        });
    }

    /// Een getypt teken (zonder Ctrl).
    fn typed(&mut self, c: char) {
        match &self.cv.edit {
            Some(Edit { mode: Mode::Math(_), .. }) => self.edit_cmd(Cmd::Char(c)),
            Some(_) => {}
            None => {
                let (col, row) = self.cross_or_default();
                if c == '"' {
                    self.start_new(col, row, Mode::Text { prefix: "\" ".into(), buf: String::new(), mono: false });
                } else {
                    self.start_new(col, row, Mode::Math(Editor::new()));
                    self.edit_cmd(Cmd::Char(c));
                }
            }
        }
    }

    fn edit_cmd(&mut self, cmd: Cmd) {
        // na een getypte letter (of Backspace) de lijst met namen bijwerken, anders sluiten
        let typing = matches!(cmd, Cmd::Char(c) if c.is_alphanumeric() || matches!(c, '_' | '.')) || cmd == Cmd::Backspace;
        self.edit_cmd_inner(cmd);
        self.cv.complete = None;
        if typing {
            self.refresh_completion(false);
        }
    }

    /// Zoek namen die passen bij wat er voor de cursor staat. `force`: ook bij één letter (Ctrl+Spatie).
    fn refresh_completion(&mut self, force: bool) {
        self.cv.complete = None;
        let Some(Edit { mode: Mode::Math(ed), .. }) = &self.cv.edit else { return };
        if ed.naming() {
            return;
        }
        let Some(p) = ed.partial_name() else { return };
        if !force && (p.chars().count() < 2 || !self.cv.set.autocomplete) {
            return;
        }
        static UNITS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        let units = UNITS.get_or_init(rb_edit::complete::unit_names);
        let names: Vec<(String, bool)> = self
            .sheet
            .regions()
            .iter()
            .filter_map(|r| match &r.kind {
                Kind::Math(rb_core::Stmt::Define { name, params, .. }) => Some((name.clone(), params.is_some())),
                _ => None,
            })
            .collect();
        let items = rb_edit::complete::suggest(&p, &names, units, 8);
        if !items.is_empty() {
            self.cv.complete = Some(Completion { items, sel: 0, moved: false });
        }
    }

    /// Neem voorstel `i` over.
    fn accept_completion(&mut self, i: usize) {
        let Some(c) = self.cv.complete.take() else { return };
        if let Some(Edit { mode: Mode::Math(ed), changed, .. }) = &mut self.cv.edit {
            ed.complete_with(&c.items[i]);
            *changed = true;
        }
    }

    /// Toetsen voor de lijst met namen: ↑/↓ kiezen, Tab (of Enter na kiezen) overnemen, Esc sluiten.
    fn completion_key(&mut self, k: KeyPress) -> bool {
        if k.ctrl && !k.alt && k.key == Key::Space {
            self.refresh_completion(true);
            return true;
        }
        let Some(c) = &mut self.cv.complete else { return false };
        if k.ctrl || k.alt {
            return false;
        }
        let n = c.items.len();
        match k.key {
            Key::Down => {
                c.sel = (c.sel + 1) % n;
                c.moved = true;
            }
            Key::Up => {
                c.sel = (c.sel + n - 1) % n;
                c.moved = true;
            }
            Key::Tab => {
                let i = c.sel;
                self.accept_completion(i);
            }
            Key::Enter if c.moved => {
                let i = c.sel;
                self.accept_completion(i);
            }
            Key::Escape => self.cv.complete = None,
            _ => return false,
        }
        true
    }

    fn edit_cmd_inner(&mut self, cmd: Cmd) {
        // in beeld schuiven alleen als de cursor of het kruisje verplaatst of er getypt wordt;
        // niet bij Delete, Backspace of Esc buiten een formule (dan sprong de pagina naar een oud kruisje)
        let moves = self.cv.edit.is_some()
            || matches!(cmd, Cmd::Left | Cmd::Right | Cmd::Up | Cmd::Down | Cmd::Enter | Cmd::Char(_) | Cmd::Template(_));
        self.cv.reveal = moves;
        if let Some(Edit { mode: Mode::Math(ed), changed, .. }) = &mut self.cv.edit {
            let before = ed.root.clone();
            // na = en → blijven bewerken: een eenheid of sleutelwoord kan nog volgen
            let is_eval = matches!(cmd, Cmd::Char('=' | '→'));
            let effect = ed.apply(cmd);
            if ed.root != before {
                *changed = true;
            }
            match effect {
                Effect::None => {}
                Effect::Commit => self.commit(is_eval),
                Effect::ToText => {
                    if let Some(e) = &mut self.cv.edit {
                        e.mode = Mode::Text { prefix: "\" ".into(), buf: String::new(), mono: false };
                        e.focus = true;
                    }
                }
                Effect::Unsupported(m) => self.status = m.to_string(),
            }
            return;
        }
        let cross = self.cross_or_default();
        match cmd {
            Cmd::Left => self.cv.cross = Some((cross.0.saturating_sub(1), cross.1)),
            Cmd::Right => self.cv.cross = Some((cross.0 + 1, cross.1)),
            Cmd::Up => self.cv.cross = Some((cross.0, cross.1.saturating_sub(1).max(1))),
            Cmd::Down | Cmd::Enter => self.cv.cross = Some((cross.0, cross.1 + 1)),
            Cmd::Backspace | Cmd::Delete if !self.cv.selected.is_empty() => self.delete_selected(),
            Cmd::Escape => self.cv.selected.clear(),
            Cmd::Template(_) | Cmd::Char(_) => {
                self.start_new(cross.0, cross.1, Mode::Math(Editor::new()));
                self.edit_cmd(cmd);
            }
            _ => {}
        }
    }

    fn pressed(&mut self, ctx: &egui::Context, k: KeyPress) -> bool {
        if self.completion_key(k) {
            return true;
        }
        let in_math = matches!(self.cv.edit, Some(Edit { mode: Mode::Math(_), .. }));
        match mathcad_key(k, in_math) {
            None => false,
            Some(Action::Unsupported(m)) => {
                self.status = m.to_string();
                in_math
            }
            Some(Action::Sheet(a)) => {
                self.sheet_action(ctx, a);
                true
            }
            Some(Action::Edit(cmd)) => {
                self.edit_cmd(cmd);
                true
            }
        }
    }

    fn sheet_action(&mut self, ctx: &egui::Context, a: SheetAction) {
        use SheetAction::*;
        match a {
            TextBox | TextBlock => {
                if let Some(Edit { mode: Mode::Math(ed), .. }) = &self.cv.edit {
                    if ed.root.is_empty() {
                        self.edit_cmd(Cmd::Char('"'));
                        return;
                    }
                }
                self.commit(false);
                let (c, r) = self.cross_or_default();
                self.start_new(c, r, Mode::Text { prefix: "\" ".into(), buf: String::new(), mono: false });
            }
            Math => {
                self.commit(false);
                let (c, r) = self.cross_or_default();
                self.start_new(c, r, Mode::Math(Editor::new()));
            }
            XyPlot => {
                self.commit(false);
                let (c, r) = self.cross_or_default();
                self.start_new(c, r, Mode::Text { prefix: String::new(), buf: "plot x = -10 ; 10 : x".into(), mono: true });
                if let Some(e) = &mut self.cv.edit {
                    e.changed = true;
                }
                self.status = "Grafiek: plot x = begin ; eind : f(x), g(x)   (Enter om te tekenen)".into();
            }
            SolveBlock => {
                self.commit(false);
                let (c, r) = self.cross_or_default();
                self.rewrite(|items| items.push(Item { col: c, row: r, text: "Given".into() }));
                self.cv.cross = Some((c, r + 2));
                self.status = "Solve block: typ de vergelijkingen eronder (Ctrl+= voor ==), sluit af met Find(x) =".into();
            }
            PageBreak => {
                self.commit(false);
                let (c, r) = self.cross_or_default();
                self.rewrite(|items| items.push(Item { col: 0, row: r, text: "pagebreak".into() }));
                self.cv.cross = Some((c, r + 1));
                self.status = "Pagina-einde: wat eronder staat, begint op een nieuwe pagina".into();
            }
            Area => {
                self.commit(false);
                // om de geselecteerde regio's heen, anders een leeg gebied op het kruisje
                let boxes: Vec<(f32, f32)> = self.cv.selected.iter().filter_map(|&j| self.region_box(j)).map(|(_, y, _, h)| (y, y + h)).collect();
                let items = self.items();
                let (top, bottom) = match boxes.is_empty() {
                    true => {
                        let r = self.cross_or_default().1;
                        (r, r + 3)
                    }
                    false => {
                        let top = self.cv.selected.iter().filter_map(|&j| items.get(j)).map(|it| it.row).min().unwrap_or(1);
                        let bottom = row_below(&self.lay().grid, boxes.iter().map(|b| b.1).fold(0.0, f32::max));
                        (top, bottom)
                    }
                };
                self.rewrite(|items| {
                    // twee rijen erbij voor de balk, één voor de eindlijn
                    for it in items.iter_mut() {
                        if it.row >= bottom {
                            it.row += 3;
                        } else if it.row >= top {
                            it.row += 2;
                        }
                    }
                    items.push(Item { col: 0, row: top, text: "area Gebied".into() });
                    items.push(Item { col: 0, row: bottom + 2, text: "area end".into() });
                });
                self.cv.selected.clear();
                self.cv.cross = Some((0, top + 2));
                self.status = "Gebied: klik op ▾ om in of uit te klappen; klik op de titel om hem te wijzigen".into();
            }
            Header | Footer => {
                self.commit(false);
                let (c, r) = self.cross_or_default();
                let text = if a == Header { "header {bestand}   {datum}" } else { "footer pagina {pagina} van {paginas}" };
                self.rewrite(|items| items.push(Item { col: 0, row: r, text: text.into() }));
                self.cv.cross = Some((c, r + 2));
                self.status = "Kop- en voettekst staan op elke pagina; {pagina}, {paginas}, {bestand} en {datum} worden ingevuld".into();
            }
            Table => {
                self.commit(false);
                let (c, r) = self.cross_or_default();
                self.start_new(c, r, Mode::Table(TableEdit::new()));
                if let Some(e) = &mut self.cv.edit {
                    e.changed = true;
                }
                self.status = "Tabel: bovenaan de namen, daaronder de eenheden; Tab naar de volgende cel, Enter een rij lager, Esc klaar".into();
            }
            Recalc | RecalcAll => {
                self.commit(false);
                self.recalc(true);
            }
            SeparateVertical | SeparateHorizontal => {
                self.commit(false);
                self.rewrite(|_| {});
                self.status = "Regio's gescheiden".into();
            }
            Help => self.show_help = true,
            Functions => self.cv.show_functions = true,
            New => self.run(ctx, MenuAction::Nieuw),
            Open => self.run(ctx, MenuAction::Openen),
            Save => {
                self.commit(false);
                self.run(ctx, MenuAction::Opslaan);
            }
            Print => {
                self.commit(false);
                self.run(ctx, MenuAction::ExportPdf);
            }
            Close => self.guarded(ctx, Pending::TabSluiten),
            Undo => self.undo(),
            Redo => self.redo(),
            Cut => self.copy(ctx, true),
            Copy => self.copy(ctx, false),
            // plakken komt binnen als Event::Paste (met de inhoud van het klembord)
            Paste => {}
        }
    }

    /// Toetsen voor het werkblad. Wordt vóór de sneltoetsen van het menu aangeroepen; wat hier
    /// gebruikt wordt, verdwijnt uit de invoer.
    pub(crate) fn canvas_keys(&mut self, ctx: &egui::Context) {
        if self.cv.preview {
            // alleen Esc: terug naar bewerken
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, EKey::Escape)) {
                self.cv.preview = false;
            }
            return;
        }
        if let Some(Edit { mode: Mode::Table(t), .. }) = &mut self.cv.edit {
            // in een tabel: Enter een rij lager (onderaan komt er een rij bij), Esc rondt af
            let focused = ctx.memory(|m| m.focused());
            let at = (0..t.rows.len() + 2).find_map(|r| (0..t.names.len()).find(|&c| focused == Some(TableEdit::cell_id(r, c))).map(|c| (r, c)));
            let (mut enter, mut done) = (false, false);
            ctx.input_mut(|i| {
                i.events.retain(|ev| match ev {
                    Event::Key { key: EKey::Enter, pressed: true, .. } => {
                        enter = true;
                        false
                    }
                    Event::Key { key: EKey::Escape, pressed: true, .. } => {
                        done = true;
                        false
                    }
                    _ => true,
                })
            });
            if enter {
                let (r, c) = at.unwrap_or((1, 0));
                if r + 1 >= t.rows.len() + 2 {
                    t.rows.push(vec![String::new(); t.names.len()]);
                }
                t.focus = Some((r + 1, c));
            }
            if done {
                self.commit(false);
            }
            return;
        }
        if let Some(Edit { mode: Mode::Text { buf, .. }, .. }) = &self.cv.edit {
            // in een tekstregio: Enter (zonder Shift) en Esc ronden af, de rest is voor het tekstvak
            let multi = buf.contains('\n');
            let mut done = false;
            ctx.input_mut(|i| {
                i.events.retain(|ev| match ev {
                    Event::Key { key: EKey::Enter, pressed: true, modifiers, .. } if !modifiers.shift && !multi => {
                        done = true;
                        false
                    }
                    Event::Key { key: EKey::Escape, pressed: true, .. } => {
                        done = true;
                        false
                    }
                    _ => true,
                })
            });
            if done {
                self.commit(false);
            }
            return;
        }
        let focused = ctx.memory(|m| m.focused());
        if focused.is_some() && focused != Some(canvas_id()) {
            return;
        }
        let shift = ctx.input(|i| i.modifiers.shift);
        let events = ctx.input(|i| i.events.clone());
        let mut keep = Vec::new();
        for ev in events {
            let used = match &ev {
                Event::Text(t) => {
                    for c in t.chars() {
                        // Shift+Spatie is een kolom erbij (komt als toets binnen)
                        if c == ' ' && shift {
                            continue;
                        }
                        if c == ' ' {
                            self.edit_cmd(Cmd::Grow);
                        } else {
                            self.typed(c);
                        }
                    }
                    true
                }
                Event::Key { key, physical_key, pressed: true, modifiers, .. } => {
                    let ctrl = modifiers.ctrl || modifiers.command;
                    // AltGr (Ctrl+Alt) maakt tekens; die komen als tekst
                    if ctrl && modifiers.alt {
                        false
                    } else {
                        match physical_key.and_then(key_of).or_else(|| key_of(*key)) {
                            Some(k) => {
                                let charlike = matches!(k, Key::Char(_)) || (k == Key::Space && !modifiers.shift);
                                if charlike && !ctrl {
                                    false
                                } else {
                                    self.pressed(ctx, KeyPress { key: k, ctrl, shift: modifiers.shift, alt: modifiers.alt })
                                }
                            }
                            None => false,
                        }
                    }
                }
                Event::Copy => {
                    self.copy(ctx, false);
                    true
                }
                Event::Cut => {
                    self.copy(ctx, true);
                    true
                }
                Event::Paste(s) => {
                    self.paste(s);
                    true
                }
                _ => false,
            };
            if !used {
                keep.push(ev);
            }
        }
        ctx.input_mut(|i| i.events = keep);
    }

    fn region_at(&self, geo: &Geo, p: Pos2) -> Option<usize> {
        self.lay().rects.iter().find(|r| geo.rect(r).expand(2.0).contains(p)).map(|r| r.region)
    }

    /// Teken het werkblad en verwerk de muis.
    pub(crate) fn canvas(&mut self, ui: &mut egui::Ui) {
        self.ensure_layout();
        let zd = ui.input(|i| i.zoom_delta());
        if zd != 1.0 {
            self.zoom = (self.zoom * zd).clamp(0.5, 4.0);
        }
        let s = setup();
        let (page_w, page_h) = (s.width, s.height.unwrap_or(842.0));
        let zoom = self.zoom;
        let npages = self.lay().pages.len() + 1;
        let avail = ui.available_width();
        let size = Vec2::new(
            avail.max(page_w * zoom + 2.0 * PAGE_GAP),
            npages as f32 * (page_h * zoom + PAGE_GAP) + PAGE_GAP,
        );
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let resp = ui.interact(rect, canvas_id(), Sense::click_and_drag());
        let geo = Geo {
            px: rect.min.x + ((size.x - page_w * zoom) / 2.0).max(PAGE_GAP),
            py: rect.min.y + PAGE_GAP,
            zoom,
            page_w,
            page_h,
            grid: self.lay().grid.clone(),
        };
        self.mouse(ui, &resp, &geo);
        if resp.has_focus() {
            let filter = EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true };
            ui.ctx().memory_mut(|m| m.set_focus_lock_filter(canvas_id(), filter));
        }
        if self.cv.edit.is_none() && self.cv.cross.is_none() && self.cv.selected.is_empty() && self.cv.drag.is_none() {
            self.cv.cross = Some(self.below_all());
        }
        self.ensure_layout();
        self.paint(ui, &geo, npages, &resp);
        // een leeg blad: aanwijzing en recente bestanden op de eerste pagina
        if self.sheet.regions().is_empty() && self.cv.edit.is_none() && !self.cv.preview {
            let center = geo.page_origin(0) + Vec2::new(page_w / 2.0, 240.0) * zoom;
            self.empty_state(ui, center);
        }
    }

    /// Hoeveel kolommen en rijen regio's verschuiven als ze van `start` naar `now` gesleept worden:
    /// op het raster (ook over ingeklapte gebieden heen), en zoveel rijen lager als nodig om niet over
    /// een andere regio te vallen.
    fn drag_offset(&self, geo: &Geo, start: Pos2, now: Pos2, regions: &[usize]) -> (i64, i64) {
        let (sx, sy) = geo.sheet(start);
        let (ex, ey) = geo.sheet(now);
        let (c0, r0) = geo.grid.cell(sx, sy);
        let (c1, r1) = geo.grid.cell(ex, ey);
        let dc = c1 as i64 - c0 as i64;
        let mut dr = r1 as i64 - r0 as i64;
        if dc == 0 && dr == 0 {
            return (0, 0);
        }
        let boxes = |moving: bool| -> Vec<(f32, f32, f32, f32)> {
            (0..self.sheet.regions().len()).filter(|j| regions.contains(j) == moving).filter_map(|j| self.region_box(j)).collect()
        };
        let (moving, others) = (boxes(true), boxes(false));
        let col_w = geo.grid.col_w;
        for _ in 0..400 {
            let hit = moving.iter().any(|&(x, y, w, h)| {
                let (x, y) = (x + dc as f32 * col_w, y + dr as f32 * ROW_H);
                // een halve punt speling: tegen elkaar aan mag
                others.iter().any(|&(ox, oy, ow, oh)| x + 0.5 < ox + ow && ox + 0.5 < x + w && y + 0.5 < oy + oh && oy + 0.5 < y + h)
            });
            if !hit {
                break;
            }
            dr += 1;
        }
        (dc, dr)
    }

    fn mouse(&mut self, ui: &egui::Ui, resp: &egui::Response, geo: &Geo) {
        // het afdrukvoorbeeld is alleen om te bekijken
        if self.cv.preview {
            return;
        }
        let press = ui.input(|i| i.pointer.press_origin());
        let latest = ui.input(|i| i.pointer.latest_pos());
        let editing = self.cv.edit.as_ref().and_then(|e| e.region);
        if resp.drag_started() {
            if let Some(start) = press {
                match self.region_at(geo, start) {
                    Some(j) if Some(j) == editing => {}
                    Some(j) => {
                        self.commit(false);
                        self.ensure_layout();
                        if let Some(j) = self.region_at(geo, start) {
                            if !self.cv.selected.contains(&j) {
                                self.cv.selected = vec![j];
                            }
                            self.cv.cross = None;
                            self.cv.drag = Some(Drag::Move { start, regions: self.cv.selected.clone() });
                        }
                        let _ = j;
                    }
                    None => {
                        self.commit(false);
                        self.cv.selected.clear();
                        self.cv.cross = None;
                        self.cv.drag = Some(Drag::Lasso { start });
                    }
                }
            }
        }
        if resp.drag_stopped() {
            let end = latest.unwrap_or(Pos2::ZERO);
            match self.cv.drag.take() {
                Some(Drag::Move { start, regions }) => {
                    let (dc, dr) = self.drag_offset(geo, start, end, &regions);
                    if dc != 0 || dr != 0 {
                        // waar de regio's terechtkomen, om ze daarna geselecteerd te houden
                        let items = self.items();
                        let target = move |it: &Item| ((it.col as i64 + dc).max(0) as usize, (it.row as i64 + dr).max(1) as usize);
                        let targets: Vec<(usize, usize)> = regions.iter().filter_map(|&j| items.get(j)).map(target).collect();
                        self.rewrite(|items| {
                            for j in regions {
                                if let Some(it) = items.get_mut(j) {
                                    (it.col, it.row) = target(it);
                                }
                            }
                        });
                        self.cv.selected = (0..self.sheet.regions().len())
                            .filter(|&k| {
                                let p = self.sheet.regions()[k].pos;
                                targets.contains(&(p.col, p.row))
                            })
                            .collect();
                    }
                }
                Some(Drag::Lasso { start }) => {
                    let r = Rect::from_two_pos(start, end);
                    self.cv.selected = self.lay().rects.iter().filter(|q| geo.rect(q).intersects(r)).map(|q| q.region).collect();
                    if self.cv.selected.is_empty() {
                        self.cv.cross = Some({
                            let (x, y) = geo.sheet(end);
                            geo.grid.cell(x, y)
                        });
                    }
                }
                None => {}
            }
        }
        if resp.clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                let at = geo.sheet(p);
                let raw = (
                    (p.x - geo.px) / geo.zoom,
                    // doorlopend, zonder de marges af te kappen (voor klikken in een regio)
                    {
                        let k = ((p.y - geo.py) / geo.stride()).floor().max(0.0) as usize;
                        (p.y - geo.page_origin(k).y) / geo.zoom + geo.grid.page_shift(k)
                    },
                );
                match self.region_at(geo, p) {
                    Some(j) if Some(j) == editing => self.click_in_edit(raw),
                    Some(_) => {
                        self.commit(false);
                        self.ensure_layout();
                        if let Some(j) = self.region_at(geo, p) {
                            self.start_region(j, Some(raw));
                        }
                    }
                    None => {
                        self.commit(false);
                        self.cv.selected.clear();
                        self.cv.cross = Some(geo.grid.cell(at.0, at.1));
                    }
                }
            }
            resp.request_focus();
        }
    }

    fn paint(&mut self, ui: &mut egui::Ui, geo: &Geo, npages: usize, resp: &egui::Response) {
        let dark = self.cv.dark;
        let tone = |c: Color32| if dark { paint::darken(c) } else { c };
        let painter = ui.painter_at(resp.rect);
        let l = self.lay();
        for k in 0..npages {
            let r = Rect::from_min_size(geo.page_origin(k), Vec2::new(geo.page_w, geo.page_h) * geo.zoom);
            // zachte schaduw: een paar doorzichtige lagen, steeds iets groter
            let shade = if dark { 70 } else { 14 };
            for (k2, grow) in [(1u8, 6.0f32), (2, 3.5), (3, 1.5)] {
                painter.rect_filled(r.translate(Vec2::new(0.0, 2.0)).expand(grow), 4.0, Color32::from_black_alpha(shade * k2 / 2));
            }
            painter.rect_filled(r, 2.0, tone(Color32::WHITE));
            if r.intersects(ui.clip_rect()) && !self.cv.preview {
                paint_background(&painter, self.cv.set.background, geo, k, self.cv.dark);
            }
        }
        // selectie en de regio onder de muis
        for q in &l.rects {
            if self.cv.selected.contains(&q.region) {
                painter.rect_filled(geo.rect(q).expand(3.0), 2.0, tone(SELECT));
            }
        }
        if self.cv.drag.is_none() {
            if let Some(h) = resp.hover_pos().filter(|_| !self.cv.preview).and_then(|p| self.region_at(geo, p)) {
                if let Some(q) = l.rects.iter().find(|q| q.region == h) {
                    painter.rect_stroke(geo.rect(q).expand(3.0), 2.0, Stroke::new(1.0, tone(Color32::from_rgb(190, 196, 210))), StrokeKind::Middle);
                }
            }
        }
        for (k, page) in l.pages.iter().enumerate() {
            paint::paint_ops(&painter, &page.ops, geo.page_origin(k), geo.zoom, self.cv.dark);
        }
        let editing = self.cv.edit.as_ref().and_then(|e| e.region);
        let plots: Vec<(usize, Rect)> = l
            .rects
            .iter()
            .filter(|q| Some(q.region) != editing)
            .filter_map(|q| q.plot.map(|p| (q.region, Rect::from_min_size(geo.local(q.page, p[0], p[1]), Vec2::new(p[2], p[3]) * geo.zoom))))
            .collect();
        // verslepen: waar de regio's terechtkomen
        if let (Some(Drag::Move { start, regions }), Some(now)) = (&self.cv.drag, resp.interact_pointer_pos()) {
            // op het raster, zoals ze na het loslaten komen te staan
            let (dc, dr) = self.drag_offset(geo, *start, now, regions);
            let d = Vec2::new(dc as f32 * geo.grid.col_w, dr as f32 * ROW_H) * geo.zoom;
            for q in l.rects.iter().filter(|q| regions.contains(&q.region)) {
                painter.rect_stroke(geo.rect(q).translate(d).expand(3.0), 2.0, Stroke::new(1.0, tone(BLUE)), StrokeKind::Middle);
            }
        }
        if let (Some(Drag::Lasso { start }), Some(now)) = (&self.cv.drag, resp.interact_pointer_pos()) {
            let r = Rect::from_two_pos(*start, now);
            painter.rect_filled(r, 0.0, tone(Color32::from_rgba_unmultiplied(30, 90, 200, 25)));
            painter.rect_stroke(r, 0.0, Stroke::new(1.0, tone(BLUE)), StrokeKind::Middle);
        }
        // het kruisje
        if let (Some((c, r)), None, false) = (self.cv.cross, &self.cv.edit, self.cv.preview) {
            let (x, y) = geo.grid.point(c, r);
            let p = geo.screen(x, y + 0.5 * ROW_H);
            let red = Stroke::new(1.5, tone(Color32::from_rgb(220, 30, 30)));
            let a = 6.0;
            painter.line_segment([p - Vec2::new(a, 0.0), p + Vec2::new(a, 0.0)], red);
            painter.line_segment([p - Vec2::new(0.0, a), p + Vec2::new(0.0, a)], red);
            if std::mem::take(&mut self.cv.reveal) {
                ui.scroll_to_rect(Rect::from_center_size(p, Vec2::splat(60.0)), None);
            }
        }
        for (region, r) in plots {
            if let Output::Plot(data) = self.sheet.output(region) {
                let data = data.clone();
                paint::plot(ui, region, r, &data);
            }
        }
        self.paint_edit(ui, geo, &painter);
    }

    fn paint_edit(&mut self, ui: &mut egui::Ui, geo: &Geo, painter: &egui::Painter) {
        let dark = self.cv.dark;
        let tone = |c: Color32| if dark { paint::darken(c) } else { c };
        let old = self.cv.edit.as_ref().and_then(|e| e.region).and_then(|j| self.region_box(j));
        let Some(e) = &self.cv.edit else { return };
        let origin = geo.screen(e.at.0, e.at.1);
        // waar de lijst met namen (automatisch aanvullen) komt: onder de cursor
        let mut popup = None;
        // de oude weergave van de regio wegpoetsen
        if let Some((x, y, w, h)) = old {
            let r = Rect::from_min_size(geo.screen(x, y), Vec2::new(w, h) * geo.zoom);
            painter.rect_filled(r.expand(2.0), 0.0, tone(Color32::WHITE));
        }
        match &e.mode {
            Mode::Math(ed) => {
                let el = self.edit_layout(e, ed);
                let b = &el.mbox;
                let frame = Rect::from_min_size(origin, Vec2::new(b.width.max(6.0), b.ascent + b.descent) * geo.zoom).expand(3.0);
                painter.rect_filled(frame, 2.0, tone(Color32::WHITE));
                painter.rect_stroke(frame, 2.0, Stroke::new(1.0, tone(Color32::from_rgb(150, 170, 205))), StrokeKind::Middle);
                let base = |dy: f32| origin.y + (b.ascent + dy) * geo.zoom;
                if let Some((x0, x1, top, bottom)) = el.selection(ed) {
                    let r = Rect::from_min_max(Pos2::new(origin.x + x0 * geo.zoom, base(top)), Pos2::new(origin.x + x1 * geo.zoom, base(bottom)));
                    painter.rect_filled(r, 0.0, tone(SELECT));
                }
                let mut ops = Vec::new();
                box_ops(b, 0.0, b.ascent, &mut ops);
                paint::paint_ops(painter, &ops, origin, geo.zoom, self.cv.dark);
                if let Some(c) = el.caret(ed) {
                    let x = origin.x + c.x * geo.zoom;
                    if std::mem::take(&mut self.cv.reveal) {
                        ui.scroll_to_rect(Rect::from_x_y_ranges(x - 30.0..=x + 30.0, base(c.top) - 30.0..=base(c.bottom) + 30.0), None);
                    }
                    let stroke = Stroke::new(1.5, tone(BLUE));
                    painter.line_segment([Pos2::new(x, base(c.top)), Pos2::new(x, base(c.bottom))], stroke);
                    // zoals in Prime: een streepje onder de cursor
                    painter.line_segment([Pos2::new(x - 3.0, base(c.bottom)), Pos2::new(x + 3.0, base(c.bottom))], stroke);
                    popup = Some(Pos2::new(x - 4.0, base(c.bottom) + 6.0));
                }
            }
            Mode::Text { .. } => {
                let s = setup();
                let w = (geo.page_w - s.margin - e.at.0).max(80.0) * geo.zoom;
                let Some(Edit { mode: Mode::Text { buf, mono, .. }, focus, changed, .. }) = &mut self.cv.edit else { return };
                let size = if *mono { 0.9 * s.text_size } else { s.text_size } * geo.zoom;
                let family = if *mono { FontFamily::Monospace } else { FontFamily::Proportional };
                let lines = buf.lines().count().max(1) as f32;
                let r = Rect::from_min_size(origin - Vec2::new(4.0, 3.0), Vec2::new(w, lines * size * 1.45 + 10.0));
                let out = ui.put(
                    r,
                    TextEdit::multiline(buf).id(text_id()).font(FontId::new(size, family)).desired_width(w - 12.0).desired_rows(1),
                );
                if *focus {
                    out.request_focus();
                    *focus = false;
                }
                if out.changed() {
                    *changed = true;
                }
            }
            Mode::Table(_) => {
                let size = setup().text_size * 0.9 * geo.zoom;
                let Some(Edit { mode: Mode::Table(t), changed, .. }) = &mut self.cv.edit else { return };
                let focus = t.focus.take();
                let mut edited = false;
                let (mut add_row, mut add_col, mut del_col) = (false, false, false);
                let rect = Rect::from_min_size(origin - Vec2::new(4.0, 3.0), Vec2::new(t.names.len() as f32 * 84.0 * geo.zoom + 140.0, 400.0));
                ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                    egui::Frame::new().fill(tone(Color32::WHITE)).stroke(Stroke::new(1.0, tone(Color32::from_rgb(150, 170, 205)))).inner_margin(4.0).show(ui, |ui| {
                        egui::Grid::new("tabel").spacing(Vec2::new(2.0, 2.0)).show(ui, |ui| {
                            let n = t.rows.len() + 2;
                            for r in 0..n {
                                for c in 0..t.names.len() {
                                    let cell = match r {
                                        0 => &mut t.names[c],
                                        1 => &mut t.units[c],
                                        _ => &mut t.rows[r - 2][c],
                                    };
                                    let hint = match r {
                                        0 => "naam",
                                        1 => "eenheid",
                                        _ => "",
                                    };
                                    let id = TableEdit::cell_id(r, c);
                                    let color = if r == 1 { Color32::from_rgb(40, 75, 150) } else { Color32::from_rgb(20, 20, 20) };
                                    let out = ui.add(
                                        TextEdit::singleline(cell)
                                            .id(id)
                                            .hint_text(hint)
                                            .font(FontId::new(size, FontFamily::Proportional))
                                            .text_color(tone(color))
                                            .desired_width(80.0 * geo.zoom),
                                    );
                                    if focus == Some((r, c)) {
                                        out.request_focus();
                                    }
                                    edited |= out.changed();
                                }
                                ui.end_row();
                            }
                        });
                        ui.horizontal(|ui| {
                            add_row = ui.small_button("+ rij").clicked();
                            add_col = ui.small_button("+ kolom").clicked();
                            del_col = t.names.len() > 1 && ui.small_button("− kolom").clicked();
                        });
                    });
                });
                if add_row {
                    t.rows.push(vec![String::new(); t.names.len()]);
                    t.focus = Some((t.rows.len() + 1, 0));
                }
                if add_col {
                    t.names.push(format!("k{}", t.names.len() + 1));
                    t.units.push(String::new());
                    for r in &mut t.rows {
                        r.push(String::new());
                    }
                    t.focus = Some((0, t.names.len() - 1));
                }
                if del_col {
                    t.names.pop();
                    t.units.pop();
                    for r in &mut t.rows {
                        r.pop();
                    }
                }
                if edited || add_col || del_col {
                    *changed = true;
                }
            }
        }
        if let (Some(at), Some(c)) = (popup, &self.cv.complete) {
            use rb_edit::complete::Kind as K;
            let mut clicked = None;
            egui::Area::new(egui::Id::new("aanvullen")).fixed_pos(at).order(egui::Order::Foreground).fade_in(false).show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    for (i, s) in c.items.iter().enumerate() {
                        let (label, hint) = match s.kind {
                            K::Variable => (s.name.clone(), "variabele"),
                            K::UserFunction => (format!("{}( )", s.name), "eigen functie"),
                            K::Function => (format!("{}( )", s.name), "functie"),
                            K::Unit => (s.name.clone(), "eenheid"),
                        };
                        ui.horizontal(|ui| {
                            if ui.selectable_label(i == c.sel, egui::RichText::new(label).monospace()).clicked() {
                                clicked = Some(i);
                            }
                            ui.label(egui::RichText::new(hint).small().weak());
                        });
                    }
                    ui.label(egui::RichText::new(t("Tab: overnemen · ↑↓ kiezen · Esc")).small().weak());
                });
            });
            if let Some(i) = clicked {
                self.accept_completion(i);
            }
        }
    }

    /// Licht of donker; ook voor de knoppen, menu's en grafieken.
    pub(crate) fn set_dark(&mut self, ctx: &egui::Context, dark: bool) {
        self.cv.dark = dark;
        ctx.set_theme(if dark { egui::Theme::Dark } else { egui::Theme::Light });
        // één stijl voor het hele programma: afgeronde hoeken, selectie in de accentkleur
        for theme in [egui::Theme::Light, egui::Theme::Dark] {
            ctx.style_mut_of(theme, |s| {
                let accent = crate::icons::ACCENT;
                s.visuals.selection.bg_fill = accent.gamma_multiply(if theme == egui::Theme::Dark { 0.55 } else { 0.25 });
                s.visuals.selection.stroke = egui::Stroke::new(1.0, accent);
                s.visuals.hyperlink_color = accent;
                s.visuals.window_corner_radius = egui::CornerRadius::same(10);
                s.visuals.menu_corner_radius = egui::CornerRadius::same(8);
                for w in [&mut s.visuals.widgets.inactive, &mut s.visuals.widgets.hovered, &mut s.visuals.widgets.active, &mut s.visuals.widgets.open] {
                    w.corner_radius = egui::CornerRadius::same(6);
                }
                s.spacing.button_padding = egui::vec2(8.0, 4.0);
                s.spacing.menu_margin = egui::Margin::same(8);
            });
        }
    }

    /// Voer een knop van het lint uit; daarna gaat het typen gewoon door op het werkblad.
    pub(crate) fn insert(&mut self, ctx: &egui::Context, ins: Ins) {
        match ins {
            Ins::Library => self.lib.show = !self.lib.show,
            Ins::Solve(var) => {
                let Some((l, r)) = self.current_equation() else {
                    self.status = "Klik eerst op een formule (zoals F == m * a of F := m * a)".into();
                    return;
                };
                // het kruisje onder de formule
                if let [j] = self.cv.selected[..] {
                    if let Some((_, y, _, h)) = self.region_box(j) {
                        let col = self.items().get(j).map_or(0, |i| i.col);
                        self.cv.cross = Some((col, row_below(&self.lay().grid, y + h)));
                    }
                }
                self.commit(false);
                let none = |_: &str| (String::new(), String::new());
                let lines = crate::library::solve_lines(&l, &r, &var, &none, &[]);
                let numeric = lines.iter().any(|l| l == "Given");
                self.insert_lines(ctx, lines);
                self.status = if numeric {
                    format!("{var} staat er meer dan eens in: numeriek opgelost met een solve block (pas de beginwaarde aan)")
                } else {
                    format!("Omgewerkt naar {var}")
                };
            }
            Ins::Image => {
                if let Some(p) = rfd::FileDialog::new().add_filter("PNG-afbeelding", &["png"]).pick_file() {
                    self.insert_image_file(&p);
                }
            }
            Ins::Sheet(a) => self.sheet_action(ctx, a),
            Ins::Cmd(cmd) => {
                self.ensure_math();
                self.edit_cmd(cmd);
            }
            Ins::Type(s) => {
                self.ensure_math();
                for ch in s.chars() {
                    self.edit_cmd(Cmd::Char(ch));
                }
            }
        }
        ctx.memory_mut(|m| m.request_focus(canvas_id()));
    }

    /// Een PNG-bestand als afbeelding op het kruisje (96 dpi, hoogstens zo breed als de pagina).
    pub(crate) fn insert_image_file(&mut self, p: &std::path::Path) {
        let bytes = match std::fs::read(p) {
            Ok(b) => b,
            Err(e) => return self.status = format!("Kan {} niet lezen: {e}", p.display()),
        };
        let Some((w, h)) = rb_export::png::size(&bytes) else {
            return self.status = "Alleen PNG-afbeeldingen kunnen worden ingevoegd".into();
        };
        let s = setup();
        let max_w = s.width - 2.0 * s.margin;
        let mut size = (w as f32 * 0.75, h as f32 * 0.75);
        if size.0 > max_w {
            size = (max_w, size.1 * max_w / size.0);
        }
        self.commit(false);
        let (c, r) = self.cross_or_default();
        let text = format!("image {:.1}x{:.1} data:image/png;base64,{}", size.0, size.1, rb_core::base64::encode(&bytes));
        self.rewrite(|items| items.push(Item { col: c, row: r, text }));
        self.cv.cross = None;
        self.status = format!("Afbeelding ingevoegd: {}", p.display());
    }

    /// De vergelijking van de formule die je bewerkt, of van de geselecteerde regio.
    pub(crate) fn current_equation(&self) -> Option<(rb_core::Expr, rb_core::Expr)> {
        let text = match (&self.cv.edit, &self.cv.selected[..]) {
            (Some(Edit { mode: Mode::Math(ed), .. }), _) => rb_edit::region_text(&ed.root),
            (None, [j]) => self.sheet.regions().get(*j)?.source.clone(),
            _ => return None,
        };
        crate::library::equation_of(&text)
    }

    /// Regels als nieuwe regio's onder elkaar op het kruisje.
    pub(crate) fn insert_lines(&mut self, ctx: &egui::Context, lines: Vec<String>) {
        if lines.is_empty() {
            return;
        }
        self.commit(false);
        let (c, r) = self.cross_or_default();
        let n = lines.len();
        self.rewrite(|items| {
            for (k, text) in lines.into_iter().enumerate() {
                items.push(Item { col: c, row: r + 2 * k, text });
            }
        });
        self.cv.cross = Some(self.free_cell(c, r + 2 * n));
        self.cv.reveal = true;
        ctx.memory_mut(|m| m.request_focus(canvas_id()));
    }

    /// Zorg dat er een formule in bewerking is (een nieuwe op het kruisje als dat nodig is).
    fn ensure_math(&mut self) {
        if !matches!(self.cv.edit, Some(Edit { mode: Mode::Math(_), .. })) {
            self.commit(false);
            let (c, r) = self.cross_or_default();
            self.start_new(c, r, Mode::Math(Editor::new()));
        }
    }

    fn settings_path(&self) -> Option<std::path::PathBuf> {
        Some(self.keymap.path.as_ref()?.parent()?.join("instellingen.txt"))
    }

    /// Lees de instellingen uit instellingen.txt (zie settings.rs).
    pub(crate) fn load_settings(&mut self) {
        let Some(text) = self.settings_path().and_then(|p| std::fs::read_to_string(p).ok()) else { return };
        self.cv.set = crate::settings::Settings::parse(&text);
        self.cv.dark = self.cv.set.thema == crate::settings::Thema::Donker;
        rb_units::set_significant_digits(self.cv.set.digits);
        crate::taal::set_english(self.cv.set.english);
    }

    pub(crate) fn save_settings(&mut self) {
        let Some(p) = self.settings_path() else { return };
        let text = self.cv.set.to_text();
        if let Err(e) = p.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|_| std::fs::write(&p, text)) {
            self.status = format!("Kan {} niet opslaan: {e}", p.display());
        }
    }

    /// Venster met de functies (F2); een klik zet de functie in de formule.
    pub(crate) fn functions_window(&mut self, ctx: &egui::Context) {
        let mut open = self.cv.show_functions;
        let mut chosen = None;
        egui::Window::new(t("Functies")).open(&mut open).default_width(420.0).show(ctx, |ui| {
            egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for n in rb_core::builtins::NAMES {
                        if ui.button(*n).clicked() {
                            chosen = Some(*n);
                        }
                    }
                });
            });
        });
        self.cv.show_functions = open;
        if let Some(n) = chosen {
            self.ensure_math();
            for ch in n.chars().chain(std::iter::once('(')) {
                self.edit_cmd(Cmd::Char(ch));
            }
            ctx.memory_mut(|m| m.request_focus(canvas_id()));
        }
    }
}

/// Raster, ruitjes of lijnen op pagina k, binnen de marges.
fn paint_background(painter: &egui::Painter, bg: Background, geo: &Geo, k: usize, dark: bool) {
    let m = geo.grid.margin;
    let (x0, x1, y0, y1) = (m, geo.page_w - m, m, geo.page_h - m);
    let at = |x: f32, y: f32| geo.local(k, x, y);
    match bg {
        Background::Leeg => {}
        Background::Raster => {
            // een puntje op elke tweede kolom van elke rij
            let dot = if dark { Color32::from_rgb(92, 100, 116) } else { Color32::from_rgb(200, 205, 215) };
            let r = (0.6 * geo.zoom).max(0.7);
            let mut y = y0;
            while y <= y1 {
                let mut x = x0;
                while x <= x1 {
                    painter.circle_filled(at(x, y), r, dot);
                    x += 2.0 * geo.grid.col_w;
                }
                y += ROW_H;
            }
        }
        Background::Ruitjes => {
            let line = Stroke::new(0.6, if dark { Color32::from_rgb(58, 70, 90) } else { Color32::from_rgb(214, 226, 240) });
            let step = 72.0 / 25.4 * 5.0;
            let mut x = x0;
            while x <= x1 + 0.1 {
                painter.line_segment([at(x, y0), at(x, y1)], line);
                x += step;
            }
            let mut y = y0;
            while y <= y1 + 0.1 {
                painter.line_segment([at(x0, y), at(x1, y)], line);
                y += step;
            }
        }
        Background::Lijnen => {
            let line = Stroke::new(0.7, if dark { Color32::from_rgb(64, 80, 106) } else { Color32::from_rgb(200, 215, 235) });
            // de lijn onder elke rij, zodat tekst erop staat
            let mut y = y0 + ROW_H;
            while y <= y1 + 0.1 {
                painter.line_segment([at(x0, y), at(x1, y)], line);
                y += ROW_H;
            }
            // kantlijn
            let margin = if dark { Color32::from_rgb(130, 70, 70) } else { Color32::from_rgb(235, 170, 170) };
            painter.line_segment([at(x0 - 8.0, y0), at(x0 - 8.0, y1)], Stroke::new(0.8, margin));
        }
    }
}

/// De eerste rij met de bovenkant onder doorlopende hoogte `y` (plus wat ruimte).
fn row_below(grid: &Grid, y: f32) -> usize {
    (((grid.unshift(y) + 4.0 - grid.margin) / ROW_H).ceil().max(0.0) as usize) + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bron_met_posities() {
        let mut items = vec![
            Item { col: 30, row: 1, text: "b := 2".into() },
            Item { col: 0, row: 1, text: "a := 1".into() },
            Item { col: 0, row: 4, text: "a + b =".into() },
        ];
        let src = serialize(&mut items);
        assert_eq!(src, "@0,1 a := 1\n@30,1 b := 2\n@0,4 a + b =\n");
        let sheet = rb_sheet::Sheet::load(&src);
        assert!(sheet.render().contains("a + b = 3"), "{}", sheet.render());
    }

    #[test]
    fn tekst_en_commentaar() {
        assert_eq!(text_prefix("\" Hallo wereld"), ("\" ".to_string(), "Hallo wereld".to_string()));
        assert_eq!(text_prefix("## Kop"), ("## ".to_string(), "Kop".to_string()));
        assert_eq!(split_comment("x := 5 // lengte"), ("x := 5", " // lengte".to_string()));
    }

    #[test]
    fn toetsen() {
        assert_eq!(key_of(EKey::D), Some(Key::Char('D')));
        assert_eq!(key_of(EKey::Num4), Some(Key::Char('4')));
        assert_eq!(key_of(EKey::CloseBracket), Some(Key::Char(']')));
        assert_eq!(key_of(EKey::Equals), Some(Key::Char('=')));
        assert_eq!(key_of(EKey::F5), Some(Key::F(5)));
        assert_eq!(key_of(EKey::Quote), Some(Key::Char('\'')), "{}", EKey::Quote.symbol_or_name());
    }
}

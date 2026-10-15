//! Paginalayout: een werkblad wordt een lijst pagina's met tekenopdrachten ([`Op`]), die de
//! SVG- en PDF-schrijver alleen nog hoeven om te zetten. Coördinaten in punten (1/72 inch), y naar
//! beneden, oorsprong linksboven op de pagina.

use crate::font::{fonts, FontId, FontMetrics};
use crate::view::{region_view, Content, View};
use rb_layout::{Item, LayoutOptions, MBox, Style};
use rb_sheet::{Kind, PlotData, Sheet};
use rb_units::format_number;

/// Een kleur (RGB).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color(pub u8, pub u8, pub u8);

pub const BLACK: Color = Color(20, 20, 20);
pub const GREY: Color = Color(110, 110, 110);
pub const LIGHT: Color = Color(220, 220, 220);
pub const RED: Color = Color(190, 30, 30);
pub const ORANGE: Color = Color(190, 110, 0);
pub const HEADING: Color = Color(25, 55, 110);
pub const RESULT: Color = Color(0, 95, 60);
pub const UNIT: Color = Color(40, 75, 150);
pub const KEYWORD: Color = Color(120, 30, 120);
pub const SERIES: [Color; 6] = [
    Color(31, 119, 180),
    Color(214, 39, 40),
    Color(44, 160, 44),
    Color(255, 127, 14),
    Color(148, 103, 189),
    Color(140, 86, 75),
];

/// Kleur bij een stijl uit de formulelayout.
pub fn style_color(s: Style) -> Color {
    match s {
        Style::Normal => BLACK,
        Style::Unit => UNIT,
        Style::Keyword => KEYWORD,
        Style::Result => RESULT,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    /// Tekst met de basislijn op `y`.
    Text { x: f32, y: f32, size: f32, text: String, font: FontId, color: Color },
    Line { x1: f32, y1: f32, x2: f32, y2: f32, width: f32, color: Color },
    Poly { points: Vec<(f32, f32)>, width: f32, color: Color },
    Rect { x: f32, y: f32, w: f32, h: f32, width: f32, color: Color },
    /// Een gevulde rechthoek.
    Fill { x: f32, y: f32, w: f32, h: f32, color: Color },
    /// Een PNG-afbeelding, linksboven op (x, y). `hash` kenmerkt de inhoud.
    Image { x: f32, y: f32, w: f32, h: f32, png: std::sync::Arc<Vec<u8>>, hash: u64 },
}

#[derive(Debug, Clone)]
pub struct Page {
    pub width: f32,
    pub height: f32,
    pub ops: Vec<Op>,
}

/// Paginaformaat en lettergroottes.
#[derive(Debug, Clone, Copy)]
pub struct Setup {
    pub width: f32,
    /// `None`: één doorlopende pagina (SVG); anders een vaste paginahoogte (PDF).
    pub height: Option<f32>,
    pub margin: f32,
    pub text_size: f32,
    pub formula_size: f32,
    /// Ook de waarschuwing "L is ook een eenheid" tonen.
    pub strict: bool,
    /// Grafieken zelf tekenen (export); de interface tekent ze interactief in [`RegionRect::plot`].
    pub static_plots: bool,
}

/// Waar een regio op de pagina staat (voor klikken in de interface en interactieve grafieken).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegionRect {
    pub region: usize,
    pub page: usize,
    /// x, y, breedte, hoogte
    pub rect: [f32; 4],
    /// Het kader van de grafiek, als de regio er een is.
    pub plot: Option<[f32; 4]>,
}

impl Setup {
    /// A4 staand.
    pub fn a4() -> Setup {
        Setup {
            width: 595.0,
            height: Some(842.0),
            margin: 50.0,
            text_size: 10.5,
            formula_size: 11.5,
            strict: false,
            static_plots: true,
        }
    }

    /// Eén doorlopende pagina met de breedte van A4.
    pub fn continuous() -> Setup {
        Setup {
            height: None,
            ..Setup::a4()
        }
    }
}

/// Zet een vak uit de formulelayout om in tekenopdrachten op (x, y) = oorsprong (basislijn).
pub fn box_ops(b: &MBox, x: f32, y: f32, out: &mut Vec<Op>) {
    for item in &b.items {
        out.push(match item {
            Item::Text { x: ix, y: iy, size, text, style } => Op::Text {
                x: x + ix,
                y: y + iy,
                size: *size,
                text: text.clone(),
                font: FontId::Main,
                color: style_color(*style),
            },
            Item::Line { x1, y1, x2, y2, width } => Op::Line {
                x1: x + x1,
                y1: y + y1,
                x2: x + x2,
                y2: y + y2,
                width: *width,
                color: BLACK,
            },
            Item::Poly { points, width } => Op::Poly {
                points: points.iter().map(|(px, py)| (x + px, y + py)).collect(),
                width: *width,
                color: BLACK,
            },
            Item::Mark { .. } => continue,
        });
    }
}

/// Woorden over regels verdelen binnen `width`.
pub fn wrap(text: &str, size: f32, width: f32, font: FontId) -> Vec<String> {
    let f = fonts();
    let mut lines = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split(' ') {
            let candidate = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if !line.is_empty() && f.width(&candidate, size, font) > width {
                lines.push(std::mem::take(&mut line));
                line = word.to_string();
            } else {
                line = candidate;
            }
        }
        lines.push(line);
    }
    lines
}

/// Een gemeten stuk inhoud: hoogte en de tekenopdrachten (relatief, bovenkant op y = 0).
struct Block {
    width: f32,
    height: f32,
    ops: Vec<Op>,
    /// Kader van een (interactief te tekenen) grafiek binnen dit blok.
    plot: Option<[f32; 4]>,
}

fn text_block(text: &str, size: f32, width: f32, font: FontId, color: Color, line_h: f32) -> Block {
    let lines = wrap(text, size, width, font);
    let asc = 0.8 * size;
    let ops = lines
        .iter()
        .enumerate()
        .map(|(k, l)| Op::Text {
            x: 0.0,
            y: asc + k as f32 * line_h * size,
            size,
            text: l.clone(),
            font,
            color,
        })
        .collect();
    let w = lines.iter().map(|l| fonts().width(l, size, font)).fold(0.0, f32::max);
    Block {
        width: w,
        height: lines.len() as f32 * line_h * size,
        ops,
        plot: None,
    }
}

/// Een tabel zoals in Mathcad: een kopregel met de namen, een regel met de eenheden, dan de cellen,
/// met lijnen eromheen.
fn grid_block(names: &[String], units: &[String], rows: &[Vec<String>], ts: f32) -> Block {
    let size = ts * 0.9;
    let (pad, line_h) = (5.0, 1.45 * size);
    let width = |s: &str, f| fonts().width(s, size, f);
    let col_w: Vec<f32> = (0..names.len())
        .map(|k| {
            let cells = rows.iter().map(|r| width(&r[k], FontId::Main));
            cells.chain([width(&names[k], FontId::Main), width(&units[k], FontId::Main)]).fold(24.0, f32::max) + 2.0 * pad
        })
        .collect();
    let total_w: f32 = col_w.iter().sum();
    let n_lines = rows.len() + 2;
    let total_h = n_lines as f32 * line_h;
    let mut ops = vec![Op::Fill { x: 0.0, y: 0.0, w: total_w, h: 2.0 * line_h, color: Color(238, 242, 248) }];
    let mut x = 0.0;
    for (k, w) in col_w.iter().enumerate() {
        let mut put = |line: usize, s: &str, color: Color| {
            // getallen rechts uitlijnen, zoals in een rekenblad
            let right = s.parse::<f64>().is_ok();
            let tx = if right { x + w - pad - width(s, FontId::Main) } else { x + pad };
            ops.push(Op::Text { x: tx, y: line as f32 * line_h + 0.75 * line_h, size, text: s.to_string(), font: FontId::Main, color });
        };
        put(0, &names[k], BLACK);
        put(1, &units[k], UNIT);
        for (r, row) in rows.iter().enumerate() {
            put(r + 2, &row[k], BLACK);
        }
        x += w;
        if k + 1 < col_w.len() {
            ops.push(Op::Line { x1: x, y1: 0.0, x2: x, y2: total_h, width: 0.5, color: LIGHT });
        }
    }
    for line in [1, 2] {
        let y = line as f32 * line_h;
        ops.push(Op::Line { x1: 0.0, y1: y, x2: total_w, y2: y, width: if line == 2 { 0.8 } else { 0.5 }, color: if line == 2 { GREY } else { LIGHT } });
    }
    ops.push(Op::Rect { x: 0.0, y: 0.0, w: total_w, h: total_h, width: 0.8, color: GREY });
    Block { width: total_w, height: total_h, ops, plot: None }
}

fn formula_block(b: &MBox) -> Block {
    let mut ops = Vec::new();
    box_ops(b, 0.0, b.ascent, &mut ops);
    Block {
        width: b.width,
        height: b.height(),
        ops,
        plot: None,
    }
}

fn stack(parts: Vec<Block>, gap: f32) -> Block {
    let mut ops = Vec::new();
    let mut y = 0.0;
    let mut w: f32 = 0.0;
    let mut plot = None;
    for (k, p) in parts.iter().enumerate() {
        if k > 0 {
            y += gap;
        }
        if let Some([px, py, pw, ph]) = p.plot {
            plot = Some([px, py + y, pw, ph]);
        }
        ops.extend(p.ops.iter().map(|o| shift(o, 0.0, y)));
        y += p.height;
        w = w.max(p.width);
    }
    Block { width: w, height: y, ops, plot }
}

pub fn shift(o: &Op, dx: f32, dy: f32) -> Op {
    match o.clone() {
        Op::Text { x, y, size, text, font, color } => Op::Text { x: x + dx, y: y + dy, size, text, font, color },
        Op::Line { x1, y1, x2, y2, width, color } => Op::Line { x1: x1 + dx, y1: y1 + dy, x2: x2 + dx, y2: y2 + dy, width, color },
        Op::Poly { points, width, color } => Op::Poly { points: points.into_iter().map(|(x, y)| (x + dx, y + dy)).collect(), width, color },
        Op::Rect { x, y, w, h, width, color } => Op::Rect { x: x + dx, y: y + dy, w, h, width, color },
        Op::Fill { x, y, w, h, color } => Op::Fill { x: x + dx, y: y + dy, w, h, color },
        Op::Image { x, y, w, h, png, hash } => Op::Image { x: x + dx, y: y + dy, w, h, png, hash },
    }
}

/// Ronde schaalverdeling: stapgrootte 1, 2 of 5 × 10^k.
pub fn nice_ticks(lo: f64, hi: f64, target: usize) -> Vec<f64> {
    let span = hi - lo;
    if span <= 0.0 || !span.is_finite() {
        return vec![lo];
    }
    let raw = span / target as f64;
    let mag = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 5.0, 10.0].iter().map(|f| f * mag).find(|s| *s >= raw).unwrap_or(10.0 * mag);
    // met een geheel stapnummer rekenen, zodat -0.3 / 0.1 niet net boven -3 uitkomt
    let k0 = (lo / step - 1e-9).ceil() as i64;
    (k0..)
        .map(|k| k as f64 * step)
        .take_while(|v| *v <= hi + step * 1e-9)
        .collect()
}

/// Teken een grafiek in een kader van `w` × `h` (linksboven op 0, 0).
pub fn plot_ops(p: &PlotData, w: f32, h: f32, size: f32) -> Vec<Op> {
    let mut ops = Vec::new();
    let (left, right, top, bottom) = (52.0, 12.0, 14.0, 30.0);
    let (px, py, pw, ph) = (left, top, w - left - right, h - top - bottom);
    let pts: Vec<[f64; 2]> = p.series.iter().flat_map(|s| s.points.iter().copied()).filter(|q| q[0].is_finite() && q[1].is_finite()).collect();
    let range = |k: usize| {
        let lo = pts.iter().map(|q| q[k]).fold(f64::INFINITY, f64::min);
        let hi = pts.iter().map(|q| q[k]).fold(f64::NEG_INFINITY, f64::max);
        if !lo.is_finite() {
            (0.0, 1.0)
        } else if hi - lo <= 1e-12 * lo.abs().max(1e-300) {
            (lo - 1.0, hi + 1.0)
        } else {
            (lo, hi)
        }
    };
    let ((x0, x1), (y0, y1)) = (range(0), range(1));
    // marge van 5% boven en onder
    let pad = 0.05 * (y1 - y0);
    let (y0, y1) = (y0 - pad, y1 + pad);
    let sx = |x: f64| px + ((x - x0) / (x1 - x0)) as f32 * pw;
    let sy = |y: f64| py + ph - ((y - y0) / (y1 - y0)) as f32 * ph;
    let small = 0.8 * size;
    for t in nice_ticks(x0, x1, 6) {
        let x = sx(t);
        ops.push(Op::Line { x1: x, y1: py, x2: x, y2: py + ph, width: 0.5, color: LIGHT });
        let label = format_number(t);
        let lw = fonts().width(&label, small, FontId::Main);
        ops.push(Op::Text { x: x - lw / 2.0, y: py + ph + small + 3.0, size: small, text: label, font: FontId::Main, color: GREY });
    }
    for t in nice_ticks(y0, y1, 5) {
        let y = sy(t);
        ops.push(Op::Line { x1: px, y1: y, x2: px + pw, y2: y, width: 0.5, color: LIGHT });
        let label = format_number(t);
        let lw = fonts().width(&label, small, FontId::Main);
        ops.push(Op::Text { x: px - lw - 4.0, y: y + small * 0.35, size: small, text: label, font: FontId::Main, color: GREY });
    }
    ops.push(Op::Rect { x: px, y: py, w: pw, h: ph, width: 0.8, color: GREY });
    for (k, s) in p.series.iter().enumerate() {
        let color = SERIES[k % SERIES.len()];
        let mut seg: Vec<(f32, f32)> = Vec::new();
        for q in &s.points {
            if q[0].is_finite() && q[1].is_finite() {
                seg.push((sx(q[0]), sy(q[1])));
            } else if !seg.is_empty() {
                ops.push(Op::Poly { points: std::mem::take(&mut seg), width: 1.3, color });
            }
        }
        if seg.len() == 1 {
            let (x, y) = seg[0];
            ops.push(Op::Rect { x: x - 1.5, y: y - 1.5, w: 3.0, h: 3.0, width: 1.0, color });
        } else if !seg.is_empty() {
            ops.push(Op::Poly { points: seg, width: 1.3, color });
        }
    }
    // aslabels en legenda
    let xl = &p.x_label;
    let xw = fonts().width(xl, small, FontId::Main);
    ops.push(Op::Text { x: px + pw / 2.0 - xw / 2.0, y: h - 4.0, size: small, text: xl.clone(), font: FontId::Main, color: BLACK });
    if !p.y_label.is_empty() {
        ops.push(Op::Text { x: 2.0, y: top - 4.0, size: small, text: p.y_label.clone(), font: FontId::Main, color: BLACK });
    }
    let mut ly = py + small + 4.0;
    let legend_w = p.series.iter().map(|s| fonts().width(&s.label, small, FontId::Main)).fold(0.0, f32::max) + 26.0;
    ops.push(Op::Fill { x: px + pw - legend_w - 4.0, y: py + 3.0, w: legend_w + 1.0, h: p.series.len() as f32 * small * 1.3 + 4.0, color: Color(255, 255, 255) });
    for (k, s) in p.series.iter().enumerate() {
        let color = SERIES[k % SERIES.len()];
        let lw = fonts().width(&s.label, small, FontId::Main);
        let lx = px + pw - lw - 6.0;
        ops.push(Op::Line { x1: lx - 16.0, y1: ly - small * 0.3, x2: lx - 4.0, y2: ly - small * 0.3, width: 1.5, color });
        ops.push(Op::Text { x: lx, y: ly, size: small, text: s.label.clone(), font: FontId::Main, color: BLACK });
        ly += small * 1.3;
    }
    ops
}

fn view_block(v: &View, setup: &Setup, avail: f32) -> Block {
    let ts = setup.text_size;
    let mut parts = vec![match &v.content {
        Content::Heading { text, level } => {
            let size = match level {
                1 => ts * 1.7,
                2 => ts * 1.4,
                _ => ts * 1.2,
            };
            text_block(text, size, avail, FontId::Main, HEADING, 1.25)
        }
        Content::Text(t) => text_block(t, ts, avail, FontId::Main, BLACK, 1.35),
        Content::Comment(t) => text_block(t, ts * 0.9, avail, FontId::Main, GREY, 1.3),
        Content::Formula(b) => formula_block(b),
        Content::Table { head, rows } => {
            let table = text_block(&rows.join("\n"), ts * 0.82, avail, FontId::Mono, BLACK, 1.3);
            stack(vec![formula_block(head), table], 4.0)
        }
        Content::Plot { source, data } => {
            let head = text_block(source, ts * 0.8, avail, FontId::Mono, GREY, 1.3);
            let w = avail.min(430.0);
            let h = 0.55 * w;
            let plot = if setup.static_plots {
                Block { width: w, height: h, ops: plot_ops(data, w, h, ts), plot: None }
            } else {
                Block { width: w, height: h, ops: Vec::new(), plot: Some([0.0, 0.0, w, h]) }
            };
            stack(vec![head, plot], 3.0)
        }
        Content::Source(s) => text_block(s, ts * 0.9, avail, FontId::Mono, BLACK, 1.3),
        Content::Grid { names, units, rows } => grid_block(names, units, rows, ts),
        Content::AreaStart { title, collapsed } => {
            let h = 1.4 * ts;
            let mark = if *collapsed { '▸' } else { '▾' };
            let label = if title.is_empty() { format!("{mark} gebied") } else { format!("{mark} {title}") };
            let ops = vec![
                Op::Fill { x: 0.0, y: 0.0, w: avail, h, color: Color(236, 241, 249) },
                Op::Line { x1: 0.0, y1: h, x2: avail, y2: h, width: 0.8, color: Color(150, 170, 205) },
                Op::Text { x: 4.0, y: 0.72 * h, size: ts * 0.85, text: label, font: FontId::Main, color: HEADING },
            ];
            Block { width: avail, height: h, ops, plot: None }
        }
        Content::AreaEnd => Block {
            width: avail,
            height: 4.0,
            ops: vec![Op::Line { x1: 0.0, y1: 2.0, x2: avail, y2: 2.0, width: 0.8, color: Color(150, 170, 205) }],
            plot: None,
        },
        // op het scherm een stippellijn; op papier niets (de volgende regio staat al op een nieuwe pagina)
        Content::PageBreak if setup.static_plots => Block { width: avail, height: 2.0, ops: Vec::new(), plot: None },
        Content::PageBreak => {
            let mut ops: Vec<Op> = (0..)
                .map(|k| k as f32 * 8.0)
                .take_while(|x| *x < avail)
                .map(|x| Op::Line { x1: x, y1: 6.0, x2: (x + 4.0).min(avail), y2: 6.0, width: 0.8, color: GREY })
                .collect();
            ops.push(Op::Text { x: avail - 70.0, y: 4.0, size: ts * 0.6, text: "pagina-einde".into(), font: FontId::Main, color: GREY });
            Block { width: avail, height: 12.0, ops, plot: None }
        }
        Content::HeaderFooter { .. } if setup.static_plots => Block { width: 1.0, height: 1.0, ops: Vec::new(), plot: None },
        Content::HeaderFooter { text, footer } => {
            let what = if *footer { "Voettekst" } else { "Koptekst" };
            text_block(&format!("{what}: {text}"), ts * 0.8, avail, FontId::Main, GREY, 1.3)
        }
        Content::Image(img) => {
            // niet breder dan de pagina
            let scale = (avail / img.width.max(1.0)).min(1.0);
            let (w, h) = (img.width * scale, img.height * scale);
            Block { width: w, height: h, ops: vec![Op::Image { x: 0.0, y: 0.0, w, h, png: img.png.clone(), hash: img.hash }], plot: None }
        }
    }];
    if let Some(e) = &v.error {
        parts.push(text_block(e, ts * 0.85, avail, FontId::Main, RED, 1.25));
    }
    for w in &v.warnings {
        parts.push(text_block(w, ts * 0.85, avail, FontId::Main, ORANGE, 1.25));
    }
    let mut b = stack(parts, 3.0);
    // een fout of waarschuwing: een gekleurde streep links langs de hele regio
    let mark = if v.error.is_some() { Some(RED) } else if !v.warnings.is_empty() { Some(ORANGE) } else { None };
    if let Some(color) = mark {
        b.ops.push(Op::Fill { x: -7.0, y: 0.0, w: 2.5, h: b.height, color });
    }
    b
}

/// Zet een werkblad om in pagina's.
pub fn layout_document(sheet: &Sheet, setup: &Setup) -> Vec<Page> {
    layout_with_regions(sheet, setup).0
}

/// Hoogte van een rij van het raster (`@kolom,rij`), in punten.
pub const ROW_H: f32 = 14.0;
/// Minimale ruimte tussen regio's onder elkaar.
const GAP: f32 = 3.0;
/// Minimale ruimte tussen regio's naast elkaar (ongeveer anderhalve kolom).
const HGAP: f32 = 10.0;

/// Het raster van het blad: een kolom is de breedte van een cijfer, een rij [`ROW_H`]. Het blad is één
/// doorlopende strook (y vanaf de bovenkant van de eerste pagina); [`Grid::page_of`] verdeelt die
/// over pagina's.
#[derive(Debug, Clone)]
pub struct Grid {
    pub col_w: f32,
    pub margin: f32,
    /// Hoogte van het beschrijfbare deel van een pagina (`None`: één doorlopende pagina).
    pub content_h: Option<f32>,
    /// Verschuivingen door ingeklapte gebieden en pagina-einden: vanaf rasterhoogte `.0` schuift
    /// alles `.1` op (oplopend; de laatste die geldt, telt). Gezet door [`layout_full`].
    pub shifts: Vec<(f32, f32)>,
}

impl Grid {
    pub fn new(setup: &Setup) -> Grid {
        Grid {
            col_w: fonts().width("0", setup.formula_size, FontId::Main),
            margin: setup.margin,
            content_h: setup.height.map(|h| h - 2.0 * setup.margin),
            shifts: Vec::new(),
        }
    }

    /// Linksboven van een rasterplek, zonder verschuivingen.
    fn raw_point(&self, col: usize, row: usize) -> (f32, f32) {
        (self.margin + col as f32 * self.col_w, self.margin + row.saturating_sub(1) as f32 * ROW_H)
    }

    fn shift_at(&self, raw_y: f32) -> f32 {
        self.shifts.iter().rev().find(|(from, _)| raw_y >= *from - 0.01).map_or(0.0, |s| s.1)
    }

    /// Van getekende hoogte terug naar rasterhoogte: het laatste verschoven stuk dat op het scherm
    /// boven `y` begint.
    pub fn unshift(&self, y: f32) -> f32 {
        y - self.shifts.iter().rev().find(|(from, dy)| y >= from + dy - 0.01).map_or(0.0, |s| s.1)
    }

    /// Linksboven van een rasterplek, in doorlopende coördinaten (zoals getekend).
    pub fn point(&self, col: usize, row: usize) -> (f32, f32) {
        let (x, y) = self.raw_point(col, row);
        (x, y + self.shift_at(y))
    }

    /// De dichtstbijzijnde rasterplek bij een punt in doorlopende coördinaten (zoals getekend).
    pub fn cell(&self, x: f32, y: f32) -> (usize, usize) {
        let y = self.unshift(y);
        let col = ((x - self.margin) / self.col_w).round().max(0.0) as usize;
        let row = ((y - self.margin) / ROW_H).round().max(0.0) as usize + 1;
        (col, row)
    }

    /// Pagina waarop doorlopende hoogte `y` valt.
    pub fn page_of(&self, y: f32) -> usize {
        match self.content_h {
            Some(ch) => ((y - self.margin) / ch).floor().max(0.0) as usize,
            None => 0,
        }
    }

    /// Verschuiving van doorlopende hoogte naar hoogte op pagina `page`.
    pub fn page_shift(&self, page: usize) -> f32 {
        self.content_h.map_or(0.0, |ch| page as f32 * ch)
    }
}

/// Als [`layout_document`], plus de plek van elke regio.
///
/// Zoals in Mathcad staat elke regio op zijn plek in het raster (`@kolom,rij`; zonder positie de
/// rij van zijn regelnummer). Een regio die over een eerdere heen zou vallen, of over een
/// paginagrens, schuift naar beneden.
pub fn layout_with_regions(sheet: &Sheet, setup: &Setup) -> (Vec<Page>, Vec<RegionRect>) {
    let (pages, rects, _) = layout_full(sheet, setup);
    (pages, rects)
}

/// Als [`layout_with_regions`], plus het raster met de verschuivingen door ingeklapte gebieden en
/// pagina-einden (voor klikken in de interface).
pub fn layout_full(sheet: &Sheet, setup: &Setup) -> (Vec<Page>, Vec<RegionRect>, Grid) {
    let opts = LayoutOptions { size: setup.formula_size };
    let mut grid = Grid::new(setup);
    let content_w = setup.width - 2.0 * setup.margin;
    let regions = sheet.regions();
    // eerst alle plekken (doorlopend), dan pas over pagina's verdelen
    let mut placed: Vec<(usize, f32, f32, Block)> = Vec::new();
    // in een ingeklapt gebied (diepte; geneste gebieden tellen mee): niet tonen; daarna schuift
    // alles op tot onder de balk
    let (mut hidden, mut area_top) = (0usize, 0.0f32);
    // na een pagina-einde: alles eronder schuift mee naar de volgende pagina
    let (mut dy, mut break_top): (f32, Option<f32>) = (0.0, None);
    let mut heads: Vec<(String, bool)> = Vec::new();
    for (j, r) in regions.iter().enumerate() {
        if let Kind::HeaderFooter { text, footer } = &r.kind {
            heads.push((text.clone(), *footer));
        }
        let (x, y0) = grid.raw_point(r.pos.col, r.pos.row);
        if hidden > 0 {
            match r.kind {
                Kind::Area { .. } => hidden += 1,
                Kind::AreaEnd => hidden -= 1,
                _ => {}
            }
            if hidden == 0 {
                // wat na het gebied komt, begint twee rijen onder de balk (zo is elke plek op het
                // scherm terug te rekenen naar één rij: de balk of wat erna komt)
                dy -= (y0 - (area_top + 2.0 * ROW_H)).max(0.0);
                grid.shifts.push((y0, dy));
            }
            continue;
        }
        if matches!(r.kind, Kind::Area { collapsed: true, .. }) {
            hidden = 1;
            area_top = y0;
        }
        if let Some(top) = break_top.take() {
            let add = (top - (y0 + dy)).max(0.0);
            if add > 0.0 {
                dy += add;
                grid.shifts.push((y0, dy));
            }
        }
        let mut y = y0 + dy;
        let mut x = x.min(setup.margin + content_w * 0.9);
        let right_edge = setup.margin + content_w;
        let avail = (setup.margin + content_w - x).max(content_w * 0.2);
        let v = region_view(sheet, j, &FontMetrics, opts, setup.strict);
        let b = view_block(&v, setup, avail);
        loop {
            let mut moved = false;
            for (_, px, py, pb) in &placed {
                // naast elkaar minstens HGAP ruimte, zodat twee formules niet als één lezen
                let h_overlap = x < px + pb.width + HGAP && *px < x + b.width.max(1.0) + HGAP;
                let v_overlap = y < py + pb.height + GAP && *py < y + b.height;
                if h_overlap && v_overlap {
                    let beside = px + 1.0 < x;
                    let to_right = px + pb.width + HGAP;
                    if beside && to_right + b.width <= right_edge {
                        // een regio rechts naast een andere: opschuiven naar rechts (zoals in Mathcad naast elkaar)
                        x = to_right;
                    } else {
                        y = py + pb.height + GAP;
                    }
                    moved = true;
                }
            }
            if let Some(ch) = grid.content_h {
                // niet over een paginagrens (tenzij de regio hoger is dan een pagina)
                let page = grid.page_of(y);
                let bottom = setup.margin + (page + 1) as f32 * ch;
                if y + b.height > bottom && b.height < ch {
                    y = bottom;
                    moved = true;
                }
            }
            if !moved {
                break;
            }
        }
        if let (Kind::PageBreak, Some(ch)) = (&r.kind, grid.content_h) {
            break_top = Some(setup.margin + (grid.page_of(y) + 1) as f32 * ch);
        }
        placed.push((j, x, y, b));
    }
    let npages = placed.iter().map(|p| grid.page_of(p.2) + 1).max().unwrap_or(1);
    let mut pages: Vec<Page> = (0..npages)
        .map(|_| Page { width: setup.width, height: setup.height.unwrap_or(0.0), ops: Vec::new() })
        .collect();
    let mut rects = Vec::new();
    let mut bottom: f32 = 0.0;
    for (j, x, y, b) in placed {
        let page_no = grid.page_of(y);
        let ly = y - grid.page_shift(page_no);
        pages[page_no].ops.extend(b.ops.iter().map(|o| shift(o, x, ly)));
        rects.push(RegionRect {
            region: j,
            page: page_no,
            rect: [x, ly, b.width, b.height],
            plot: b.plot.map(|[px, py, pw, ph]| [x + px, ly + py, pw, ph]),
        });
        bottom = bottom.max(y + b.height);
    }
    if setup.height.is_none() {
        pages[0].height = bottom.max(setup.margin) + setup.margin;
    }
    // kop- en voetteksten op elke pagina, in de marge
    let n = pages.len();
    let date = today();
    for (p, page) in pages.iter_mut().enumerate() {
        let (mut top, mut foot) = (Vec::new(), Vec::new());
        for (t, footer) in &heads {
            let t = t
                .replace("{pagina}", &(p + 1).to_string())
                .replace("{paginas}", &n.to_string())
                .replace("{bestand}", sheet.name())
                .replace("{datum}", &date);
            if *footer { foot.push(t) } else { top.push(t) }
        }
        let size = setup.text_size * 0.75;
        for (k, t) in top.iter().enumerate() {
            let y = setup.margin * 0.55 + k as f32 * 1.3 * size;
            page.ops.push(Op::Text { x: setup.margin, y, size, text: t.clone(), font: FontId::Main, color: GREY });
        }
        for (k, t) in foot.iter().rev().enumerate() {
            let y = page.height - setup.margin * 0.45 - k as f32 * 1.3 * size;
            page.ops.push(Op::Text { x: setup.margin, y, size, text: t.clone(), font: FontId::Main, color: GREY });
        }
    }
    (pages, rects, grid)
}

/// De datum van vandaag (dd-mm-jjjj), zonder tijdzone (UTC).
fn today() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (y, m, d) = civil_from_days((secs / 86_400) as i64);
    format!("{d:02}-{m:02}-{y}")
}

/// Dagen sinds 1970-01-01 naar (jaar, maand, dag) (algoritme van Howard Hinnant).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schaalverdeling() {
        assert_eq!(nice_ticks(0.0, 10.0, 5), vec![0.0, 2.0, 4.0, 6.0, 8.0, 10.0]);
        let t = nice_ticks(-0.3, 0.3, 6);
        assert_eq!(t.len(), 7, "{t:?}");
        assert!((t[0] + 0.3).abs() < 1e-12 && t[3] == 0.0);
        let t = nice_ticks(0.0, 7.3, 5);
        assert_eq!(t.first(), Some(&0.0));
        assert!(t.len() >= 4);
    }

    #[test]
    fn tekst_wordt_afgebroken() {
        let lines = wrap("een twee drie vier vijf zes zeven acht", 10.0, 60.0, FontId::Main);
        assert!(lines.len() > 2, "{lines:?}");
        assert!(lines.iter().all(|l| fonts().width(l, 10.0, FontId::Main) <= 60.0 || !l.contains(' ')));
    }

    fn texts(p: &Page) -> Vec<String> {
        p.ops.iter().filter_map(|o| if let Op::Text { text, .. } = o { Some(text.clone()) } else { None }).collect()
    }

    #[test]
    fn gebieden_en_kop_en_voettekst() {
        // ingeklapt: de inhoud wordt doorgerekend maar niet getoond
        let src = "area ingeklapt Invoer\na := 5\nverborgen.waarde := 7\narea end\nb := a * 2 =\n";
        let sheet = Sheet::load(src);
        assert_eq!(sheet.output(4), &rb_sheet::Output::Value("10".into()));
        let (pages, rects) = layout_with_regions(&sheet, &Setup::a4());
        let t = texts(&pages[0]).concat();
        assert!(t.contains("Invoer") && !t.contains("verborgen") && t.contains("10"), "{t}");
        assert_eq!(rects.len(), 2, "alleen de balk en b");
        // uitgeklapt: alles staat er
        let sheet = Sheet::load(&src.replace("ingeklapt ", ""));
        let (pages, _) = layout_with_regions(&sheet, &Setup::a4());
        assert!(texts(&pages[0]).concat().contains("verborgen"));
        // `area := 5` is gewoon een variabele
        assert!(matches!(Sheet::load("area := 5").regions()[0].kind, Kind::Math(_)));
        // de tekens van de balk bestaan in de lettertypen
        assert!(fonts().main.glyph('▸').is_some() || fonts().mono.glyph('▸').is_some());
        assert!(fonts().main.glyph('▾').is_some() || fonts().mono.glyph('▾').is_some());

        // pagina-einde: wat eronder staat, begint op pagina 2; kop- en voettekst op elke pagina
        let mut sheet = Sheet::load("header Verslag {bestand}\nfooter pagina {pagina} van {paginas}\na := 1\npagebreak\nb := 2 =\n");
        sheet.set_name("proef.rkn");
        let mut setup = Setup::a4();
        setup.static_plots = true;
        let (pages, rects) = layout_with_regions(&sheet, &setup);
        assert_eq!(pages.len(), 2);
        assert_eq!(rects.last().unwrap().page, 1);
        for (k, p) in pages.iter().enumerate() {
            let t = texts(p);
            assert!(t.contains(&"Verslag proef.rkn".to_string()), "{t:?}");
            assert!(t.contains(&format!("pagina {} van 2", k + 1)), "{t:?}");
            assert!(!t.iter().any(|x| x.starts_with("Koptekst")), "op papier geen regio voor de koptekst");
        }
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_739), (2026, 10, 13));
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
    }

    #[test]
    fn pagina_einde() {
        let src: String = (0..120).map(|k| format!("a.{k} := {k} m\n")).collect();
        let sheet = Sheet::load(&src);
        let pages = layout_document(&sheet, &Setup::a4());
        assert!(pages.len() >= 2, "{} pagina's", pages.len());
        for p in &pages {
            for o in &p.ops {
                if let Op::Text { y, .. } = o {
                    assert!(*y <= p.height, "tekst buiten de pagina");
                }
            }
        }
        let one = layout_document(&sheet, &Setup::continuous());
        assert_eq!(one.len(), 1);
        assert!(one[0].height > 842.0);
    }
}

#[cfg(test)]
mod raster {
    use super::*;

    /// Wat de interface doet: elke regio terugrekenen van zijn plek op het scherm naar een rij.
    /// Dat moet de rij uit de bron opleveren, ook na een ingeklapt gebied of een pagina-einde.
    #[test]
    fn terugrekenen_naar_rijen() {
        let src = "@0,1 x := 1\n@0,3 area ingeklapt Invoer\n@0,5 a := 5 m\n@0,7 b := 3 m\n@0,9 area end\n@0,11 c := a + b =\n@0,13 pagebreak\n@0,15 d := c * 2 =\n@40,15 e := 3\n";
        let sheet = Sheet::load(src);
        let (_, rects, grid) = layout_full(&sheet, &Setup::a4());
        for q in &rects {
            let r = &sheet.regions()[q.region];
            let (col, row) = grid.cell(q.rect[0], q.rect[1] + grid.page_shift(q.page));
            assert_eq!((col, row), (r.pos.col, r.pos.row), "{}", r.source);
        }
        // de regio na het gebied staat net onder de balk, d op pagina 2
        let page_of = |name: &str| rects.iter().find(|q| sheet.regions()[q.region].source.starts_with(name)).unwrap().page;
        assert_eq!((page_of("c :="), page_of("d :=")), (0, 1));
    }
}

//! Paginalayout: een werkblad wordt een lijst pagina's met tekenopdrachten ([`Op`]), die de
//! SVG- en PDF-schrijver alleen nog hoeven om te zetten. Coördinaten in punten (1/72 inch), y naar
//! beneden, oorsprong linksboven op de pagina.

use crate::font::{fonts, FontId, FontMetrics};
use crate::view::{region_view, Content, View};
use rb_layout::{Item, LayoutOptions, MBox, Style};
use rb_sheet::{Kind, PlotData, Sheet, Surface};
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
    /// Een gevulde veelhoek, met een rand als `width > 0` (vlakjes van een 3D-oppervlak).
    Polygon { points: Vec<(f32, f32)>, fill: Color, width: f32, color: Color },
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
        Op::Polygon { points, fill, width, color } => Op::Polygon { points: points.into_iter().map(|(x, y)| (x + dx, y + dy)).collect(), fill, width, color },
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

/// Schaalverdeling van een logaritmische as, in log10-waarden tussen `lo` en `hi`: (waarde, hoofdstreep).
/// Hoofdstrepen op elke macht van 10, tussenstrepen op 2..9 × 10^k (bij veel decaden weggelaten).
pub fn log_ticks(lo: f64, hi: f64) -> Vec<(f64, bool)> {
    if !(lo.is_finite() && hi.is_finite()) || hi <= lo {
        return Vec::new();
    }
    let (k0, k1) = (lo.floor() as i64, hi.ceil() as i64);
    // te veel decaden: alleen elke n-de macht
    let every = ((k1 - k0) / 10 + 1).max(1);
    let minor = k1 - k0 <= 8;
    let mut out = Vec::new();
    for k in k0..=k1 {
        if k % every == 0 && (lo - 1e-9..=hi + 1e-9).contains(&(k as f64)) {
            out.push((k as f64, true));
        }
        if minor {
            for m in 2..=9 {
                let v = k as f64 + (m as f64).log10();
                if (lo..=hi).contains(&v) {
                    out.push((v, false));
                }
            }
        }
    }
    out
}

/// Een polylijn afknippen op het kader `[x0, x1] × [y0, y1]` (Liang–Barsky per stuk).
fn clip(points: &[[f64; 2]], x0: f64, x1: f64, y0: f64, y1: f64) -> Vec<Vec<[f64; 2]>> {
    let inside = |q: [f64; 2]| (x0..=x1).contains(&q[0]) && (y0..=y1).contains(&q[1]);
    if points.len() == 1 {
        return if inside(points[0]) { vec![points.to_vec()] } else { Vec::new() };
    }
    let mut out: Vec<Vec<[f64; 2]>> = Vec::new();
    let mut cur: Vec<[f64; 2]> = Vec::new();
    for w in points.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let (mut t0, mut t1) = (0.0f64, 1.0f64);
        let mut keep = true;
        for (p, q) in [(-dx, a[0] - x0), (dx, x1 - a[0]), (-dy, a[1] - y0), (dy, y1 - a[1])] {
            if p == 0.0 {
                keep &= q >= 0.0;
            } else if p < 0.0 {
                t0 = t0.max(q / p);
            } else {
                t1 = t1.min(q / p);
            }
        }
        if !keep || t0 > t1 {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            continue;
        }
        let pa = [a[0] + t0 * dx, a[1] + t0 * dy];
        let pb = [a[0] + t1 * dx, a[1] + t1 * dy];
        if cur.last() != Some(&pa) {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            cur.push(pa);
        }
        cur.push(pb);
        if t1 < 1.0 {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Een waarde naar de as: log10 bij een logaritmische as (0 en negatief worden NaN).
pub fn axis_value(v: f64, log: bool) -> f64 {
    if !log {
        v
    } else if v > 0.0 {
        v.log10()
    } else {
        f64::NAN
    }
}

/// Kleur bij t ∈ [0, 1] (laag paars, midden blauwgroen, hoog geel), voor oppervlakken en hoogtelijnen.
pub fn heat(t: f64) -> Color {
    const STOPS: [(f64, f64, f64); 3] = [(68.0, 1.0, 84.0), (33.0, 145.0, 140.0), (253.0, 231.0, 37.0)];
    let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.5 };
    let (i, f) = if t < 0.5 { (0, t * 2.0) } else { (1, (t - 0.5) * 2.0) };
    let (a, b) = (STOPS[i], STOPS[i + 1]);
    let mix = |p: f64, q: f64| (p + (q - p) * f).round() as u8;
    Color(mix(a.0, b.0), mix(a.1, b.1), mix(a.2, b.2))
}

/// Standaardblik op een 3D-oppervlak: (draaiing om de verticale as, hoogte), in radialen.
pub const SURFACE_VIEW: (f64, f64) = (-0.65, 0.55);

/// Teken een grafiek in een kader van `w` × `h` (linksboven op 0, 0).
pub fn plot_ops(p: &PlotData, w: f32, h: f32, size: f32) -> Vec<Op> {
    if let Some(s) = &p.surface {
        return surface_ops(s, &p.x_label, w, h, size, SURFACE_VIEW);
    }
    let mut ops = Vec::new();
    let two_axes = !p.series2.is_empty();
    let (left, right, top, bottom) = (52.0, if two_axes { 52.0 } else { 12.0 }, 14.0, 30.0);
    let (px, py, pw, ph) = (left, top, w - left - right, h - top - bottom);
    // punten op de assen (log10 bij een logaritmische as)
    let tf = |q: &[f64; 2]| [axis_value(q[0], p.log_x), axis_value(q[1], p.log_y)];
    let series: Vec<Vec<[f64; 2]>> = p.series.iter().map(|s| s.points.iter().map(tf).collect()).collect();
    let series2: Vec<Vec<[f64; 2]>> = p.series2.iter().map(|s| s.points.iter().map(tf).collect()).collect();
    let finite = |list: &[Vec<[f64; 2]>]| -> Vec<[f64; 2]> { list.iter().flatten().copied().filter(|q| q[0].is_finite() && q[1].is_finite()).collect() };
    let (pts, pts2) = (finite(&series), finite(&series2));
    let range = |pts: &[[f64; 2]], k: usize| {
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
    let padded = |(a, b): (f64, f64)| {
        let pad = 0.05 * (b - a);
        (a - pad, b + pad)
    };
    // vaste grenzen gaan voor; anders de data, met 5% marge boven en onder
    let fixed = |r: Option<(f64, f64)>, log: bool| r.map(|(a, b)| (axis_value(a, log), axis_value(b, log))).filter(|(a, b)| a < b);
    let all_pts: Vec<[f64; 2]> = pts.iter().chain(&pts2).copied().collect();
    let (mut x0, mut x1) = fixed(p.x_range, p.log_x).unwrap_or_else(|| range(&all_pts, 0));
    let (mut y0, mut y1) = fixed(p.y_range, p.log_y).unwrap_or_else(|| padded(range(&pts, 1)));
    // poolgrafiek: x en y even groot, zodat een cirkel rond blijft
    if p.polar {
        let s = ((x1 - x0) / pw as f64).max((y1 - y0) / ph as f64);
        let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        (x0, x1) = (cx - s * pw as f64 / 2.0, cx + s * pw as f64 / 2.0);
        (y0, y1) = (cy - s * ph as f64 / 2.0, cy + s * ph as f64 / 2.0);
    }
    let (y20, y21) = padded(range(&pts2, 1));
    let sx = |x: f64| px + ((x - x0) / (x1 - x0)) as f32 * pw;
    let sy = |y: f64| py + ph - ((y - y0) / (y1 - y0)) as f32 * ph;
    let sy2 = |y: f64| py + ph - ((y - y20) / (y21 - y20)) as f32 * ph;
    let small = 0.8 * size;
    // (waarde op de as, label); een leeg label is een tussenstreep
    let ticks = |lo: f64, hi: f64, log: bool, n: usize| -> Vec<(f64, String)> {
        if log {
            let t = log_ticks(lo, hi);
            // minder dan twee machten van 10 in beeld: ook 2 en 5 benoemen
            let few = t.iter().filter(|(_, major)| *major).count() < 2;
            t.into_iter()
                .map(|(v, major)| {
                    let m = 10f64.powf(v - v.floor()).round();
                    let named = major || (few && (m == 2.0 || m == 5.0));
                    (v, if named { format_number(10f64.powf(v)) } else { String::new() })
                })
                .collect()
        } else {
            nice_ticks(lo, hi, n).into_iter().map(|t| (t, format_number(t))).collect()
        }
    };
    for (t, label) in ticks(x0, x1, p.log_x, 6) {
        let x = sx(t);
        let color = if label.is_empty() { Color(238, 238, 238) } else { LIGHT };
        ops.push(Op::Line { x1: x, y1: py, x2: x, y2: py + ph, width: 0.5, color });
        let lw = fonts().width(&label, small, FontId::Main);
        ops.push(Op::Text { x: x - lw / 2.0, y: py + ph + small + 3.0, size: small, text: label, font: FontId::Main, color: GREY });
    }
    for (t, label) in ticks(y0, y1, p.log_y, 5) {
        let y = sy(t);
        let color = if label.is_empty() { Color(238, 238, 238) } else { LIGHT };
        ops.push(Op::Line { x1: px, y1: y, x2: px + pw, y2: y, width: 0.5, color });
        let lw = fonts().width(&label, small, FontId::Main);
        ops.push(Op::Text { x: px - lw - 4.0, y: y + small * 0.35, size: small, text: label, font: FontId::Main, color: GREY });
    }
    // de rechter y-as: eigen schaal, alleen streepjes en getallen
    if two_axes {
        for t in nice_ticks(y20, y21, 5) {
            let y = sy2(t);
            ops.push(Op::Line { x1: px + pw, y1: y, x2: px + pw + 3.0, y2: y, width: 0.6, color: GREY });
            ops.push(Op::Text { x: px + pw + 5.0, y: y + small * 0.35, size: small, text: format_number(t), font: FontId::Main, color: GREY });
        }
    }
    ops.retain(|o| !matches!(o, Op::Text { text, .. } if text.is_empty()));
    ops.push(Op::Rect { x: px, y: py, w: pw, h: ph, width: 0.8, color: GREY });
    let draw = |ops: &mut Vec<Op>, list: &[Vec<[f64; 2]>], first: usize, sy: &dyn Fn(f64) -> f32, (lo, hi): (f64, f64)| {
        for (k, s) in list.iter().enumerate() {
            let color = SERIES[(first + k) % SERIES.len()];
            // een NaN onderbreekt de lijn; wat buiten de grenzen valt wordt afgeknipt
            for run in s.split(|q| !q[0].is_finite() || !q[1].is_finite()).filter(|r| !r.is_empty()) {
                for seg in clip(run, x0, x1, lo, hi) {
                    let seg: Vec<(f32, f32)> = seg.iter().map(|q| (sx(q[0]), sy(q[1]))).collect();
                    if run.len() == 1 {
                        let (x, y) = seg[0];
                        ops.push(Op::Rect { x: x - 1.5, y: y - 1.5, w: 3.0, h: 3.0, width: 1.0, color });
                    } else {
                        ops.push(Op::Poly { points: seg, width: 1.3, color });
                    }
                }
            }
        }
    };
    draw(&mut ops, &series, 0, &sy, (y0, y1));
    draw(&mut ops, &series2, series.len(), &sy2, (y20, y21));
    // aslabels en legenda
    let xl = &p.x_label;
    let xw = fonts().width(xl, small, FontId::Main);
    ops.push(Op::Text { x: px + pw / 2.0 - xw / 2.0, y: h - 4.0, size: small, text: xl.clone(), font: FontId::Main, color: BLACK });
    if !p.y_label.is_empty() {
        ops.push(Op::Text { x: 2.0, y: top - 4.0, size: small, text: p.y_label.clone(), font: FontId::Main, color: BLACK });
    }
    if two_axes && !p.y2_label.is_empty() {
        let lw = fonts().width(&p.y2_label, small, FontId::Main);
        ops.push(Op::Text { x: w - lw - 2.0, y: top - 4.0, size: small, text: p.y2_label.clone(), font: FontId::Main, color: BLACK });
    }
    let labels: Vec<String> = p
        .series
        .iter()
        .map(|s| s.label.clone())
        .chain(p.series2.iter().map(|s| format!("{} (rechts)", s.label)))
        .collect();
    let mut ly = py + small + 4.0;
    let legend_w = labels.iter().map(|l| fonts().width(l, small, FontId::Main)).fold(0.0, f32::max) + 26.0;
    ops.push(Op::Fill { x: px + pw - legend_w - 4.0, y: py + 3.0, w: legend_w + 1.0, h: labels.len() as f32 * small * 1.3 + 4.0, color: Color(255, 255, 255) });
    for (k, l) in labels.iter().enumerate() {
        let color = SERIES[k % SERIES.len()];
        let lw = fonts().width(l, small, FontId::Main);
        let lx = px + pw - lw - 6.0;
        ops.push(Op::Line { x1: lx - 16.0, y1: ly - small * 0.3, x2: lx - 4.0, y2: ly - small * 0.3, width: 1.5, color });
        ops.push(Op::Text { x: lx, y: ly, size: small, text: l.clone(), font: FontId::Main, color: BLACK });
        ly += small * 1.3;
    }
    ops
}

/// Hoogtelijnen (marching squares) van een oppervlak: per niveau de lijnstukjes in (x, y).
pub fn contour_segments(s: &Surface, level: f64) -> Vec<[[f64; 2]; 2]> {
    let (nx, ny) = (s.xs.len(), s.ys.len());
    let z = |i: usize, j: usize| s.z[j * nx + i];
    let mut out = Vec::new();
    for j in 0..ny.saturating_sub(1) {
        for i in 0..nx.saturating_sub(1) {
            // hoeken tegen de klok in: (i,j) (i+1,j) (i+1,j+1) (i,j+1)
            let c = [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)];
            let v: Vec<f64> = c.iter().map(|&(a, b)| z(a, b)).collect();
            if v.iter().any(|x| !x.is_finite()) {
                continue;
            }
            // snijpunten op de vier randen
            let mut cross = Vec::new();
            for e in 0..4 {
                let (a, b) = (e, (e + 1) % 4);
                if (v[a] < level) != (v[b] < level) {
                    let t = (level - v[a]) / (v[b] - v[a]);
                    let (pa, pb) = ([s.xs[c[a].0], s.ys[c[a].1]], [s.xs[c[b].0], s.ys[c[b].1]]);
                    cross.push([pa[0] + t * (pb[0] - pa[0]), pa[1] + t * (pb[1] - pa[1])]);
                }
            }
            match cross.len() {
                2 => out.push([cross[0], cross[1]]),
                // zadelpunt: twee stukjes
                4 => {
                    out.push([cross[0], cross[1]]);
                    out.push([cross[2], cross[3]]);
                }
                _ => {}
            }
        }
    }
    out
}

/// Een oppervlak tekenen: als 3D-vlakjes gezien vanuit `view`, of als hoogtelijnen (`contour`).
pub fn surface_ops(s: &Surface, x_label: &str, w: f32, h: f32, size: f32, view: (f64, f64)) -> Vec<Op> {
    let mut ops = Vec::new();
    let small = 0.8 * size;
    let zs: Vec<f64> = s.z.iter().copied().filter(|z| z.is_finite()).collect();
    let (zmin, zmax) = (zs.iter().copied().fold(f64::INFINITY, f64::min), zs.iter().copied().fold(f64::NEG_INFINITY, f64::max));
    let zspan = if zmax > zmin { zmax - zmin } else { 1.0 };
    let (nx, ny) = (s.xs.len(), s.ys.len());
    let (xa, xb, ya, yb) = (s.xs[0], s.xs[nx - 1], s.ys[0], s.ys[ny - 1]);
    if s.contour {
        let (left, right, top, bottom) = (52.0, 60.0, 14.0, 30.0);
        let (px, py, pw, ph) = (left, top, w - left - right, h - top - bottom);
        let sx = |x: f64| px + ((x - xa) / (xb - xa)) as f32 * pw;
        let sy = |y: f64| py + ph - ((y - ya) / (yb - ya)) as f32 * ph;
        for t in nice_ticks(xa.min(xb), xa.max(xb), 6) {
            ops.push(Op::Line { x1: sx(t), y1: py, x2: sx(t), y2: py + ph, width: 0.5, color: LIGHT });
            let label = format_number(t);
            let lw = fonts().width(&label, small, FontId::Main);
            ops.push(Op::Text { x: sx(t) - lw / 2.0, y: py + ph + small + 3.0, size: small, text: label, font: FontId::Main, color: GREY });
        }
        for t in nice_ticks(ya.min(yb), ya.max(yb), 5) {
            ops.push(Op::Line { x1: px, y1: sy(t), x2: px + pw, y2: sy(t), width: 0.5, color: LIGHT });
            let label = format_number(t);
            let lw = fonts().width(&label, small, FontId::Main);
            ops.push(Op::Text { x: px - lw - 4.0, y: sy(t) + small * 0.35, size: small, text: label, font: FontId::Main, color: GREY });
        }
        ops.push(Op::Rect { x: px, y: py, w: pw, h: ph, width: 0.8, color: GREY });
        let levels: Vec<f64> = nice_ticks(zmin, zmax, 10).into_iter().filter(|l| *l > zmin && *l < zmax).collect();
        for (k, l) in levels.iter().enumerate() {
            let color = heat((l - zmin) / zspan);
            for seg in contour_segments(s, *l) {
                ops.push(Op::Line { x1: sx(seg[0][0]), y1: sy(seg[0][1]), x2: sx(seg[1][0]), y2: sy(seg[1][1]), width: 1.1, color });
            }
            // de schaal rechts: een blokje per niveau
            let ly = py + ph - (k as f32 + 1.0) * (ph / (levels.len() as f32 + 1.0));
            ops.push(Op::Fill { x: px + pw + 6.0, y: ly - 4.0, w: 8.0, h: 8.0, color });
            ops.push(Op::Text { x: px + pw + 17.0, y: ly + small * 0.35, size: small, text: format_number(*l), font: FontId::Main, color: GREY });
        }
        let xw = fonts().width(x_label, small, FontId::Main);
        ops.push(Op::Text { x: px + pw / 2.0 - xw / 2.0, y: h - 4.0, size: small, text: x_label.to_string(), font: FontId::Main, color: BLACK });
        ops.push(Op::Text { x: 2.0, y: top - 4.0, size: small, text: s.y_label.clone(), font: FontId::Main, color: BLACK });
        let zw = fonts().width(&s.z_label, small, FontId::Main);
        ops.push(Op::Text { x: w - zw - 2.0, y: top - 4.0, size: small, text: s.z_label.clone(), font: FontId::Main, color: BLACK });
        return ops;
    }
    // 3D: x, y en z naar [-1, 1], draaien om de verticale as (az) en kantelen (el)
    let (az, el) = view;
    let norm = |v: f64, a: f64, b: f64| if b != a { 2.0 * (v - a) / (b - a) - 1.0 } else { 0.0 };
    let project = |x: f64, y: f64, z: f64| -> (f64, f64, f64) {
        let (u, v, zz) = (norm(x, xa, xb), norm(y, ya, yb), 0.8 * norm(z, zmin, zmin + zspan));
        let (rx, ry) = (u * az.cos() - v * az.sin(), u * az.sin() + v * az.cos());
        // kantelen om de horizontale as: diepte en hoogte op het scherm
        let depth = ry * el.cos() - zz * el.sin();
        let up = ry * el.sin() + zz * el.cos();
        (rx, up, depth)
    };
    let (cx, cy) = (w as f64 / 2.0, (h as f64 - 14.0) / 2.0 + 4.0);
    let scale = (w as f64 / 3.2).min((h as f64 - 14.0) / 3.1);
    let screen = |(rx, up, _): (f64, f64, f64)| ((cx + rx * scale) as f32, (cy - up * scale) as f32);
    // de bodem en de verticale as
    let corners = [(xa, ya), (xb, ya), (xb, yb), (xa, yb)];
    let base: Vec<(f32, f32)> = corners.iter().map(|&(x, y)| screen(project(x, y, zmin))).collect();
    ops.push(Op::Polygon { points: base.clone(), fill: Color(246, 247, 250), width: 0.6, color: LIGHT });
    let back = corners.iter().map(|&(x, y)| project(x, y, zmin)).enumerate().max_by(|a, b| a.1 .2.total_cmp(&b.1 .2)).map(|(k, _)| k).unwrap_or(0);
    let (bx, by) = corners[back];
    let (ax0, ay0) = screen(project(bx, by, zmin));
    let (ax1, ay1) = screen(project(bx, by, zmax));
    ops.push(Op::Line { x1: ax0, y1: ay0, x2: ax1, y2: ay1, width: 0.6, color: GREY });
    // vlakjes van achter naar voor (schilder-algoritme)
    let mut quads: Vec<(f64, Vec<(f32, f32)>, Color)> = Vec::new();
    for j in 0..ny - 1 {
        for i in 0..nx - 1 {
            let c = [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)];
            let zc: Vec<f64> = c.iter().map(|&(a, b)| s.z[b * nx + a]).collect();
            if zc.iter().any(|z| !z.is_finite()) {
                continue;
            }
            let pr: Vec<(f64, f64, f64)> = c.iter().zip(&zc).map(|(&(a, b), &z)| project(s.xs[a], s.ys[b], z)).collect();
            let depth = pr.iter().map(|q| q.2).sum::<f64>() / 4.0;
            let mean = zc.iter().sum::<f64>() / 4.0;
            quads.push((depth, pr.into_iter().map(screen).collect(), heat((mean - zmin) / zspan)));
        }
    }
    quads.sort_by(|a, b| b.0.total_cmp(&a.0));
    let edge = if nx * ny <= 1600 { 0.25 } else { 0.0 };
    for (_, pts, fill) in quads {
        ops.push(Op::Polygon { points: pts, fill, width: edge, color: Color(70, 70, 80) });
    }
    // labels: x en y halverwege de randen, z bij de as
    let mid = |a: (f32, f32), b: (f32, f32)| ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    let (lx, ly) = mid(base[0], base[1]);
    ops.push(Op::Text { x: lx, y: ly + small + 4.0, size: small, text: x_label.to_string(), font: FontId::Main, color: BLACK });
    let (lx, ly) = mid(base[1], base[2]);
    ops.push(Op::Text { x: lx + 4.0, y: ly + small + 4.0, size: small, text: s.y_label.clone(), font: FontId::Main, color: BLACK });
    ops.push(Op::Text { x: ax1 + 4.0, y: ay1, size: small, text: format!("{} {}", format_number(zmax), s.z_label), font: FontId::Main, color: GREY });
    ops.push(Op::Text { x: ax0 + 4.0, y: ay0, size: small, text: format_number(zmin), font: FontId::Main, color: GREY });
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
    fn logaritmische_schaal() {
        // 10 .. 100000: vijf machten van 10, met tussenstrepen
        let t = log_ticks(1.0, 5.0);
        let major: Vec<f64> = t.iter().filter(|(_, m)| *m).map(|(v, _)| *v).collect();
        assert_eq!(major, vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        assert!(t.iter().any(|(v, m)| !m && (v - (1.0 + 2f64.log10())).abs() < 1e-12));
        // veel decaden: geen tussenstrepen en niet elke macht
        let t = log_ticks(-20.0, 20.0);
        assert!(t.iter().all(|(_, m)| *m) && t.len() < 20);
        assert_eq!(axis_value(100.0, true), 2.0);
        assert!(axis_value(0.0, true).is_nan() && axis_value(-1.0, false) == -1.0);
    }

    #[test]
    fn hoogtelijnen() {
        // z = x + y op een 3 × 3 rooster: de lijn z = 1 loopt van (1, 0) naar (0, 1)
        let s = Surface {
            xs: vec![0.0, 1.0, 2.0],
            ys: vec![0.0, 1.0, 2.0],
            z: vec![0.0, 1.0, 2.0, 1.0, 2.0, 3.0, 2.0, 3.0, 4.0],
            y_label: String::new(),
            z_label: String::new(),
            contour: true,
        };
        let segs = contour_segments(&s, 1.5);
        assert!(!segs.is_empty());
        assert!(segs.iter().flatten().all(|p| (p[0] + p[1] - 1.5).abs() < 1e-12));
        // tekenen: 3D en hoogtelijnen geven allebei iets
        assert!(surface_ops(&s, "x", 400.0, 220.0, 10.0, SURFACE_VIEW).iter().any(|o| matches!(o, Op::Line { .. })));
        let flat = Surface { contour: false, ..s };
        assert!(surface_ops(&flat, "x", 400.0, 220.0, 10.0, SURFACE_VIEW).iter().any(|o| matches!(o, Op::Polygon { .. })));
        assert!(heat(0.0) != heat(1.0));
    }

    #[test]
    fn afknippen() {
        // een lijn van (-1, 0) naar (2, 3) binnen [0, 1] × [0, 10]: van (0, 1) tot (1, 2)
        let s = clip(&[[-1.0, 0.0], [2.0, 3.0]], 0.0, 1.0, 0.0, 10.0);
        assert_eq!(s, vec![vec![[0.0, 1.0], [1.0, 2.0]]]);
        // uit beeld en weer terug: twee stukken
        let s = clip(&[[0.0, 0.0], [1.0, 5.0], [2.0, 0.0]], 0.0, 2.0, 0.0, 2.5);
        assert_eq!(s.len(), 2);
        assert!(clip(&[[5.0, 5.0]], 0.0, 1.0, 0.0, 1.0).is_empty());
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

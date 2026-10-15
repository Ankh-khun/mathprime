//! Vakken en de bouwstenen van de formulelayout.

/// Opmaak van een stuk tekst. De weergave kiest kleur en gewicht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// Namen, getallen en operatoren.
    Normal,
    /// Eenheden (Mathcad toont ze in dezelfde letter, wij in een iets andere kleur).
    Unit,
    /// Sleutelwoorden in programmablokken (`for`, `while`, ...) en het vergelijkingsteken `=`.
    Keyword,
    /// De uitkomst achter `=`.
    Result,
}

/// Eén tekenopdracht, relatief ten opzichte van de oorsprong van het vak.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// Tekst met de basislijn op `y`.
    Text { x: f32, y: f32, size: f32, text: String, style: Style },
    /// Een rechte lijn.
    Line { x1: f32, y1: f32, x2: f32, y2: f32, width: f32 },
    /// Een open polylijn.
    Poly { points: Vec<(f32, f32)>, width: f32 },
    /// Onzichtbare cursorplek (voor de formule-editor): x en de hoogte van boven tot onder.
    Mark { id: u32, x: f32, top: f32, bottom: f32 },
}

impl Item {
    fn shifted(&self, dx: f32, dy: f32) -> Item {
        match self {
            Item::Text { x, y, size, text, style } => Item::Text { x: x + dx, y: y + dy, size: *size, text: text.clone(), style: *style },
            Item::Line { x1, y1, x2, y2, width } => Item::Line { x1: x1 + dx, y1: y1 + dy, x2: x2 + dx, y2: y2 + dy, width: *width },
            Item::Poly { points, width } => Item::Poly { points: points.iter().map(|(x, y)| (x + dx, y + dy)).collect(), width: *width },
            Item::Mark { id, x, top, bottom } => Item::Mark { id: *id, x: x + dx, top: top + dy, bottom: bottom + dy },
        }
    }
}

/// Lettermaten. De interface en de export geven elk hun eigen implementatie (met hetzelfde lettertype).
pub trait Metrics {
    /// Breedte van `text` bij lettergrootte `size`.
    fn width(&self, text: &str, size: f32) -> f32;
    /// Hoogte boven de basislijn als fractie van de lettergrootte.
    fn ascent(&self) -> f32 {
        0.78
    }
    /// Diepte onder de basislijn als fractie van de lettergrootte.
    fn descent(&self) -> f32 {
        0.22
    }
}

/// Een vak: breedte, hoogte boven (`ascent`) en onder (`descent`) de basislijn, en de inhoud.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MBox {
    pub width: f32,
    pub ascent: f32,
    pub descent: f32,
    pub items: Vec<Item>,
}

impl MBox {
    pub fn height(&self) -> f32 {
        self.ascent + self.descent
    }

    /// Voeg `other` toe met zijn oorsprong op (dx, dy).
    pub fn place(&mut self, other: &MBox, dx: f32, dy: f32) {
        self.items.extend(other.items.iter().map(|i| i.shifted(dx, dy)));
    }

    /// Lege ruimte.
    pub fn space(width: f32) -> MBox {
        MBox { width, ..MBox::default() }
    }
}

/// Afmetingen in fracties van de lettergrootte.
pub const AXIS: f32 = 0.27; // hoogte van de breukstreep (as van de formule)
pub const SCRIPT: f32 = 0.7; // verkleining voor machten en indices
pub const MIN_SIZE: f32 = 6.0;
pub const RULE: f32 = 0.055; // lijndikte

pub fn script_size(size: f32) -> f32 {
    (size * SCRIPT).max(MIN_SIZE)
}

pub fn text(m: &dyn Metrics, s: &str, size: f32, style: Style) -> MBox {
    MBox {
        width: m.width(s, size),
        ascent: m.ascent() * size,
        descent: m.descent() * size,
        items: vec![Item::Text { x: 0.0, y: 0.0, size, text: s.to_string(), style }],
    }
}

/// Vakken naast elkaar op dezelfde basislijn.
pub fn row(parts: Vec<MBox>) -> MBox {
    let mut out = MBox::default();
    for p in parts {
        out.place(&p, out.width, 0.0);
        out.width += p.width;
        out.ascent = out.ascent.max(p.ascent);
        out.descent = out.descent.max(p.descent);
    }
    out
}

/// Een breuk: teller boven, noemer onder een streep op de hoogte van de as.
pub fn frac(num: MBox, den: MBox, size: f32) -> MBox {
    let gap = 0.14 * size;
    let pad = 0.12 * size;
    let axis = AXIS * size;
    let w = num.width.max(den.width) + 2.0 * pad;
    let mut out = MBox {
        width: w,
        ascent: axis + gap + num.descent + num.ascent,
        descent: -axis + gap + den.ascent + den.descent,
        items: vec![Item::Line { x1: 0.0, y1: -axis, x2: w, y2: -axis, width: RULE * size }],
    };
    out.place(&num, (w - num.width) / 2.0, -axis - gap - num.descent);
    out.place(&den, (w - den.width) / 2.0, -axis + gap + den.ascent);
    out.descent = out.descent.max(0.0);
    out
}

/// Macht: `sup` rechtsboven `base`.
pub fn sup(base: MBox, exp: MBox, size: f32) -> MBox {
    let shift = (0.42 * size).max(base.ascent - 0.5 * exp.ascent);
    let mut out = base.clone();
    out.place(&exp, base.width + 0.04 * size, -shift);
    out.width = base.width + 0.04 * size + exp.width;
    out.ascent = base.ascent.max(shift + exp.ascent);
    out
}

/// Index: `sub` rechtsonder `base`.
pub fn sub(base: MBox, idx: MBox, size: f32) -> MBox {
    let shift = (0.22 * size).max(base.descent + 0.3 * idx.ascent - 0.1 * size);
    let mut out = base.clone();
    out.place(&idx, base.width + 0.02 * size, shift);
    out.width = base.width + 0.02 * size + idx.width;
    out.descent = base.descent.max(shift + idx.descent);
    out
}

/// Wortelteken met streep over `inner`, eventueel met een kleine wortelexponent.
pub fn radical(inner: MBox, index: Option<MBox>, size: f32) -> MBox {
    let gap = 0.12 * size;
    let top = inner.ascent + gap;
    let bottom = inner.descent.max(0.1 * size);
    let sign_w = 0.55 * size;
    let lead = index.as_ref().map_or(0.0, |i| (i.width - 0.3 * size).max(0.0));
    let x0 = lead;
    let lw = RULE * size;
    let pts = vec![
        (x0, -0.35 * size),
        (x0 + 0.15 * size, -0.42 * size),
        (x0 + 0.3 * size, bottom),
        (x0 + sign_w, -top),
        (x0 + sign_w + inner.width + 0.1 * size, -top),
    ];
    let mut out = MBox {
        width: x0 + sign_w + inner.width + 0.15 * size,
        ascent: top + lw,
        descent: bottom,
        items: vec![Item::Poly { points: pts, width: lw }],
    };
    out.place(&inner, x0 + sign_w + 0.05 * size, 0.0);
    if let Some(i) = index {
        let y = -0.45 * size - i.descent;
        out.place(&i, 0.0, y);
        out.ascent = out.ascent.max(-y + i.ascent);
    }
    out
}

/// Haken om `inner`: `(`, `[` of `|`. De haken groeien mee met de inhoud.
pub fn fence(inner: MBox, kind: char, size: f32) -> MBox {
    let top = inner.ascent.max(0.75 * size) + 0.05 * size;
    let bottom = inner.descent.max(0.2 * size) + 0.05 * size;
    let h = top + bottom;
    let w = match kind {
        '|' => 0.25 * size,
        _ => (0.28 + 0.05 * (h / size - 1.0).max(0.0)).min(0.5) * size,
    };
    let lw = RULE * size * 1.1;
    let side = |x0: f32, left: bool| -> Item {
        let dir = if left { 1.0 } else { -1.0 };
        match kind {
            '(' => {
                let n = 12;
                let pts = (0..=n)
                    .map(|k| {
                        let t = k as f32 / n as f32;
                        let y = -top + h * t;
                        let bulge = (1.0 - (2.0 * t - 1.0).powi(2)).sqrt();
                        (x0 + dir * (0.75 * w * (1.0 - bulge) + 0.1 * w), y)
                    })
                    .collect();
                Item::Poly { points: pts, width: lw }
            }
            '[' => {
                let inner_x = x0 + dir * 0.55 * w;
                Item::Poly {
                    points: vec![(inner_x, -top), (x0 + dir * 0.15 * w, -top), (x0 + dir * 0.15 * w, bottom), (inner_x, bottom)],
                    width: lw,
                }
            }
            _ => Item::Line { x1: x0 + dir * 0.4 * w, y1: -top, x2: x0 + dir * 0.4 * w, y2: bottom, width: lw },
        }
    };
    let mut out = MBox {
        width: 2.0 * w + inner.width,
        ascent: top,
        descent: bottom,
        items: vec![side(0.0, true), side(2.0 * w + inner.width, false)],
    };
    out.place(&inner, w, 0.0);
    out
}

/// Een raster van vakken (rij voor rij) tussen rechte haken, gecentreerd op de as.
pub fn matrix(cells: Vec<Vec<MBox>>, size: f32) -> MBox {
    let rows = cells.len();
    let cols = cells.first().map_or(0, |r| r.len());
    let col_gap = 0.9 * size;
    let row_gap = 0.25 * size;
    let col_w: Vec<f32> = (0..cols).map(|c| cells.iter().map(|r| r[c].width).fold(0.0, f32::max)).collect();
    let row_a: Vec<f32> = cells.iter().map(|r| r.iter().map(|b| b.ascent).fold(0.0, f32::max)).collect();
    let row_d: Vec<f32> = cells.iter().map(|r| r.iter().map(|b| b.descent).fold(0.0, f32::max)).collect();
    let height: f32 = row_a.iter().sum::<f32>() + row_d.iter().sum::<f32>() + row_gap * (rows.saturating_sub(1)) as f32;
    let width: f32 = col_w.iter().sum::<f32>() + col_gap * (cols.saturating_sub(1)) as f32;
    let top = AXIS * size + height / 2.0;
    let mut grid = MBox {
        width,
        ascent: top,
        descent: height - top,
        items: Vec::new(),
    };
    let mut y = -top;
    for (r, row_cells) in cells.iter().enumerate() {
        y += row_a[r];
        let mut x = 0.0;
        for (c, cell) in row_cells.iter().enumerate() {
            // getallen rechts uitlijnen, zoals in een tabel
            grid.place(cell, x + col_w[c] - cell.width, y);
            x += col_w[c] + col_gap;
        }
        y += row_d[r] + row_gap;
    }
    grid.descent = grid.descent.max(0.0);
    fence(grid, '[', size)
}

/// Soort groot teken.
#[derive(Clone, Copy)]
pub enum BigOp {
    Sum,
    Prod,
    Integral,
}

/// Σ, ∏ of ∫ als lijntekening, met grenzen erboven en eronder (Σ, ∏) of rechts (∫).
pub fn bigop(op: BigOp, lower: Option<MBox>, upper: Option<MBox>, size: f32) -> MBox {
    let lw = RULE * size * 1.2;
    let axis = AXIS * size;
    let (h, w) = match op {
        BigOp::Integral => (1.9 * size, 0.55 * size),
        _ => (1.25 * size, 0.95 * size),
    };
    let top = axis + h / 2.0;
    let bot = h / 2.0 - axis;
    let sign = match op {
        BigOp::Sum => Item::Poly {
            points: vec![(w, -top + 0.15 * size), (w, -top), (0.0, -top), (0.5 * w, -axis), (0.0, bot), (w, bot), (w, bot - 0.15 * size)],
            width: lw,
        },
        BigOp::Prod => Item::Poly {
            points: vec![(0.0, bot), (0.12 * w, bot), (0.12 * w, -top), (0.88 * w, -top), (0.88 * w, bot), (w, bot)],
            width: lw,
        },
        BigOp::Integral => {
            let n = 16;
            let pts = (0..=n)
                .map(|k| {
                    let t = k as f32 / n as f32;
                    let y = -top + h * t;
                    // schuine S-vorm met krullen aan de uiteinden
                    let x = 0.5 * w + 0.22 * w * (1.0 - 2.0 * t) + 0.25 * w * ((t * std::f32::consts::PI * 2.0).sin()) * (1.0 - (2.0 * t - 1.0).abs()).powf(4.0);
                    (x, y)
                })
                .collect();
            Item::Poly { points: pts, width: lw }
        }
    };
    let mut out = MBox {
        width: w,
        ascent: top,
        descent: bot,
        items: vec![sign],
    };
    match op {
        BigOp::Integral => {
            let mut extra: f32 = 0.0;
            if let Some(u) = upper {
                out.place(&u, w + 0.05 * size, -top + u.ascent);
                extra = extra.max(u.width);
            }
            if let Some(l) = lower {
                out.place(&l, w - 0.15 * size, bot + l.ascent * 0.6);
                out.descent = out.descent.max(bot + l.ascent * 0.6 + l.descent);
                extra = extra.max(l.width - 0.2 * size);
            }
            out.width = w + extra + 0.1 * size;
        }
        _ => {
            let wide = [lower.as_ref(), upper.as_ref()].iter().flatten().map(|b| b.width).fold(w, f32::max);
            let shift = (wide - w) / 2.0;
            let mut centred = MBox { width: wide, ascent: top, descent: bot, items: Vec::new() };
            centred.place(&out, shift, 0.0);
            if let Some(u) = upper {
                let y = -top - 0.1 * size - u.descent;
                centred.place(&u, (wide - u.width) / 2.0, y);
                centred.ascent = centred.ascent.max(-y + u.ascent);
            }
            if let Some(l) = lower {
                let y = bot + 0.1 * size + l.ascent;
                centred.place(&l, (wide - l.width) / 2.0, y);
                centred.descent = centred.descent.max(y + l.descent);
            }
            out = centred;
        }
    }
    out
}

/// Regels onder elkaar (programmablok), met een verticale streep links zoals in Mathcad.
/// De basislijn van het geheel ligt bij de middelste regel.
pub fn program(lines: Vec<MBox>, size: f32) -> MBox {
    let gap = 0.3 * size;
    let indent = 0.45 * size;
    let total: f32 = lines.iter().map(|l| l.height()).sum::<f32>() + gap * lines.len().saturating_sub(1) as f32;
    let top = AXIS * size + total / 2.0;
    let mut out = MBox {
        width: 0.0,
        ascent: top,
        descent: (total - top).max(0.0),
        items: Vec::new(),
    };
    let mut y = -top;
    for l in &lines {
        y += l.ascent;
        out.place(l, indent, y);
        out.width = out.width.max(indent + l.width);
        y += l.descent + gap;
    }
    out.items.push(Item::Line {
        x1: 0.12 * size,
        y1: -top,
        x2: 0.12 * size,
        y2: total - top,
        width: RULE * size * 1.2,
    });
    out
}

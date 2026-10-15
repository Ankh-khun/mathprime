//! Opmaak van de bewerkbare formule, met cursorplekken.
//!
//! Gebruikt de bouwstenen van `rb-layout`, zodat een formule tijdens het bewerken er hetzelfde uitziet
//! als daarna. Tussen alle atomen komt een onzichtbare [`Item::Mark`]; [`EditLayout::caret`] zoekt de
//! plek van de cursor op en [`EditLayout::hit`] de cursorpositie bij een muisklik.

use crate::tree::{Atom, Big, Block, Ctrl, Cursor, Editor, Kw, Op};
use rb_layout::geom::{bigop, fence, frac, matrix, program, radical, row, script_size, text, BigOp, RULE};
use rb_layout::{Item, MBox, Metrics, Style};

pub struct EditLayout {
    pub mbox: MBox,
    /// Cursorposities per markering (index = id van de markering).
    pub marks: Vec<Cursor>,
}

/// Een cursorplek: x en de verticale uitgestrektheid (relatief aan de oorsprong van het vak).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Caret {
    pub x: f32,
    pub top: f32,
    pub bottom: f32,
}

impl EditLayout {
    fn mark(&self, c: &Cursor) -> Option<Caret> {
        let id = self.marks.iter().position(|m| m == c)? as u32;
        self.mbox.items.iter().find_map(|i| match i {
            Item::Mark { id: mid, x, top, bottom } if *mid == id => Some(Caret { x: *x, top: *top, bottom: *bottom }),
            _ => None,
        })
    }

    /// De plek van de cursor.
    pub fn caret(&self, ed: &Editor) -> Option<Caret> {
        self.mark(&ed.cursor)
    }

    /// Het gemarkeerde gebied van de selectie: (x0, x1, boven, onder).
    pub fn selection(&self, ed: &Editor) -> Option<(f32, f32, f32, f32)> {
        let (a, b) = ed.selection?;
        let at = |pos| self.mark(&Cursor { path: ed.cursor.path.clone(), pos });
        let (ca, cb) = (at(a)?, at(b)?);
        Some((ca.x, cb.x, ca.top.min(cb.top), ca.bottom.max(cb.bottom)))
    }

    /// De cursorpositie die het dichtst bij punt (x, y) ligt.
    pub fn hit(&self, x: f32, y: f32) -> Option<Cursor> {
        let mut best: Option<(f32, u32)> = None;
        for i in &self.mbox.items {
            if let Item::Mark { id, x: mx, top, bottom } = i {
                // verticaal: afstand tot het bereik; voorkeur voor het kleinste omringende blok
                let dy = if y < *top { top - y } else if y > *bottom { y - bottom } else { 0.0 };
                let d = (mx - x).abs() + 3.0 * dy + 0.01 * (bottom - top);
                if best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, *id));
                }
            }
        }
        best.map(|(_, id)| self.marks[id as usize].clone())
    }
}

struct L<'a> {
    m: &'a dyn Metrics,
    marks: Vec<Cursor>,
    result: Option<MBox>,
}

fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '\'')
}

impl L<'_> {
    fn mark(&mut self, path: &[(usize, usize)], pos: usize) -> MBox {
        let id = self.marks.len() as u32;
        self.marks.push(Cursor { path: path.to_vec(), pos });
        MBox {
            width: 0.0,
            ascent: 0.0,
            descent: 0.0,
            items: vec![Item::Mark { id, x: 0.0, top: 0.0, bottom: 0.0 }],
        }
    }

    fn txt(&self, s: &str, size: f32, style: Style) -> MBox {
        text(self.m, s, size, style)
    }

    fn op(&self, op: Op, size: f32, unary: bool) -> MBox {
        let style = if op == Op::BoolEq { Style::Keyword } else { Style::Normal };
        let t = self.txt(op.symbol(), size, style);
        let pad = 0.22 * size;
        match op {
            Op::Comma => row(vec![t, MBox::space(pad)]),
            Op::Factorial => t,
            Op::Not => row(vec![t, MBox::space(0.05 * size)]),
            Op::Minus if unary => t,
            _ => row(vec![MBox::space(pad), t, MBox::space(pad)]),
        }
    }

    /// Een leeg blok: het zwarte vierkantje van Mathcad (als omlijning).
    fn placeholder(&self, size: f32) -> MBox {
        let (w, h) = (0.42 * size, 0.55 * size);
        let x0 = 0.06 * size;
        MBox {
            width: w + 2.0 * x0,
            ascent: h + 0.05 * size,
            descent: 0.02 * size,
            items: vec![Item::Poly {
                points: vec![(x0, 0.0), (x0 + w, 0.0), (x0 + w, -h), (x0, -h), (x0, 0.0)],
                width: RULE * size,
            }],
        }
    }

    fn block(&mut self, b: &Block, path: &mut Vec<(usize, usize)>, size: f32) -> MBox {
        let first_mark = self.marks.len() as u32;
        let mut parts: Vec<MBox> = Vec::new();
        if b.is_empty() {
            parts.push(self.mark(path, 0));
            parts.push(self.placeholder(size));
        } else {
            let mut in_name = false;
            let mut subscript = false;
            for (k, a) in b.atoms.iter().enumerate() {
                parts.push(self.mark(path, k));
                // naam met letterlijk subscript: `M.max` → M met index max (de punt is onzichtbaar)
                if let Atom::Char(c) = a {
                    if *c == '.' && in_name && matches!(b.atoms.get(k + 1), Some(Atom::Char(n)) if n.is_alphanumeric()) {
                        subscript = true;
                        continue;
                    }
                    if !in_name {
                        in_name = c.is_alphabetic() || matches!(c, '∞' | '°');
                    }
                    if !is_name_char(*c) && *c != '.' && *c != '∞' && *c != '°' {
                        in_name = false;
                        subscript = false;
                    }
                    let s = if subscript { script_size(size) } else { size };
                    let mut t = self.txt(&c.to_string(), s, Style::Normal);
                    if subscript {
                        let drop = 0.22 * size;
                        t = MBox { width: t.width, ascent: (t.ascent - drop).max(0.0), descent: t.descent + drop, items: shifted(&t.items, 0.0, drop) };
                    }
                    parts.push(t);
                    continue;
                }
                in_name = false;
                subscript = false;
                path.push((k, 0));
                let part = match a {
                    Atom::Char(_) => unreachable!(),
                    // tekst: letterlijk, met een cursorplek tussen elk teken
                    Atom::Str(inner) => {
                        let mut parts = vec![self.txt("\u{201C}", size, Style::Unit)];
                        for (k2, a2) in inner.atoms.iter().enumerate() {
                            parts.push(self.mark(path, k2));
                            if let Atom::Char(c) = a2 {
                                parts.push(self.txt(&c.to_string(), size, Style::Unit));
                            }
                        }
                        parts.push(self.mark(path, inner.len()));
                        path.pop();
                        parts.push(self.txt("\u{201D}", size, Style::Unit));
                        row(parts)
                    }
                    Atom::Op(op) => {
                        path.pop();
                        let unary = *op == Op::Minus && (k == 0 || matches!(b.atoms[k - 1], Atom::Op(_)));
                        let mut p = self.op(*op, size, unary);
                        // het resultaat staat direct achter de = van een resultaatregel
                        if *op == Op::Eval && path.is_empty() {
                            if let Some(r) = self.result.take() {
                                p = row(vec![p, r, MBox::space(0.25 * size)]);
                            }
                        }
                        p
                    }
                    Atom::Kw(kw) => {
                        path.pop();
                        let w = match kw {
                            Kw::Break => "break",
                            Kw::Continue => "continue",
                            Kw::Return => "return",
                        };
                        row(vec![self.txt(w, size, Style::Keyword), MBox::space(0.25 * size)])
                    }
                    Atom::Frac { num, den } => {
                        let n = self.block(num, path, size);
                        path.last_mut().unwrap().1 = 1;
                        let d = self.block(den, path, size);
                        path.pop();
                        frac(n, d, size)
                    }
                    Atom::Pow(e) | Atom::Index(e) => {
                        let small = script_size(size);
                        let inner = self.block(e, path, small);
                        path.pop();
                        let base_ascent = parts.iter().rev().find(|p| p.width > 0.0).map_or(0.75 * size, |p| p.ascent);
                        let base_descent = parts.iter().rev().find(|p| p.width > 0.0).map_or(0.2 * size, |p| p.descent);
                        if matches!(a, Atom::Pow(_)) {
                            let shift = (0.42 * size).max(base_ascent - 0.5 * inner.ascent);
                            raised(&inner, -shift, 0.04 * size)
                        } else {
                            let shift = (0.22 * size).max(base_descent + 0.3 * inner.ascent - 0.1 * size);
                            raised(&inner, shift, 0.02 * size)
                        }
                    }
                    Atom::Sqrt(x) => {
                        let inner = self.block(x, path, size);
                        path.pop();
                        radical(inner, None, size)
                    }
                    Atom::Root { index, radicand } => {
                        let i = self.block(index, path, script_size(size));
                        path.last_mut().unwrap().1 = 1;
                        let r = self.block(radicand, path, size);
                        path.pop();
                        radical(r, Some(i), size)
                    }
                    Atom::Paren(x) | Atom::Abs(x) => {
                        let inner = self.block(x, path, size);
                        path.pop();
                        fence(inner, if matches!(a, Atom::Paren(_)) { '(' } else { '|' }, size)
                    }
                    Atom::Matrix { rows, cols, cells } => {
                        let mut grid = Vec::new();
                        for r in 0..*rows {
                            let mut line = Vec::new();
                            for c in 0..*cols {
                                path.last_mut().unwrap().1 = r * cols + c;
                                line.push(self.block(&cells[r * cols + c], path, size));
                            }
                            grid.push(line);
                        }
                        path.pop();
                        matrix(grid, size)
                    }
                    Atom::Big { kind, slots } => {
                        let small = script_size(size);
                        let sb = |l: &mut Self, s: usize, sz: f32, path: &mut Vec<(usize, usize)>| {
                            path.last_mut().unwrap().1 = s;
                            l.block(&slots[s], path, sz)
                        };
                        let out = match kind {
                            Big::Sum | Big::Prod => {
                                let var = sb(self, 0, small, path);
                                let from = sb(self, 1, small, path);
                                let lower = row(vec![var, self.txt("=", small, Style::Normal), from]);
                                let upper = sb(self, 2, small, path);
                                let body = sb(self, 3, size, path);
                                let k = if *kind == Big::Sum { BigOp::Sum } else { BigOp::Prod };
                                row(vec![bigop(k, Some(lower), Some(upper), size), MBox::space(0.15 * size), body])
                            }
                            Big::Integral => {
                                let lo = sb(self, 0, small, path);
                                let hi = sb(self, 1, small, path);
                                let body = sb(self, 2, size, path);
                                let var = sb(self, 3, size, path);
                                row(vec![
                                    bigop(BigOp::Integral, Some(lo), Some(hi), size),
                                    MBox::space(0.1 * size),
                                    body,
                                    MBox::space(0.2 * size),
                                    self.txt("d", size, Style::Normal),
                                    var,
                                ])
                            }
                            Big::Deriv => {
                                let var = sb(self, 0, size, path);
                                let body = sb(self, 1, size, path);
                                let order = if slots[2].is_empty() {
                                    // lege orde: onzichtbaar (eerste afgeleide), wel bereikbaar met de pijltjes
                                    path.last_mut().unwrap().1 = 2;
                                    let m = self.mark(path, 0);
                                    Some(m)
                                } else {
                                    Some(sb(self, 2, small, path))
                                };
                                let d_top = row(vec![self.txt("d", size, Style::Normal), raised(&order.unwrap(), -0.42 * size, 0.04 * size)]);
                                let d_bottom = row(vec![self.txt("d", size, Style::Normal), var]);
                                row(vec![frac(d_top, d_bottom, size), MBox::space(0.1 * size), body])
                            }
                        };
                        path.pop();
                        out
                    }
                    Atom::Program(lines) => {
                        let mut boxes = Vec::new();
                        for (n, line) in lines.iter().enumerate() {
                            path.last_mut().unwrap().1 = n;
                            boxes.push(self.block(line, path, size));
                        }
                        path.pop();
                        program(boxes, size)
                    }
                    Atom::Ctrl { kind, head, body } => {
                        let mut header = vec![self.txt(kind.keyword(), size, Style::Keyword), MBox::space(0.3 * size)];
                        for (h, hb) in head.iter().enumerate() {
                            path.last_mut().unwrap().1 = h;
                            if *kind == Ctrl::For && h == 1 {
                                header.push(row(vec![MBox::space(0.2 * size), self.txt("∈", size, Style::Normal), MBox::space(0.2 * size)]));
                            }
                            header.push(self.block(hb, path, size));
                        }
                        let mut lines = Vec::new();
                        for (n, line) in body.iter().enumerate() {
                            path.last_mut().unwrap().1 = head.len() + n;
                            lines.push(self.block(line, path, size));
                        }
                        path.pop();
                        let head_box = row(header);
                        let body_box = program(lines, size);
                        below(head_box, body_box, 0.6 * size, 0.2 * size)
                    }
                };
                parts.push(part);
            }
            parts.push(self.mark(path, b.len()));
            // na een operator zonder rechterkant: het invulvakje van Mathcad (`x := ▪`, `a + ▪`)
            if matches!(b.atoms.last(), Some(Atom::Op(op)) if *op != Op::Factorial) {
                parts.push(self.placeholder(size));
            }
        }
        let mut out = row(parts);
        out.ascent = out.ascent.max(0.75 * size);
        out.descent = out.descent.max(0.22 * size);
        // de markeringen van dit blok krijgen de hoogte van het hele blok
        let last_mark = self.marks.len() as u32;
        for it in &mut out.items {
            if let Item::Mark { id, top, bottom, .. } = it {
                if *id >= first_mark && *id < last_mark && *top == 0.0 && *bottom == 0.0 {
                    *top = -out.ascent;
                    *bottom = out.descent;
                }
            }
        }
        out
    }
}

fn shifted(items: &[Item], dx: f32, dy: f32) -> Vec<Item> {
    let mut b = MBox::default();
    b.place(&MBox { items: items.to_vec(), ..MBox::default() }, dx, dy);
    b.items
}

/// `inner` verschoven over dy (negatief = omhoog), met een kleine ruimte ervoor.
fn raised(inner: &MBox, dy: f32, gap: f32) -> MBox {
    MBox {
        width: inner.width + gap,
        ascent: (inner.ascent - dy).max(0.0),
        descent: (inner.descent + dy).max(0.0),
        items: shifted(&inner.items, gap, dy),
    }
}

/// `body` onder `head`, ingesprongen; de basislijn is die van `head`.
fn below(head: MBox, body: MBox, indent: f32, gap: f32) -> MBox {
    let y = head.descent + gap + body.ascent;
    let mut out = MBox {
        width: head.width.max(indent + body.width),
        ascent: head.ascent,
        descent: y + body.descent,
        items: Vec::new(),
    };
    out.place(&head, 0.0, 0.0);
    out.place(&body, indent, y);
    out
}

/// Maak de opmaak van een editor. `result`: het berekende resultaat, dat achter de `=` komt.
pub fn layout(ed: &Editor, m: &dyn Metrics, size: f32, result: Option<MBox>) -> EditLayout {
    let mut l = L { m, marks: Vec::new(), result };
    let mut mbox = l.block(&ed.root, &mut Vec::new(), size);
    // symbolisch: de uitkomst komt achter de pijl en het sleutelwoord
    if let Some(r) = l.result.take().filter(|_| ed.root.atoms.contains(&Atom::Op(Op::Arrow))) {
        mbox = row(vec![mbox, MBox::space(0.3 * size), r]);
    }
    EditLayout { mbox, marks: l.marks }
}

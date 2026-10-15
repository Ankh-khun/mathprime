//! Van syntaxboom naar vakken.

use crate::geom::*;
use rb_core::{matrix_cells, parse_expr_str, render_value, scalar_parts, BStmt, BinOp, BindOp, CmpOp, Display, Expr, Stmt, Value};

/// Instellingen voor de formuleweergave.
#[derive(Debug, Clone, Copy)]
pub struct LayoutOptions {
    /// Lettergrootte van de formule (pixels of punten).
    pub size: f32,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        LayoutOptions { size: 16.0 }
    }
}

struct L<'a> {
    m: &'a dyn Metrics,
    /// Eenheden opmaken: delen als `/` op één regel en namen in de eenheidsstijl.
    unit_mode: bool,
}

/// Voorrang voor het plaatsen van haakjes (hoger bindt sterker).
fn prec(e: &Expr) -> u8 {
    match e {
        Expr::Range { .. } => 0,
        Expr::Or(..) => 1,
        Expr::And(..) => 2,
        Expr::Not(_) => 3,
        Expr::Cmp(..) => 4,
        Expr::Bin(BinOp::Add | BinOp::Sub, ..) => 5,
        Expr::Bin(BinOp::Mul, ..) => 6,
        Expr::Bin(BinOp::Div, ..) => 6,
        Expr::Neg(_) => 7,
        Expr::Bin(BinOp::Pow, ..) => 8,
        Expr::Bind { .. } => 6,
        Expr::Num(v) if *v != 0.0 && (v.abs() >= 1e6 || v.abs() < 1e-4) => 6,
        _ => 9,
    }
}

/// Een 'atoom' heeft nooit haakjes nodig als grondtal van een macht.
fn is_atom(e: &Expr) -> bool {
    matches!(e, Expr::Var(_) | Expr::Call(..) | Expr::Index(..) | Expr::Matrix(_) | Expr::Interp(..)) || (matches!(e, Expr::Num(_)) && prec(e) == 9)
}

/// Eindigt de weergave van `e` op een getal (zoals `F · 2`)?
fn ends_with_number(e: &Expr) -> bool {
    match e {
        Expr::Num(_) => true,
        Expr::Bin(BinOp::Mul, _, b) => ends_with_number(b),
        _ => false,
    }
}

fn cmp_symbol(op: CmpOp) -> &'static str {
    match op {
        CmpOp::Lt => "<",
        CmpOp::Le => "≤",
        CmpOp::Gt => ">",
        CmpOp::Ge => "≥",
        CmpOp::Eq => "=",
        CmpOp::Ne => "≠",
    }
}

impl L<'_> {
    fn text(&self, s: &str, size: f32) -> MBox {
        text(self.m, s, size, if self.unit_mode { Style::Unit } else { Style::Normal })
    }

    fn styled(&self, s: &str, size: f32, style: Style) -> MBox {
        text(self.m, s, size, style)
    }

    /// Een operator met wat ruimte eromheen.
    fn op(&self, s: &str, size: f32) -> MBox {
        let pad = if self.unit_mode { 0.0 } else { 0.22 * size };
        row(vec![MBox::space(pad), self.text(s, size), MBox::space(pad)])
    }

    fn paren_if(&self, e: &Expr, cond: bool, size: f32) -> MBox {
        let b = self.expr(e, size);
        if cond {
            fence(b, '(', size)
        } else {
            b
        }
    }

    fn number(&self, v: f64, size: f32) -> MBox {
        if v != 0.0 && (v.abs() >= 1e6 || v.abs() < 1e-4) {
            let s = format!("{v:e}");
            let (mant, exp) = s.split_once('e').unwrap_or((&s, "0"));
            let base = if mant == "1" { "10".to_string() } else { format!("{mant}·10") };
            return sup(self.text(&base, size), self.text(exp, script_size(size)), size);
        }
        self.text(&format!("{v}"), size)
    }

    /// Een naam; `M.max` wordt M met index max, zoals een letterlijk subscript in Mathcad.
    fn name(&self, n: &str, size: f32) -> MBox {
        match n.split_once('.') {
            Some((base, idx)) if !base.is_empty() && !idx.is_empty() => sub(self.text(base, size), self.text(idx, script_size(size)), size),
            _ => self.text(n, size),
        }
    }

    fn list(&self, items: &[Expr], size: f32) -> MBox {
        let mut parts = Vec::new();
        for (k, a) in items.iter().enumerate() {
            if k > 0 {
                parts.push(self.text(",", size));
                parts.push(MBox::space(0.25 * size));
            }
            parts.push(self.expr(a, size));
        }
        if parts.is_empty() {
            parts.push(MBox::space(0.1 * size));
        }
        row(parts)
    }

    fn call(&self, name: &str, args: &[Expr], size: f32) -> MBox {
        match (name, args) {
            ("sqrt", [x]) => radical(self.expr(x, size), None, size),
            ("nthroot" | "root", [x, n]) => radical(self.expr(x, size), Some(self.expr(n, script_size(size))), size),
            ("abs", [x]) => fence(self.expr(x, size), '|', size),
            ("factorial", [x]) => row(vec![self.paren_if(x, !is_atom(x), size), self.text("!", size)]),
            _ => row(vec![self.name(name, size), fence(self.list(args, size), '(', size)]),
        }
    }

    pub(crate) fn expr(&self, e: &Expr, size: f32) -> MBox {
        match e {
            Expr::Num(v) => self.number(*v, size),
            Expr::Imag(v) => self.text(&format!("{v}i"), size),
            // tekst tussen aanhalingstekens, zoals in Mathcad
            Expr::Str(s) => self.styled(&format!("\u{201C}{s}\u{201D}"), size, Style::Unit),
            Expr::Var(n) => self.name(n, size),
            Expr::Neg(x) => row(vec![self.text("−", size), self.paren_if(x, prec(x) < 7, size)]),
            Expr::Not(x) => row(vec![self.text("¬", size), self.paren_if(x, prec(x) < 4, size)]),
            Expr::Bin(op, a, b) => self.bin(*op, a, b, size),
            Expr::Cmp(ops, xs) => {
                let mut parts = vec![self.paren_if(&xs[0], prec(&xs[0]) <= 4, size)];
                for (k, op) in ops.iter().enumerate() {
                    let pad = 0.25 * size;
                    let style = if *op == CmpOp::Eq { Style::Keyword } else { Style::Normal };
                    parts.push(row(vec![MBox::space(pad), self.styled(cmp_symbol(*op), size, style), MBox::space(pad)]));
                    parts.push(self.paren_if(&xs[k + 1], prec(&xs[k + 1]) <= 4, size));
                }
                row(parts)
            }
            Expr::And(a, b) | Expr::Or(a, b) => {
                let (sym, p) = if matches!(e, Expr::And(..)) { ("∧", 2) } else { ("∨", 1) };
                row(vec![self.paren_if(a, prec(a) < p, size), self.op(sym, size), self.paren_if(b, prec(b) <= p, size)])
            }
            Expr::Call(name, args) => self.call(name, args, size),
            Expr::Index(base, idx) => sub(self.paren_if(base, !is_atom(base), size), self.list(idx, script_size(size)), size),
            Expr::Matrix(rows) => matrix(rows.iter().map(|r| r.iter().map(|x| self.expr(x, size)).collect()).collect(), size),
            Expr::Bind { op, body, var, args } => self.bind(*op, body, var, args, size),
            Expr::Range { start, second, end } => {
                let mut parts = vec![self.expr(start, size)];
                if let Some(s) = second {
                    parts.push(self.text(",", size));
                    parts.push(MBox::space(0.25 * size));
                    parts.push(self.expr(s, size));
                }
                parts.push(self.op("..", size));
                parts.push(self.expr(end, size));
                row(parts)
            }
            Expr::Block(stmts) => program(self.block(stmts, size), size),
            Expr::Interp(_, arg) => row(vec![self.text("tabel", size), fence(self.expr(arg, size), '(', size)]),
        }
    }

    fn bin(&self, op: BinOp, a: &Expr, b: &Expr, size: f32) -> MBox {
        match op {
            BinOp::Add | BinOp::Sub => {
                let sym = if op == BinOp::Add { "+" } else { "−" };
                let right_parens = if op == BinOp::Sub { prec(b) <= 5 } else { prec(b) < 5 } || matches!(b, Expr::Neg(_));
                row(vec![self.paren_if(a, prec(a) < 5, size), self.op(sym, size), self.paren_if(b, right_parens, size)])
            }
            BinOp::Mul => {
                let left_number = matches!(a, Expr::Num(_) | Expr::Imag(_) | Expr::Matrix(_)) || matches!(a, Expr::Neg(x) if matches!(**x, Expr::Num(_)));
                let right_named = matches!(b, Expr::Var(_) | Expr::Call(..) | Expr::Index(..) | Expr::Matrix(_))
                    || matches!(b, Expr::Bin(BinOp::Pow, base, _) if matches!(**base, Expr::Var(_)));
                // `F.1 * 2 m`: een eenheid direct achter een getal krijgt ook een spatie
                let unit_after_number = matches!(b, Expr::Var(n) if rb_units::lookup_unit(n).is_some()) && ends_with_number(a);
                let joiner = if self.unit_mode {
                    self.text("·", size)
                } else if (left_number && right_named) || unit_after_number {
                    MBox::space(0.18 * size)
                } else {
                    self.op("·", size)
                };
                row(vec![
                    self.paren_if(a, prec(a) < 6, size),
                    joiner,
                    self.paren_if(b, prec(b) < 6 || matches!(b, Expr::Neg(_)), size),
                ])
            }
            BinOp::Div => {
                if self.unit_mode {
                    row(vec![
                        self.paren_if(a, prec(a) < 6, size),
                        self.text("/", size),
                        self.paren_if(b, prec(b) <= 6, size),
                    ])
                } else {
                    frac(self.expr(a, size), self.expr(b, size), size)
                }
            }
            BinOp::Pow => sup(self.paren_if(a, !is_atom(a), size), self.expr(b, script_size(size)), size),
        }
    }

    fn bind(&self, op: BindOp, body: &Expr, var: &str, args: &[Expr], size: f32) -> MBox {
        let small = script_size(size);
        let body_box = |cond: bool| self.paren_if(body, cond, size);
        match op {
            BindOp::Sum | BindOp::Prod => {
                let lower = row(vec![self.name(var, small), self.text("=", small), self.expr(&args[0], small)]);
                let upper = self.expr(&args[1], small);
                let kind = if op == BindOp::Sum { BigOp::Sum } else { BigOp::Prod };
                row(vec![bigop(kind, Some(lower), Some(upper), size), MBox::space(0.15 * size), body_box(prec(body) < 6)])
            }
            BindOp::Integral => row(vec![
                bigop(BigOp::Integral, Some(self.expr(&args[0], small)), Some(self.expr(&args[1], small)), size),
                MBox::space(0.1 * size),
                body_box(prec(body) < 6),
                MBox::space(0.2 * size),
                self.text("d", size),
                self.name(var, size),
            ]),
            BindOp::Deriv => {
                let order = match args.get(1) {
                    Some(Expr::Num(n)) if *n > 1.0 => Some(*n),
                    _ => None,
                };
                let with_order = |b: MBox| match order {
                    Some(n) => sup(b, self.text(&format!("{n}"), small), size),
                    None => b,
                };
                let num = with_order(self.text("d", size));
                let den = with_order(row(vec![self.text("d", size), self.name(var, size)]));
                let core = row(vec![frac(num, den, size), MBox::space(0.1 * size), body_box(prec(body) < 6)]);
                // d/dx f(x) in het punt x zelf (symbolisch): zonder streep, zoals in Mathcad
                if matches!(&args[0], Expr::Var(v) if v == var) {
                    return core;
                }
                // evaluatiestreep met het punt: |x = a
                let bar_h = (core.ascent, core.descent);
                let bar = MBox {
                    width: 0.25 * size,
                    ascent: bar_h.0,
                    descent: bar_h.1,
                    items: vec![Item::Line { x1: 0.12 * size, y1: -bar_h.0, x2: 0.12 * size, y2: bar_h.1, width: RULE * size }],
                };
                let at = row(vec![self.name(var, small), self.text("=", small), self.expr(&args[0], small)]);
                row(vec![core, sub(bar, at, size)])
            }
            BindOp::Root => {
                let mut items = vec![body.clone(), Expr::Var(var.to_string())];
                items.extend(args.iter().cloned());
                row(vec![self.text("root", size), fence(self.list(&items, size), '(', size)])
            }
            BindOp::OdeSolve => {
                let mut items = vec![Expr::Var(var.to_string())];
                items.extend(args.iter().cloned());
                row(vec![self.text("odesolve", size), fence(self.list(&items, size), '(', size)])
            }
        }
    }

    fn keyword(&self, s: &str, size: f32) -> MBox {
        row(vec![self.styled(s, size, Style::Keyword), MBox::space(0.3 * size)])
    }

    /// De regels van een programmablok; geneste blokken springen in onder hun kop.
    fn block(&self, stmts: &[BStmt], size: f32) -> Vec<MBox> {
        let nested = |body: &[BStmt]| program(self.block(body, size), size);
        let mut out = Vec::new();
        for s in stmts {
            match s {
                BStmt::Assign { name, index, value } => {
                    let lhs = match index {
                        Some(idx) => sub(self.name(name, size), self.list(idx, script_size(size)), size),
                        None => self.name(name, size),
                    };
                    out.push(row(vec![lhs, self.op("←", size), self.expr(value, size)]));
                }
                BStmt::If { branches, els } => {
                    for (k, (cond, body)) in branches.iter().enumerate() {
                        let kw = if k == 0 { "if" } else { "else if" };
                        out.push(row(vec![self.keyword(kw, size), self.expr(cond, size)]));
                        out.push(row(vec![MBox::space(0.6 * size), nested(body)]));
                    }
                    if let Some(body) = els {
                        out.push(self.keyword("else", size));
                        out.push(row(vec![MBox::space(0.6 * size), nested(body)]));
                    }
                }
                BStmt::For { var, iter, body } => {
                    out.push(row(vec![self.keyword("for", size), self.name(var, size), self.op("∈", size), self.expr(iter, size)]));
                    out.push(row(vec![MBox::space(0.6 * size), nested(body)]));
                }
                BStmt::While { cond, body } => {
                    out.push(row(vec![self.keyword("while", size), self.expr(cond, size)]));
                    out.push(row(vec![MBox::space(0.6 * size), nested(body)]));
                }
                BStmt::Break => out.push(self.keyword("break", size)),
                BStmt::Continue => out.push(self.keyword("continue", size)),
                BStmt::Return(e) => out.push(row(vec![self.keyword("return", size), self.expr(e, size)])),
                BStmt::Expr(e) => out.push(self.expr(e, size)),
            }
        }
        out
    }
}

/// Een eenheid (`kN·m`, `m/s^2`, `(N/mm)^2`) op één regel; terugval op gewone tekst.
fn unit_box(m: &dyn Metrics, unit: &str, size: f32) -> MBox {
    let l = L { m, unit_mode: true };
    match parse_expr_str(unit) {
        Ok(e) => l.expr(&e, size),
        Err(_) => text(m, unit, size, Style::Unit),
    }
}

/// Getal plus eenheid.
fn with_unit(m: &dyn Metrics, number: MBox, unit: &str, size: f32) -> MBox {
    if unit.is_empty() {
        number
    } else {
        row(vec![number, MBox::space(0.25 * size), unit_box(m, unit, size)])
    }
}

/// Een getal uit de uitkomst zoals Mathcad het toont: 3.6e7 als 3.6·10⁷, -2 met een echt minteken.
fn result_number(m: &dyn Metrics, s: &str, size: f32) -> MBox {
    let s = s.replacen('-', "−", 1);
    if let Some((mant, exp)) = s.split_once('e') {
        let digits = |x: &str| !x.is_empty() && x.trim_start_matches(['−', '-', '+']).chars().all(|c| c.is_ascii_digit() || c == '.');
        if digits(mant) && digits(exp) {
            let exp = exp.trim_start_matches('+').replace('-', "−");
            let base = if mant == "1" { "10".to_string() } else { format!("{mant}·10") };
            return sup(text(m, &base, size, Style::Result), text(m, &exp, script_size(size), Style::Result), size);
        }
    }
    text(m, &s, size, Style::Result)
}

/// Het resultaat van een resultaatregel als vak (getal met eenheid, matrix, ...).
pub fn value_box(m: &dyn Metrics, value: &Value, display: &Option<Display>, opts: LayoutOptions) -> MBox {
    let size = opts.size;
    if let Value::Str(s) = value {
        return text(m, &format!("\u{201C}{s}\u{201D}"), size, Style::Result);
    }
    if let Ok(Some((num, unit))) = scalar_parts(value, display) {
        return with_unit(m, result_number(m, &num, size), &unit, size);
    }
    if let Value::Matrix(mat) = value {
        if let Ok(cells) = matrix_cells(mat, display) {
            let grid = (0..cells.rows)
                .map(|r| (0..cells.cols).map(|c| result_number(m, &cells.cells[r * cells.cols + c], size)).collect())
                .collect();
            return with_unit(m, matrix(grid, size), &cells.unit, size);
        }
    }
    let t = render_value(value, display).unwrap_or_default();
    text(m, &t, size, Style::Result)
}

/// Een regel als formule. `result`: de uitkomst van een resultaatregel (als die er is).
pub fn stmt_box(m: &dyn Metrics, stmt: &Stmt, result: Option<MBox>, opts: LayoutOptions) -> MBox {
    let size = opts.size;
    let l = L { m, unit_mode: false };
    match stmt {
        Stmt::Define { name, params, index, body, show } => {
            let lhs = match (params, index) {
                (Some(p), _) => {
                    let args: Vec<Expr> = p.iter().map(|n| Expr::Var(n.clone())).collect();
                    row(vec![l.name(name, size), fence(l.list(&args, size), '(', size)])
                }
                (None, Some(idx)) => sub(l.name(name, size), l.list(idx, script_size(size)), size),
                (None, None) => l.name(name, size),
            };
            let mut parts = vec![lhs, l.op(":=", size), l.expr(body, size)];
            // `a := ... = waarde`, zoals in Mathcad
            if show.is_some() {
                parts.push(l.op("=", size));
                if let Some(r) = result {
                    parts.push(r);
                }
            }
            row(parts)
        }
        Stmt::Eval { expr, .. } => {
            let mut parts = vec![l.expr(expr, size), l.op("=", size)];
            if let Some(r) = result {
                parts.push(r);
            }
            row(parts)
        }
        // `uitdrukking sleutelwoord → uitkomst`, het sleutelwoord klein vóór de pijl (zoals Mathcad)
        Stmt::Symbolic { expr, op, .. } => {
            let mut parts = vec![l.expr(expr, size)];
            let k = op.keyword();
            if !k.is_empty() {
                parts.push(MBox::space(0.3 * size));
                parts.push(text(m, &k, script_size(size), Style::Keyword));
            }
            parts.push(l.op("→", size));
            if let Some(r) = result {
                parts.push(r);
            }
            row(parts)
        }
    }
}

/// Een losse uitdrukking als formule (bijvoorbeeld een beperking in een Given-blok).
pub fn expr_box(m: &dyn Metrics, e: &Expr, opts: LayoutOptions) -> MBox {
    L { m, unit_mode: false }.expr(e, opts.size)
}

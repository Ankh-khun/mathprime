//! Van structuurboom naar de tekstsyntax van een `.rkn`-regel, en terug (vanuit de geparseerde regel).

use crate::tree::{Atom, Big, Block, Ctrl, Kw, Op};
use rb_core::{parse_stmt, BStmt, BinOp, BindOp, CmpOp, Expr, Stmt, SymOp};

// ---------------------------------------------------------------------------------------------
// Boom → tekst

/// Is dit blok één eenvoudig woord (naam of getal), zodat er geen haakjes omheen hoeven?
fn simple(b: &Block) -> bool {
    !b.is_empty() && b.atoms.iter().all(|a| matches!(a, Atom::Char(c) if c.is_alphanumeric() || matches!(c, '.' | '_' | '\'' | '∞' | '°')))
}

fn group(b: &Block) -> String {
    let t = block_text(b);
    // `5kN` wordt `5 kN`: een product, dus wel haakjes
    if simple(b) && !t.contains(' ') {
        t
    } else {
        format!("({t})")
    }
}

fn op_text(op: Op, first: bool) -> &'static str {
    match op {
        Op::Define => " := ",
        Op::Eval => " = ",
        Op::Plus => " + ",
        Op::Minus if first => "-",
        Op::Minus => " - ",
        Op::Times => " * ",
        Op::Comma => ", ",
        Op::Lt => " < ",
        Op::Gt => " > ",
        Op::Le => " <= ",
        Op::Ge => " >= ",
        Op::Ne => " != ",
        Op::BoolEq => " == ",
        Op::And => " and ",
        Op::Or => " or ",
        Op::Not => "not ",
        Op::Assign => " := ",
        Op::Factorial => "!",
        Op::Arrow => " → ",
    }
}

/// Eén blok als tekst (zonder programmaregels; die maakt [`region_text`]).
/// De inhoud van een tekstatoom, letterlijk (zonder de spaties die `block_text` tussen getal en naam zet).
pub fn str_content(b: &Block) -> String {
    b.atoms.iter().filter_map(|a| if let Atom::Char(c) = a { Some(*c) } else { None }).collect()
}

pub fn block_text(b: &Block) -> String {
    let mut s = String::new();
    for (k, a) in b.atoms.iter().enumerate() {
        let first = k == 0 || matches!(b.atoms[k - 1], Atom::Op(o) if o != Op::Factorial);
        match a {
            Atom::Char(c) => {
                if needs_space(b, k) {
                    s.push(' ');
                }
                s.push(*c);
            }
            Atom::Op(op) if exponent_sign(b, k) => s.push(if *op == Op::Minus { '-' } else { '+' }),
            Atom::Op(op) => s.push_str(op_text(*op, first)),
            Atom::Frac { num, den } => {
                s.push_str(&group(num));
                s.push('/');
                s.push_str(&group(den));
            }
            Atom::Pow(e) => {
                s.push('^');
                s.push_str(&group(e));
            }
            Atom::Index(i) => s.push_str(&format!("[{}]", block_text(i))),
            Atom::Str(t) => {
                if needs_space(b, k) {
                    s.push(' ');
                }
                s.push('"');
                s.push_str(&str_content(t).replace('"', "\"\""));
                s.push('"');
            }
            Atom::Sqrt(x) => s.push_str(&format!("sqrt({})", block_text(x))),
            Atom::Root { index, radicand } => s.push_str(&format!("nthroot({}, {})", block_text(radicand), block_text(index))),
            Atom::Paren(x) => s.push_str(&format!("({})", block_text(x))),
            Atom::Abs(x) => s.push_str(&format!("abs({})", block_text(x))),
            Atom::Matrix { rows, cols, cells } => {
                let rows_t: Vec<String> = (0..*rows)
                    .map(|r| (0..*cols).map(|c| block_text(&cells[r * cols + c])).collect::<Vec<_>>().join(", "))
                    .collect();
                s.push_str(&format!("[{}]", rows_t.join("; ")));
            }
            Atom::Big { kind, slots } => {
                let t: Vec<String> = slots.iter().map(block_text).collect();
                s.push_str(&match kind {
                    Big::Sum => format!("sum({}, {}, {}, {})", t[3], t[0], t[1], t[2]),
                    Big::Prod => format!("prod({}, {}, {}, {})", t[3], t[0], t[1], t[2]),
                    Big::Integral => format!("integral({}, {}, {}, {})", t[2], t[3], t[0], t[1]),
                    // zoals Mathcad: de afgeleide in de huidige waarde van de variabele
                    Big::Deriv => {
                        if slots[2].is_empty() {
                            format!("deriv({}, {}, {})", t[1], t[0], t[0])
                        } else {
                            format!("deriv({}, {}, {}, {})", t[1], t[0], t[0], t[2])
                        }
                    }
                });
            }
            // een programma midden in een regel kan niet in tekst; region_text behandelt het als rechterkant
            Atom::Program(_) | Atom::Ctrl { .. } => s.push_str(" {programma} "),
            Atom::Kw(Kw::Break) => s.push_str("break"),
            Atom::Kw(Kw::Continue) => s.push_str("continue"),
            Atom::Kw(Kw::Return) => s.push_str("return "),
        }
    }
    s.trim().to_string()
}

fn char_at(b: &Block, k: usize) -> Option<char> {
    match b.atoms.get(k) {
        Some(Atom::Char(c)) => Some(*c),
        _ => None,
    }
}

/// Is het atoom op `k` het teken van een exponent, zoals de `-` in `1e-3`?
fn exponent_sign(b: &Block, k: usize) -> bool {
    matches!(b.atoms.get(k), Some(Atom::Op(Op::Minus | Op::Plus)))
        && k >= 2
        && matches!(char_at(b, k - 1), Some('e' | 'E'))
        && char_at(b, k - 2).is_some_and(|c| c.is_ascii_digit() || c == '.')
        && char_at(b, k + 1).is_some_and(|c| c.is_ascii_digit())
}

/// Een getal direct gevolgd door een naam (`5kN`, `2x`) wordt een product: daar hoort een spatie,
/// behalve bij een exponent (`1e3`) en een imaginair getal (`2i`, `1j`).
fn needs_space(b: &Block, k: usize) -> bool {
    let Some(c) = char_at(b, k) else { return false };
    if !c.is_alphabetic() || k == 0 {
        return false;
    }
    let is_num = |ch: char| ch.is_ascii_digit() || ch == '.';
    if !char_at(b, k - 1).is_some_and(is_num) {
        return false;
    }
    // `1..n`: na een bereikoperator begint gewoon een naam
    if k >= 2 && char_at(b, k - 1) == Some('.') && char_at(b, k - 2) == Some('.') {
        return false;
    }
    // begin van de reeks cijfers: hoort die bij een naam (zoals `x2a`), dan is het één naam
    let mut j = k;
    while j > 0 && char_at(b, j - 1).is_some_and(is_num) {
        j -= 1;
    }
    if j > 0 && char_at(b, j - 1).is_some_and(|ch| ch.is_alphanumeric() || ch == '_') {
        return false;
    }
    let next = char_at(b, k + 1);
    if matches!(c, 'e' | 'E') && (next.is_some_and(|n| n.is_ascii_digit()) || exponent_sign(b, k + 1)) {
        return false;
    }
    if matches!(c, 'i' | 'j') && !next.is_some_and(|n| n.is_alphanumeric()) {
        return false;
    }
    true
}

/// De regels van een programma, ingesprongen.
fn program_lines(lines: &[Block], indent: usize, out: &mut Vec<String>) {
    let pad = "    ".repeat(indent);
    for line in lines {
        if line.is_empty() {
            continue;
        }
        match line.atoms.first() {
            Some(Atom::Ctrl { kind, head, body }) => {
                let head_t = match kind {
                    Ctrl::For => format!("for {} in {}", block_text(&head[0]), block_text(&head[1])),
                    Ctrl::Else => "else".to_string(),
                    k => format!("{} {}", k.keyword(), block_text(&head[0])),
                };
                out.push(format!("{pad}{head_t}"));
                program_lines(body, indent + 1, out);
            }
            _ => out.push(format!("{pad}{}", block_text(line))),
        }
    }
}

/// De hele regio als tekst (één regel, of meerdere bij een programma).
pub fn region_text(root: &Block) -> String {
    if let Some(k) = root.atoms.iter().position(|a| matches!(a, Atom::Program(_))) {
        if let Atom::Program(lines) = &root.atoms[k] {
            let head = Block::new(root.atoms[..k].to_vec());
            let mut out = vec![format!("{} {{", block_text(&head))];
            program_lines(lines, 1, &mut out);
            out.push("}".to_string());
            return out.join("\n");
        }
    }
    block_text(root)
}

// ---------------------------------------------------------------------------------------------
// Tekst → boom

fn chars(s: &str) -> Vec<Atom> {
    s.chars().map(Atom::Char).collect()
}

fn prec(e: &Expr) -> u8 {
    match e {
        Expr::Range { .. } => 0,
        Expr::Or(..) => 1,
        Expr::And(..) => 2,
        Expr::Not(_) => 3,
        Expr::Cmp(..) => 4,
        Expr::Bin(BinOp::Add | BinOp::Sub, ..) => 5,
        Expr::Bin(BinOp::Mul, ..) => 6,
        Expr::Neg(_) => 7,
        Expr::Bin(BinOp::Pow, ..) => 8,
        _ => 9,
    }
}

fn number(v: f64) -> String {
    let s = format!("{v}");
    if s.len() > 12 && (v.abs() >= 1e6 || (v != 0.0 && v.abs() < 1e-4)) {
        format!("{v:e}")
    } else {
        s
    }
}

struct Builder;

impl Builder {
    fn paren_if(&self, e: &Expr, cond: bool, out: &mut Vec<Atom>) {
        if cond {
            out.push(Atom::Paren(self.block(e)));
        } else {
            self.expr(e, out);
        }
    }

    fn block(&self, e: &Expr) -> Block {
        let mut v = Vec::new();
        self.expr(e, &mut v);
        Block::new(v)
    }

    fn list(&self, items: &[Expr]) -> Block {
        let mut v = Vec::new();
        for (k, a) in items.iter().enumerate() {
            if k > 0 {
                v.push(Atom::Op(Op::Comma));
            }
            self.expr(a, &mut v);
        }
        Block::new(v)
    }

    fn expr(&self, e: &Expr, out: &mut Vec<Atom>) {
        match e {
            Expr::Num(v) => out.extend(chars(&number(*v))),
            Expr::Imag(v) => out.extend(chars(&format!("{}i", number(*v)))),
            Expr::Str(s) => out.push(Atom::Str(Block::new(s.chars().map(Atom::Char).collect()))),
            Expr::Var(n) => out.extend(chars(n)),
            Expr::Neg(x) => {
                out.push(Atom::Op(Op::Minus));
                self.paren_if(x, prec(x) < 7, out);
            }
            Expr::Not(x) => {
                out.push(Atom::Op(Op::Not));
                self.paren_if(x, prec(x) < 4, out);
            }
            Expr::Bin(op, a, b) => match op {
                BinOp::Add | BinOp::Sub => {
                    self.paren_if(a, prec(a) < 5, out);
                    out.push(Atom::Op(if *op == BinOp::Add { Op::Plus } else { Op::Minus }));
                    let right = if *op == BinOp::Sub { prec(b) <= 5 } else { prec(b) < 5 } || matches!(**b, Expr::Neg(_));
                    self.paren_if(b, right, out);
                }
                BinOp::Mul => {
                    self.paren_if(a, prec(a) < 6, out);
                    // `5 kN` blijft zonder ·, zoals Mathcad een getal met eenheid toont
                    let juxt = matches!(**a, Expr::Num(_)) && matches!(**b, Expr::Var(_));
                    if !juxt {
                        out.push(Atom::Op(Op::Times));
                    }
                    self.paren_if(b, prec(b) < 6 || matches!(**b, Expr::Neg(_)), out);
                }
                BinOp::Div => out.push(Atom::Frac { num: self.block(a), den: self.block(b) }),
                BinOp::Pow => {
                    let atom = matches!(**a, Expr::Var(_) | Expr::Call(..) | Expr::Index(..) | Expr::Matrix(_))
                        || (matches!(**a, Expr::Num(_)) && prec(a) == 9);
                    self.paren_if(a, !atom, out);
                    out.push(Atom::Pow(self.block(b)));
                }
            },
            Expr::Cmp(ops, xs) => {
                self.paren_if(&xs[0], prec(&xs[0]) <= 4, out);
                for (k, op) in ops.iter().enumerate() {
                    out.push(Atom::Op(match op {
                        CmpOp::Lt => Op::Lt,
                        CmpOp::Le => Op::Le,
                        CmpOp::Gt => Op::Gt,
                        CmpOp::Ge => Op::Ge,
                        CmpOp::Eq => Op::BoolEq,
                        CmpOp::Ne => Op::Ne,
                    }));
                    self.paren_if(&xs[k + 1], prec(&xs[k + 1]) <= 4, out);
                }
            }
            Expr::And(a, b) | Expr::Or(a, b) => {
                let (op, p) = if matches!(e, Expr::And(..)) { (Op::And, 2) } else { (Op::Or, 1) };
                self.paren_if(a, prec(a) < p, out);
                out.push(Atom::Op(op));
                self.paren_if(b, prec(b) <= p, out);
            }
            Expr::Call(name, args) => match (name.as_str(), args.as_slice()) {
                ("sqrt", [x]) => out.push(Atom::Sqrt(self.block(x))),
                ("nthroot", [x, n]) => out.push(Atom::Root { index: self.block(n), radicand: self.block(x) }),
                ("abs", [x]) => out.push(Atom::Abs(self.block(x))),
                ("factorial", [x]) => {
                    self.paren_if(x, prec(x) < 9, out);
                    out.push(Atom::Op(Op::Factorial));
                }
                _ => {
                    out.extend(chars(name));
                    out.push(Atom::Paren(self.list(args)));
                }
            },
            Expr::Index(base, idx) => {
                self.paren_if(base, prec(base) < 9, out);
                out.push(Atom::Index(self.list(idx)));
            }
            Expr::Matrix(rows) => {
                let cols = rows.first().map_or(0, |r| r.len());
                out.push(Atom::Matrix {
                    rows: rows.len(),
                    cols,
                    cells: rows.iter().flatten().map(|x| self.block(x)).collect(),
                });
            }
            Expr::Bind { op, body, var, args } => {
                let v = Block::new(chars(var));
                match op {
                    BindOp::Sum | BindOp::Prod => out.push(Atom::Big {
                        kind: if *op == BindOp::Sum { Big::Sum } else { Big::Prod },
                        slots: vec![v, self.block(&args[0]), self.block(&args[1]), self.block(body)],
                    }),
                    BindOp::Integral => out.push(Atom::Big {
                        kind: Big::Integral,
                        slots: vec![self.block(&args[0]), self.block(&args[1]), self.block(body), v],
                    }),
                    BindOp::Deriv => {
                        let order = args.get(1).map(|o| self.block(o)).unwrap_or_default();
                        // d/dx in het punt x zelf is de Mathcad-vorm; een ander punt als deriv(...)
                        if matches!(&args[0], Expr::Var(n) if n == var) {
                            out.push(Atom::Big { kind: Big::Deriv, slots: vec![v, self.block(body), order] });
                        } else {
                            out.extend(chars("deriv"));
                            let mut items = vec![(**body).clone(), Expr::Var(var.clone())];
                            items.extend(args.iter().cloned());
                            out.push(Atom::Paren(self.list(&items)));
                        }
                    }
                    BindOp::Root | BindOp::OdeSolve => {
                        let name = if *op == BindOp::Root { "root" } else { "odesolve" };
                        out.extend(chars(name));
                        // root(f, x, ..); odesolve(t, b) of bij een stelsel odesolve([x; y], t, b)
                        let mut items = if *op == BindOp::Root || matches!(**body, Expr::Matrix(_)) {
                            vec![(**body).clone(), Expr::Var(var.clone())]
                        } else {
                            vec![Expr::Var(var.clone())]
                        };
                        items.extend(args.iter().cloned());
                        out.push(Atom::Paren(self.list(&items)));
                    }
                }
            }
            Expr::Range { start, second, end } => {
                self.expr(start, out);
                if let Some(s) = second {
                    out.push(Atom::Op(Op::Comma));
                    self.expr(s, out);
                }
                out.extend(chars(".."));
                self.expr(end, out);
            }
            Expr::Block(stmts) => out.push(Atom::Program(self.program(stmts))),
            Expr::Interp(_, arg) => {
                out.extend(chars("tabel"));
                out.push(Atom::Paren(self.block(arg)));
            }
        }
    }

    fn program(&self, stmts: &[BStmt]) -> Vec<Block> {
        let mut lines = Vec::new();
        for s in stmts {
            match s {
                BStmt::Assign { name, index, value } => {
                    let mut v = chars(name);
                    if let Some(idx) = index {
                        v.push(Atom::Index(self.list(idx)));
                    }
                    v.push(Atom::Op(Op::Assign));
                    self.expr(value, &mut v);
                    lines.push(Block::new(v));
                }
                BStmt::If { branches, els } => {
                    for (k, (c, body)) in branches.iter().enumerate() {
                        let kind = if k == 0 { Ctrl::If } else { Ctrl::ElseIf };
                        lines.push(Block::new(vec![Atom::Ctrl { kind, head: vec![self.block(c)], body: self.program(body) }]));
                    }
                    if let Some(body) = els {
                        lines.push(Block::new(vec![Atom::Ctrl { kind: Ctrl::Else, head: vec![], body: self.program(body) }]));
                    }
                }
                BStmt::For { var, iter, body } => lines.push(Block::new(vec![Atom::Ctrl {
                    kind: Ctrl::For,
                    head: vec![Block::new(chars(var)), self.block(iter)],
                    body: self.program(body),
                }])),
                BStmt::While { cond, body } => lines.push(Block::new(vec![Atom::Ctrl {
                    kind: Ctrl::While,
                    head: vec![self.block(cond)],
                    body: self.program(body),
                }])),
                BStmt::Break => lines.push(Block::new(vec![Atom::Kw(Kw::Break)])),
                BStmt::Continue => lines.push(Block::new(vec![Atom::Kw(Kw::Continue)])),
                BStmt::Return(e) => {
                    let mut v = vec![Atom::Kw(Kw::Return)];
                    self.expr(e, &mut v);
                    lines.push(Block::new(v));
                }
                BStmt::Expr(e) => lines.push(self.block(e)),
            }
        }
        lines
    }
}

/// Zet de tekst van een regio (een `.rkn`-regel of programmablok) om in een bewerkbare boom.
/// `None` als de tekst niet te lezen is (de interface laat dan de tekst zelf bewerken).
pub fn from_text(src: &str) -> Option<Block> {
    if let Ok(stmt) = parse_stmt(src, false) {
        return Some(from_stmt(&stmt));
    }
    // een losse uitdrukking, zoals een beperking in een Given-blok (`x + y == 10`) of `Given`
    let e = rb_core::parse_expr_str(src).ok()?;
    let mut v = Vec::new();
    Builder.expr(&e, &mut v);
    Some(Block::new(v))
}

/// Een uitdrukking als tekst in de schrijfwijze van de editor (`(q * L^2)/8`).
pub fn expr_text(e: &Expr) -> String {
    let mut v = Vec::new();
    Builder.expr(e, &mut v);
    block_text(&Block::new(v))
}

pub fn from_stmt(stmt: &Stmt) -> Block {
    let b = Builder;
    let mut v = Vec::new();
    match stmt {
        Stmt::Define { name, params, index, body, show } => {
            // een stelsel: [x; y] := odesolve([x; y], t, b)
            match stmt.ode_system() {
                Some(names) => b.expr(&Expr::Matrix(names.into_iter().map(|n| vec![Expr::Var(n)]).collect()), &mut v),
                None => v.extend(chars(name)),
            }
            if let Some(p) = params {
                let args: Vec<Expr> = p.iter().map(|n| Expr::Var(n.clone())).collect();
                v.push(Atom::Paren(b.list(&args)));
            }
            if let Some(idx) = index {
                v.push(Atom::Index(b.list(idx)));
            }
            v.push(Atom::Op(Op::Define));
            b.expr(body, &mut v);
            if let Some(s) = show {
                v.push(Atom::Op(Op::Eval));
                if let Some((u, _)) = &s.unit {
                    b.expr(u, &mut v);
                }
            }
        }
        Stmt::Eval { expr, unit, .. } => {
            b.expr(expr, &mut v);
            v.push(Atom::Op(Op::Eval));
            if let Some((u, _)) = unit {
                b.expr(u, &mut v);
            }
        }
        // het sleutelwoord achter de pijl (`→ solve, x`, `→ substitute, x = 2`)
        Stmt::Symbolic { expr, op, .. } => {
            b.expr(expr, &mut v);
            v.push(Atom::Op(Op::Arrow));
            match op {
                SymOp::Evaluate => {}
                SymOp::Simplify | SymOp::Expand | SymOp::Factor => v.extend(chars(&op.keyword())),
                SymOp::Solve(x) => {
                    v.extend(chars("solve"));
                    v.push(Atom::Op(Op::Comma));
                    v.extend(chars(x));
                }
                SymOp::Substitute(x, val) => {
                    v.extend(chars("substitute"));
                    v.push(Atom::Op(Op::Comma));
                    v.extend(chars(x));
                    v.push(Atom::Op(Op::BoolEq));
                    b.expr(val, &mut v);
                }
                SymOp::Limit(x, a) | SymOp::Series(x, a, _) => {
                    v.extend(chars(if matches!(op, SymOp::Limit(..)) { "limit" } else { "series" }));
                    v.push(Atom::Op(Op::Comma));
                    v.extend(chars(x));
                    v.push(Atom::Op(Op::BoolEq));
                    b.expr(a, &mut v);
                    if let SymOp::Series(_, _, n) = op {
                        v.push(Atom::Op(Op::Comma));
                        v.extend(chars(&n.to_string()));
                    }
                }
                SymOp::Float(n) => {
                    v.extend(chars("float"));
                    v.push(Atom::Op(Op::Comma));
                    v.extend(chars(&n.to_string()));
                }
            }
        }
    }
    Block::new(v)
}

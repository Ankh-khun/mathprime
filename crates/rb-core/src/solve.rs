//! Een vergelijking omwerken naar één onbekende, zoals je het op papier doet: de bewerkingen rond
//! de onbekende één voor één naar de andere kant brengen (+ ↔ −, · ↔ /, xⁿ ↔ ⁿ√, sin ↔ asin,
//! ln ↔ exp, ...). Dat kan als de onbekende precies één keer in de vergelijking staat; anders geeft
//! [`isolate`] `None` en moet er numeriek opgelost worden (een solve block).

use crate::ast::{BinOp, CmpOp, Expr, Stmt};

/// Hoe vaak `var` in `e` voorkomt.
pub fn count(e: &Expr, var: &str) -> usize {
    match e {
        Expr::Var(n) => usize::from(n == var),
        Expr::Num(_) | Expr::Imag(_) | Expr::Str(_) => 0,
        Expr::Neg(x) | Expr::Not(x) => count(x, var),
        Expr::Bin(_, a, b) | Expr::And(a, b) | Expr::Or(a, b) => count(a, var) + count(b, var),
        Expr::Cmp(_, xs) => xs.iter().map(|x| count(x, var)).sum(),
        Expr::Call(_, args) => args.iter().map(|x| count(x, var)).sum(),
        Expr::Index(b, idx) => count(b, var) + idx.iter().map(|x| count(x, var)).sum::<usize>(),
        Expr::Matrix(rows) => rows.iter().flatten().map(|x| count(x, var)).sum(),
        // gebonden variabelen, bereiken en blokken: niet omwerkbaar; tel als "meer dan één"
        _ => {
            if e.names().contains(var) {
                2
            } else {
                0
            }
        }
    }
}

/// De twee kanten van een vergelijking: `a == b` (ook in een definitie `a := b`).
pub fn sides(stmt: &Stmt) -> Option<(Expr, Expr)> {
    match stmt {
        Stmt::Define { name, params: None, index: None, body, .. } => Some((Expr::Var(name.clone()), body.clone())),
        _ => None,
    }
}

/// De twee kanten van een vergelijking als uitdrukking (`a == b`).
pub fn expr_sides(e: &Expr) -> Option<(Expr, Expr)> {
    match e {
        Expr::Cmp(ops, xs) if ops.as_slice() == [CmpOp::Eq] && xs.len() == 2 => Some((xs[0].clone(), xs[1].clone())),
        _ => None,
    }
}

/// De namen in een vergelijking die als onbekende in aanmerking komen (geen functies).
pub fn unknowns(lhs: &Expr, rhs: &Expr) -> Vec<String> {
    let mut names = Vec::new();
    fn walk(e: &Expr, out: &mut Vec<String>) {
        match e {
            Expr::Var(n) => {
                if !out.contains(n) {
                    out.push(n.clone());
                }
            }
            Expr::Neg(x) | Expr::Not(x) => walk(x, out),
            Expr::Bin(_, a, b) | Expr::And(a, b) | Expr::Or(a, b) => {
                walk(a, out);
                walk(b, out);
            }
            Expr::Cmp(_, xs) | Expr::Call(_, xs) => xs.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    walk(lhs, &mut names);
    walk(rhs, &mut names);
    names
}

fn bin(op: BinOp, a: Expr, b: Expr) -> Expr {
    Expr::Bin(op, Box::new(a), Box::new(b))
}

fn call(f: &str, args: Vec<Expr>) -> Expr {
    Expr::Call(f.to_string(), args)
}

fn neg(e: Expr) -> Expr {
    match e {
        Expr::Neg(x) => *x,
        Expr::Num(v) => Expr::Num(-v),
        x => Expr::Neg(Box::new(x)),
    }
}

/// `b` tot de macht `1/n`: als wortel geschreven waar dat kan.
fn root(b: Expr, n: Expr) -> Expr {
    match n {
        Expr::Num(v) if v == 2.0 => call("sqrt", vec![b]),
        Expr::Num(v) if v.fract() == 0.0 && v > 2.0 => call("nthroot", vec![b, Expr::Num(v)]),
        Expr::Num(v) if v == 1.0 => b,
        n => bin(BinOp::Pow, b, bin(BinOp::Div, Expr::Num(1.0), n)),
    }
}

/// Werk `lhs == rhs` om naar `var` (die er precies één keer in moet staan). Geeft de uitdrukking
/// voor `var`.
pub fn isolate(lhs: &Expr, rhs: &Expr, var: &str) -> Option<Expr> {
    let (mut a, mut b) = match (count(lhs, var), count(rhs, var)) {
        (1, 0) => (lhs.clone(), rhs.clone()),
        (0, 1) => (rhs.clone(), lhs.clone()),
        _ => return None,
    };
    // `a` bevat de onbekende, `b` niet; houd a == b waar
    for _ in 0..200 {
        a = match a {
            Expr::Var(ref n) if n == var => return Some(b),
            Expr::Neg(x) => {
                b = neg(b);
                *x
            }
            Expr::Bin(op, x, y) => {
                let in_x = count(&x, var) > 0;
                match (op, in_x) {
                    (BinOp::Add, true) => {
                        b = bin(BinOp::Sub, b, *y);
                        *x
                    }
                    (BinOp::Add, false) => {
                        b = bin(BinOp::Sub, b, *x);
                        *y
                    }
                    (BinOp::Sub, true) => {
                        b = bin(BinOp::Add, b, *y);
                        *x
                    }
                    (BinOp::Sub, false) => {
                        b = bin(BinOp::Sub, *x, b);
                        *y
                    }
                    (BinOp::Mul, true) => {
                        b = bin(BinOp::Div, b, *y);
                        *x
                    }
                    (BinOp::Mul, false) => {
                        b = bin(BinOp::Div, b, *x);
                        *y
                    }
                    (BinOp::Div, true) => {
                        b = bin(BinOp::Mul, b, *y);
                        *x
                    }
                    (BinOp::Div, false) => {
                        b = bin(BinOp::Div, *x, b);
                        *y
                    }
                    (BinOp::Pow, true) => {
                        b = root(b, *y);
                        *x
                    }
                    (BinOp::Pow, false) => {
                        // x^y = b  →  y = ln(b) / ln(x)
                        b = bin(BinOp::Div, call("ln", vec![b]), call("ln", vec![*x]));
                        *y
                    }
                }
            }
            Expr::Call(f, mut args) => {
                let inverse = |g: &str| Some(call(g, vec![b.clone()]));
                let new_b = match (f.as_str(), args.len()) {
                    ("sqrt", 1) => Some(bin(BinOp::Pow, b.clone(), Expr::Num(2.0))),
                    ("cbrt", 1) => Some(bin(BinOp::Pow, b.clone(), Expr::Num(3.0))),
                    ("nthroot", 2) if count(&args[0], var) > 0 => Some(bin(BinOp::Pow, b.clone(), args[1].clone())),
                    ("exp", 1) => inverse("ln"),
                    ("ln", 1) => inverse("exp"),
                    ("log", 1) => Some(bin(BinOp::Pow, Expr::Num(10.0), b.clone())),
                    ("sin", 1) => inverse("asin"),
                    ("cos", 1) => inverse("acos"),
                    ("tan", 1) => inverse("atan"),
                    ("asin", 1) => inverse("sin"),
                    ("acos", 1) => inverse("cos"),
                    ("atan", 1) => inverse("tan"),
                    ("sinh", 1) => inverse("asinh"),
                    ("cosh", 1) => inverse("acosh"),
                    ("tanh", 1) => inverse("atanh"),
                    _ => None,
                };
                b = new_b?;
                args.swap_remove(0)
            }
            _ => return None,
        };
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_expr_str;
    use crate::Env;

    /// Werk om en controleer met getallen: de omgewerkte uitdrukking in de oorspronkelijke
    /// vergelijking invullen moet links = rechts geven.
    fn check(eq: &str, var: &str) -> Expr {
        check_with(eq, var, &[])
    }

    fn check_with(eq: &str, var: &str, values: &[(&str, f64)]) -> Expr {
        let (l, r) = expr_sides(&parse_expr_str(eq).unwrap()).unwrap();
        let sol = isolate(&l, &r, var).unwrap_or_else(|| panic!("{eq}: niet om te werken naar {var}"));
        let mut env = Env::new();
        for (k, n) in unknowns(&l, &r).iter().enumerate() {
            if n != var {
                let v = values.iter().find(|(m, _)| m == n).map_or(0.3 + 0.37 * k as f64, |(_, v)| *v);
                env.set_var(n, rb_units::Quantity::scalar(v));
            }
        }
        let x = env.eval_expr(&sol).unwrap().as_quantity("x").unwrap();
        env.set_var(var, x);
        let (lv, rv) = (env.eval_expr(&l).unwrap().as_quantity("l").unwrap(), env.eval_expr(&r).unwrap().as_quantity("r").unwrap());
        assert!((lv.value - rv.value).abs() < 1e-9 * lv.value.abs().max(1.0), "{eq} naar {var}: {sol:?} geeft {} ≠ {}", lv.value, rv.value);
        sol
    }

    #[test]
    fn omwerken_en_controleren() {
        for v in ["F", "m", "a"] {
            check("F == m * a", v);
        }
        for v in ["σ", "M", "W"] {
            check("σ == M / W", v);
        }
        for v in ["P", "ρ", "A", "v", "C"] {
            check("P == 1/2 * ρ * A * v^3 * C", v);
        }
        for v in ["w", "q", "L", "E", "I"] {
            check("w == 5 * q * L^4 / (384 * E * I)", v);
        }
        for v in ["T", "T0", "k", "t"] {
            check("T == T0 * exp(-k * t)", v);
        }
        for v in ["y", "a", "b", "x"] {
            check("y == a - b / x", v);
        }
        check_with("c == sqrt(a^2 + b^2)", "a", &[("c", 5.0), ("b", 3.0)]);
        check_with("h == L * sin(α)", "α", &[("h", 1.0), ("L", 2.0)]);
        // v^3 → derdemachtswortel
        assert!(matches!(check("P == k * v^3", "v"), Expr::Call(ref f, _) if f == "nthroot"));
    }

    #[test]
    fn niet_omwerkbaar() {
        let (l, r) = expr_sides(&parse_expr_str("x == x^2 - 2").unwrap()).unwrap();
        assert!(isolate(&l, &r, "x").is_none());
        assert_eq!(count(&r, "x"), 1);
    }
}

//! Symbolisch rekenen (de pijl → van Mathcad): vereenvoudigen, uitwerken, ontbinden, afleiden,
//! integreren, oplossen en substitueren, op de uitdrukkingen van de taal ([`Expr`]).
//!
//! De kern is een normaalvorm: een som van termen, elke term een coëfficiënt maal een product van
//! "atomen" (namen, functieaanroepen, sommen die niet uitgewerkt worden) met een exponent. Daarin
//! vallen gelijksoortige termen en machten vanzelf samen; terugschrijven geeft een nette uitdrukking
//! (`3·x^2 + 2·x`, breuken als `a/b`, wortels als `sqrt`). Exacte getallen blijven exact: `sqrt(2)`
//! wordt geen 1.41421, en coëfficiënten worden waar mogelijk breuken (`x/2`).

use crate::ast::{BinOp, BindOp, CmpOp, Expr};
use std::collections::BTreeMap;

/// Een term: coëfficiënt maal een product van atomen met exponenten (sleutel: de tekst van het atoom).
#[derive(Clone, Debug)]
struct Term {
    coef: f64,
    mono: BTreeMap<String, (Expr, f64)>,
}

type Poly = Vec<Term>;

fn num(v: f64) -> Poly {
    if v == 0.0 {
        Vec::new()
    } else {
        vec![Term { coef: v, mono: BTreeMap::new() }]
    }
}

fn atom(e: Expr, p: f64) -> Poly {
    if p == 0.0 {
        return num(1.0);
    }
    let mut mono = BTreeMap::new();
    mono.insert(print(&e), (e, p));
    vec![Term { coef: 1.0, mono }]
}

fn mono_key(t: &Term) -> String {
    t.mono.iter().map(|(k, (_, p))| format!("{k}^{p};")).collect()
}

fn collect(p: Poly) -> Poly {
    let mut order: Vec<String> = Vec::new();
    let mut map: BTreeMap<String, Term> = BTreeMap::new();
    for t in p {
        let k = mono_key(&t);
        match map.get_mut(&k) {
            Some(e) => e.coef += t.coef,
            None => {
                order.push(k.clone());
                map.insert(k, t);
            }
        }
    }
    order.into_iter().filter_map(|k| map.remove(&k)).filter(|t| t.coef.abs() > 1e-13 * (1.0 + t.coef.abs())).collect()
}

fn scale(p: Poly, f: f64) -> Poly {
    p.into_iter().map(|t| Term { coef: t.coef * f, ..t }).collect()
}

fn add(a: Poly, b: Poly) -> Poly {
    collect(a.into_iter().chain(b).collect())
}

fn mul_term(a: &Term, b: &Term) -> Term {
    let mut mono = a.mono.clone();
    for (k, (e, p)) in &b.mono {
        let entry = mono.entry(k.clone()).or_insert((e.clone(), 0.0));
        entry.1 += p;
    }
    mono.retain(|_, (_, p)| p.abs() > 1e-12);
    Term { coef: a.coef * b.coef, mono }
}

/// Een som die niet uitgewerkt wordt, als één atoom.
fn as_single(p: Poly) -> Poly {
    if p.len() <= 1 {
        p
    } else {
        atom(from_poly(&p), 1.0)
    }
}

fn mul(a: Poly, b: Poly, expand: bool) -> Poly {
    if a.is_empty() || b.is_empty() {
        return Vec::new();
    }
    let (a, b) = if expand { (a, b) } else { (as_single(a), as_single(b)) };
    let mut out = Vec::with_capacity(a.len() * b.len());
    for x in &a {
        for y in &b {
            out.push(mul_term(x, y));
        }
    }
    collect(out)
}

/// Is dit een "mooi" getal (geheel of een eenvoudige breuk)?
fn nice(v: f64) -> bool {
    rational(v).is_some()
}

/// Een getal als breuk p/q met q ≤ 1000 (als dat precies genoeg is).
fn rational(v: f64) -> Option<(i64, i64)> {
    if !v.is_finite() || v.abs() > 1e15 {
        return None;
    }
    for q in 1..=1000i64 {
        let p = (v * q as f64).round();
        if (p / q as f64 - v).abs() <= 1e-12 * v.abs().max(1.0) {
            return Some((p as i64, q));
        }
    }
    None
}

fn pow(a: Poly, n: f64, expand: bool) -> Poly {
    if n == 0.0 {
        return num(1.0);
    }
    if n == 1.0 {
        return a;
    }
    if a.is_empty() {
        return if n > 0.0 { Vec::new() } else { atom(Expr::Bin(BinOp::Pow, Box::new(Expr::Num(0.0)), Box::new(Expr::Num(n))), 1.0) };
    }
    if a.len() == 1 {
        let t = &a[0];
        let mut mono: BTreeMap<String, (Expr, f64)> = t.mono.iter().map(|(k, (e, p))| (k.clone(), (e.clone(), p * n))).collect();
        // sqrt(8) = 2·sqrt(2): het grootste kwadraat gaat buiten de wortel
        if n == 0.5 && t.coef > 1.0 && t.coef.fract() == 0.0 && t.coef < 1e12 && t.coef.sqrt().fract() != 0.0 {
            let c = t.coef as u64;
            let k = (1..=(c as f64).sqrt() as u64).rev().find(|k| c % (k * k) == 0).unwrap_or(1);
            if k > 1 {
                let rest = Term { coef: (c / (k * k)) as f64, mono: t.mono.clone() };
                return scale(pow(vec![rest], 0.5, expand), k as f64);
            }
        }
        let c = t.coef.powf(n);
        let coef = if c.is_finite() && (n.fract() == 0.0 || nice(c)) {
            c
        } else {
            // sqrt(2) blijft sqrt(2): het getal als atoom
            let (base, sign) = if t.coef < 0.0 && n.fract() == 0.0 { (-t.coef, if (n as i64) % 2 == 0 { 1.0 } else { -1.0 }) } else { (t.coef, 1.0) };
            let e = Expr::Num(base);
            let k = print(&e);
            mono.insert(k, (e, n));
            sign
        };
        return vec![Term { coef, mono }];
    }
    if expand && n.fract() == 0.0 && (1.0..=12.0).contains(&n) {
        let mut out = a.clone();
        for _ in 1..(n as usize) {
            out = mul(out, a.clone(), true);
        }
        return out;
    }
    let base = from_poly(&a);
    atom(base, n)
}

/// De symbolische functies die we exact kennen in bijzondere punten.
fn fold_call(f: &str, args: &[Expr]) -> Option<Expr> {
    let zero = matches!(args, [Expr::Num(v)] if *v == 0.0);
    let one = matches!(args, [Expr::Num(v)] if *v == 1.0);
    match f {
        "sin" | "tan" | "asin" | "atan" | "sinh" | "tanh" if zero => Some(Expr::Num(0.0)),
        "cos" | "cosh" | "exp" if zero => Some(Expr::Num(1.0)),
        "ln" | "log" if one => Some(Expr::Num(0.0)),
        "sqrt" => match args {
            [Expr::Num(v)] if *v >= 0.0 && v.sqrt().fract() == 0.0 => Some(Expr::Num(v.sqrt())),
            _ => None,
        },
        // sin, cos en tan bij een veelvoud van π/2 (sin(π) = 0, cos(π) = −1)
        "sin" | "cos" | "tan" => {
            let k = half_pi_multiple(args.first()?)?;
            let (s, c) = match k.rem_euclid(4) {
                0 => (0.0, 1.0),
                1 => (1.0, 0.0),
                2 => (0.0, -1.0),
                _ => (-1.0, 0.0),
            };
            match f {
                "sin" => Some(Expr::Num(s)),
                "cos" => Some(Expr::Num(c)),
                _ if c != 0.0 => Some(Expr::Num(s / c)),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Is `e` (alleen getallen en π) een geheel veelvoud k van π/2? Geeft k.
fn half_pi_multiple(e: &Expr) -> Option<i64> {
    if !contains(e, "π") || e.names().iter().any(|n| n != "π") {
        return None;
    }
    let v = constant(&to_poly(&substitute(e, "π", &Expr::Num(std::f64::consts::PI)), false).ok()?)?;
    let k = v / std::f64::consts::FRAC_PI_2;
    ((k - k.round()).abs() < 1e-9).then(|| k.round() as i64)
}

fn to_poly(e: &Expr, expand: bool) -> Result<Poly, String> {
    Ok(match e {
        Expr::Num(v) => num(*v),
        Expr::Var(n) => atom(Expr::Var(n.clone()), 1.0),
        Expr::Neg(x) => scale(to_poly(x, expand)?, -1.0),
        Expr::Bin(op, a, b) => {
            let (pa, pb) = (to_poly(a, expand)?, to_poly(b, expand)?);
            match op {
                BinOp::Add => add(pa, pb),
                BinOp::Sub => add(pa, scale(pb, -1.0)),
                BinOp::Mul => mul(pa, pb, expand),
                BinOp::Div => {
                    if pb.is_empty() {
                        return Err("deling door nul".into());
                    }
                    mul(pa, pow(pb, -1.0, expand), expand)
                }
                BinOp::Pow => match constant(&pb) {
                    Some(c) => pow(pa, c, expand),
                    None => atom(Expr::Bin(BinOp::Pow, Box::new(from_poly(&pa)), Box::new(from_poly(&pb))), 1.0),
                },
            }
        }
        Expr::Call(f, args) => {
            let args: Vec<Expr> = args.iter().map(|a| simplify(a)).collect::<Result<_, _>>()?;
            if f == "sqrt" && args.len() == 1 {
                return Ok(pow(to_poly(&args[0], expand)?, 0.5, expand));
            }
            match fold_call(f, &args) {
                Some(v) => to_poly(&v, expand)?,
                None => atom(Expr::Call(f.clone(), args), 1.0),
            }
        }
        Expr::Bind { op: BindOp::Deriv, body, var, args } if matches!(args.as_slice(), [Expr::Var(p)] if p == var) => to_poly(&diff(body, var)?, expand)?,
        Expr::Bind { op: BindOp::Integral, body, var, args } if args.len() == 2 => match integrate(body, var) {
            Some(f) => {
                let at = |v: &Expr| substitute(&f, var, v);
                to_poly(&Expr::Bin(BinOp::Sub, Box::new(at(&args[1])), Box::new(at(&args[0]))), expand)?
            }
            None => atom(e.clone(), 1.0),
        },
        Expr::Bind { op: op @ (BindOp::Sum | BindOp::Prod), body, var, args } if args.len() == 2 => {
            match (constant(&to_poly(&args[0], false)?), constant(&to_poly(&args[1], false)?)) {
                (Some(a), Some(b)) if a.fract() == 0.0 && b.fract() == 0.0 && b - a <= 1000.0 => {
                    let mut acc = if *op == BindOp::Sum { num(0.0) } else { num(1.0) };
                    let mut k = a;
                    while k <= b {
                        let term = to_poly(&substitute(body, var, &Expr::Num(k)), expand)?;
                        acc = if *op == BindOp::Sum { add(acc, term) } else { mul(acc, term, expand) };
                        k += 1.0;
                    }
                    acc
                }
                _ => atom(e.clone(), 1.0),
            }
        }
        _ => atom(e.clone(), 1.0),
    })
}

/// Een constante (een term zonder atomen)?
fn constant(p: &Poly) -> Option<f64> {
    match p.as_slice() {
        [] => Some(0.0),
        [t] if t.mono.is_empty() => Some(t.coef),
        _ => None,
    }
}

fn number(v: f64) -> Expr {
    if v < 0.0 {
        Expr::Neg(Box::new(number(-v)))
    } else {
        match rational(v) {
            Some((p, q)) if q > 1 => Expr::Bin(BinOp::Div, Box::new(Expr::Num(p as f64)), Box::new(Expr::Num(q as f64))),
            _ => Expr::Num(v),
        }
    }
}

/// Een atoom tot de macht `p` (p > 0).
fn power(e: &Expr, p: f64) -> Expr {
    if p == 1.0 {
        return e.clone();
    }
    if p == 0.5 {
        return Expr::Call("sqrt".into(), vec![e.clone()]);
    }
    let exp = match rational(p) {
        Some((a, b)) if b > 1 => Expr::Bin(BinOp::Div, Box::new(Expr::Num(a as f64)), Box::new(Expr::Num(b as f64))),
        _ => Expr::Num(p),
    };
    let base = match e {
        Expr::Var(_) | Expr::Num(_) | Expr::Call(..) => e.clone(),
        _ => e.clone(),
    };
    Expr::Bin(BinOp::Pow, Box::new(base), Box::new(exp))
}

fn product(factors: Vec<Expr>) -> Option<Expr> {
    factors.into_iter().reduce(|a, b| Expr::Bin(BinOp::Mul, Box::new(a), Box::new(b)))
}

/// Een term (zonder teken) als uitdrukking.
fn term_expr(t: &Term) -> Expr {
    let c = t.coef.abs();
    let (p, q) = rational(c).map_or((c, 1.0), |(p, q)| (p as f64, q as f64));
    let mut top: Vec<Expr> = Vec::new();
    let mut bottom: Vec<Expr> = Vec::new();
    // namen eerst, dan getallen onder een wortel, dan functies: x·exp(x), 2·sqrt(2)
    let mut factors: Vec<&(Expr, f64)> = t.mono.values().collect();
    factors.sort_by_key(|(e, _)| match e {
        Expr::Var(_) => 0,
        Expr::Num(_) => 1,
        Expr::Call(..) => 3,
        _ => 2,
    });
    for (e, x) in factors {
        if *x > 0.0 {
            top.push(power(e, *x));
        } else {
            bottom.push(power(e, -*x));
        }
    }
    if rational(c).is_none() {
        top.insert(0, Expr::Num(c));
    } else if p != 1.0 || top.is_empty() {
        top.insert(0, Expr::Num(p));
    }
    if q != 1.0 {
        bottom.insert(0, Expr::Num(q));
    }
    let num = product(top).unwrap_or(Expr::Num(1.0));
    match product(bottom) {
        Some(den) => Expr::Bin(BinOp::Div, Box::new(num), Box::new(den)),
        None => num,
    }
}

fn degree(t: &Term) -> f64 {
    t.mono.values().map(|(_, p)| p.max(0.0)).sum()
}

fn from_poly(p: &Poly) -> Expr {
    if p.is_empty() {
        return Expr::Num(0.0);
    }
    let mut terms: Vec<&Term> = p.iter().collect();
    terms.sort_by(|a, b| degree(b).partial_cmp(&degree(a)).unwrap_or(std::cmp::Ordering::Equal).then_with(|| mono_key(a).cmp(&mono_key(b))));
    let mut out: Option<Expr> = None;
    for t in terms {
        let e = term_expr(t);
        out = Some(match out {
            None if t.coef < 0.0 => Expr::Neg(Box::new(e)),
            None => e,
            Some(acc) if t.coef < 0.0 => Expr::Bin(BinOp::Sub, Box::new(acc), Box::new(e)),
            Some(acc) => Expr::Bin(BinOp::Add, Box::new(acc), Box::new(e)),
        });
    }
    out.unwrap_or(Expr::Num(0.0))
}

/// Vereenvoudig: getallen uitrekenen, gelijksoortige termen en machten samennemen. Producten van
/// sommen blijven staan (zie [`expand`]).
pub fn simplify(e: &Expr) -> Result<Expr, String> {
    Ok(from_poly(&to_poly(e, false)?))
}

/// Werk uit: haakjes wegwerken (producten en gehele machten van sommen).
pub fn expand(e: &Expr) -> Result<Expr, String> {
    Ok(from_poly(&to_poly(e, true)?))
}

/// Vervang de naam `x` door `v`.
pub fn substitute(e: &Expr, x: &str, v: &Expr) -> Expr {
    let s = |e: &Expr| Box::new(substitute(e, x, v));
    match e {
        Expr::Var(n) if n == x => v.clone(),
        Expr::Neg(a) => Expr::Neg(s(a)),
        Expr::Not(a) => Expr::Not(s(a)),
        Expr::Bin(op, a, b) => Expr::Bin(*op, s(a), s(b)),
        Expr::And(a, b) => Expr::And(s(a), s(b)),
        Expr::Or(a, b) => Expr::Or(s(a), s(b)),
        Expr::Cmp(ops, xs) => Expr::Cmp(ops.clone(), xs.iter().map(|a| substitute(a, x, v)).collect()),
        Expr::Call(f, args) => Expr::Call(f.clone(), args.iter().map(|a| substitute(a, x, v)).collect()),
        Expr::Index(b, idx) => Expr::Index(s(b), idx.iter().map(|a| substitute(a, x, v)).collect()),
        Expr::Matrix(rows) => Expr::Matrix(rows.iter().map(|r| r.iter().map(|a| substitute(a, x, v)).collect()).collect()),
        // een gebonden variabele met dezelfde naam schermt af
        Expr::Bind { var, .. } if var == x => e.clone(),
        Expr::Bind { op, body, var, args } => Expr::Bind { op: *op, body: s(body), var: var.clone(), args: args.iter().map(|a| substitute(a, x, v)).collect() },
        _ => e.clone(),
    }
}

fn contains(e: &Expr, x: &str) -> bool {
    e.names().contains(x)
}

fn n(v: f64) -> Box<Expr> {
    Box::new(Expr::Num(v))
}

fn b(e: Expr) -> Box<Expr> {
    Box::new(e)
}

/// De afgeleide naar `x` (nog niet vereenvoudigd).
fn d(e: &Expr, x: &str) -> Result<Expr, String> {
    use BinOp::*;
    if !contains(e, x) {
        return Ok(Expr::Num(0.0));
    }
    Ok(match e {
        Expr::Var(_) => Expr::Num(1.0),
        Expr::Neg(a) => Expr::Neg(b(d(a, x)?)),
        Expr::Bin(Add, a, c) => Expr::Bin(Add, b(d(a, x)?), b(d(c, x)?)),
        Expr::Bin(Sub, a, c) => Expr::Bin(Sub, b(d(a, x)?), b(d(c, x)?)),
        Expr::Bin(Mul, a, c) => Expr::Bin(Add, b(Expr::Bin(Mul, b(d(a, x)?), c.clone())), b(Expr::Bin(Mul, a.clone(), b(d(c, x)?)))),
        Expr::Bin(Div, a, c) => Expr::Bin(
            Div,
            b(Expr::Bin(Sub, b(Expr::Bin(Mul, b(d(a, x)?), c.clone())), b(Expr::Bin(Mul, a.clone(), b(d(c, x)?))))),
            b(Expr::Bin(Pow, c.clone(), n(2.0))),
        ),
        Expr::Bin(Pow, a, c) if !contains(c, x) => Expr::Bin(Mul, b(Expr::Bin(Mul, c.clone(), b(Expr::Bin(Pow, a.clone(), b(Expr::Bin(Sub, c.clone(), n(1.0))))))), b(d(a, x)?)),
        Expr::Bin(Pow, a, c) if !contains(a, x) => Expr::Bin(Mul, b(Expr::Bin(Mul, b(e.clone()), b(Expr::Call("ln".into(), vec![(**a).clone()])))), b(d(c, x)?)),
        Expr::Bin(Pow, a, c) => Expr::Bin(
            Mul,
            b(e.clone()),
            b(Expr::Bin(
                Add,
                b(Expr::Bin(Mul, b(d(c, x)?), b(Expr::Call("ln".into(), vec![(**a).clone()])))),
                b(Expr::Bin(Div, b(Expr::Bin(Mul, c.clone(), b(d(a, x)?))), a.clone())),
            )),
        ),
        Expr::Call(f, args) if args.len() == 1 => {
            let u = args[0].clone();
            let du = d(&u, x)?;
            let call = |g: &str, a: Expr| Expr::Call(g.into(), vec![a]);
            let outer = match f.as_str() {
                "sin" => call("cos", u),
                "cos" => Expr::Neg(b(call("sin", u))),
                "tan" => Expr::Bin(Div, n(1.0), b(Expr::Bin(Pow, b(call("cos", u)), n(2.0)))),
                "exp" => call("exp", u),
                "ln" => Expr::Bin(Div, n(1.0), b(u)),
                "log" => Expr::Bin(Div, n(1.0), b(Expr::Bin(Mul, b(u), b(call("ln", Expr::Num(10.0)))))),
                "sqrt" => Expr::Bin(Div, n(1.0), b(Expr::Bin(Mul, n(2.0), b(call("sqrt", u))))),
                "asin" => Expr::Bin(Div, n(1.0), b(call("sqrt", Expr::Bin(Sub, n(1.0), b(Expr::Bin(Pow, b(u), n(2.0))))))),
                "acos" => Expr::Neg(b(Expr::Bin(Div, n(1.0), b(call("sqrt", Expr::Bin(Sub, n(1.0), b(Expr::Bin(Pow, b(u), n(2.0))))))))),
                "atan" => Expr::Bin(Div, n(1.0), b(Expr::Bin(Add, n(1.0), b(Expr::Bin(Pow, b(u), n(2.0)))))),
                "sinh" => call("cosh", u),
                "cosh" => call("sinh", u),
                "tanh" => Expr::Bin(Div, n(1.0), b(Expr::Bin(Pow, b(call("cosh", u)), n(2.0)))),
                "abs" => Expr::Bin(Div, b(u.clone()), b(call("abs", u))),
                other => return Err(format!("de afgeleide van {other} is (nog) niet symbolisch bekend")),
            };
            Expr::Bin(Mul, b(outer), b(du))
        }
        _ => return Err("deze uitdrukking kan niet symbolisch worden afgeleid".into()),
    })
}

/// De afgeleide naar `x`, vereenvoudigd.
pub fn diff(e: &Expr, x: &str) -> Result<Expr, String> {
    simplify(&d(e, x)?)
}

/// `u = a·x + c`: geeft a (een getal ≠ 0), anders `None`.
fn linear_coef(u: &Expr, x: &str) -> Option<f64> {
    match constant(&to_poly(&d(u, x).ok()?, true).ok()?)? {
        a if a != 0.0 && a.is_finite() => Some(a),
        _ => None,
    }
}

/// Primitieve van één factor `f^p` met een lineair argument: sin, cos, tan, exp, sinh, cosh van
/// `a·x + c`, en machten van `a·x + c` (met 1/(a·x + c) → ln).
fn primitive_atom(fe: &Expr, p: f64, x: &str) -> Option<Expr> {
    let over = |e: Expr, a: f64| Expr::Bin(BinOp::Div, b(e), n(a));
    if let Expr::Call(f, args) = fe {
        if p != 1.0 || args.len() != 1 {
            return None;
        }
        let a = linear_coef(&args[0], x)?;
        let call = |g: &str| Expr::Call(g.into(), args.clone());
        let anti = match f.as_str() {
            "sin" => Expr::Neg(b(call("cos"))),
            "cos" => call("sin"),
            "exp" => fe.clone(),
            "tan" => Expr::Neg(b(Expr::Call("ln".into(), vec![call("cos")]))),
            "sinh" => call("cosh"),
            "cosh" => call("sinh"),
            _ => return None,
        };
        return Some(over(anti, a));
    }
    let a = linear_coef(fe, x)?;
    Some(if p == -1.0 {
        over(Expr::Call("ln".into(), vec![fe.clone()]), a)
    } else {
        over(Expr::Bin(BinOp::Pow, b(fe.clone()), n(p + 1.0)), a * (p + 1.0))
    })
}

/// Een primitieve naar `x`, of `None`: veeltermen, sin/cos/tan/exp/sinh/cosh en machten van een lineair
/// argument (`sin(2 x + 1)`, `1/(3 x - 2)`, `sqrt(x + 4)`), en `x^n` maal sin, cos of exp (partieel).
pub fn integrate(e: &Expr, x: &str) -> Option<Expr> {
    integrate_depth(e, x, 0)
}

fn integrate_depth(e: &Expr, x: &str, depth: usize) -> Option<Expr> {
    if depth > 12 {
        return None;
    }
    let p = to_poly(e, true).ok()?;
    let xkey = print(&Expr::Var(x.to_string()));
    let mut out: Poly = Vec::new();
    for t in p {
        let mut t = t;
        let n = t.mono.remove(&xkey).map_or(0.0, |(_, p)| p);
        // de andere factoren mogen x niet bevatten, behalve één factor met een lineair argument
        let with_x: Vec<String> = t.mono.iter().filter(|(_, (e, _))| contains(e, x)).map(|(k, _)| k.clone()).collect();
        match (with_x.as_slice(), n) {
            ([], n) if n == -1.0 => out.extend(mul(vec![t], atom(Expr::Call("ln".into(), vec![Expr::Var(x.into())]), 1.0), true)),
            ([], n) => {
                let mut t2 = t.clone();
                t2.coef /= n + 1.0;
                out.extend(mul(vec![t2], atom(Expr::Var(x.into()), n + 1.0), true));
            }
            ([k], n) if n == 0.0 => {
                let (fe, fp) = t.mono.remove(k)?;
                let anti = primitive_atom(&fe, fp, x)?;
                out.extend(mul(vec![t], to_poly(&anti, true).ok()?, true));
            }
            // partieel: ∫ x^n f(u) dx = x^n F(u) − n ∫ x^(n−1) F(u) dx, voor f = sin, cos, exp
            ([k], n) if n > 0.0 && n.fract() == 0.0 => {
                let (fe, fp) = t.mono.remove(k)?;
                if !matches!(&fe, Expr::Call(f, _) if matches!(f.as_str(), "sin" | "cos" | "exp")) {
                    return None;
                }
                let big_f = primitive_atom(&fe, fp, x)?;
                let xn = |p: f64| Expr::Bin(BinOp::Pow, b(Expr::Var(x.into())), self::n(p));
                let rest = integrate_depth(&Expr::Bin(BinOp::Mul, b(xn(n - 1.0)), b(big_f.clone())), x, depth + 1)?;
                let whole = Expr::Bin(
                    BinOp::Sub,
                    b(Expr::Bin(BinOp::Mul, b(xn(n)), b(big_f))),
                    b(Expr::Bin(BinOp::Mul, self::n(n), b(rest))),
                );
                out.extend(mul(vec![t], to_poly(&whole, true).ok()?, true));
            }
            _ => return None,
        }
    }
    Some(from_poly(&collect(out)))
}

/// Is dit een eindig getal (na vereenvoudigen)?
fn finite_value(e: &Expr) -> Option<f64> {
    constant(&to_poly(e, false).ok()?).filter(|v| v.is_finite())
}

/// Als `e` een veelterm in `x` is (gehele machten ≥ 0): de graad en de coëfficiënt van de hoogste macht.
fn leading(e: &Expr, x: &str) -> Option<(f64, Expr)> {
    let p = to_poly(e, true).ok()?;
    let xkey = print(&Expr::Var(x.to_string()));
    let mut terms: Vec<(f64, Term)> = Vec::new();
    for mut t in p {
        let k = t.mono.remove(&xkey).map_or(0.0, |(_, k)| k);
        if k < 0.0 || k.fract() != 0.0 || t.mono.values().any(|(e, _)| contains(e, x)) {
            return None;
        }
        terms.push((k, t));
    }
    let top = terms.iter().map(|(k, _)| *k).fold(0.0, f64::max);
    let lead: Poly = terms.into_iter().filter(|(k, _)| *k == top).map(|(_, t)| t).collect();
    if lead.is_empty() {
        return Some((0.0, Expr::Num(0.0)));
    }
    Some((top, from_poly(&collect(lead))))
}

fn is_infinity(e: &Expr) -> Option<f64> {
    match e {
        Expr::Var(v) if v == "∞" || v == "inf" => Some(1.0),
        Expr::Neg(x) => is_infinity(x).map(|s| -s),
        _ => None,
    }
}

/// De limiet van `e` voor `x → a`. Eerst invullen; geeft dat 0/0, dan de regel van l'Hôpital (tot zes keer).
/// Voor `x → ±∞`: met `x = 1/t` naar `t → 0`, en bij gebroken veeltermen de hoogste machten.
pub fn limit(e: &Expr, x: &str, a: &Expr) -> Result<Expr, String> {
    if let Some(sign) = is_infinity(a) {
        // een (gebroken) veelterm: de hoogste machten bepalen de limiet
        let (p, q) = match e {
            Expr::Bin(BinOp::Div, p, q) => ((**p).clone(), (**q).clone()),
            other => (other.clone(), Expr::Num(1.0)),
        };
        if let (Some((dp, lp)), Some((dq, lq))) = (leading(&p, x), leading(&q, x)) {
            return if dp < dq {
                Ok(Expr::Num(0.0))
            } else if dp == dq {
                simplify(&Expr::Bin(BinOp::Div, b(lp), b(lq)))
            } else {
                Err(format!("limit: oneindig voor {x} → {}∞", if sign < 0.0 { "-" } else { "" }))
            };
        }
        let t = "\u{0}t";
        let inv = Expr::Bin(BinOp::Div, n(sign), b(Expr::Var(t.into())));
        return limit(&substitute(e, x, &inv), t, &Expr::Num(0.0)).map_err(|_| format!("limit: geen limiet gevonden voor {x} → ∞"));
    }
    let mut cur = e.clone();
    for _ in 0..=6 {
        let (num_e, den_e) = match &cur {
            Expr::Bin(BinOp::Div, p, q) => ((**p).clone(), (**q).clone()),
            other => match simplify(other)? {
                Expr::Bin(BinOp::Div, p, q) => (*p, *q),
                s => (s, Expr::Num(1.0)),
            },
        };
        let at = |f: &Expr| simplify(&substitute(f, x, a)).ok();
        let (nv, dv) = (at(&num_e), at(&den_e));
        match (nv.as_ref().and_then(finite_value), dv.as_ref().and_then(finite_value)) {
            // 0/0: l'Hôpital
            (Some(p), Some(q)) if p == 0.0 && q == 0.0 => {
                cur = Expr::Bin(BinOp::Div, b(diff(&num_e, x)?), b(diff(&den_e, x)?));
                continue;
            }
            (Some(_), Some(q)) if q == 0.0 => return Err(format!("limit: oneindig of bestaat niet voor {x} → {}", print(a))),
            _ => {}
        }
        // gewoon invullen
        if let (Some(p), Some(q)) = (nv, dv) {
            return simplify(&Expr::Bin(BinOp::Div, b(p), b(q)));
        }
        return Err(format!("limit: geen limiet gevonden voor {x} → {}", print(a)));
    }
    Err(format!("limit: geen limiet gevonden voor {x} → {} (na zes keer l'Hôpital)", print(a)))
}

/// Taylorreeks van `e` rond `x = a`, met de termen tot en met de macht `order − 1` (zoals Mathcad).
pub fn series(e: &Expr, x: &str, a: &Expr, order: usize) -> Result<Expr, String> {
    let mut deriv = e.clone();
    let mut total = Expr::Num(0.0);
    let mut fact = 1.0;
    let dx = if finite_value(a) == Some(0.0) { Expr::Var(x.into()) } else { Expr::Bin(BinOp::Sub, b(Expr::Var(x.into())), b(a.clone())) };
    for k in 0..order {
        if k > 0 {
            fact *= k as f64;
            deriv = diff(&deriv, x)?;
        }
        let ck = limit(&deriv, x, a)?;
        let term = Expr::Bin(BinOp::Mul, b(Expr::Bin(BinOp::Div, b(ck), n(fact))), b(Expr::Bin(BinOp::Pow, b(dx.clone()), n(k as f64))));
        total = Expr::Bin(BinOp::Add, b(total), b(term));
    }
    simplify(&total)
}

/// Los `lhs == rhs` op naar `x`: exact voor graad 1 en 2 (ook met andere symbolen), anders door
/// omwerken (zie [`crate::solve::isolate`]).
pub fn solve(lhs: &Expr, rhs: &Expr, x: &str) -> Result<Vec<Expr>, String> {
    let f = Expr::Bin(BinOp::Sub, b(lhs.clone()), b(rhs.clone()));
    let p = to_poly(&f, true)?;
    let xkey = print(&Expr::Var(x.to_string()));
    // coëfficiënten per macht van x
    let mut coefs: BTreeMap<i64, Poly> = BTreeMap::new();
    let mut polynomial = true;
    for t in &p {
        let mut t = t.clone();
        let k = t.mono.remove(&xkey).map_or(0.0, |(_, p)| p);
        if k.fract() != 0.0 || k < 0.0 || t.mono.values().any(|(e, _)| contains(e, x)) {
            polynomial = false;
            break;
        }
        coefs.entry(k as i64).or_default().push(t);
    }
    let c = |k: i64| -> Expr { coefs.get(&k).map_or(Expr::Num(0.0), |p| from_poly(p)) };
    let deg = coefs.keys().max().copied().unwrap_or(0);
    if polynomial && deg == 1 {
        return Ok(vec![simplify(&Expr::Neg(b(Expr::Bin(BinOp::Div, b(c(0)), b(c(1))))))?]);
    }
    if polynomial && deg == 2 {
        let (a2, a1, a0) = (c(2), c(1), c(0));
        let disc = simplify(&Expr::Bin(
            BinOp::Sub,
            b(Expr::Bin(BinOp::Pow, b(a1.clone()), n(2.0))),
            b(Expr::Bin(BinOp::Mul, n(4.0), b(Expr::Bin(BinOp::Mul, b(a2.clone()), b(a0.clone()))))),
        ))?;
        if let Some(dv) = constant(&to_poly(&disc, true)?) {
            if dv < 0.0 {
                return Err("geen reële oplossingen (de discriminant is negatief)".into());
            }
        }
        let root = Expr::Call("sqrt".into(), vec![disc]);
        let two_a = Expr::Bin(BinOp::Mul, n(2.0), b(a2));
        let mk = |sign: BinOp| -> Result<Expr, String> {
            simplify(&Expr::Bin(BinOp::Div, b(Expr::Bin(sign, b(Expr::Neg(b(a1.clone()))), b(root.clone()))), b(two_a.clone())))
        };
        let (r1, r2) = (mk(BinOp::Sub)?, mk(BinOp::Add)?);
        return Ok(if print(&r1) == print(&r2) { vec![r1] } else { vec![r1, r2] });
    }
    if polynomial && deg == 0 {
        return Err(format!("{x} komt niet voor in de vergelijking"));
    }
    match crate::solve::isolate(lhs, rhs, x) {
        Some(e) => Ok(vec![simplify(&e)?]),
        None if polynomial => Err(format!("een veelterm van graad {deg}: gebruik polyroots of root")),
        None => Err(format!("{x} kan niet symbolisch worden vrijgemaakt; gebruik root of een solve block")),
    }
}

/// Ontbind in factoren: een gemeenschappelijke factor buiten haakjes, en een kwadratische veelterm
/// in één naam met rationale nulpunten als (x − a)(x − b).
pub fn factor(e: &Expr) -> Result<Expr, String> {
    let p = to_poly(e, true)?;
    if p.len() <= 1 {
        return Ok(from_poly(&p));
    }
    // gemeenschappelijke factor: kleinste exponent per atoom dat in alle termen voorkomt, en de
    // grootste gemene deler van gehele coëfficiënten
    let mut common = p[0].mono.clone();
    for t in &p[1..] {
        common.retain(|k, (_, ex)| {
            if let Some((_, e2)) = t.mono.get(k) {
                *ex = ex.min(*e2);
                true
            } else {
                false
            }
        });
    }
    common.retain(|_, (_, ex)| *ex > 0.0);
    let ints: Option<Vec<i64>> = p.iter().map(|t| if t.coef.fract() == 0.0 { Some(t.coef.abs() as i64) } else { None }).collect();
    let g = ints.map_or(1, |v| v.into_iter().fold(0, gcd)).max(1) as f64;
    let g = if p[0].coef < 0.0 { -g } else { g };
    let common_term = Term { coef: g, mono: common.clone() };
    let inv = Term { coef: 1.0 / g, mono: common.iter().map(|(k, (e, x))| (k.clone(), (e.clone(), -x))).collect() };
    let rest: Poly = p.iter().map(|t| mul_term(t, &inv)).collect();
    let inner = quadratic_factors(&rest).unwrap_or_else(|| from_poly(&rest));
    let outer = from_poly(&vec![common_term]);
    Ok(match outer {
        Expr::Num(v) if v == 1.0 => inner,
        o => Expr::Bin(BinOp::Mul, b(o), b(inner)),
    })
}

fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// a·x² + b·x + c met rationale nulpunten als a·(x − r1)·(x − r2).
fn quadratic_factors(p: &Poly) -> Option<Expr> {
    let names: Vec<String> = p.iter().flat_map(|t| t.mono.values().map(|(e, _)| print(e))).collect();
    let x = names.first()?.clone();
    if names.iter().any(|k| *k != x) {
        return None;
    }
    let var = p.iter().find_map(|t| t.mono.values().next().map(|(e, _)| e.clone()))?;
    let mut c = [0.0; 3];
    for t in p {
        let k = t.mono.values().next().map_or(0.0, |(_, e)| *e);
        if k.fract() != 0.0 || !(0.0..=2.0).contains(&k) {
            return None;
        }
        c[k as usize] += t.coef;
    }
    if c[2] == 0.0 {
        return None;
    }
    let disc = c[1] * c[1] - 4.0 * c[2] * c[0];
    if disc < 0.0 || !nice(disc.sqrt()) {
        return None;
    }
    let (r1, r2) = ((-c[1] - disc.sqrt()) / (2.0 * c[2]), (-c[1] + disc.sqrt()) / (2.0 * c[2]));
    if !nice(r1) || !nice(r2) {
        return None;
    }
    let factor = |r: f64| -> Expr {
        if r == 0.0 {
            var.clone()
        } else if r < 0.0 {
            Expr::Bin(BinOp::Add, b(var.clone()), b(number(-r)))
        } else {
            Expr::Bin(BinOp::Sub, b(var.clone()), b(number(r)))
        }
    };
    let pair = if r1 == r2 { Expr::Bin(BinOp::Pow, b(factor(r1)), n(2.0)) } else { Expr::Bin(BinOp::Mul, b(factor(r2)), b(factor(r1))) };
    Some(if c[2] == 1.0 { pair } else { Expr::Bin(BinOp::Mul, b(number(c[2])), b(pair)) })
}

// ---------------------------------------------------------------------------------------------
// Uitdrukkingen als tekst (voor sleutels, de tekstweergave en het bewerken)

fn prec(e: &Expr) -> u8 {
    match e {
        Expr::Cmp(..) | Expr::And(..) | Expr::Or(..) | Expr::Not(_) => 1,
        Expr::Bin(BinOp::Add | BinOp::Sub, ..) => 2,
        Expr::Bin(BinOp::Mul | BinOp::Div, ..) => 3,
        Expr::Neg(_) => 4,
        Expr::Bin(BinOp::Pow, ..) => 5,
        Expr::Num(v) if *v < 0.0 => 4,
        _ => 6,
    }
}

fn num_text(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// Een uitdrukking als tekst in de syntax van MathPrime (met `*` voor elk product).
pub fn print(e: &Expr) -> String {
    let wrap = |x: &Expr, min: u8| -> String {
        let s = print(x);
        if prec(x) < min {
            format!("({s})")
        } else {
            s
        }
    };
    match e {
        Expr::Num(v) => num_text(*v),
        Expr::Imag(v) => format!("{}i", num_text(*v)),
        Expr::Str(s) => format!("\"{}\"", s.replace('"', "\"\"")),
        Expr::Var(n) => n.clone(),
        // -b/a is hetzelfde als -(b/a): alleen haakjes om een som
        Expr::Neg(x) => format!("-{}", wrap(x, 3)),
        Expr::Not(x) => format!("not {}", wrap(x, 2)),
        Expr::Bin(op, a, c) => match op {
            BinOp::Add => format!("{} + {}", wrap(a, 2), wrap(c, 2)),
            BinOp::Sub => format!("{} - {}", wrap(a, 2), wrap(c, 3)),
            BinOp::Mul => format!("{} * {}", wrap(a, 3), wrap(c, 3)),
            BinOp::Div => format!("{} / {}", wrap(a, 3), wrap(c, 4)),
            BinOp::Pow => format!("{}^{}", wrap(a, 6), wrap(c, 6)),
        },
        Expr::Cmp(ops, xs) => {
            let mut s = wrap(&xs[0], 2);
            for (k, op) in ops.iter().enumerate() {
                let o = match op {
                    CmpOp::Lt => "<",
                    CmpOp::Le => "<=",
                    CmpOp::Gt => ">",
                    CmpOp::Ge => ">=",
                    CmpOp::Eq => "==",
                    CmpOp::Ne => "!=",
                };
                s.push_str(&format!(" {o} {}", wrap(&xs[k + 1], 2)));
            }
            s
        }
        Expr::And(a, c) => format!("{} and {}", wrap(a, 2), wrap(c, 2)),
        Expr::Or(a, c) => format!("{} or {}", wrap(a, 2), wrap(c, 2)),
        Expr::Call(f, args) => format!("{f}({})", args.iter().map(print).collect::<Vec<_>>().join(", ")),
        Expr::Index(v, idx) => format!("{}[{}]", wrap(v, 6), idx.iter().map(print).collect::<Vec<_>>().join(", ")),
        Expr::Matrix(rows) => format!("[{}]", rows.iter().map(|r| r.iter().map(print).collect::<Vec<_>>().join(", ")).collect::<Vec<_>>().join("; ")),
        Expr::Range { start, second, end } => match second {
            Some(s) => format!("{}, {} .. {}", print(start), print(s), print(end)),
            None => format!("{} .. {}", print(start), print(end)),
        },
        Expr::Bind { op, body, var, args } => {
            let f = match op {
                BindOp::Sum => "sum",
                BindOp::Prod => "prod",
                BindOp::Integral => "integral",
                BindOp::Deriv => "deriv",
                BindOp::Root => "root",
                BindOp::OdeSolve => "odesolve",
            };
            let mut parts = vec![print(body), var.clone()];
            parts.extend(args.iter().map(print));
            format!("{f}({})", parts.join(", "))
        }
        Expr::Block(_) | Expr::Interp(..) => "…".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_expr_str;

    fn p(s: &str) -> Expr {
        parse_expr_str(s).unwrap()
    }

    fn simp(s: &str) -> String {
        print(&simplify(&p(s)).unwrap())
    }

    #[test]
    fn vereenvoudigen() {
        assert_eq!(simp("2*x + 3*x"), "5 * x");
        assert_eq!(simp("x*x*x"), "x^3");
        assert_eq!(simp("x + 1 - x"), "1");
        assert_eq!(simp("x^2 / x"), "x");
        assert_eq!(simp("6*a*b / (3*a)"), "2 * b");
        assert_eq!(simp("x / 2 + x / 2"), "x");
        assert_eq!(simp("sqrt(8)"), "2 * sqrt(2)");
        assert_eq!(simp("sqrt(x^2)"), "x");
        assert_eq!(simp("sin(0) + cos(0)"), "1");
        assert_eq!(simp("1/3 + 1/6"), "1 / 2");
        // producten van sommen blijven staan
        assert_eq!(simp("(x + 1) * (x + 1)"), "(x + 1)^2");
    }

    #[test]
    fn uitwerken() {
        assert_eq!(print(&expand(&p("(x + 1)^2")).unwrap()), "x^2 + 2 * x + 1");
        assert_eq!(print(&expand(&p("(a + b) * (a - b)")).unwrap()), "a^2 - b^2");
        assert_eq!(print(&expand(&p("2*(x + 3)")).unwrap()), "2 * x + 6");
    }

    #[test]
    fn afleiden() {
        let dd = |s: &str| print(&diff(&p(s), "x").unwrap());
        assert_eq!(dd("x^3 + 2*x"), "3 * x^2 + 2");
        assert_eq!(dd("sin(x)"), "cos(x)");
        assert_eq!(dd("x * exp(x)"), "x * exp(x) + exp(x)");
        assert_eq!(dd("ln(x)"), "1 / x");
        assert_eq!(dd("a * x^2"), "2 * a * x");
        assert_eq!(dd("sqrt(x)"), "1 / (2 * sqrt(x))");
    }

    #[test]
    fn integreren() {
        let ii = |s: &str| print(&integrate(&p(s), "x").unwrap());
        assert_eq!(ii("3*x^2"), "x^3");
        assert_eq!(ii("x"), "x^2 / 2");
        assert_eq!(ii("cos(x)"), "sin(x)");
        assert_eq!(ii("1/x"), "ln(x)");
        // bepaalde integraal via de vereenvoudiging
        assert_eq!(simp("integral(x^2, x, 0, 3)"), "9");
        assert!(integrate(&p("sin(x^2)"), "x").is_none());
    }

    #[test]
    fn oplossen() {
        let s = |eq: &str| {
            let (l, r) = crate::solve::expr_sides(&p(eq)).unwrap();
            solve(&l, &r, "x").unwrap().iter().map(print).collect::<Vec<_>>()
        };
        assert_eq!(s("2*x + 3 == 7"), vec!["2"]);
        assert_eq!(s("x^2 - 4 == 0"), vec!["-2", "2"]);
        assert_eq!(s("x^2 - 2*x + 1 == 0"), vec!["1"]);
        assert_eq!(s("a*x + b == 0"), vec!["-b / a"]);
        assert_eq!(s("x^2 == 2"), vec!["-sqrt(2)", "sqrt(2)"]);
        let (l, r) = crate::solve::expr_sides(&p("x^2 == -1")).unwrap();
        assert!(solve(&l, &r, "x").is_err());
    }

    #[test]
    fn ontbinden() {
        let f = |s: &str| print(&factor(&p(s)).unwrap());
        assert_eq!(f("x^2 - 4"), "(x - 2) * (x + 2)");
        assert_eq!(f("2*x^2 + 4*x"), "2 * x * (x + 2)");
        assert_eq!(f("x^2 + 2*x + 1"), "(x + 1)^2");
        assert_eq!(f("6*a*b + 9*a"), "3 * a * (2 * b + 3)");
    }

    #[test]
    fn substitueren() {
        let e = substitute(&p("x^2 + y"), "x", &p("t + 1"));
        assert_eq!(print(&expand(&e).unwrap()), "t^2 + 2 * t + y + 1");
    }
}

#[cfg(test)]
mod taal {
    use crate::Env;

    fn run(src: &str) -> String {
        let mut env = Env::new();
        let mut last = String::new();
        for line in src.lines() {
            if let Some(out) = env.run_line(line).unwrap_or_else(|e| panic!("{line}: {e}")) {
                last = out;
            }
        }
        last
    }

    #[test]
    fn de_pijl_in_de_taal() {
        assert_eq!(run("(x + 1)^2 → expand"), "(x + 1)^2 → x^2 + 2 * x + 1");
        assert_eq!(run("x^2 - 4 == 0 → solve, x"), "x^2 - 4 == 0 → [-2; 2]");
        assert_eq!(run("x^2 - 5*x + 6 → factor"), "x^2 - 5*x + 6 → (x - 3) * (x - 2)");
        assert_eq!(run("a*x^2 + b → substitute, x = 2"), "a*x^2 + b → 4 * a + b");
        // bekende waarden worden ingevuld, eigen functies uitgeschreven (zoals Mathcad)
        assert_eq!(run("a := 3\nf(t) := t^2 + a\nf(y) + y →"), "f(y) + y → y^2 + y + 3");
        assert_eq!(run("deriv(x^3, x, x) →"), "deriv(x^3, x, x) → 3 * x^2");
        assert_eq!(run("integral(t, t, 0, x) → simplify"), "integral(t, t, 0, x) → x^2 / 2");
        // een onbekend sleutelwoord geeft een duidelijke fout
        assert!(Env::new().run_line("x → vereenvoudigen").is_err());
    }

    #[test]
    fn limieten_reeksen_integralen_float() {
        // l'Hôpital
        assert_eq!(run("sin(x) / x → limit, x = 0"), "sin(x) / x → 1");
        assert_eq!(run("(x^2 - 1) / (x - 1) → limit, x = 1"), "(x^2 - 1) / (x - 1) → 2");
        assert_eq!(run("(1 - cos(x)) / x^2 → limit, x = 0"), "(1 - cos(x)) / x^2 → 1 / 2");
        // naar oneindig
        assert_eq!(run("(3*x^2 + 1) / (x^2 + 5) → limit, x = ∞"), "(3*x^2 + 1) / (x^2 + 5) → 3");
        // Taylor rond 0 tot en met x^4
        assert_eq!(run("cos(x) → series, x, 5"), "cos(x) → x^4 / 24 - x^2 / 2 + 1");
        assert_eq!(run("exp(x) → series, x = 0, 3"), "exp(x) → x^2 / 2 + x + 1");
        // integralen met een lineair argument en partieel
        assert_eq!(run("integral(sin(2*x), x, 0, π/2) →"), "integral(sin(2*x), x, 0, π/2) → 1");
        assert_eq!(run("integral(x * exp(x), x, 0, 1) →"), "integral(x * exp(x), x, 0, 1) → 1");
        assert_eq!(run("integral(1 / (2*x + 1), x, 0, 1) → float, 6"), "integral(1 / (2*x + 1), x, 0, 1) → 0.549306");
        // float
        assert_eq!(run("sqrt(2) * x → float, 4"), "sqrt(2) * x → 1.414 * x");
    }

    #[test]
    fn eenheden_bij_de_pijl() {
        // waarden met eenheid worden ingevuld als getal maal eenheid
        assert_eq!(run("F := 10 kN\nF * x →"), "F * x → 10 * kN * x");
        assert_eq!(run("q := 3 kN/m\nq * x^2 / 2 →"), "q * x^2 / 2 → 3 * kN * x^2 / (2 * m)");
        // alleen getallen en eenheden over: één grootheid
        assert_eq!(run("F := 10 kN\na := 2 m\nF * a →"), "F * a → 20 * kN * m");
        assert_eq!(run("a := 1 m\nb := 50 cm\na + b →"), "a + b → 1.5 * m");
        assert_eq!(run("a := 1 m\nb := 50 cm\na / b →"), "a / b → 2");
        // de onbekende bij solve blijft staan
        assert_eq!(run("k := 2 N/mm\nk * u == 10 N → solve, u"), "k * u == 10 N → 5 * mm");
    }
}

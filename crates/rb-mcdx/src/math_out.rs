//! Formules van MathPrime als Mathcad-XML (`ml:*`): de omkering van `math.rs`.

use rb_core::{BinOp, BindOp, CmpOp, Expr, Stmt, SymOp};
use std::collections::BTreeSet;

/// Wat een naam op het blad is: bepaalt het label in Mathcad (variabele, eenheid, functie).
#[derive(Clone)]
pub struct Names {
    /// Namen die het blad definieert (variabelen en eigen functies), plus parameters.
    pub defined: BTreeSet<String>,
    /// Eigen functies.
    pub functions: BTreeSet<String>,
}

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

type R = Result<String, String>;

/// Een naam als `ml:id`; `M.max` wordt M met subscript max.
pub fn id(name: &str, label: Option<&str>) -> String {
    let label = label.map_or(String::new(), |l| format!(" labels=\"{l}\""));
    let content = match name.split_once('.') {
        Some((base, sub)) if !base.is_empty() && !sub.is_empty() => {
            // een accent hoort in Mathcad bij de basis: P.T' wordt P' met subscript T
            let primes = sub.len() - sub.trim_end_matches('\'').len();
            let sub = &sub[..sub.len() - primes];
            format!(
                "<Span xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\" xmlns:pw=\"clr-namespace:Ptc.Wpf;assembly=Ptc.Core\">{}{}<pw:Subscript>{}</pw:Subscript></Span>",
                esc(base),
                "'".repeat(primes),
                esc(sub)
            )
        }
        _ => esc(name),
    };
    format!("<ml:id xml:space=\"preserve\"{label}>{content}</ml:id>")
}

fn apply(op: &str, args: &[String]) -> String {
    format!("<ml:apply><ml:{op} />{}</ml:apply>", args.concat())
}

fn parens(s: String) -> String {
    format!("<ml:parens>{s}</ml:parens>")
}

fn real(v: f64) -> String {
    let s = if v.fract() == 0.0 && v.abs() < 1e15 { format!("{}", v as i64) } else { format!("{v}") };
    format!("<ml:real>{s}</ml:real>")
}

/// Voorrang zoals Mathcad hem toont: optellen < vermenigvuldigen < min < macht < atoom.
fn prec(e: &Expr) -> u8 {
    match e {
        Expr::Bin(BinOp::Add | BinOp::Sub, ..) => 1,
        Expr::Bin(BinOp::Mul, ..) => 2,
        Expr::Bin(BinOp::Div, ..) => 4, // een breuk is in Mathcad tweedimensionaal
        Expr::Neg(_) => 3,
        Expr::Bin(BinOp::Pow, ..) => 4,
        Expr::Cmp(..) | Expr::And(..) | Expr::Or(..) | Expr::Not(_) => 0,
        _ => 5,
    }
}

/// Is dit alleen eenheden (en getallen)? Dan is `5 · m` in Mathcad `scale`.
fn unit_only(e: &Expr, names: &Names) -> bool {
    match e {
        Expr::Var(n) => !names.defined.contains(n) && rb_units::lookup_unit(n).is_some(),
        Expr::Bin(BinOp::Mul | BinOp::Div, a, b) => unit_only(a, names) && unit_only(b, names),
        Expr::Bin(BinOp::Pow, a, b) => unit_only(a, names) && matches!(**b, Expr::Num(_) | Expr::Neg(_)),
        _ => false,
    }
}

/// Een eenheid (achter `=`): elke naam is daar een eenheid.
pub fn unit(e: &Expr) -> R {
    match e {
        Expr::Var(n) => Ok(id(n, Some("UNIT"))),
        Expr::Num(v) => Ok(real(*v)),
        Expr::Neg(x) => Ok(apply("neg", &[unit(x)?])),
        Expr::Bin(op, a, b) => {
            let o = match op {
                BinOp::Mul => "mult",
                BinOp::Div => "div",
                BinOp::Pow => "pow",
                _ => return Err("een eenheid met + of −".into()),
            };
            let a = if *op == BinOp::Pow && !matches!(**a, Expr::Var(_)) { parens(unit(a)?) } else { unit(a)? };
            Ok(apply(o, &[a, unit(b)?]))
        }
        _ => Err("deze eenheid".into()),
    }
}

pub fn expr(e: &Expr, n: &Names) -> R {
    let wrap = |x: &Expr, min: u8| -> R {
        let s = expr(x, n)?;
        Ok(if prec(x) < min { parens(s) } else { s })
    };
    Ok(match e {
        Expr::Num(v) => real(*v),
        Expr::Str(s) => format!("<ml:str xml:space=\"preserve\">{}</ml:str>", esc(s)),
        Expr::Var(name) => {
            let label = if n.defined.contains(name) {
                Some("VARIABLE")
            } else if rb_units::lookup_unit(name).is_some() {
                Some("UNIT")
            } else {
                None
            };
            id(name, label)
        }
        Expr::Neg(x) => apply("neg", &[wrap(x, 3)?]),
        Expr::Bin(BinOp::Mul, a, b) if matches!(**a, Expr::Num(_)) && unit_only(b, n) => apply("scale", &[expr(a, n)?, unit(b)?]),
        Expr::Bin(op, a, b) => match op {
            BinOp::Add => apply("plus", &[wrap(a, 1)?, wrap(b, 1)?]),
            BinOp::Sub => apply("minus", &[wrap(a, 1)?, wrap(b, 2)?]),
            BinOp::Mul => apply("mult", &[wrap(a, 2)?, wrap(b, 2)?]),
            BinOp::Div => apply("div", &[expr(a, n)?, expr(b, n)?]),
            BinOp::Pow => apply("pow", &[wrap(a, 5)?, expr(b, n)?]),
        },
        Expr::Cmp(ops, xs) => {
            // een keten a == b == c als geneste vergelijkingen (zoals Mathcad een uitleg-keten bewaart)
            let mut acc = expr(&xs[0], n)?;
            for (k, op) in ops.iter().enumerate() {
                let o = match op {
                    CmpOp::Eq => "equal",
                    CmpOp::Ne => "notEqual",
                    CmpOp::Lt => "lessThan",
                    CmpOp::Le => "lessOrEqual",
                    CmpOp::Gt => "greaterThan",
                    CmpOp::Ge => "greaterOrEqual",
                };
                acc = apply(o, &[acc, expr(&xs[k + 1], n)?]);
            }
            acc
        }
        Expr::And(a, b) => apply("and", &[expr(a, n)?, expr(b, n)?]),
        Expr::Or(a, b) => apply("or", &[expr(a, n)?, expr(b, n)?]),
        Expr::Not(x) => apply("not", &[expr(x, n)?]),
        Expr::Call(f, args) => call(f, args, n)?,
        Expr::Index(v, idx) => {
            let i = if idx.len() == 1 { expr(&idx[0], n)? } else { sequence(idx, n)? };
            apply("indexer", &[wrap(v, 5)?, i])
        }
        Expr::Matrix(rows) => {
            let (r, c) = (rows.len(), rows.first().map_or(0, Vec::len));
            // Mathcad bewaart een matrix kolom voor kolom
            let mut cells = String::new();
            for j in 0..c {
                for row in rows {
                    cells.push_str(&expr(row.get(j).ok_or("een matrix met ongelijke rijen")?, n)?);
                }
            }
            format!("<ml:matrix rows=\"{r}\" cols=\"{c}\">{cells}</ml:matrix>")
        }
        Expr::Range { start, second, end } => match second {
            Some(s) => format!("<ml:range><ml:sequence>{}{}</ml:sequence>{}</ml:range>", expr(start, n)?, expr(s, n)?, expr(end, n)?),
            None => format!("<ml:range>{}{}</ml:range>", expr(start, n)?, expr(end, n)?),
        },
        Expr::Bind { op, body, var, args } => bind(*op, body, var, args, n)?,
        Expr::Imag(_) => return Err("een imaginair getal (zoals 2i)".into()),
        Expr::Block(_) => return Err("een programmablok".into()),
        Expr::Interp(..) => return Err("een tabelfunctie".into()),
    })
}

fn sequence(args: &[Expr], n: &Names) -> R {
    Ok(format!("<ml:sequence>{}</ml:sequence>", args.iter().map(|a| expr(a, n)).collect::<Result<Vec<_>, _>>()?.concat()))
}

fn args_xml(args: &[Expr], n: &Names) -> R {
    if args.len() == 1 {
        expr(&args[0], n)
    } else {
        sequence(args, n)
    }
}

fn call(f: &str, args: &[Expr], n: &Names) -> R {
    let one = |op: &str| -> R {
        match args {
            [x] => Ok(apply(op, &[expr(x, n)?])),
            _ => Err(format!("{f} met {} argumenten", args.len())),
        }
    };
    match f {
        "sqrt" => match args {
            [x] => Ok(apply("nthRoot", &["<ml:placeholder />".into(), expr(x, n)?])),
            _ => Err("sqrt met meer argumenten".into()),
        },
        "nthroot" => match args {
            [x, k] => Ok(apply("nthRoot", &[expr(k, n)?, expr(x, n)?])),
            _ => Err("nthroot met een verkeerd aantal argumenten".into()),
        },
        "abs" => one("absval"),
        "factorial" => one("factorial"),
        "transpose" => one("transpose"),
        "det" => one("determinant"),
        // Mathcad kent geen inverse(): de inverse is M^-1
        "inverse" => match args {
            [m] => Ok(apply("pow", &[expr(m, n)?, apply("neg", &[real(1.0)])])),
            _ => Err("inverse met meer argumenten".into()),
        },
        _ => {
            // oplossers zijn in Mathcad sleutelwoorden in kleine letters
            let (name, label) = match f {
                "Find" | "Minerr" | "Minimize" | "Maximize" => (f.to_ascii_lowercase(), "KEYWORD"),
                _ if n.functions.contains(f) => (f.to_string(), "VARIABLE"),
                _ => (f.to_string(), "FUNCTION"),
            };
            Ok(format!("<ml:apply>{}{}</ml:apply>", id(&name, Some(label)), args_xml(args, n)?))
        }
    }
}

fn bind(op: BindOp, body: &Expr, var: &str, args: &[Expr], n: &Names) -> R {
    let lambda = || -> R { Ok(format!("<ml:lambda><ml:boundVars>{}</ml:boundVars>{}</ml:lambda>", id(var, Some("VARIABLE")), expr(body, n)?)) };
    let bounds = |lo: &Expr, hi: &Expr| -> R {
        Ok(format!("<ml:lowerBound>{}</ml:lowerBound><ml:upperBound>{}</ml:upperBound>", expr(lo, n)?, expr(hi, n)?))
    };
    match (op, args) {
        (BindOp::Integral, [lo, hi]) => Ok(format!("<ml:apply><ml:integral />{}{}</ml:apply>", lambda()?, bounds(lo, hi)?)),
        (BindOp::Sum, [lo, hi]) => Ok(format!("<ml:apply><ml:summation />{}{}</ml:apply>", lambda()?, bounds(lo, hi)?)),
        (BindOp::Prod, [lo, hi]) => Ok(format!("<ml:apply><ml:product />{}{}</ml:apply>", lambda()?, bounds(lo, hi)?)),
        // d/dx f(x), in de huidige waarde van x
        (BindOp::Deriv, [Expr::Var(p)]) if p == var => Ok(format!("<ml:apply><ml:derivative />{}</ml:apply>", lambda()?)),
        (BindOp::Root, _) => {
            let mut all = vec![body.clone(), Expr::Var(var.to_string())];
            all.extend(args.iter().cloned());
            // root is in Mathcad een sleutelwoord, net als find
            Ok(format!("<ml:apply>{}{}</ml:apply>", id("root", Some("KEYWORD")), sequence(&all, n)?))
        }
        (BindOp::Deriv, _) => Err("een afgeleide in een ander punt of van hogere orde".into()),
        (BindOp::OdeSolve, _) => Err("odesolve".into()),
        _ => Err("een som, product of integraal met een onbekende vorm".into()),
    }
}

/// De weergave-eenheid achter `=` (leeg vakje als er geen is).
fn unit_override(u: &Option<(Expr, String)>) -> R {
    Ok(format!(
        "<ml:unitOverride>{}</ml:unitOverride>",
        match u {
            Some((e, _)) => unit(e)?,
            None => "<ml:placeholder />".into(),
        }
    ))
}

/// Een hele regel als inhoud van `<math>`.
pub fn statement(s: &Stmt, n: &Names) -> R {
    match s {
        Stmt::Define { name, params, index, body, show } => {
            let lhs = match (params, index) {
                (Some(p), _) => format!(
                    "<ml:function>{}<ml:boundVars>{}</ml:boundVars></ml:function>",
                    id(name, Some("VARIABLE")),
                    p.iter().map(|x| id(x, Some("VARIABLE"))).collect::<String>()
                ),
                (None, Some(idx)) => {
                    let i = if idx.len() == 1 { expr(&idx[0], n)? } else { sequence(idx, n)? };
                    apply("indexer", &[id(name, Some("VARIABLE")), i])
                }
                (None, None) => id(name, Some("VARIABLE")),
            };
            // in de body van f(x) := ... is x een variabele, ook als x ook een eenheid is
            let with_params;
            let n = match params {
                Some(p) => {
                    let mut m = n.clone();
                    m.defined.extend(p.iter().cloned());
                    with_params = m;
                    &with_params
                }
                None => n,
            };
            let rhs = expr(body, n)?;
            let rhs = match show {
                Some(sh) => format!("<ml:eval>{rhs}{}</ml:eval>", unit_override(&sh.unit)?),
                None => rhs,
            };
            Ok(format!("<ml:define>{lhs}{rhs}</ml:define>"))
        }
        Stmt::Eval { expr: e, unit, .. } => Ok(format!("<ml:eval>{}{}</ml:eval>", expr(e, n)?, unit_override(unit)?)),
        // symbolisch: de uitdrukking, het sleutelwoord als `command`; Mathcad rekent de uitkomst zelf
        Stmt::Symbolic { expr: e, op, .. } => {
            let command = match op {
                SymOp::Evaluate => String::new(),
                SymOp::Simplify | SymOp::Expand | SymOp::Factor => id(&op.keyword(), Some("KEYWORD")),
                // een sleutelwoord met argument: `<ml:apply>sleutelwoord argument</ml:apply>`
                // (de vorm die Mathcad zelf opslaat)
                SymOp::Solve(x) => format!("<ml:apply>{}{}</ml:apply>", id("solve", Some("KEYWORD")), id(x, Some("VARIABLE"))),
                SymOp::Substitute(x, v) => format!(
                    "<ml:apply>{}{}</ml:apply>",
                    id("substitute", Some("KEYWORD")),
                    apply("equal", &[id(x, Some("VARIABLE")), expr(v, n)?])
                ),
            };
            let command = if command.is_empty() { command } else { format!("<ml:command>{command}</ml:command>") };
            Ok(format!("<ml:symEval>{}{command}</ml:symEval>", expr(e, n)?))
        }
    }
}

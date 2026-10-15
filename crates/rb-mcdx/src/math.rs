//! Formules uit `worksheet.xml` (`ml:*`) omzetten naar de syntax van MathPrime.

use crate::xml::{Child, Node};

// voorrang: hoe hoger, hoe sterker het bindt
const CMP: u8 = 1;
const ADD: u8 = 2;
const MUL: u8 = 3;
const NEG: u8 = 4;
const POW: u8 = 5;
const ATOM: u8 = 6;

type R = Result<(String, u8), String>;

fn wrap((s, p): (String, u8), min: u8) -> String {
    if p < min {
        format!("({s})")
    } else {
        s
    }
}

/// Een naam uit `ml:id`: `V<Subscript>ref</Subscript>` wordt `V.ref`. Tekens die MathPrime niet in
/// namen kent, worden `_`.
pub fn name(n: &Node) -> String {
    let (mut base, mut sub) = (String::new(), String::new());
    fn walk(n: &Node, base: &mut String, sub: &mut String) {
        for c in &n.children {
            match c {
                Child::Text(t) => base.push_str(t),
                Child::Elem(e) if e.local() == "Subscript" => sub.push_str(&e.text()),
                Child::Elem(e) => walk(e, base, sub),
            }
        }
    }
    walk(n, &mut base, &mut sub);
    let clean = |s: &str| -> String {
        s.trim().chars().map(|c| if c.is_alphanumeric() || matches!(c, '_' | '\'' | '∞' | '°') { c } else { '_' }).collect()
    };
    let mut out = clean(&base);
    let sub = clean(&sub);
    if !sub.is_empty() {
        // een accent hoort achter de hele naam: P'.T wordt P.T'
        let primes = out.len() - out.trim_end_matches('\'').len();
        out.truncate(out.len() - primes);
        out.push('.');
        out.push_str(&sub);
        out.push_str(&"'".repeat(primes));
    }
    if out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

/// Een functienaam van Mathcad als functie van MathPrime (`find` → `Find`).
fn function_name(n: &str) -> String {
    let names = rb_core::builtins::NAMES;
    if names.contains(&n) {
        return n.to_string();
    }
    names.iter().find(|m| m.eq_ignore_ascii_case(n)).map_or(n.to_string(), |m| m.to_string())
}

/// Eenheden die in Mathcad anders heten.
fn unit_name(n: &str) -> String {
    match n {
        "ohm" => "Ω".into(),
        "yr" | "year" => "yr".into(),
        "deg" | "°" => "deg".into(),
        "lbf" | "kip" | "psi" | "ksi" => n.into(),
        _ => n.into(),
    }
}

fn elems(n: &Node) -> Vec<&Node> {
    n.elems().collect()
}

/// Een uitdrukking als tekst (zonder voorrang).
pub fn expr_text(n: &Node) -> Result<String, String> {
    Ok(expr(n)?.0)
}

fn args_of<'a>(args: &[&'a Node]) -> Vec<&'a Node> {
    // f(a, b) staat als apply(f, sequence(a, b))
    match args {
        [s] if s.local() == "sequence" => elems(s),
        _ => args.to_vec(),
    }
}

fn list(args: &[&Node]) -> Result<String, String> {
    Ok(args_of(args).into_iter().map(|a| expr(a).map(|e| e.0)).collect::<Result<Vec<_>, _>>()?.join(", "))
}

pub fn expr(n: &Node) -> R {
    match n.local() {
        "id" => {
            let nm = name(n);
            Ok(match n.attr("labels") {
                Some("UNIT") => (unit_name(&nm), ATOM),
                Some("FUNCTION") | Some("KEYWORD") => (function_name(&nm), ATOM),
                _ => (nm, ATOM),
            })
        }
        "real" => {
            let t = n.text().trim().to_string();
            Ok(if t.starts_with('-') { (t, NEG) } else { (t, ATOM) })
        }
        "parens" => Ok((format!("({})", expr_text(n.elems().next().ok_or("lege haakjes")?)?), ATOM)),
        "placeholder" => Err("een leeg vakje".into()),
        "str" => Ok((format!("\"{}\"", n.text().replace('"', "\"\"")), ATOM)),
        "sequence" => Ok((list(&elems(n))?, 0)),
        "matrix" => matrix(n),
        "range" => {
            let e = elems(n);
            match e.as_slice() {
                [seq, end] if seq.local() == "sequence" => {
                    let s = elems(seq);
                    let parts: Vec<String> = s.iter().map(|x| expr_text(x)).collect::<Result<_, _>>()?;
                    Ok((format!("{} .. {}", parts.join(", "), expr_text(end)?), 0))
                }
                [a, b] => Ok((format!("{} .. {}", expr_text(a)?, expr_text(b)?), 0)),
                _ => Err("bereik met een onbekende vorm".into()),
            }
        }
        "function" => {
            let e = elems(n);
            let f = e.first().ok_or("functie zonder naam")?;
            let params = e.get(1).map(|b| elems(b).into_iter().map(name).collect::<Vec<_>>().join(", ")).unwrap_or_default();
            Ok((format!("{}({params})", name(f)), ATOM))
        }
        "apply" => apply(n),
        "eval" => Err("'=' binnen een uitdrukking".into()),
        other => Err(format!("'{other}' in een formule")),
    }
}

fn matrix(n: &Node) -> R {
    let rows: usize = n.attr("rows").and_then(|v| v.parse().ok()).ok_or("matrix zonder afmeting")?;
    let cols: usize = n.attr("cols").and_then(|v| v.parse().ok()).ok_or("matrix zonder afmeting")?;
    let e = elems(n);
    if e.len() != rows * cols {
        return Err("matrix met een onverwacht aantal elementen".into());
    }
    // Mathcad bewaart een matrix kolom voor kolom
    let mut text = Vec::new();
    for r in 0..rows {
        let row: Vec<String> = (0..cols).map(|c| expr_text(e[c * rows + r])).collect::<Result<_, _>>()?;
        text.push(row.join(", "));
    }
    Ok((format!("[{}]", text.join("; ")), ATOM))
}

fn bin(a: &Node, b: &Node, op: &str, p: u8) -> R {
    Ok((format!("{}{op}{}", wrap(expr(a)?, p), wrap(expr(b)?, p + 1)), p))
}

fn apply(n: &Node) -> R {
    let e = elems(n);
    let (op, args) = e.split_first().ok_or("lege toepassing")?;
    let one = || args.first().copied().ok_or_else(|| format!("'{}' zonder argument", op.local()));
    let two = || match args {
        [a, b] => Ok((*a, *b)),
        _ => Err(format!("'{}' verwacht twee argumenten", op.local())),
    };
    match op.local() {
        "plus" => {
            let (a, b) = two()?;
            Ok((format!("{} + {}", wrap(expr(a)?, ADD), wrap(expr(b)?, ADD)), ADD))
        }
        "minus" if args.len() == 1 => Ok((format!("-{}", wrap(expr(one()?)?, POW)), NEG)),
        "minus" => {
            let (a, b) = two()?;
            bin(a, b, " - ", ADD)
        }
        "neg" => Ok((format!("-{}", wrap(expr(one()?)?, POW)), NEG)),
        "mult" => {
            let (a, b) = two()?;
            Ok((format!("{} * {}", wrap(expr(a)?, MUL), wrap(expr(b)?, MUL)), MUL))
        }
        // getal maal eenheid: `75 mV`
        "scale" => {
            let (a, b) = two()?;
            let (na, nb) = (expr(a)?, expr(b)?);
            if a.local() == "real" && na.1 == ATOM && nb.1 == ATOM {
                Ok((format!("{} {}", na.0, wrap(nb, POW)), MUL))
            } else {
                Ok((format!("{} * {}", wrap(na, MUL), wrap(nb, MUL)), MUL))
            }
        }
        "div" => {
            let (a, b) = two()?;
            bin(a, b, " / ", MUL)
        }
        "pow" => {
            let (a, b) = two()?;
            Ok((format!("{}^{}", wrap(expr(a)?, ATOM), wrap(expr(b)?, ATOM)), POW))
        }
        "nthRoot" => {
            let (k, x) = two()?;
            if k.local() == "placeholder" {
                Ok((format!("sqrt({})", expr_text(x)?), ATOM))
            } else {
                Ok((format!("nthroot({}, {})", expr_text(x)?, expr_text(k)?), ATOM))
            }
        }
        "sqrt" => Ok((format!("sqrt({})", expr_text(one()?)?), ATOM)),
        "absval" => Ok((format!("abs({})", expr_text(one()?)?), ATOM)),
        "percent" => Ok((format!("({} / 100)", expr_text(one()?)?), ATOM)),
        "factorial" => Ok((format!("{}!", wrap(expr(one()?)?, ATOM)), POW)),
        "transpose" => Ok((format!("transpose({})", expr_text(one()?)?), ATOM)),
        "determinant" => Ok((format!("det({})", expr_text(one()?)?), ATOM)),
        "equal" => cmp(two()?, " == "),
        "lessThan" => cmp(two()?, " < "),
        "greaterThan" => cmp(two()?, " > "),
        "lessOrEqual" => cmp(two()?, " <= "),
        "greaterOrEqual" => cmp(two()?, " >= "),
        "notEqual" => cmp(two()?, " != "),
        "and" => cmp(two()?, " and "),
        "or" => cmp(two()?, " or "),
        "not" => Ok((format!("not {}", wrap(expr(one()?)?, NEG)), CMP)),
        "indexer" => {
            let (v, i) = args.split_first().ok_or("index zonder vector")?;
            Ok((format!("{}[{}]", wrap(expr(v)?, ATOM), list(i)?), ATOM))
        }
        "integral" => {
            // integral(lambda(var, body), lowerBound, upperBound)
            let lam = args.iter().find(|a| a.local() == "lambda").ok_or("integraal zonder functie")?;
            let lo = args.iter().find(|a| a.local() == "lowerBound").and_then(|b| b.elems().next()).ok_or("integraal zonder ondergrens")?;
            let hi = args.iter().find(|a| a.local() == "upperBound").and_then(|b| b.elems().next()).ok_or("integraal zonder bovengrens")?;
            let (var, body) = lambda(lam)?;
            Ok((format!("integral({body}, {var}, {}, {})", expr_text(lo)?, expr_text(hi)?), ATOM))
        }
        "summation" | "product" => {
            let lam = args.iter().find(|a| a.local() == "lambda").ok_or("som zonder functie")?;
            let lo = args.iter().find(|a| a.local() == "lowerBound").and_then(|b| b.elems().next()).ok_or("som zonder ondergrens")?;
            let hi = args.iter().find(|a| a.local() == "upperBound").and_then(|b| b.elems().next()).ok_or("som zonder bovengrens")?;
            let (var, body) = lambda(lam)?;
            let f = if op.local() == "summation" { "sum" } else { "prod" };
            Ok((format!("{f}({body}, {var}, {}, {})", expr_text(lo)?, expr_text(hi)?), ATOM))
        }
        "derivative" => {
            let lam = args.iter().find(|a| a.local() == "lambda").ok_or("afgeleide zonder functie")?;
            let (var, body) = lambda(lam)?;
            Ok((format!("deriv({body}, {var}, {var})"), ATOM))
        }
        // een functieaanroep: apply(id, argumenten)
        "id" => {
            let f = match op.attr("labels") {
                Some("VARIABLE") => name(op),
                _ => function_name(&name(op)),
            };
            Ok((format!("{f}({})", list(args)?), ATOM))
        }
        other => Err(format!("de operator '{other}'")),
    }
}

fn cmp((a, b): (&Node, &Node), op: &str) -> R {
    // een keten a = b = c (uitleg in Mathcad) zonder haakjes
    let left = expr(a)?;
    let chained = op == " == " && a.local() == "apply" && a.elems().next().is_some_and(|o| o.local() == "equal");
    let left = if chained { left.0 } else { wrap(left, ADD) };
    Ok((format!("{left}{op}{}", wrap(expr(b)?, ADD)), CMP))
}

fn lambda(l: &Node) -> Result<(String, String), String> {
    let e = elems(l);
    let vars = e.iter().find(|n| n.local() == "boundVars").ok_or("functie zonder variabele")?;
    let var = vars.elems().next().map(name).ok_or("functie zonder variabele")?;
    let body = e.iter().find(|n| n.local() != "boundVars").ok_or("functie zonder inhoud")?;
    Ok((var, expr_text(body)?))
}

/// De eenheid achter `=` (`ml:unitOverride`); leeg als er een leeg vakje staat.
fn unit_override(ev: &Node) -> Result<String, String> {
    match ev.child("unitOverride").and_then(|u| u.elems().next()) {
        Some(u) if u.local() != "placeholder" => Ok(format!(" {}", expr_text(u)?)),
        _ => Ok(String::new()),
    }
}

/// Een hele regio als één of meer regels MathPrime. Meer dan één regel als Mathcad een vector
/// links van `:=` heeft (`[a; b] := find(...)`): dan eerst een hulpvariabele, daarna de elementen.
pub fn statement(n: &Node, helpers: &mut usize) -> Result<Vec<String>, String> {
    match n.local() {
        "define" | "globalDefine" => {
            let e = elems(n);
            let [lhs, rhs] = e.as_slice() else { return Err("definitie met een onbekende vorm".into()) };
            let (body, show) = if rhs.local() == "eval" {
                let inner = rhs.elems().next().ok_or("leeg resultaat")?;
                (expr_text(inner)?, Some(unit_override(rhs)?))
            } else {
                (expr_text(rhs)?, None)
            };
            let show = show.map_or(String::new(), |u| format!(" ={u}"));
            if lhs.local() == "matrix" {
                // [a; b] := find(...): hulpvariabele en dan de elementen
                *helpers += 1;
                let h = format!("opl.{}", *helpers);
                let mut out = vec![format!("{h} := {body}")];
                for (k, v) in elems(lhs).into_iter().enumerate() {
                    out.push(format!("{} := {h}[{k}]{show}", expr_text(v)?));
                }
                return Ok(out);
            }
            Ok(vec![format!("{} := {body}{show}", expr_text(lhs)?)])
        }
        "eval" => {
            let inner = n.elems().next().ok_or("leeg resultaat")?;
            Ok(vec![format!("{} ={}", expr_text(inner)?, unit_override(n)?)])
        }
        // symbolisch: `uitdrukking → sleutelwoord`; de uitkomst (`symResult`) rekenen we zelf
        "symEval" => {
            let inner = n.elems().next().ok_or("lege symbolische regel")?;
            // Mathcad schrijft `<ml:command><ml:placeholder /></ml:command>` bij een kale pijl
            let commands: Vec<&Node> = n
                .elems()
                .filter(|c| c.local() == "command" && !c.elems().all(|k| k.local() == "placeholder"))
                .collect();
            let keyword = match commands.as_slice() {
                [] => String::new(),
                [c] => {
                    // `<ml:command><ml:apply>solve x</ml:apply></ml:command>`, of zonder apply
                    let parts = match elems(c).as_slice() {
                        [a] if a.local() == "apply" => elems(a),
                        p => p.to_vec(),
                    };
                    let k = parts.first().map(|k| name(k)).unwrap_or_default();
                    match (k.as_str(), parts.get(1)) {
                        ("simplify" | "expand" | "factor", None) => k,
                        ("solve", Some(x)) => format!("solve, {}", expr_text(x)?),
                        // `x = 2` staat als vergelijking in het bestand
                        ("substitute", Some(eq)) => format!("substitute, {}", expr_text(eq)?.replacen("==", "=", 1)),
                        _ => return Err(format!("symbolisch sleutelwoord '{k}' wordt niet ondersteund")),
                    }
                }
                _ => return Err("meerdere symbolische sleutelwoorden achter elkaar worden niet ondersteund".into()),
            };
            Ok(vec![format!("{} → {keyword}", expr_text(inner)?).trim_end().to_string()])
        }
        _ => Ok(vec![expr_text(n)?]),
    }
}

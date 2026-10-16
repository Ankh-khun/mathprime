//! Financiële functies zoals in Mathcad Prime (en Excel): `pv fv pmt nper rate npv irr mirr cnper crate
//! eff nom cumint cumprinc fvadj fvc`.
//!
//! Tekenafspraak zoals Excel: geld dat je ontvangt is positief, geld dat je betaalt negatief. Een lening
//! van 1000 (`pv = 1000`) geeft een negatieve aflossing. `type` is 0 (betaling aan het eind van de
//! periode, standaard) of 1 (aan het begin). Rentes en aantallen perioden zijn dimensieloos; bedragen
//! mogen een eenheid hebben, maar dan allemaal dezelfde.

use crate::error::Error;
use crate::fit::real_vector;
use crate::value::Value;
use rb_units::{Dim, Quantity};

pub const NAMES: &[&str] = &[
    "pv", "fv", "pmt", "nper", "rate", "npv", "irr", "mirr", "cnper", "crate", "eff", "nom", "cumint",
    "cumprinc", "fvadj", "fvc",
];

fn dom(name: &str, msg: &str) -> Error {
    Error::Domain(format!("{name}: {msg}"))
}

fn arity_range(name: &str, a: &[Value], lo: usize, hi: usize) -> Result<(), Error> {
    if (lo..=hi).contains(&a.len()) {
        Ok(())
    } else {
        Err(Error::Arity {
            name: name.to_string(),
            expected: if lo == hi { lo.to_string() } else { format!("{lo} tot {hi}") },
            got: a.len(),
        })
    }
}

fn num(name: &str, v: &Value) -> Result<Quantity, Error> {
    match v {
        Value::Num(q) => Ok(*q),
        other => Err(dom(name, &format!("verwacht een getal, geen {}", other.kind()))),
    }
}

/// Een dimensieloos getal (rente, aantal perioden, type).
fn plain(name: &str, v: &Value, what: &str) -> Result<f64, Error> {
    Ok(num(name, v)?.dimensionless_value(&format!("{what} in {name}"))?)
}

fn kind(name: &str, v: Option<&Value>) -> Result<f64, Error> {
    match v {
        None => Ok(0.0),
        Some(v) => match plain(name, v, "type")? {
            t if t == 0.0 || t == 1.0 => Ok(t),
            _ => Err(dom(name, "type moet 0 (eind van de periode) of 1 (begin) zijn")),
        },
    }
}

/// Bedragen: allemaal dezelfde eenheid. Ontbrekende bedragen tellen als 0.
fn amounts(name: &str, vs: &[Option<&Value>]) -> Result<(Vec<f64>, Dim), Error> {
    let mut dim: Option<Dim> = None;
    let mut out = Vec::with_capacity(vs.len());
    for v in vs {
        match v {
            None => out.push(0.0),
            Some(v) => {
                let q = num(name, v)?;
                match dim {
                    None => dim = Some(q.dim),
                    Some(d) if d != q.dim && q.value != 0.0 => {
                        return Err(dom(name, "alle bedragen moeten dezelfde eenheid hebben"));
                    }
                    _ => {}
                }
                out.push(q.value);
            }
        }
    }
    Ok((out, dim.unwrap_or(Dim::NONE)))
}

/// ((1 + r)^n − 1) / r, ook voor r → 0.
fn annuity(r: f64, n: f64) -> f64 {
    if r.abs() < 1e-12 {
        n
    } else {
        ((1.0 + r).powf(n) - 1.0) / r
    }
}

pub fn fv(r: f64, n: f64, pmt: f64, pv: f64, t: f64) -> f64 {
    -(pv * (1.0 + r).powf(n) + pmt * (1.0 + r * t) * annuity(r, n))
}

pub fn pv(r: f64, n: f64, pmt: f64, fv: f64, t: f64) -> f64 {
    -(fv + pmt * (1.0 + r * t) * annuity(r, n)) / (1.0 + r).powf(n)
}

pub fn pmt(r: f64, n: f64, pv: f64, fv: f64, t: f64) -> f64 {
    -(pv * (1.0 + r).powf(n) + fv) / ((1.0 + r * t) * annuity(r, n))
}

/// Newton op g met numerieke afgeleide, zonder onder −100 % rente te springen.
fn newton(g: impl Fn(f64) -> f64, guess: f64) -> Option<f64> {
    let mut r = guess;
    for _ in 0..200 {
        let h = 1e-7 * (1.0 + r.abs());
        let (f, d) = (g(r), (g(r + h) - g(r - h)) / (2.0 * h));
        if !f.is_finite() || d == 0.0 || !d.is_finite() {
            return None;
        }
        let mut step = f / d;
        while r - step <= -1.0 {
            step /= 2.0;
        }
        r -= step;
        if step.abs() <= 1e-13 * (1.0 + r.abs()) {
            return Some(r);
        }
    }
    None
}

/// Netto contante waarde van een reeks (bedrag i aan het eind van periode i + 1).
fn npv(r: f64, v: &[f64]) -> f64 {
    v.iter().enumerate().map(|(i, x)| x / (1.0 + r).powi(i as i32 + 1)).sum()
}

/// Rente in periode `per` (vanaf 1), zoals IPMT in Excel.
fn ipmt(r: f64, per: usize, n: f64, pv0: f64, t: f64) -> f64 {
    let p = pmt(r, n, pv0, 0.0, t);
    if t == 1.0 {
        if per == 1 {
            0.0
        } else {
            (fv(r, per as f64 - 2.0, p, pv0, 1.0) - p) * r
        }
    } else {
        fv(r, per as f64 - 1.0, p, pv0, 0.0) * r
    }
}

pub fn call(name: &str, a: &[Value]) -> Option<Result<Value, Error>> {
    if !NAMES.contains(&name) {
        return None;
    }
    Some(eval(name, a))
}

fn eval(name: &str, a: &[Value]) -> Result<Value, Error> {
    let q = |v: f64, d: Dim| Ok(Value::Num(Quantity::new(v, d)));
    let s = |v: f64| Ok(Value::Num(Quantity::scalar(v)));
    let rate_of = |k: usize| plain(name, &a[k], "de rente");
    let periods = |k: usize| -> Result<f64, Error> {
        let n = plain(name, &a[k], "het aantal perioden")?;
        if n <= 0.0 {
            return Err(dom(name, "het aantal perioden moet groter dan 0 zijn"));
        }
        Ok(n)
    };
    let finite = |v: f64| -> Result<f64, Error> {
        if v.is_finite() {
            Ok(v)
        } else {
            Err(dom(name, "geen eindige uitkomst voor deze invoer"))
        }
    };
    let both_signs = |v: &[f64]| -> Result<(), Error> {
        if v.iter().any(|x| *x > 0.0) && v.iter().any(|x| *x < 0.0) {
            Ok(())
        } else {
            Err(dom(name, "de reeks moet minstens één positief en één negatief bedrag hebben"))
        }
    };
    match name {
        // pv(rate, nper, pmt, [fv], [type]) en fv(rate, nper, pmt, [pv], [type])
        "pv" | "fv" => {
            arity_range(name, a, 3, 5)?;
            let (r, n) = (rate_of(0)?, periods(1)?);
            let (m, d) = amounts(name, &[Some(&a[2]), a.get(3)])?;
            let t = kind(name, a.get(4))?;
            let v = if name == "pv" { pv(r, n, m[0], m[1], t) } else { fv(r, n, m[0], m[1], t) };
            q(finite(v)?, d)
        }
        // pmt(rate, nper, pv, [fv], [type])
        "pmt" => {
            arity_range(name, a, 3, 5)?;
            let (r, n) = (rate_of(0)?, periods(1)?);
            let (m, d) = amounts(name, &[Some(&a[2]), a.get(3)])?;
            q(finite(pmt(r, n, m[0], m[1], kind(name, a.get(4))?))?, d)
        }
        // nper(rate, pmt, pv, [fv], [type])
        "nper" => {
            arity_range(name, a, 3, 5)?;
            let r = rate_of(0)?;
            let (m, _) = amounts(name, &[Some(&a[1]), Some(&a[2]), a.get(3)])?;
            let (p, pv0, fv0) = (m[0], m[1], m[2]);
            let t = kind(name, a.get(4))?;
            let n = if r.abs() < 1e-12 {
                -(pv0 + fv0) / p
            } else {
                let k = p * (1.0 + r * t);
                ((k - fv0 * r) / (k + pv0 * r)).ln() / (1.0 + r).ln()
            };
            s(finite(n)?)
        }
        // rate(nper, pmt, pv, [fv], [type], [guess])
        "rate" => {
            arity_range(name, a, 3, 6)?;
            let n = periods(0)?;
            let (m, _) = amounts(name, &[Some(&a[1]), Some(&a[2]), a.get(3)])?;
            let t = kind(name, a.get(4))?;
            let guess = match a.get(5) {
                Some(v) => plain(name, v, "de startwaarde")?,
                None => 0.1,
            };
            let r = newton(|r| fv(r, n, m[0], m[1], t) - m[2], guess);
            s(r.ok_or_else(|| dom(name, "geen rente gevonden; probeer een andere startwaarde"))?)
        }
        // npv(rate, v)
        "npv" => {
            arity_range(name, a, 2, 2)?;
            let (v, d) = real_vector(name, &a[1])?;
            q(npv(rate_of(0)?, &v), d)
        }
        // irr(v, [guess])
        "irr" => {
            arity_range(name, a, 1, 2)?;
            let (v, _) = real_vector(name, &a[0])?;
            both_signs(&v)?;
            let guess = match a.get(1) {
                Some(g) => plain(name, g, "de startwaarde")?,
                None => 0.1,
            };
            let g = |r: f64| v.iter().enumerate().map(|(i, x)| x / (1.0 + r).powi(i as i32)).sum::<f64>();
            s(newton(g, guess).ok_or_else(|| dom(name, "geen intern rendement gevonden; probeer een andere startwaarde"))?)
        }
        // mirr(v, fin_rate, reinv_rate)
        "mirr" => {
            arity_range(name, a, 3, 3)?;
            let (v, _) = real_vector(name, &a[0])?;
            let (fr, rr) = (rate_of(1)?, rate_of(2)?);
            let n = v.len();
            if n < 2 {
                return Err(dom(name, "vraagt minstens twee bedragen"));
            }
            both_signs(&v)?;
            let pos: f64 = v.iter().enumerate().filter(|(_, x)| **x > 0.0).map(|(i, x)| x * (1.0 + rr).powi((n - 1 - i) as i32)).sum();
            let neg: f64 = v.iter().enumerate().filter(|(_, x)| **x < 0.0).map(|(i, x)| x / (1.0 + fr).powi(i as i32)).sum();
            s((pos / -neg).powf(1.0 / (n - 1) as f64) - 1.0)
        }
        // cnper(rate, pv, fv): aantal perioden om van pv naar fv te groeien
        "cnper" => {
            arity_range(name, a, 3, 3)?;
            let r = rate_of(0)?;
            let (m, _) = amounts(name, &[Some(&a[1]), Some(&a[2])])?;
            if r <= -1.0 || r == 0.0 || m[0] == 0.0 || m[1] / m[0] <= 0.0 {
                return Err(dom(name, "vraagt een rente ≠ 0 en pv en fv met hetzelfde teken"));
            }
            s((m[1] / m[0]).ln() / (1.0 + r).ln())
        }
        // crate(nper, pv, fv): rente per periode om van pv naar fv te groeien
        "crate" => {
            arity_range(name, a, 3, 3)?;
            let n = periods(0)?;
            let (m, _) = amounts(name, &[Some(&a[1]), Some(&a[2])])?;
            if m[0] == 0.0 || m[1] / m[0] <= 0.0 {
                return Err(dom(name, "pv en fv moeten hetzelfde teken hebben"));
            }
            s((m[1] / m[0]).powf(1.0 / n) - 1.0)
        }
        // eff(rate, nper): effectieve rente bij een nominale rente die nper keer per jaar wordt bijgeschreven
        "eff" => {
            arity_range(name, a, 2, 2)?;
            let (r, n) = (rate_of(0)?, periods(1)?);
            s((1.0 + r / n).powf(n) - 1.0)
        }
        // nom(rate, nper): de omkering van eff
        "nom" => {
            arity_range(name, a, 2, 2)?;
            let (r, n) = (rate_of(0)?, periods(1)?);
            if r <= -1.0 {
                return Err(dom(name, "de rente moet groter dan −1 zijn"));
            }
            s(n * ((1.0 + r).powf(1.0 / n) - 1.0))
        }
        // cumint/cumprinc(rate, nper, pv, start, end, [type])
        "cumint" | "cumprinc" => {
            arity_range(name, a, 5, 6)?;
            let (r, n) = (rate_of(0)?, periods(1)?);
            let (m, d) = amounts(name, &[Some(&a[2])])?;
            let whole = |k: usize, what: &str| -> Result<usize, Error> {
                let x = plain(name, &a[k], what)?;
                if x.fract() != 0.0 || x < 1.0 || x > n {
                    return Err(dom(name, &format!("{what} moet een geheel getal van 1 tot nper zijn")));
                }
                Ok(x as usize)
            };
            let (from, to) = (whole(3, "de beginperiode")?, whole(4, "de eindperiode")?);
            if from > to {
                return Err(dom(name, "de beginperiode ligt na de eindperiode"));
            }
            let t = kind(name, a.get(5))?;
            let p = pmt(r, n, m[0], 0.0, t);
            let int: f64 = (from..=to).map(|k| ipmt(r, k, n, m[0], t)).sum();
            let v = if name == "cumint" { int } else { p * (to - from + 1) as f64 - int };
            q(finite(v)?, d)
        }
        // fvadj(prin, v): waarde van een hoofdsom na de rentes in v
        "fvadj" => {
            arity_range(name, a, 2, 2)?;
            let p = num(name, &a[0])?;
            let (v, _) = real_vector(name, &a[1])?;
            q(p.value * v.iter().map(|r| 1.0 + r).product::<f64>(), p.dim)
        }
        // fvc(rate, v): eindwaarde van bedragen aan het eind van elke periode
        "fvc" => {
            arity_range(name, a, 2, 2)?;
            let r = rate_of(0)?;
            let (v, d) = real_vector(name, &a[1])?;
            let n = v.len();
            q(v.iter().enumerate().map(|(i, x)| x * (1.0 + r).powi((n - 1 - i) as i32)).sum(), d)
        }
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Matrix;

    fn n(x: f64) -> Value {
        Value::Num(Quantity::scalar(x))
    }

    fn column(v: &[f64]) -> Value {
        Value::Matrix(Matrix::column(v.iter().map(|&x| Quantity::scalar(x)).collect()))
    }

    fn get(name: &str, a: &[Value]) -> f64 {
        match call(name, a).unwrap().unwrap() {
            Value::Num(q) => q.value,
            other => panic!("{other:?}"),
        }
    }

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() <= 1e-6 * (1.0 + b.abs()), "{a} ≠ {b}");
    }

    #[test]
    fn zoals_excel() {
        // voorbeelden uit de Excel-documentatie
        close(get("pmt", &[n(0.08 / 12.0), n(10.0), n(10000.0)]), -1037.032089);
        close(get("pv", &[n(0.08 / 12.0), n(240.0), n(500.0)]), -59777.14585);
        close(get("fv", &[n(0.06 / 12.0), n(10.0), n(-200.0), n(-500.0), n(1.0)]), 2581.403374);
        close(get("nper", &[n(0.12 / 12.0), n(-100.0), n(-1000.0), n(10000.0), n(1.0)]), 59.67386567);
        close(get("rate", &[n(48.0), n(-200.0), n(8000.0)]), 0.007701472);
        close(get("npv", &[n(0.1), column(&[-10000.0, 3000.0, 4200.0, 6800.0])]), 1188.443412);
        close(get("irr", &[column(&[-70000.0, 12000.0, 15000.0, 18000.0, 21000.0, 26000.0])]), 0.086630948);
        close(get("mirr", &[column(&[-120000.0, 39000.0, 30000.0, 21000.0, 37000.0, 46000.0]), n(0.1), n(0.12)]), 0.126094809);
        close(get("eff", &[n(0.0525), n(4.0)]), 0.053542667);
        close(get("nom", &[n(0.053543), n(4.0)]), 0.052500319);
        close(get("cumint", &[n(0.09 / 12.0), n(360.0), n(125000.0), n(13.0), n(24.0), n(0.0)]), -11135.23213);
        close(get("cumprinc", &[n(0.09 / 12.0), n(360.0), n(125000.0), n(13.0), n(24.0), n(0.0)]), -934.1071234);
        close(get("cumint", &[n(0.09 / 12.0), n(360.0), n(125000.0), n(1.0), n(1.0), n(0.0)]), -937.5);
        close(get("fvadj", &[n(1.0), column(&[0.09, 0.11, 0.1])]), 1.33089);
    }

    #[test]
    fn groei_en_rente_nul() {
        close(get("crate", &[n(10.0), n(100.0), n(200.0)]), 2f64.powf(0.1) - 1.0);
        close(get("cnper", &[n(0.05), n(100.0), n(200.0)]), 2f64.ln() / 1.05f64.ln());
        close(get("pmt", &[n(0.0), n(10.0), n(1000.0)]), -100.0);
        close(get("fvc", &[n(0.1), column(&[100.0, 100.0])]), 210.0);
        assert!(call("irr", &[column(&[1.0, 2.0])]).unwrap().is_err());
    }
}

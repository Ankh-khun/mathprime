//! Waarden met een onzekerheid: `L := 2 m ± 1 mm`. Foutenvoortplanting van de eerste orde (zoals de GUM).
//!
//! Elke `±` is een eigen bron met een nummer; een uitkomst onthoudt per bron hoeveel die bijdraagt
//! (gevoeligheid × onzekerheid). Daardoor valt de bron in `x - x` weg en telt hij in `x + x` dubbel, en worden
//! alleen onafhankelijke bronnen kwadratisch opgeteld. Optellen tot en met delen gaat exact; machten en
//! functies met een numerieke afgeleide in de nominale waarde.

use crate::ast::BinOp;
use crate::error::Error;
use crate::value::{self, Value};
use rb_units::{Dim, Quantity};
use std::sync::atomic::{AtomicU32, Ordering};

static NEXT_SOURCE: AtomicU32 = AtomicU32::new(1);

#[derive(Debug, Clone, PartialEq)]
pub struct Uncertain {
    pub value: f64,
    pub dim: Dim,
    /// Per bron de bijdrage aan de uitkomst, in SI.
    pub terms: Vec<(u32, f64)>,
}

fn dom(msg: impl Into<String>) -> Error {
    Error::Domain(msg.into())
}

impl Uncertain {
    /// `a ± u`: een nieuwe bron.
    pub fn new(a: Quantity, u: Quantity) -> Result<Value, Error> {
        a.same_dim(u, "±")?;
        if u.value < 0.0 {
            return Err(dom("de onzekerheid achter ± mag niet negatief zijn"));
        }
        let id = NEXT_SOURCE.fetch_add(1, Ordering::Relaxed);
        Ok(Value::Unc(Uncertain { value: a.value, dim: a.dim, terms: vec![(id, u.value)] }))
    }

    pub fn nominal(&self) -> Quantity {
        Quantity::new(self.value, self.dim)
    }

    /// De gecombineerde onzekerheid (wortel uit de som van de kwadraten).
    pub fn std(&self) -> f64 {
        self.terms.iter().map(|(_, t)| t * t).sum::<f64>().sqrt()
    }

    pub fn uncertainty(&self) -> Quantity {
        Quantity::new(self.std(), self.dim)
    }

    /// Dezelfde bronnen, maar met een andere waarde en eenheid en elke bijdrage maal `factor`.
    pub fn scaled(&self, value: f64, dim: Dim, factor: f64) -> Uncertain {
        Uncertain { value, dim, terms: self.terms.iter().map(|(i, t)| (*i, t * factor)).collect() }
    }
}

/// Tel de bijdragen per bron op; wat wegvalt, verdwijnt (ook numerieke ruis). Zonder bronnen: een gewoon getal.
fn merged(q: Quantity, parts: &[(&[(u32, f64)], f64)]) -> Value {
    let mut terms: Vec<(u32, f64)> = Vec::new();
    let mut largest = 0.0f64;
    for (source, factor) in parts {
        for (id, t) in source.iter() {
            let c = t * factor;
            largest = largest.max(c.abs());
            match terms.iter_mut().find(|(i, _)| i == id) {
                Some((_, sum)) => *sum += c,
                None => terms.push((*id, c)),
            }
        }
    }
    terms.retain(|(_, t)| t.abs() > 1e-12 * largest);
    if terms.is_empty() {
        Value::Num(q)
    } else {
        Value::Unc(Uncertain { value: q.value, dim: q.dim, terms })
    }
}

pub fn any(a: &[Value]) -> bool {
    a.iter().any(|v| matches!(v, Value::Unc(_)))
}

/// De waarde zonder onzekerheid.
pub fn plain(v: &Value) -> Value {
    match v {
        Value::Unc(u) => Value::Num(u.nominal()),
        other => other.clone(),
    }
}

fn no_terms() -> &'static [(u32, f64)] {
    &[]
}

/// Een bewerking waar een onzekere waarde bij zit. `None`: er zit er geen bij.
pub fn binop(op: BinOp, a: &Value, b: &Value) -> Option<Result<Value, Error>> {
    if !matches!(a, Value::Unc(_)) && !matches!(b, Value::Unc(_)) {
        return None;
    }
    Some(binop_inner(op, a, b))
}

fn binop_inner(op: BinOp, a: &Value, b: &Value) -> Result<Value, Error> {
    let number = |v: &Value| -> Result<Quantity, Error> {
        match v {
            Value::Num(q) => Ok(*q),
            Value::Unc(u) => Ok(u.nominal()),
            other => Err(dom(format!("een waarde met ± kan (nog) niet samen met een {}", other.kind()))),
        }
    };
    let (qa, qb) = (number(a)?, number(b)?);
    let terms = |v: &'_ Value| -> Vec<(u32, f64)> {
        match v {
            Value::Unc(u) => u.terms.clone(),
            _ => no_terms().to_vec(),
        }
    };
    let (ta, tb) = (terms(a), terms(b));
    let r = value::binop(op, &Value::Num(qa), &Value::Num(qb))?;
    let Value::Num(q) = r else { return Err(dom("een waarde met ± geeft hier geen getal")) };
    let (va, vb) = (qa.value, qb.value);
    let parts: Option<[f64; 2]> = match op {
        BinOp::Add => Some([1.0, 1.0]),
        BinOp::Sub => Some([1.0, -1.0]),
        BinOp::Mul => Some([vb, va]),
        BinOp::Div => Some([1.0 / vb, -va / (vb * vb)]),
        _ => None,
    };
    match parts {
        Some([da, db]) => Ok(merged(q, &[(&ta, da), (&tb, db)])),
        None => lift(&[a.clone(), b.clone()], &mut |args: &[Value]| value::binop(op, &args[0], &args[1])),
    }
}

/// Reken `f` uit met de nominale waarden en bepaal de gevoeligheden numeriek.
pub fn lift(args: &[Value], f: &mut dyn FnMut(&[Value]) -> Result<Value, Error>) -> Result<Value, Error> {
    let noms: Vec<Value> = args.iter().map(plain).collect();
    let y0 = match f(&noms)? {
        Value::Num(q) => q,
        other => return Err(dom(format!("een waarde met ± geeft hier geen getal maar een {}", other.kind()))),
    };
    let mut slopes: Vec<(Vec<(u32, f64)>, f64)> = Vec::new();
    for (k, arg) in args.iter().enumerate() {
        let Value::Unc(u) = arg else { continue };
        let s = u.std();
        if s == 0.0 {
            continue;
        }
        let h = 1e-4 * s;
        let at = |x: f64, f: &mut dyn FnMut(&[Value]) -> Result<Value, Error>| -> Option<f64> {
            let mut shifted = noms.clone();
            shifted[k] = Value::Num(Quantity::new(x, u.dim));
            match f(&shifted) {
                Ok(Value::Num(q)) if q.value.is_finite() => Some(q.value),
                _ => None,
            }
        };
        // centraal; bij de rand van het domein (√x bij 0) eenzijdig
        let slope = match (at(u.value + h, f), at(u.value - h, f)) {
            (Some(a), Some(b)) => (a - b) / (2.0 * h),
            (Some(a), None) => (a - y0.value) / h,
            (None, Some(b)) => (y0.value - b) / h,
            (None, None) => return Err(dom("de onzekerheid is hier niet te bepalen: de functie bestaat niet naast deze waarde")),
        };
        slopes.push((u.terms.clone(), slope));
    }
    let parts: Vec<(&[(u32, f64)], f64)> = slopes.iter().map(|(t, s)| (t.as_slice(), *s)).collect();
    Ok(merged(y0, &parts))
}

/// De functies van deze module; `None` voor andere namen.
pub fn call(name: &str, a: &[Value]) -> Option<Result<Value, Error>> {
    let arity = |n: usize| -> Result<(), Error> {
        if a.len() == n {
            Ok(())
        } else {
            Err(Error::Arity { name: name.to_string(), expected: n.to_string(), got: a.len() })
        }
    };
    Some(match name {
        // de vorm waarin `a ± b` in de boom staat
        "plusminus" => arity(2).and_then(|_| match (&a[0], &a[1]) {
            (Value::Num(x), Value::Num(u)) => Uncertain::new(*x, *u),
            // een waarde die zelf al onzeker is, krijgt er een bron bij
            (Value::Unc(x), Value::Num(u)) => {
                let Value::Unc(extra) = Uncertain::new(x.nominal(), *u)? else { unreachable!() };
                let mut terms = x.terms.clone();
                terms.extend(extra.terms);
                Ok(Value::Unc(Uncertain { value: x.value, dim: x.dim, terms }))
            }
            _ => Err(dom("± hoort tussen twee getallen, zoals 2 m ± 1 mm")),
        }),
        // nomval(x) en unc(x): de waarde en de onzekerheid los
        "nomval" => arity(1).map(|_| plain(&a[0])),
        "unc" => arity(1).and_then(|_| match &a[0] {
            Value::Unc(u) => Ok(Value::Num(u.uncertainty())),
            Value::Num(q) => Ok(Value::Num(Quantity::new(0.0, q.dim))),
            other => Err(dom(format!("unc werkt niet met een {}", other.kind()))),
        }),
        _ => return None,
    })
}

/// De tekst `(2.0000 ± 0.0010) m`: de onzekerheid in twee cijfers, de waarde tot op dezelfde plek. `div` en
/// `unit`: de weergave-eenheid.
pub fn text(u: &Uncertain, div: f64, unit: &str) -> String {
    let (v, s) = (u.value / div, u.std() / div);
    let body = if s > 0.0 && s.is_finite() {
        let decimals = 1 - s.abs().log10().floor() as i32;
        if (0..=12).contains(&decimals) && v.abs() < 1e12 {
            format!("{:.*} ± {:.*}", decimals as usize, v, decimals as usize, s)
        } else if decimals < 0 && v.abs() < 1e15 {
            // een onzekerheid groter dan 100: afronden voor de komma
            let f = 10f64.powi(-decimals);
            format!("{:.0} ± {:.0}", (v / f).round() * f, (s / f).round() * f)
        } else {
            format!("{} ± {}", rb_units::format_number(v), rb_units::format_number(s))
        }
    } else {
        format!("{} ± 0", rb_units::format_number(v))
    };
    if unit.is_empty() {
        body
    } else {
        format!("({body}) {unit}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(v: f64) -> Value {
        Value::Num(Quantity::scalar(v))
    }

    fn unc(v: f64, u: f64) -> Value {
        Uncertain::new(Quantity::scalar(v), Quantity::scalar(u)).unwrap()
    }

    fn parts(v: &Value) -> (f64, f64) {
        match v {
            Value::Unc(u) => (u.value, u.std()),
            Value::Num(q) => (q.value, 0.0),
            o => panic!("{o:?}"),
        }
    }

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() <= 1e-9 * (1.0 + b.abs()), "{a} ≠ {b}");
    }

    #[test]
    fn voortplanting() {
        let (x, y) = (unc(10.0, 0.3), unc(5.0, 0.4));
        // onafhankelijk: kwadratisch
        let s = value::binop(BinOp::Add, &x, &y).unwrap();
        close(parts(&s).1, 0.5);
        let p = value::binop(BinOp::Mul, &x, &y).unwrap();
        close(parts(&p).0, 50.0);
        close(parts(&p).1, ((5.0f64 * 0.3).powi(2) + (10.0f64 * 0.4).powi(2)).sqrt());
        let d = value::binop(BinOp::Div, &x, &y).unwrap();
        close(parts(&d).1, 2.0 * ((0.3f64 / 10.0).powi(2) + (0.4f64 / 5.0).powi(2)).sqrt());
        // dezelfde bron: niet kwadratisch
        assert_eq!(parts(&value::binop(BinOp::Sub, &x, &x).unwrap()), (0.0, 0.0));
        close(parts(&value::binop(BinOp::Add, &x, &x).unwrap()).1, 0.6);
        assert_eq!(parts(&value::binop(BinOp::Div, &x, &x).unwrap()), (1.0, 0.0));
        // met een gewoon getal
        close(parts(&value::binop(BinOp::Mul, &x, &q(3.0)).unwrap()).1, 0.9);
        // macht, numeriek: d(x²) = 2x·u
        close(parts(&value::binop(BinOp::Pow, &x, &q(2.0)).unwrap()).1, 6.0);
        // een getal min onzekerheid houdt het teken van de bijdrage goed
        let n = value::neg(&x).unwrap();
        close(parts(&value::binop(BinOp::Add, &n, &x).unwrap()).0, 0.0);
        assert_eq!(parts(&value::binop(BinOp::Add, &n, &x).unwrap()).1, 0.0);
    }

    #[test]
    fn functies_en_tekst() {
        let x = unc(4.0, 0.2);
        let r = crate::builtins::call("sqrt", std::slice::from_ref(&x), 0).unwrap().unwrap();
        close(parts(&r).0, 2.0);
        close(parts(&r).1, 0.05); // ½ x^-½ u
        close(parts(&crate::builtins::call("nomval", std::slice::from_ref(&x), 0).unwrap().unwrap()).0, 4.0);
        close(parts(&crate::builtins::call("unc", std::slice::from_ref(&x), 0).unwrap().unwrap()).0, 0.2);
        // de functie bestaat maar aan één kant van de waarde
        let edge = unc(0.0, 0.01);
        assert!(crate::builtins::call("sqrt", &[edge], 0).unwrap().is_ok());
        let Value::Unc(u) = unc(2.0, 0.001) else { unreachable!() };
        assert_eq!(text(&u, 1.0, "m"), "(2.0000 ± 0.0010) m");
        let Value::Unc(u) = unc(1234.6, 12.0) else { unreachable!() };
        assert_eq!(text(&u, 1.0, ""), "1235 ± 12");
        let Value::Unc(u) = unc(123456.0, 1200.0) else { unreachable!() };
        assert_eq!(text(&u, 1.0, ""), "123500 ± 1200");
    }
}

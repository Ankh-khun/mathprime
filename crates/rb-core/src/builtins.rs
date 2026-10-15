//! Ingebouwde functies. Hoeken zijn dimensieloos: `sin(30 deg)` werkt omdat `deg` π/180 is.
//!
//! Functies van één reëel getal werken ook element voor element op een vector of matrix
//! (`sqrt([4, 9])` is `[2, 3]`); in Mathcad moet je daar de vectorize-operator voor gebruiken.

use crate::error::Error;
use crate::linalg;
use crate::value::{self, Complex, Matrix, Value};
use rb_units::Quantity;
use std::f64::consts::PI;

fn arity<T>(name: &str, a: &[T], n: usize) -> Result<(), Error> {
    if a.len() == n {
        Ok(())
    } else {
        Err(Error::Arity {
            name: name.to_string(),
            expected: n.to_string(),
            got: a.len(),
        })
    }
}

fn at_least_one<T>(name: &str, a: &[T]) -> Result<(), Error> {
    if a.is_empty() {
        Err(Error::Arity {
            name: name.to_string(),
            expected: "minstens 1".to_string(),
            got: 0,
        })
    } else {
        Ok(())
    }
}

fn dimless(name: &str, q: Quantity) -> Result<f64, Error> {
    Ok(q.dimensionless_value(&format!("het argument van {name}"))?)
}

fn domain(name: &str, what: &str) -> Error {
    Error::Domain(format!("{name}: {what}"))
}

/// Alle argumenten moeten dezelfde dimensie hebben.
fn same_dims(a: &[Quantity], op: &'static str) -> Result<(), Error> {
    for q in &a[1..] {
        a[0].same_dim(*q, op)?;
    }
    Ok(())
}

/// Een dimensieloos geheel getal.
fn integer(name: &str, q: Quantity) -> Result<i64, Error> {
    let x = dimless(name, q)?;
    if x.fract() != 0.0 || x.abs() > 9.0e15 {
        return Err(domain(name, "verwacht een geheel getal"));
    }
    Ok(x as i64)
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// Functies van reële getallen. `None` betekent: geen reële functie met deze naam.
fn call_real(name: &str, a: &[Quantity]) -> Option<Result<Quantity, Error>> {
    let scalar = |r: f64| Ok(Quantity::scalar(r));
    Some(match name {
        "sqrt" => arity(name, a, 1).and_then(|_| {
            if a[0].value < 0.0 {
                return Err(domain(name, "wortel uit een negatief getal in een matrix (complexe matrices worden nog niet ondersteund)"));
            }
            Ok(a[0].root(2)?)
        }),
        "cbrt" => arity(name, a, 1).and_then(|_| Ok(a[0].root(3)?)),
        "root" => arity(name, a, 2).and_then(|_| {
            let n = dimless(name, a[1])?;
            if n.fract() != 0.0 || !(1.0..=64.0).contains(&n) {
                return Err(domain(name, "n moet een geheel getal van 1 tot en met 64 zijn"));
            }
            if a[0].value < 0.0 && (n as u32) % 2 == 0 {
                return Err(domain(name, "even wortel uit een negatief getal"));
            }
            Ok(a[0].root(n as u32)?)
        }),
        "abs" => arity(name, a, 1).map(|_| Quantity::new(a[0].value.abs(), a[0].dim)),
        "sin" | "cos" | "tan" | "cot" | "sec" | "csc" | "asin" | "acos" | "atan" | "acot" | "sinh"
        | "cosh" | "tanh" | "coth" | "asinh" | "acosh" | "atanh" | "acoth" | "exp" | "ln" | "log"
        | "log2" => arity(name, a, 1).and_then(|_| {
            let x = dimless(name, a[0])?;
            let r = match name {
                "sin" => x.sin(),
                "cos" => x.cos(),
                "tan" => x.tan(),
                "cot" => 1.0 / x.tan(),
                "sec" => 1.0 / x.cos(),
                "csc" => 1.0 / x.sin(),
                "asin" | "acos" => {
                    if x.abs() > 1.0 {
                        return Err(domain(name, "argument moet tussen -1 en 1 liggen"));
                    }
                    if name == "asin" {
                        x.asin()
                    } else {
                        x.acos()
                    }
                }
                "atan" => x.atan(),
                "acot" => PI / 2.0 - x.atan(),
                "sinh" => x.sinh(),
                "cosh" => x.cosh(),
                "tanh" => x.tanh(),
                "coth" => 1.0 / x.tanh(),
                "asinh" => x.asinh(),
                "acosh" => {
                    if x < 1.0 {
                        return Err(domain(name, "argument moet minstens 1 zijn"));
                    }
                    x.acosh()
                }
                "atanh" => {
                    if x.abs() >= 1.0 {
                        return Err(domain(name, "argument moet tussen -1 en 1 liggen (grenzen niet inbegrepen)"));
                    }
                    x.atanh()
                }
                "acoth" => {
                    if x.abs() <= 1.0 {
                        return Err(domain(name, "argument moet buiten [-1, 1] liggen"));
                    }
                    (1.0 / x).atanh()
                }
                "exp" => x.exp(),
                "ln" if x == 0.0 => return Err(domain(name, "ln(0) bestaat niet (−∞)")),
                _ => {
                    if x <= 0.0 {
                        return Err(domain(name, "argument moet groter dan 0 zijn"));
                    }
                    match name {
                        "ln" => x.ln(),
                        "log" => x.log10(),
                        _ => x.log2(),
                    }
                }
            };
            Ok(Quantity::scalar(r))
        }),
        "atan2" => arity(name, a, 2).and_then(|_| {
            a[0].same_dim(a[1], "atan2")?;
            Ok(Quantity::scalar(a[0].value.atan2(a[1].value)))
        }),
        "hypot" => at_least_one(name, a).and_then(|_| {
            same_dims(a, "hypot")?;
            let s: f64 = a.iter().map(|q| q.value * q.value).sum();
            Ok(Quantity::new(s.sqrt(), a[0].dim))
        }),
        "mod" => arity(name, a, 2).and_then(|_| {
            a[0].same_dim(a[1], "mod")?;
            if a[1].value == 0.0 {
                return Err(domain(name, "deling door nul"));
            }
            Ok(Quantity::new(a[0].value.rem_euclid(a[1].value), a[0].dim))
        }),
        "floor" | "ceil" | "trunc" => arity(name, a, 1).and_then(|_| {
            let x = dimless(name, a[0])?;
            scalar(match name {
                "floor" => x.floor(),
                "ceil" => x.ceil(),
                _ => x.trunc(),
            })
        }),
        "round" => {
            if a.len() == 1 || a.len() == 2 {
                (|| {
                    let x = dimless(name, a[0])?;
                    let digits = if a.len() == 2 { dimless(name, a[1])? } else { 0.0 };
                    if digits.fract() != 0.0 || digits.abs() > 15.0 {
                        return Err(domain(name, "het aantal decimalen moet een geheel getal zijn"));
                    }
                    let f = 10f64.powi(digits as i32);
                    Ok(Quantity::scalar((x * f).round() / f))
                })()
            } else {
                Err(Error::Arity {
                    name: name.to_string(),
                    expected: "1 of 2".to_string(),
                    got: a.len(),
                })
            }
        }
        // teken: -1, 0 of 1 (ook voor grootheden met eenheid)
        "sign" => arity(name, a, 1).and_then(|_| {
            let x = a[0].value;
            scalar(if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 })
        }),
        // Heaviside-stapfunctie, Φ(0) = 1 zoals in Mathcad
        "Φ" | "Heaviside" => arity(name, a, 1).and_then(|_| scalar(if a[0].value >= 0.0 { 1.0 } else { 0.0 })),
        // Kronecker-delta
        "δ" | "kronecker" => arity(name, a, 2).and_then(|_| {
            a[0].same_dim(a[1], "δ")?;
            scalar(if a[0].value == a[1].value { 1.0 } else { 0.0 })
        }),
        "gcd" | "lcm" => at_least_one(name, a).and_then(|_| {
            let mut acc: u64 = 0;
            for q in a {
                let v = integer(name, *q)?.unsigned_abs();
                acc = if name == "gcd" {
                    gcd(acc, v)
                } else if acc == 0 {
                    v
                } else if v == 0 {
                    0
                } else {
                    acc / gcd(acc, v) * v
                };
            }
            scalar(acc as f64)
        }),
        "factorial" | "fact" => arity(name, a, 1).and_then(|_| {
            let n = integer(name, a[0])?;
            if !(0..=170).contains(&n) {
                return Err(domain("faculteit", "n moet een geheel getal van 0 tot en met 170 zijn"));
            }
            scalar((1..=n).fold(1.0, |acc, k| acc * k as f64))
        }),
        "combin" | "permut" => arity(name, a, 2).and_then(|_| {
            let (n, k) = (integer(name, a[0])?, integer(name, a[1])?);
            if n < 0 || k < 0 || k > n {
                return Err(domain(name, "verwacht 0 ≤ k ≤ n"));
            }
            let perm = (0..k).fold(1.0, |acc, i| acc * (n - i) as f64);
            if name == "permut" {
                scalar(perm)
            } else {
                scalar((0..k).fold(1.0, |acc, i| acc * (n - i) as f64 / (i + 1) as f64).round())
            }
        }),
        _ => return call_special(name, a),
    })
}

/// Een kans (0 ≤ p ≤ 1, dimensieloos).
fn prob(name: &str, q: Quantity) -> Result<f64, Error> {
    let p = dimless(name, q)?;
    if !(0.0..=1.0).contains(&p) {
        return Err(domain(name, "een kans moet tussen 0 en 1 liggen"));
    }
    Ok(p)
}

fn positive(name: &str, what: &str, x: f64) -> Result<f64, Error> {
    if x > 0.0 {
        Ok(x)
    } else {
        Err(domain(name, &format!("{what} moet groter dan 0 zijn")))
    }
}

/// Speciale functies en kansverdelingen (zie [`crate::special`]).
fn call_special(name: &str, a: &[Quantity]) -> Option<Result<Quantity, Error>> {
    use crate::special as sp;
    let s = |r: f64| Ok(Quantity::scalar(r));
    let x = |k: usize| dimless(name, a[k]);
    let order = |k: usize| -> Result<i32, Error> {
        let n = integer(name, a[k])?;
        if n.abs() > 1000 {
            return Err(domain(name, "de orde moet een geheel getal tot 1000 zijn"));
        }
        Ok(n as i32)
    };
    // normale verdeling met optioneel μ en σ (mogen een eenheid hebben)
    let norm_args = |k: usize| -> Result<(f64, f64, rb_units::Dim), Error> {
        match a.len() {
            n if n == k + 1 => Ok((0.0, 1.0, rb_units::Dim::NONE)),
            n if n == k + 3 => {
                a[k + 1].same_dim(a[k + 2], "de normale verdeling (μ en σ)")?;
                Ok((a[k + 1].value, positive(name, "σ", a[k + 2].value)?, a[k + 1].dim))
            }
            n => Err(Error::Arity { name: name.to_string(), expected: format!("{} of {}", k + 1, k + 3), got: n }),
        }
    };
    Some(match name {
        "erf" | "erfc" | "Γ" | "gamma" | "lnGamma" | "Ψ" | "digamma" | "J0" | "J1" | "Y0" | "Y1"
        | "I0" | "I1" | "K0" | "K1" => arity(name, a, 1).and_then(|_| {
            let v = x(0)?;
            match name {
                "erf" => s(sp::erf(v)),
                "erfc" => s(sp::erfc(v)),
                "Γ" | "gamma" => s(sp::gamma(v)?),
                "lnGamma" => s(sp::ln_gamma(v)),
                "Ψ" | "digamma" => s(sp::digamma(v)?),
                "J0" => s(sp::bessel_j(0, v)),
                "J1" => s(sp::bessel_j(1, v)),
                "Y0" => s(sp::bessel_y(0, v)?),
                "Y1" => s(sp::bessel_y(1, v)?),
                "I0" => s(sp::bessel_i(0, v)),
                "I1" => s(sp::bessel_i(1, v)),
                "K0" => s(sp::bessel_k(0, v)?),
                _ => s(sp::bessel_k(1, v)?),
            }
        }),
        "Jn" | "Yn" | "In" | "Kn" => arity(name, a, 2).and_then(|_| {
            let (n, v) = (order(0)?, x(1)?);
            match name {
                "Jn" => s(sp::bessel_j(n, v)),
                "Yn" => s(sp::bessel_y(n, v)?),
                "In" => s(sp::bessel_i(n, v)),
                _ => s(sp::bessel_k(n, v)?),
            }
        }),
        "Beta" => arity(name, a, 2).and_then(|_| s(sp::beta(positive(name, "a", x(0)?)?, positive(name, "b", x(1)?)?))),
        // normale verdeling: dnorm(x, μ, σ), pnorm(x, μ, σ), qnorm(p, μ, σ)
        "dnorm" | "pnorm" => norm_args(0).and_then(|(mu, sigma, dim)| {
            if a[0].dim != dim {
                return Err(rb_units::UnitError::Mismatch { op: "de normale verdeling (x en μ)", left: a[0].dim, right: dim }.into());
            }
            let z = (a[0].value - mu) / sigma;
            if name == "pnorm" {
                s(sp::pnorm(z))
            } else {
                let inv = rb_units::Dim::NONE.checked_div(dim).ok_or(rb_units::UnitError::Overflow)?;
                Ok(Quantity::new(sp::dnorm(z) / sigma, inv))
            }
        }),
        "qnorm" => norm_args(0).and_then(|(mu, sigma, dim)| {
            let p = prob(name, a[0])?;
            Ok(Quantity::new(mu + sigma * sp::qnorm(p), dim))
        }),
        "dlnorm" | "plnorm" | "qlnorm" => arity(name, a, 3).and_then(|_| {
            let (v, mu, sigma) = (x(0)?, x(1)?, positive(name, "σ", x(2)?)?);
            match name {
                "dlnorm" if v <= 0.0 => s(0.0),
                "dlnorm" => s(sp::dnorm((v.ln() - mu) / sigma) / (sigma * v)),
                "plnorm" if v <= 0.0 => s(0.0),
                "plnorm" => s(sp::pnorm((v.ln() - mu) / sigma)),
                _ => s((mu + sigma * sp::qnorm(prob(name, a[0])?)).exp()),
            }
        }),
        "dt" | "pt" | "qt" | "dchisq" | "pchisq" | "qchisq" => arity(name, a, 2).and_then(|_| {
            let d = positive(name, "het aantal vrijheidsgraden", x(1)?)?;
            match name {
                "dt" => s(sp::dt(x(0)?, d)),
                "pt" => s(sp::pt(x(0)?, d)),
                "qt" => s(sp::invert(|t| sp::pt(t, d), prob(name, a[0])?, -10.0, 10.0)),
                "dchisq" => s(sp::dchisq(x(0)?, d)),
                "pchisq" => s(sp::pchisq(x(0)?, d)),
                _ => s(sp::invert(|t| sp::pchisq(t, d), prob(name, a[0])?, 0.0, 10.0)),
            }
        }),
        "dF" | "pF" | "qF" => arity(name, a, 3).and_then(|_| {
            let (d1, d2) = (positive(name, "d1", x(1)?)?, positive(name, "d2", x(2)?)?);
            match name {
                "dF" => s(sp::df(x(0)?, d1, d2)),
                "pF" => s(sp::pf(x(0)?, d1, d2)),
                _ => s(sp::invert(|t| sp::pf(t, d1, d2), prob(name, a[0])?, 0.0, 10.0)),
            }
        }),
        // exponentieel: x mag een eenheid hebben, r heeft dan de omgekeerde eenheid
        "dexp" | "pexp" | "qexp" => arity(name, a, 2).and_then(|_| {
            let r = a[1];
            positive(name, "r", r.value)?;
            if name == "qexp" {
                let p = prob(name, a[0])?;
                let dim = rb_units::Dim::NONE.checked_div(r.dim).ok_or(rb_units::UnitError::Overflow)?;
                return Ok(Quantity::new(-(1.0 - p).ln() / r.value, dim));
            }
            let rx = a[0].mul(r)?.dimensionless_value("r·x bij de exponentiële verdeling")?;
            match name {
                "dexp" => Ok(Quantity::new(if rx < 0.0 { 0.0 } else { r.value * (-rx).exp() }, r.dim)),
                _ => s(if rx < 0.0 { 0.0 } else { 1.0 - (-rx).exp() }),
            }
        }),
        "dunif" | "punif" | "qunif" => arity(name, a, 3).and_then(|_| {
            let (lo, hi) = (a[1], a[2]);
            lo.same_dim(hi, "de uniforme verdeling")?;
            if hi.value <= lo.value {
                return Err(domain(name, "b moet groter zijn dan a"));
            }
            let w = hi.value - lo.value;
            match name {
                "qunif" => Ok(Quantity::new(lo.value + prob(name, a[0])? * w, lo.dim)),
                _ => {
                    a[0].same_dim(lo, "de uniforme verdeling")?;
                    let v = a[0].value;
                    if name == "dunif" {
                        let inv = rb_units::Dim::NONE.checked_div(lo.dim).ok_or(rb_units::UnitError::Overflow)?;
                        Ok(Quantity::new(if (lo.value..=hi.value).contains(&v) { 1.0 / w } else { 0.0 }, inv))
                    } else {
                        s(((v - lo.value) / w).clamp(0.0, 1.0))
                    }
                }
            }
        }),
        "dbinom" | "pbinom" => arity(name, a, 3).and_then(|_| {
            let (k, n, p) = (x(0)?, integer(name, a[1])? as f64, prob(name, a[2])?);
            s(if name == "dbinom" { sp::dbinom(k, n, p) } else { sp::pbinom(k, n, p) })
        }),
        "dpois" | "ppois" => arity(name, a, 2).and_then(|_| {
            let (k, l) = (x(0)?, positive(name, "λ", x(1)?)?);
            s(if name == "dpois" { sp::dpois(k, l) } else { sp::ppois(k, l) })
        }),
        "dweibull" | "pweibull" | "qweibull" => arity(name, a, 2).and_then(|_| {
            let k = positive(name, "de vormparameter", x(1)?)?;
            match name {
                "dweibull" => {
                    let v = x(0)?;
                    s(if v < 0.0 { 0.0 } else { k * v.powf(k - 1.0) * (-v.powf(k)).exp() })
                }
                "pweibull" => {
                    let v = x(0)?;
                    s(if v < 0.0 { 0.0 } else { 1.0 - (-v.powf(k)).exp() })
                }
                _ => s((-(1.0 - prob(name, a[0])?).ln()).powf(1.0 / k)),
            }
        }),
        _ => return None,
    })
}


/// Namen van reële functies die element voor element op een matrix werken.
const ELEMENTWISE: &[&str] = &[
    "sqrt", "cbrt", "abs", "sin", "cos", "tan", "cot", "sec", "csc", "asin", "acos", "atan", "acot",
    "sinh", "cosh", "tanh", "coth", "asinh", "acosh", "atanh", "acoth", "exp", "ln", "log", "log2",
    "floor", "ceil", "trunc", "round", "sign", "Φ", "Heaviside", "factorial", "fact",
    "erf", "erfc", "Γ", "gamma", "lnGamma", "Ψ", "digamma", "J0", "J1", "Y0", "Y1", "I0", "I1", "K0", "K1",
];

fn mat<'a>(name: &str, v: &'a Value) -> Result<&'a Matrix, Error> {
    match v {
        Value::Matrix(m) => Ok(m),
        other => Err(domain(name, &format!("verwacht een vector of matrix, geen {}", other.kind()))),
    }
}

fn vector<'a>(name: &str, v: &'a Value) -> Result<&'a Matrix, Error> {
    let m = mat(name, v)?;
    if m.is_vector() {
        Ok(m)
    } else {
        Err(domain(name, &format!("verwacht een vector, geen {} matrix", m.shape())))
    }
}

/// Alle reële getallen uit de argumenten (getallen en elementen van matrices).
fn flatten(name: &str, a: &[Value]) -> Result<Vec<Quantity>, Error> {
    let mut out = Vec::new();
    for v in a {
        match v {
            Value::Num(q) => out.push(*q),
            Value::Matrix(m) if !m.is_complex() => out.extend_from_slice(&m.data),
            other => return Err(domain(name, &format!("werkt niet met een {}{}", if matches!(other, Value::Matrix(_)) { "complexe " } else { "" }, other.kind()))),
        }
    }
    if out.is_empty() {
        return Err(domain(name, "geen waarden"));
    }
    Ok(out)
}

fn mean_of(xs: &[Quantity]) -> Result<Quantity, Error> {
    same_dims(xs, "middelen")?;
    let s: f64 = xs.iter().map(|q| q.value).sum();
    Ok(Quantity::new(s / xs.len() as f64, xs[0].dim))
}

/// Variantie; `sample` deelt door n-1 (Var, Stdev), anders door n (var, stdev), zoals in Mathcad.
fn variance(name: &str, xs: &[Quantity], sample: bool) -> Result<Quantity, Error> {
    let m = mean_of(xs)?;
    let n = xs.len() as f64;
    if sample && xs.len() < 2 {
        return Err(domain(name, "vraagt minstens 2 waarden"));
    }
    let ss: f64 = xs.iter().map(|q| (q.value - m.value).powi(2)).sum();
    let dim = m.dim.checked_powi(2).ok_or(rb_units::UnitError::Overflow)?;
    Ok(Quantity::new(ss / if sample { n - 1.0 } else { n }, dim))
}

fn to_value(m: Matrix) -> Value {
    Value::Matrix(m)
}

/// Een getal of matrix als matrix (een getal is 1×1).
fn as_matrix(name: &str, v: &Value) -> Result<Matrix, Error> {
    match v {
        Value::Num(_) | Value::Complex(_) => Ok(Matrix::from_complex(1, 1, vec![v.as_complex(name)?])),
        Value::Matrix(m) => Ok(m.clone()),
        other => Err(domain(name, &format!("werkt niet met een {}", other.kind()))),
    }
}

/// Blokken naast elkaar (`augment`) of onder elkaar (`stack`); losse getallen tellen als 1×1.
fn concat(name: &str, a: &[Value], side_by_side: bool) -> Result<Value, Error> {
    at_least_one(name, a)?;
    let blocks: Vec<Matrix> = a.iter().map(|v| as_matrix(name, v)).collect::<Result<_, _>>()?;
    let blocks: Vec<Matrix> = if side_by_side {
        blocks
    } else {
        blocks.iter().map(Matrix::transpose).collect()
    };
    let rows = blocks[0].rows;
    if blocks.iter().any(|b| b.rows != rows) {
        return Err(domain(
            name,
            if side_by_side {
                "alle delen moeten evenveel rijen hebben"
            } else {
                "alle delen moeten evenveel kolommen hebben"
            },
        ));
    }
    let cols: usize = blocks.iter().map(|b| b.cols).sum();
    let mut elems = Vec::with_capacity(rows * cols);
    for r in 0..rows {
        for b in &blocks {
            for c in 0..b.cols {
                elems.push(b.cget_rc(r, c));
            }
        }
    }
    let m = Matrix::from_complex(rows, cols, elems);
    Ok(to_value(if side_by_side { m } else { m.transpose() }))
}

fn index_arg(name: &str, v: &Value, origin: i64, len: usize) -> Result<usize, Error> {
    let k = integer(name, v.as_quantity("een index")?)? - origin;
    if k < 0 || k as usize >= len {
        return Err(domain(
            name,
            &format!("index {} valt buiten het bereik {} t/m {}", k + origin, origin, origin + len as i64 - 1),
        ));
    }
    Ok(k as usize)
}

/// Sorteer de rijen (`csort`, op kolom `k`) of kolommen (`rsort`, op rij `k`) van een reële matrix.
fn sort_lines(name: &str, m: &Matrix, k: usize, rows: bool) -> Result<Value, Error> {
    if m.is_complex() {
        return Err(domain(name, "werkt niet met een complexe matrix"));
    }
    let key: Vec<Quantity> = if rows {
        (0..m.rows).map(|i| m.get(i, k)).collect()
    } else {
        (0..m.cols).map(|j| m.get(k, j)).collect()
    };
    same_dims(&key, "sorteren")?;
    let mut order: Vec<usize> = (0..key.len()).collect();
    order.sort_by(|&p, &q| key[p].value.total_cmp(&key[q].value));
    let mut data = Vec::with_capacity(m.len());
    for i in 0..m.rows {
        for j in 0..m.cols {
            data.push(if rows { m.get(order[i], j) } else { m.get(i, order[j]) });
        }
    }
    Ok(to_value(Matrix::new(m.rows, m.cols, data)))
}

fn equal_q(a: Quantity, b: Quantity) -> bool {
    a.dim == b.dim && (a.value == b.value || (a.value - b.value).abs() <= 1e-12 * a.value.abs().max(b.value.abs()))
}

/// Functies die met complexe getallen, vectoren of matrices werken.
fn call_value(name: &str, a: &[Value], origin: i64) -> Option<Result<Value, Error>> {
    let one = |f: &dyn Fn(&Value) -> Result<Value, Error>| arity(name, a, 1).and_then(|_| f(&a[0]));
    Some(match (name, a) {
        // --- complexe getallen
        ("sqrt", [Value::Num(q)]) if q.value < 0.0 => {
            Complex::from_q(*q).root(2).map(|z| Value::Complex(z).normalize())
        }
        ("sqrt", [Value::Complex(z)]) => z.root(2).map(|z| Value::Complex(z).normalize()),
        ("abs", [Value::Complex(z)]) => Ok(Value::Num(Quantity::new(z.abs(), z.dim))),
        ("abs", [Value::Matrix(m)]) => {
            if !m.is_vector() {
                return Some(Err(domain(name, "van een matrix bestaat geen abs; gebruik det(M) of norm(M)")));
            }
            norm(m).map(Value::Num)
        }
        ("exp", [Value::Complex(z)]) => {
            if !z.dim.is_none() {
                return Some(Err(domain(name, "het argument moet dimensieloos zijn")));
            }
            Ok(Value::Complex(z.exp()).normalize())
        }
        ("ln", [Value::Num(q)]) if q.value < 0.0 && q.dim.is_none() => {
            Ok(Value::Complex(Complex::from_q(*q).ln()).normalize())
        }
        ("ln", [Value::Complex(z)]) => {
            if !z.dim.is_none() || (z.re == 0.0 && z.im == 0.0) {
                return Some(Err(domain(name, "het argument moet dimensieloos en niet 0 zijn")));
            }
            Ok(Value::Complex(z.ln()).normalize())
        }
        ("Re" | "Im" | "arg" | "conj", _) => one(&|v| {
            let f = |z: Complex| -> Complex {
                match name {
                    "Re" => Complex::new(z.re, 0.0, z.dim),
                    "Im" => Complex::new(z.im, 0.0, z.dim),
                    "arg" => Complex::new(z.arg(), 0.0, rb_units::Dim::NONE),
                    _ => z.conj(),
                }
            };
            match v {
                Value::Num(_) | Value::Complex(_) => Ok(Value::from(f(v.as_complex(name)?))),
                Value::Matrix(m) => Ok(to_value(m.cmap(|z| Ok(f(z)))?)),
                other => Err(domain(name, &format!("werkt niet met een {}", other.kind()))),
            }
        }),

        // --- statistiek en verzamelingen (getallen en vectoren door elkaar)
        ("min" | "max", _) => flatten(name, a).and_then(|xs| {
            same_dims(&xs, "vergelijken")?;
            let mut best = xs[0];
            for q in &xs[1..] {
                if (name == "min" && q.value < best.value) || (name == "max" && q.value > best.value) {
                    best = *q;
                }
            }
            Ok(Value::Num(best))
        }),
        ("sum", [Value::Matrix(m)]) if m.is_complex() => {
            let mut acc = m.cget(0);
            for k in 1..m.len() {
                match acc.add(m.cget(k)) {
                    Ok(z) => acc = z,
                    Err(e) => return Some(Err(e)),
                }
            }
            Ok(Value::from(acc))
        }
        ("sum", [v]) => flatten(name, std::slice::from_ref(v)).and_then(|xs| {
            same_dims(&xs, "optellen")?;
            Ok(Value::Num(Quantity::new(xs.iter().map(|q| q.value).sum(), xs[0].dim)))
        }),
        ("prod", [v]) => flatten(name, std::slice::from_ref(v)).and_then(|xs| {
            let mut acc = Quantity::scalar(1.0);
            for q in xs {
                acc = acc.mul(q)?;
            }
            Ok(Value::Num(acc))
        }),
        ("mean", _) => flatten(name, a).and_then(|xs| mean_of(&xs).map(Value::Num)),
        ("median", _) => flatten(name, a).and_then(|mut xs| {
            same_dims(&xs, "vergelijken")?;
            xs.sort_by(|p, q| p.value.total_cmp(&q.value));
            let n = xs.len();
            let v = if n % 2 == 1 {
                xs[n / 2].value
            } else {
                0.5 * (xs[n / 2 - 1].value + xs[n / 2].value)
            };
            Ok(Value::Num(Quantity::new(v, xs[0].dim)))
        }),
        // covariantie (populatie, zoals Mathcads cvar) en correlatiecoëfficiënt van Pearson
        ("corr" | "cvar", _) => arity(name, a, 2).and_then(|_| {
            let (xs, ys) = (flatten(name, &a[..1])?, flatten(name, &a[1..])?);
            if xs.len() != ys.len() {
                return Err(domain(name, "beide vectoren moeten evenveel elementen hebben"));
            }
            let (mx, my) = (mean_of(&xs)?, mean_of(&ys)?);
            let n = xs.len() as f64;
            let cov: f64 = xs.iter().zip(&ys).map(|(x, y)| (x.value - mx.value) * (y.value - my.value)).sum::<f64>() / n;
            if name == "cvar" {
                return Ok(Value::Num(Quantity::new(cov, mx.dim.checked_mul(my.dim).ok_or(rb_units::UnitError::Overflow)?)));
            }
            let sx = (xs.iter().map(|x| (x.value - mx.value).powi(2)).sum::<f64>() / n).sqrt();
            let sy = (ys.iter().map(|y| (y.value - my.value).powi(2)).sum::<f64>() / n).sqrt();
            if sx == 0.0 || sy == 0.0 {
                return Err(domain(name, "een van de vectoren is constant"));
            }
            Ok(Value::scalar(cov / (sx * sy)))
        }),
        // percentile(v, p): lineair geïnterpoleerd, p tussen 0 en 1
        ("percentile", _) => arity(name, a, 2).and_then(|_| {
            let mut xs = flatten(name, &a[..1])?;
            same_dims(&xs, "sorteren")?;
            let p = prob(name, a[1].as_quantity("p")?)?;
            xs.sort_by(|p, q| p.value.total_cmp(&q.value));
            let pos = p * (xs.len() - 1) as f64;
            let (k, f) = (pos.floor() as usize, pos.fract());
            let v = if k + 1 < xs.len() { xs[k].value + f * (xs[k + 1].value - xs[k].value) } else { xs[k].value };
            Ok(Value::Num(Quantity::new(v, xs[0].dim)))
        }),
        // hist(n, v): n gelijke klassen tussen min en max; hist(grenzen, v): klassen [g_i, g_i+1)
        ("hist", _) => arity(name, a, 2).and_then(|_| {
            let data = flatten(name, &a[1..])?;
            same_dims(&data, "hist")?;
            let edges: Vec<f64> = match &a[0] {
                Value::Num(q) => {
                    let n = integer(name, *q)?;
                    if !(1..=100_000).contains(&n) {
                        return Err(domain(name, "het aantal klassen moet 1 tot en met 100000 zijn"));
                    }
                    let lo = data.iter().map(|q| q.value).fold(f64::INFINITY, f64::min);
                    let hi = data.iter().map(|q| q.value).fold(f64::NEG_INFINITY, f64::max);
                    (0..=n).map(|k| lo + (hi - lo) * k as f64 / n as f64).collect()
                }
                v => {
                    let e = flatten(name, std::slice::from_ref(v))?;
                    e[0].same_dim(data[0], "hist (grenzen en gegevens)")?;
                    if e.len() < 2 || e.windows(2).any(|w| w[1].value <= w[0].value) {
                        return Err(domain(name, "de klassegrenzen moeten oplopen"));
                    }
                    e.iter().map(|q| q.value).collect()
                }
            };
            let last = edges.len() - 2;
            let mut counts = vec![0.0; edges.len() - 1];
            for q in &data {
                let v = q.value;
                if let Some(k) = (0..=last).find(|&k| v >= edges[k] && (v < edges[k + 1] || (k == last && v == edges[k + 1]))) {
                    counts[k] += 1.0;
                }
            }
            Ok(to_value(Matrix::column(counts.into_iter().map(Quantity::scalar).collect())))
        }),
        ("var" | "Var", _) => flatten(name, a).and_then(|xs| variance(name, &xs, name == "Var").map(Value::Num)),
        ("stdev" | "Stdev", _) => flatten(name, a).and_then(|xs| {
            Ok(Value::Num(variance(name, &xs, name == "Stdev")?.root(2)?))
        }),

        // --- vectoren en matrices
        ("rows" | "cols", _) => one(&|v| {
            let m = mat(name, v)?;
            Ok(Value::scalar(if name == "rows" { m.rows } else { m.cols } as f64))
        }),
        ("length", _) => one(&|v| Ok(Value::scalar(vector(name, v)?.len() as f64))),
        ("last", _) => one(&|v| Ok(Value::scalar((vector(name, v)?.len() as i64 - 1 + origin) as f64))),
        ("transpose", _) => one(&|v| match v {
            Value::Num(_) | Value::Complex(_) => Ok(v.clone()),
            v => Ok(to_value(mat(name, v)?.transpose())),
        }),
        ("det", _) => one(&|v| linalg::det(mat(name, v)?)),
        ("inverse", _) => one(&|v| linalg::inverse(mat(name, v)?).map(to_value)),
        ("identity", _) => one(&|v| {
            let n = integer(name, v.as_quantity("de grootte")?)?;
            if !(1..=1000).contains(&n) {
                return Err(domain(name, "de grootte moet 1 tot en met 1000 zijn"));
            }
            Ok(to_value(Matrix::identity(n as usize)))
        }),
        ("augment", _) => concat(name, a, true),
        ("stack", _) => concat(name, a, false),
        ("submatrix", _) => arity(name, a, 5).and_then(|_| {
            let m = mat(name, &a[0])?;
            let r0 = index_arg(name, &a[1], origin, m.rows)?;
            let r1 = index_arg(name, &a[2], origin, m.rows)?;
            let c0 = index_arg(name, &a[3], origin, m.cols)?;
            let c1 = index_arg(name, &a[4], origin, m.cols)?;
            if r1 < r0 || c1 < c0 {
                return Err(domain(name, "het einde van een bereik ligt voor het begin"));
            }
            let mut elems = Vec::new();
            for r in r0..=r1 {
                for c in c0..=c1 {
                    elems.push(m.cget_rc(r, c));
                }
            }
            Ok(to_value(Matrix::from_complex(r1 - r0 + 1, c1 - c0 + 1, elems)))
        }),
        ("diag", _) => one(&|v| {
            let d = vector(name, v)?;
            let n = d.len();
            let zero = Complex::new(0.0, 0.0, d.data[0].dim);
            let mut elems = vec![zero; n * n];
            for i in 0..n {
                elems[i * n + i] = d.cget(i);
            }
            Ok(to_value(Matrix::from_complex(n, n, elems)))
        }),
        ("tr", _) => one(&|v| {
            let m = mat(name, v)?;
            if !m.is_square() {
                return Err(domain(name, "vraagt een vierkante matrix"));
            }
            let mut acc = m.cget_rc(0, 0);
            for i in 1..m.rows {
                acc = acc.add(m.cget_rc(i, i))?;
            }
            Ok(Value::from(acc))
        }),
        ("lsolve", _) => arity(name, a, 2)
            .and_then(|_| linalg::lsolve(mat(name, &a[0])?, mat(name, &a[1])?).map(to_value)),
        ("dot", _) => arity(name, a, 2).and_then(|_| value::dot(vector(name, &a[0])?, vector(name, &a[1])?)),
        ("cross", _) => arity(name, a, 2).and_then(|_| {
            let (u, v) = (vector(name, &a[0])?, vector(name, &a[1])?);
            if u.len() != 3 || v.len() != 3 {
                return Err(domain(name, "vraagt twee vectoren met 3 elementen"));
            }
            let (u, v) = (u.elems(), v.elems());
            let c = |i: usize, j: usize| -> Result<Complex, Error> { u[i].mul(v[j])?.sub(u[j].mul(v[i])?) };
            let elems = vec![c(1, 2)?, c(2, 0)?, c(0, 1)?];
            let shape = mat(name, &a[0])?;
            Ok(to_value(Matrix::from_complex(if shape.rows == 1 { 1 } else { 3 }, if shape.rows == 1 { 3 } else { 1 }, elems)))
        }),
        ("norm" | "norme", _) => one(&|v| norm(mat(name, v)?).map(Value::Num)),
        ("norm1", _) => one(&|v| linalg::norm_1_inf(mat(name, v)?, false).map(Value::Num)),
        ("normi", _) => one(&|v| linalg::norm_1_inf(mat(name, v)?, true).map(Value::Num)),
        ("norm2", _) => one(&|v| linalg::norm2(mat(name, v)?).map(Value::Num)),
        ("cond2", _) => one(&|v| linalg::cond2(mat(name, v)?).map(Value::scalar)),
        ("rank", _) => one(&|v| linalg::rank(mat(name, v)?).map(|r| Value::scalar(r as f64))),
        ("eigenvals", _) => one(&|v| linalg::eigenvals(mat(name, v)?).map(to_value)),
        ("eigenvecs", _) => one(&|v| linalg::eigenvecs(mat(name, v)?).map(to_value)),
        ("eigenvec", _) => arity(name, a, 2)
            .and_then(|_| linalg::eigenvec(mat(name, &a[0])?, a[1].as_complex("de eigenwaarde")?).map(to_value)),
        ("cholesky", _) => one(&|v| linalg::cholesky(mat(name, v)?).map(to_value)),
        ("svd", _) => one(&|v| linalg::svd(mat(name, v)?).map(to_value)),
        ("svds", _) => one(&|v| linalg::svds(mat(name, v)?).map(to_value)),
        ("qr", _) => one(&|v| linalg::qr(mat(name, v)?).map(to_value)),
        ("lu", _) => one(&|v| linalg::lu_decomp(mat(name, v)?).map(to_value)),
        ("polyroots", _) => one(&|v| linalg::polyroots(mat(name, v)?).map(to_value)),
        ("fft" | "cfft", _) => one(&|v| linalg::fft(mat(name, v)?, false).map(to_value)),
        ("ifft" | "icfft", _) => one(&|v| linalg::fft(mat(name, v)?, true).map(to_value)),
        ("nthroot", _) => arity(name, a, 2).and_then(|_| match call_real("root", &[a[0].as_quantity("x")?, a[1].as_quantity("n")?]) {
            Some(r) => r.map(Value::Num),
            None => unreachable!(),
        }),
        ("sort", _) => one(&|v| {
            let m = vector(name, v)?;
            if m.is_complex() {
                return Err(domain(name, "complexe getallen kun je niet sorteren"));
            }
            same_dims(&m.data, "sorteren")?;
            let mut data = m.data.clone();
            data.sort_by(|p, q| p.value.total_cmp(&q.value));
            Ok(to_value(Matrix::new(m.rows, m.cols, data)))
        }),
        ("csort" | "rsort", _) => arity(name, a, 2).and_then(|_| {
            let m = mat(name, &a[0])?;
            let rows = name == "csort";
            let k = index_arg(name, &a[1], origin, if rows { m.cols } else { m.rows })?;
            sort_lines(name, m, k, rows)
        }),
        ("reverse", _) => one(&|v| {
            let m = vector(name, v)?;
            let mut elems = m.elems();
            elems.reverse();
            Ok(to_value(Matrix::from_complex(m.rows, m.cols, elems)))
        }),
        // match(z, A): indices (ORIGIN) waar A gelijk is aan z
        ("match", _) => arity(name, a, 2).and_then(|_| {
            let z = a[0].as_quantity("de zoekwaarde")?;
            let m = mat(name, &a[1])?;
            let mut hits = Vec::new();
            for i in 0..m.rows {
                for j in 0..m.cols {
                    if equal_q(m.get(i, j), z) {
                        hits.push((i as i64 + origin, j as i64 + origin));
                    }
                }
            }
            if hits.is_empty() {
                return Err(domain(name, "de waarde komt niet voor"));
            }
            let data = if m.is_vector() {
                hits.iter().map(|&(i, j)| Quantity::scalar(if m.cols == 1 { i } else { j } as f64)).collect()
            } else {
                return Ok(to_value(Matrix::new(
                    hits.len(),
                    2,
                    hits.iter().flat_map(|&(i, j)| [Quantity::scalar(i as f64), Quantity::scalar(j as f64)]).collect(),
                )));
            };
            Ok(to_value(Matrix::column(data)))
        }),
        // lookup(z, A, B): de waarden in B op de plaatsen waar A gelijk is aan z
        ("lookup", _) => arity(name, a, 3).and_then(|_| {
            let z = a[0].as_quantity("de zoekwaarde")?;
            let (ma, mb) = (mat(name, &a[1])?, mat(name, &a[2])?);
            if (ma.rows, ma.cols) != (mb.rows, mb.cols) {
                return Err(domain(name, "A en B moeten even groot zijn"));
            }
            let found: Vec<Complex> = (0..ma.len()).filter(|&k| equal_q(ma.data[k], z)).map(|k| mb.cget(k)).collect();
            if found.is_empty() {
                return Err(domain(name, "de waarde komt niet voor"));
            }
            Ok(to_value(Matrix::from_complex(found.len(), 1, found)))
        }),
        // vlookup(z, A, c): waarden in kolom c van de rijen waarvan de eerste kolom gelijk is aan z
        ("vlookup", _) => arity(name, a, 3).and_then(|_| {
            let z = a[0].as_quantity("de zoekwaarde")?;
            let m = mat(name, &a[1])?;
            let c = index_arg(name, &a[2], origin, m.cols)?;
            let found: Vec<Complex> = (0..m.rows).filter(|&i| equal_q(m.get(i, 0), z)).map(|i| m.cget_rc(i, c)).collect();
            if found.is_empty() {
                return Err(domain(name, "de waarde komt niet voor in de eerste kolom"));
            }
            Ok(to_value(Matrix::from_complex(found.len(), 1, found)))
        }),
        _ => return None,
    })
}

/// Euclidische norm van een vector, of de Frobenius-norm van een matrix (ook complex).
fn norm(m: &Matrix) -> Result<Quantity, Error> {
    same_dims(&m.data, "de norm")?;
    Ok(Quantity::new(
        m.elems().iter().map(|z| z.abs() * z.abs()).sum::<f64>().sqrt(),
        m.data[0].dim,
    ))
}

/// Roep een ingebouwde functie aan. `None` betekent: geen ingebouwde functie met deze naam.
/// `origin` is de eerste index (ORIGIN, standaard 0).
pub fn call(name: &str, a: &[Value], origin: i64) -> Option<Result<Value, Error>> {
    if let Some(r) = crate::strings::call(name, a) {
        return Some(r);
    }
    if let Some(r) = call_value(name, a, origin) {
        return Some(r);
    }
    if let Some(r) = crate::fit::call(name, a) {
        return Some(r);
    }
    if a.len() == 1 && ELEMENTWISE.contains(&name) {
        if let Value::Matrix(m) = &a[0] {
            return Some(
                m.map(|q| call_real(name, &[q]).expect("bekende functie"))
                    .map(Value::Matrix),
            );
        }
    }
    // eerst kijken of de naam bestaat, dan pas de argumenten controleren
    if call_real(name, &[]).is_none() {
        return None;
    }
    let qs = match a
        .iter()
        .map(|v| match v {
            Value::Num(q) => Ok(*q),
            other => Err(domain(name, &format!("werkt niet met een {}", other.kind()))),
        })
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(qs) => qs,
        Err(e) => return Some(Err(e)),
    };
    call_real(name, &qs).map(|r| r.map(Value::Num))
}

/// Namen van alle ingebouwde functies (voor documentatie en het REPL-commando `:functies`).
pub const NAMES: &[&str] = &[
    // wortels, machten, logaritmen
    "sqrt", "cbrt", "nthroot", "exp", "ln", "log", "log2",
    // goniometrie (hoeken dimensieloos)
    "sin", "cos", "tan", "cot", "sec", "csc", "asin", "acos", "atan", "acot", "atan2",
    "sinh", "cosh", "tanh", "coth", "asinh", "acosh", "atanh", "acoth",
    // afronden en overig
    "abs", "sign", "floor", "ceil", "trunc", "round", "mod", "hypot", "min", "max",
    "Φ", "Heaviside", "δ", "kronecker", "gcd", "lcm", "factorial", "fact", "combin", "permut",
    // complex
    "Re", "Im", "arg", "conj",
    // speciale functies
    "erf", "erfc", "Γ", "gamma", "lnGamma", "Beta", "Ψ", "digamma",
    "J0", "J1", "Jn", "Y0", "Y1", "Yn", "I0", "I1", "In", "K0", "K1", "Kn",
    // statistiek
    "sum", "prod", "mean", "median", "var", "Var", "stdev", "Stdev", "corr", "cvar", "percentile",
    "hist",
    // verdelingen (dichtheid, verdelingsfunctie, kwantiel) en toevalsgetallen
    "dnorm", "pnorm", "qnorm", "dlnorm", "plnorm", "qlnorm", "dt", "pt", "qt", "dchisq", "pchisq",
    "qchisq", "dF", "pF", "qF", "dexp", "pexp", "qexp", "dunif", "punif", "qunif", "dbinom",
    "pbinom", "dpois", "ppois", "dweibull", "pweibull", "qweibull", "rnd", "runif", "rnorm",
    // vectoren en matrices
    "rows", "cols", "length", "last", "transpose", "det", "inverse", "identity", "augment",
    "stack", "submatrix", "diag", "tr", "lsolve", "dot", "cross", "norm", "norme", "norm1",
    "norm2", "normi", "cond2", "rank", "eigenvals", "eigenvecs", "eigenvec", "cholesky", "svd",
    "svds", "qr", "lu", "sort", "csort", "rsort", "reverse", "match", "lookup", "vlookup",
    "polyroots", "fft", "ifft", "cfft", "icfft",
    // interpolatie en regressie
    "linterp", "lspline", "pspline", "cspline", "interp", "slope", "intercept", "line", "stderr",
    "polyfit", "linfit", "genfit",
    // differentiaalvergelijkingen en oplossers
    "rkfixed", "Rkadapt", "odesolve", "root", "Find", "Minerr", "Minimize", "Maximize",
    // tekst
    "concat", "strlen", "substr", "search", "str2num", "num2str", "str2vec", "vec2str", "IsString", "error",
    // bestanden (via het werkblad, zie rb-sheet/src/io.rs)
    "READCSV", "WRITECSV", "READEXCEL", "WRITEEXCEL", "READFILE", "WRITEFILE", "READPRN", "WRITEPRN",
    // speciale vormen (in de evaluator)
    "if", "integral", "deriv", "Σ", "∑", "∏", "∫",
];


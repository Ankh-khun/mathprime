//! Interpolatie en regressie op vectoren met eenheden: `linterp`, splines (`lspline`, `pspline`,
//! `cspline` met `interp`), `slope`, `intercept`, `line`, `stderr` en `polyfit`.
//! (`linfit` en `genfit` krijgen een functie als argument en staan in de evaluator.)

use crate::error::Error;
use crate::numeric;
use crate::value::{Matrix, Value};
use rb_units::{Dim, Quantity, UnitError};

fn dom(name: &str, msg: &str) -> Error {
    Error::Domain(format!("{name}: {msg}"))
}

fn arity(name: &str, a: &[Value], n: usize) -> Result<(), Error> {
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

/// Een reële vector met één eenheid: (waarden in SI, eenheid).
pub(crate) fn real_vector(name: &str, v: &Value) -> Result<(Vec<f64>, Dim), Error> {
    match v {
        Value::Matrix(m) if m.is_vector() && !m.is_complex() => {
            let d = m.common_dim().ok_or_else(|| dom(name, "alle elementen van een vector moeten dezelfde eenheid hebben"))?;
            Ok((m.data.iter().map(|q| q.value).collect(), d))
        }
        other => Err(dom(name, &format!("verwacht een reële vector, geen {}", other.kind()))),
    }
}

/// vx en vy: even lang, vx strikt oplopend (minstens `min` punten).
pub(crate) fn xy(name: &str, vx: &Value, vy: &Value, min: usize, ascending: bool) -> Result<(Vec<f64>, Dim, Vec<f64>, Dim), Error> {
    let (x, dx) = real_vector(name, vx)?;
    let (y, dy) = real_vector(name, vy)?;
    if x.len() != y.len() {
        return Err(dom(name, "vx en vy moeten evenveel elementen hebben"));
    }
    if x.len() < min {
        return Err(dom(name, &format!("vraagt minstens {min} punten")));
    }
    if ascending && x.windows(2).any(|w| w[1] <= w[0]) {
        return Err(dom(name, "vx moet strikt oplopen"));
    }
    Ok((x, dx, y, dy))
}

fn div(a: Dim, b: Dim) -> Result<Dim, Error> {
    a.checked_div(b).ok_or_else(|| UnitError::Overflow.into())
}

fn column(vals: &[f64], dim: Dim) -> Value {
    Value::Matrix(Matrix::column(vals.iter().map(|&v| Quantity::new(v, dim)).collect()))
}

/// Het interval i met x[i] ≤ t ≤ x[i+1]; daarbuiten het eerste of laatste (extrapolatie).
fn segment(x: &[f64], t: f64) -> usize {
    let n = x.len();
    match x.iter().position(|&xi| xi > t) {
        Some(0) => 0,
        Some(k) => (k - 1).min(n - 2),
        None => n - 2,
    }
}

/// Tweede afgeleiden M van een kubische spline. Eindvoorwaarde: `l` natuurlijk (M = 0),
/// `p` parabolisch (M0 = M1), `c` kubisch (M lineair doorgetrokken: M0 = 2M1 − M2).
fn spline_second_derivatives(x: &[f64], y: &[f64], end: char) -> Result<Vec<f64>, Error> {
    let n = x.len();
    let end = if n < 4 && end == 'c' { 'p' } else { end };
    let mut a = vec![0.0; n * n];
    let mut b = vec![0.0; n];
    for i in 1..n - 1 {
        let (h0, h1) = (x[i] - x[i - 1], x[i + 1] - x[i]);
        a[i * n + i - 1] = h0;
        a[i * n + i] = 2.0 * (h0 + h1);
        a[i * n + i + 1] = h1;
        b[i] = 6.0 * ((y[i + 1] - y[i]) / h1 - (y[i] - y[i - 1]) / h0);
    }
    let last = n - 1;
    match end {
        'l' => {
            a[0] = 1.0;
            a[last * n + last] = 1.0;
        }
        'p' => {
            a[0] = 1.0;
            a[1] = -1.0;
            a[last * n + last] = 1.0;
            a[last * n + last - 1] = -1.0;
        }
        _ => {
            a[0] = 1.0;
            a[1] = -2.0;
            a[2] = 1.0;
            a[last * n + last] = 1.0;
            a[last * n + last - 1] = -2.0;
            a[last * n + last - 2] = 1.0;
        }
    }
    if n == 2 {
        return Ok(vec![0.0, 0.0]);
    }
    numeric::solve_dense(a, n, b).ok_or_else(|| dom("spline", "het splinestelsel is singulier"))
}

/// Gewone kleinste-kwadratenrechte: (helling, snijpunt, standaardfout).
fn line_fit(x: &[f64], y: &[f64]) -> Result<(f64, f64, f64), Error> {
    let n = x.len() as f64;
    let (mx, my) = (x.iter().sum::<f64>() / n, y.iter().sum::<f64>() / n);
    let sxx: f64 = x.iter().map(|v| (v - mx).powi(2)).sum();
    if sxx == 0.0 {
        return Err(dom("slope", "alle x-waarden zijn gelijk"));
    }
    let sxy: f64 = x.iter().zip(y).map(|(a, b)| (a - mx) * (b - my)).sum();
    let slope = sxy / sxx;
    let icpt = my - slope * mx;
    let ss: f64 = x.iter().zip(y).map(|(a, b)| (b - icpt - slope * a).powi(2)).sum();
    let se = if x.len() > 2 { (ss / (n - 2.0)).sqrt() } else { 0.0 };
    Ok((slope, icpt, se))
}

pub fn call(name: &str, a: &[Value]) -> Option<Result<Value, Error>> {
    Some(match name {
        "linterp" => (|| {
            arity(name, a, 3)?;
            let (x, dx, y, dy) = xy(name, &a[0], &a[1], 2, true)?;
            let t = a[2].as_quantity("x")?;
            t.same_dim(Quantity::new(0.0, dx), "linterp (x en vx)")?;
            let i = segment(&x, t.value);
            let f = (t.value - x[i]) / (x[i + 1] - x[i]);
            Ok(Value::Num(Quantity::new(y[i] + f * (y[i + 1] - y[i]), dy)))
        })(),
        "lspline" | "pspline" | "cspline" => (|| {
            arity(name, a, 2)?;
            let (x, dx, y, dy) = xy(name, &a[0], &a[1], 2, true)?;
            let m = spline_second_derivatives(&x, &y, name.chars().next().unwrap())?;
            Ok(column(&m, div(dy, dx.checked_powi(2).ok_or(UnitError::Overflow)?)?))
        })(),
        "interp" => (|| {
            arity(name, a, 4)?;
            let (x, dx, y, dy) = xy(name, &a[1], &a[2], 2, true)?;
            let (m, _) = real_vector(name, &a[0])?;
            if m.len() != x.len() {
                return Err(dom(name, "vs hoort bij andere gegevens (gebruik lspline, pspline of cspline van vx en vy)"));
            }
            let t = a[3].as_quantity("x")?;
            t.same_dim(Quantity::new(0.0, dx), "interp (x en vx)")?;
            let i = segment(&x, t.value);
            let h = x[i + 1] - x[i];
            let ca = (x[i + 1] - t.value) / h;
            let cb = 1.0 - ca;
            let v = ca * y[i] + cb * y[i + 1] + ((ca.powi(3) - ca) * m[i] + (cb.powi(3) - cb) * m[i + 1]) * h * h / 6.0;
            Ok(Value::Num(Quantity::new(v, dy)))
        })(),
        "slope" | "intercept" | "line" | "stderr" => (|| {
            arity(name, a, 2)?;
            let (x, dx, y, dy) = xy(name, &a[0], &a[1], 2, false)?;
            let (s, c, se) = line_fit(&x, &y)?;
            let ds = div(dy, dx)?;
            Ok(match name {
                "slope" => Value::Num(Quantity::new(s, ds)),
                "intercept" => Value::Num(Quantity::new(c, dy)),
                "stderr" => Value::Num(Quantity::new(se, dy)),
                _ => Value::Matrix(Matrix::column(vec![Quantity::new(c, dy), Quantity::new(s, ds)])),
            })
        })(),
        // polyfit(vx, vy, k): coëfficiënten c0 ... ck (oplopend, zoals polyroots ze verwacht)
        "polyfit" => (|| {
            arity(name, a, 3)?;
            let (x, dx, y, dy) = xy(name, &a[0], &a[1], 1, false)?;
            let k = a[2].as_quantity("de graad")?.dimensionless_value("de graad")?;
            if k.fract() != 0.0 || !(0.0..=20.0).contains(&k) {
                return Err(dom(name, "de graad moet een geheel getal van 0 tot en met 20 zijn"));
            }
            let k = k as usize;
            // schalen voor een beter geconditioneerd stelsel
            let s = x.iter().fold(0.0f64, |m, v| m.max(v.abs())).max(f64::MIN_POSITIVE);
            let mut mat = Vec::with_capacity(x.len() * (k + 1));
            for xi in &x {
                for j in 0..=k {
                    mat.push((xi / s).powi(j as i32));
                }
            }
            let c = numeric::least_squares(&mat, x.len(), k + 1, &y)?;
            let data = (0..=k)
                .map(|j| {
                    let d = div(dy, dx.checked_powi(j as i32).ok_or(UnitError::Overflow)?)?;
                    Ok(Quantity::new(c[j] / s.powi(j as i32), d))
                })
                .collect::<Result<Vec<_>, Error>>()?;
            Ok(Value::Matrix(Matrix::column(data)))
        })(),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(xs: &[f64]) -> Value {
        Value::Matrix(Matrix::column(xs.iter().map(|&x| Quantity::scalar(x)).collect()))
    }

    fn num(r: Result<Value, Error>) -> f64 {
        match r.unwrap() {
            Value::Num(q) => q.value,
            o => panic!("{o:?}"),
        }
    }

    #[test]
    fn spline_reproduceert_een_kubisch_polynoom() {
        // cspline (lineaire M aan de randen) is exact voor een kubisch polynoom
        let xs: Vec<f64> = (0..6).map(|k| k as f64).collect();
        let ys: Vec<f64> = xs.iter().map(|x| x * x * x - 2.0 * x).collect();
        let vs = call("cspline", &[v(&xs), v(&ys)]).unwrap().unwrap();
        let y = num(call("interp", &[vs, v(&xs), v(&ys), Value::scalar(2.5)]).unwrap());
        assert!((y - (2.5f64.powi(3) - 5.0)).abs() < 1e-12, "{y}");
        // natuurlijke spline door een rechte is de rechte
        let vs = call("lspline", &[v(&[0.0, 1.0, 3.0]), v(&[1.0, 3.0, 7.0])]).unwrap().unwrap();
        let y = num(call("interp", &[vs, v(&[0.0, 1.0, 3.0]), v(&[1.0, 3.0, 7.0]), Value::scalar(2.0)]).unwrap());
        assert!((y - 5.0).abs() < 1e-14);
    }

    #[test]
    fn lineaire_interpolatie_en_regressie() {
        let y = num(call("linterp", &[v(&[0.0, 1.0, 2.0]), v(&[0.0, 10.0, 30.0]), Value::scalar(1.5)]).unwrap());
        assert_eq!(y, 20.0);
        // extrapolatie met het laatste stuk
        let y = num(call("linterp", &[v(&[0.0, 1.0, 2.0]), v(&[0.0, 10.0, 30.0]), Value::scalar(3.0)]).unwrap());
        assert_eq!(y, 50.0);
        let s = num(call("slope", &[v(&[0.0, 1.0, 2.0, 3.0]), v(&[1.0, 3.0, 5.0, 7.0])]).unwrap());
        assert!((s - 2.0).abs() < 1e-14);
        // polyfit door exacte parabool 1 + 2x + 3x²
        let xs = [-1.0, 0.0, 1.0, 2.0, 3.0];
        let ys: Vec<f64> = xs.iter().map(|x| 1.0 + 2.0 * x + 3.0 * x * x).collect();
        match call("polyfit", &[v(&xs), v(&ys), Value::scalar(2.0)]).unwrap().unwrap() {
            Value::Matrix(m) => {
                for (k, want) in [1.0, 2.0, 3.0].iter().enumerate() {
                    assert!((m.data[k].value - want).abs() < 1e-12);
                }
            }
            o => panic!("{o:?}"),
        }
    }
}

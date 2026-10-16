//! Speciale functies (gamma, beta, erf, Bessel, digamma) en kansverdelingen.
//!
//! Alles werkt op dimensieloze `f64`; eenheden regelt [`crate::builtins`].
//! Methoden: Lanczos voor ln Γ, reeks/kettingbreuk voor de onvolledige gamma- en betafunctie
//! (Numerical Recipes §6.2, §6.4), trapeziumregel op periodieke integranden voor J_n en I_n
//! (convergeert exponentieel snel) en adaptieve Gauss-Kronrod voor Y_n en K_n.

use crate::error::Error;
use crate::numeric;
use std::f64::consts::PI;

fn dom(msg: impl Into<String>) -> Error {
    Error::Domain(msg.into())
}

// ---------------------------------------------------------------------------------------------
// Gamma en beta

const LANCZOS: [f64; 9] = [
    0.999_999_999_999_809_9,
    676.520_368_121_885_1,
    -1_259.139_216_722_402_8,
    771.323_428_777_653_1,
    -176.615_029_162_140_6,
    12.507_343_278_686_905,
    -0.138_571_095_265_720_12,
    9.984_369_578_019_572e-6,
    1.505_632_735_149_311_6e-7,
];

/// ln |Γ(x)|.
pub fn ln_gamma(x: f64) -> f64 {
    if x < 0.5 {
        // reflectie: Γ(x) Γ(1-x) = π / sin(πx)
        return (PI / (PI * x).sin().abs()).ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let mut a = LANCZOS[0];
    let t = x + 7.5;
    for (i, c) in LANCZOS.iter().enumerate().skip(1) {
        a += c / (x + i as f64);
    }
    0.5 * (2.0 * PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
}

/// Γ(x) voor reële x (niet 0, -1, -2, ...).
pub fn gamma(x: f64) -> Result<f64, Error> {
    if x <= 0.0 && x.fract() == 0.0 {
        return Err(dom("Γ bestaat niet in 0 en de negatieve gehele getallen"));
    }
    if x.fract() == 0.0 && x <= 171.0 {
        return Ok((1..x as u64).fold(1.0, |acc, k| acc * k as f64));
    }
    if x < 0.5 {
        return Ok(PI / ((PI * x).sin() * gamma(1.0 - x)?));
    }
    Ok(ln_gamma(x).exp())
}

pub fn beta(a: f64, b: f64) -> f64 {
    (ln_gamma(a) + ln_gamma(b) - ln_gamma(a + b)).exp()
}

/// Digamma ψ(x) = Γ'(x)/Γ(x).
pub fn digamma(mut x: f64) -> Result<f64, Error> {
    if x <= 0.0 && x.fract() == 0.0 {
        return Err(dom("Ψ bestaat niet in 0 en de negatieve gehele getallen"));
    }
    let mut acc = 0.0;
    if x < 0.0 {
        // ψ(1-x) - ψ(x) = π cot(πx)
        acc -= PI / (PI * x).tan();
        x = 1.0 - x;
    }
    while x < 12.0 {
        acc -= 1.0 / x;
        x += 1.0;
    }
    let f = 1.0 / (x * x);
    let series = f * (1.0 / 12.0 - f * (1.0 / 120.0 - f * (1.0 / 252.0 - f * (1.0 / 240.0 - f / 132.0))));
    Ok(acc + x.ln() - 0.5 / x - series)
}

/// Geregulariseerde onvolledige gammafunctie P(a, x).
pub fn gamma_p(a: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x < a + 1.0 {
        // reeks
        let (mut sum, mut del, mut ap) = (1.0 / a, 1.0 / a, a);
        for _ in 0..1000 {
            ap += 1.0;
            del *= x / ap;
            sum += del;
            if del.abs() < sum.abs() * 1e-16 {
                break;
            }
        }
        sum * (-x + a * x.ln() - ln_gamma(a)).exp()
    } else {
        1.0 - gamma_q_cf(a, x)
    }
}

/// Q(a, x) = 1 - P(a, x).
pub fn gamma_q(a: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 1.0;
    }
    if x < a + 1.0 {
        1.0 - gamma_p(a, x)
    } else {
        gamma_q_cf(a, x)
    }
}

fn gamma_q_cf(a: f64, x: f64) -> f64 {
    const TINY: f64 = 1e-300;
    let mut b = x + 1.0 - a;
    let mut c = 1.0 / TINY;
    let mut d = 1.0 / b;
    let mut h = d;
    for i in 1..1000 {
        let an = -(i as f64) * (i as f64 - a);
        b += 2.0;
        d = an * d + b;
        if d.abs() < TINY {
            d = TINY;
        }
        c = b + an / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < 1e-16 {
            break;
        }
    }
    (-x + a * x.ln() - ln_gamma(a)).exp() * h
}

/// Geregulariseerde onvolledige betafunctie I_x(a, b).
pub fn beta_inc(a: f64, b: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let bt = (ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b) + a * x.ln() + b * (1.0 - x).ln()).exp();
    if x < (a + 1.0) / (a + b + 2.0) {
        bt * beta_cf(a, b, x) / a
    } else {
        1.0 - bt * beta_cf(b, a, 1.0 - x) / b
    }
}

fn beta_cf(a: f64, b: f64, x: f64) -> f64 {
    const TINY: f64 = 1e-300;
    let (qab, qap, qam) = (a + b, a + 1.0, a - 1.0);
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < TINY {
        d = TINY;
    }
    d = 1.0 / d;
    let mut h = d;
    for m in 1..1000 {
        let m = m as f64;
        let m2 = 2.0 * m;
        let aa = m * (b - m) * x / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < TINY {
            d = TINY;
        }
        c = 1.0 + aa / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        h *= d * c;
        let aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < TINY {
            d = TINY;
        }
        c = 1.0 + aa / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < 1e-16 {
            break;
        }
    }
    h
}

pub fn erf(x: f64) -> f64 {
    if x >= 0.0 {
        gamma_p(0.5, x * x)
    } else {
        -gamma_p(0.5, x * x)
    }
}

pub fn erfc(x: f64) -> f64 {
    if x >= 0.0 {
        gamma_q(0.5, x * x)
    } else {
        1.0 + gamma_p(0.5, x * x)
    }
}

// ---------------------------------------------------------------------------------------------
// Bessel (gehele orde)

/// Trapeziumregel voor een 2π-periodieke integrand: exponentieel nauwkeurig.
fn periodic_mean(f: impl Fn(f64) -> f64, x: f64) -> f64 {
    let n = (2.0 * x.abs() + 64.0).ceil() as usize;
    let h = 2.0 * PI / n as f64;
    (0..n).map(|k| f(k as f64 * h)).sum::<f64>() / n as f64
}

/// J_n(x) = (1/2π) ∫ cos(nτ − x sin τ) dτ over één periode.
pub fn bessel_j(n: i32, x: f64) -> f64 {
    periodic_mean(|t| (n as f64 * t - x * t.sin()).cos(), x)
}

/// I_n(x) = (1/2π) ∫ e^{x cos τ} cos(nτ) dτ over één periode.
pub fn bessel_i(n: i32, x: f64) -> f64 {
    periodic_mean(|t| (x * t.cos()).exp() * (n as f64 * t).cos(), x)
}

/// Y_n(x), x > 0: (1/π)∫₀^π sin(x sin τ − nτ) dτ − (1/π)∫₀^∞ (e^{nt} + (−1)ⁿ e^{−nt}) e^{−x sinh t} dt.
pub fn bessel_y(n: i32, x: f64) -> Result<f64, Error> {
    if x <= 0.0 {
        return Err(dom("Y_n(x) bestaat alleen voor x > 0"));
    }
    let nf = n as f64;
    let a = numeric::integrate(&mut |t| Ok((x * t.sin() - nf * t).sin()), 0.0, PI)?;
    let sign = if n % 2 == 0 { 1.0 } else { -1.0 };
    let b = numeric::integrate(
        &mut |t| {
            let e = -x * t.sinh();
            // e^{nt}·e^{−x sinh t} als exp(nt − x sinh t) om overflow te voorkomen
            Ok(if e < -745.0 { 0.0 } else { (nf * t + e).exp() + sign * (-nf * t + e).exp() })
        },
        0.0,
        f64::INFINITY,
    )?;
    Ok((a - b) / PI)
}

/// K_n(x) = ∫₀^∞ e^{−x cosh t} cosh(nt) dt, x > 0.
pub fn bessel_k(n: i32, x: f64) -> Result<f64, Error> {
    if x <= 0.0 {
        return Err(dom("K_n(x) bestaat alleen voor x > 0"));
    }
    let nf = n as f64;
    numeric::integrate(
        &mut |t| {
            let e = -x * t.cosh();
            Ok(if e < -745.0 { 0.0 } else { 0.5 * ((nf * t + e).exp() + (-nf * t + e).exp()) })
        },
        0.0,
        f64::INFINITY,
    )
}

// ---------------------------------------------------------------------------------------------
// Bessel van willekeurige (reële) orde, bolfuncties van Bessel en Airy

fn is_int(v: f64) -> bool {
    v.fract() == 0.0 && v.abs() <= 1000.0
}

/// J_ν(x) voor reële ν. Gehele orde via [`bessel_j`]; anders x ≥ 0 en de integraal van Schläfli:
/// J_ν(x) = (1/π)∫₀^π cos(ντ − x sin τ) dτ − (sin νπ/π)∫₀^∞ e^{−x sinh t − νt} dt.
pub fn bessel_jv(nu: f64, x: f64) -> Result<f64, Error> {
    if is_int(nu) {
        return Ok(bessel_j(nu as i32, x));
    }
    if x < 0.0 {
        return Err(dom("J_ν(x) met niet-gehele orde bestaat alleen voor x ≥ 0"));
    }
    if x == 0.0 {
        return if nu > 0.0 { Ok(0.0) } else { Err(dom("J_ν(0) is oneindig voor ν < 0")) };
    }
    let a = numeric::integrate(&mut |t| Ok((nu * t - x * t.sin()).cos()), 0.0, PI)?;
    let b = numeric::integrate(
        &mut |t| {
            let e = -x * t.sinh() - nu * t;
            Ok(if e < -745.0 { 0.0 } else { e.exp() })
        },
        0.0,
        f64::INFINITY,
    )?;
    Ok((a - (nu * PI).sin() * b) / PI)
}

/// Y_ν(x), x > 0: (1/π)∫₀^π sin(x sin τ − ντ) dτ − (1/π)∫₀^∞ (e^{νt} + cos(νπ) e^{−νt}) e^{−x sinh t} dt.
pub fn bessel_yv(nu: f64, x: f64) -> Result<f64, Error> {
    if is_int(nu) {
        return bessel_y(nu as i32, x);
    }
    if x <= 0.0 {
        return Err(dom("Y_ν(x) bestaat alleen voor x > 0"));
    }
    let a = numeric::integrate(&mut |t| Ok((x * t.sin() - nu * t).sin()), 0.0, PI)?;
    let c = (nu * PI).cos();
    let b = numeric::integrate(
        &mut |t| {
            let e = -x * t.sinh();
            Ok(if e < -745.0 { 0.0 } else { (nu * t + e).exp() + c * (-nu * t + e).exp() })
        },
        0.0,
        f64::INFINITY,
    )?;
    Ok((a - b) / PI)
}

/// I_ν(x), x ≥ 0: (1/π)∫₀^π e^{x cos τ} cos(ντ) dτ − (sin νπ/π)∫₀^∞ e^{−x cosh t − νt} dt.
pub fn bessel_iv(nu: f64, x: f64) -> Result<f64, Error> {
    if is_int(nu) {
        return Ok(bessel_i(nu as i32, x));
    }
    if x < 0.0 {
        return Err(dom("I_ν(x) met niet-gehele orde bestaat alleen voor x ≥ 0"));
    }
    if x == 0.0 {
        return if nu > 0.0 { Ok(0.0) } else { Err(dom("I_ν(0) is oneindig voor ν < 0")) };
    }
    let a = numeric::integrate(&mut |t| Ok((x * t.cos()).exp() * (nu * t).cos()), 0.0, PI)?;
    let b = numeric::integrate(
        &mut |t| {
            let e = -x * t.cosh() - nu * t;
            Ok(if e < -745.0 { 0.0 } else { e.exp() })
        },
        0.0,
        f64::INFINITY,
    )?;
    Ok((a - (nu * PI).sin() * b) / PI)
}

/// K_ν(x) = ∫₀^∞ e^{−x cosh t} cosh(νt) dt, x > 0 (elke reële orde).
pub fn bessel_kv(nu: f64, x: f64) -> Result<f64, Error> {
    if x <= 0.0 {
        return Err(dom("K_ν(x) bestaat alleen voor x > 0"));
    }
    numeric::integrate(
        &mut |t| {
            let e = -x * t.cosh();
            Ok(if e < -745.0 { 0.0 } else { 0.5 * ((nu * t + e).exp() + (-nu * t + e).exp()) })
        },
        0.0,
        f64::INFINITY,
    )
}

/// Bolfunctie van Bessel j_n(x) = √(π/2x) J_{n+½}(x).
pub fn sph_j(n: i32, x: f64) -> Result<f64, Error> {
    if x == 0.0 {
        return match n {
            0 => Ok(1.0),
            n if n > 0 => Ok(0.0),
            _ => Err(dom("js(n, 0) is oneindig voor n < 0")),
        };
    }
    let r = (PI / (2.0 * x.abs())).sqrt() * bessel_jv(n as f64 + 0.5, x.abs())?;
    // j_n is even voor even n en oneven voor oneven n
    Ok(if x < 0.0 && n.rem_euclid(2) == 1 { -r } else { r })
}

/// Bolfunctie van Bessel y_n(x) = √(π/2x) Y_{n+½}(x), x > 0.
pub fn sph_y(n: i32, x: f64) -> Result<f64, Error> {
    if x <= 0.0 {
        return Err(dom("ys(n, x) bestaat alleen voor x > 0"));
    }
    Ok((PI / (2.0 * x)).sqrt() * bessel_yv(n as f64 + 0.5, x)?)
}

const AI0: f64 = 0.355_028_053_887_817_2;
const BI0: f64 = 0.614_926_627_446_000_7;

/// Airy Ai(x) via Bessel van orde 1/3.
pub fn airy_ai(x: f64) -> Result<f64, Error> {
    if x == 0.0 {
        return Ok(AI0);
    }
    let z = 2.0 / 3.0 * x.abs().powf(1.5);
    if x > 0.0 {
        if z > 700.0 {
            return Ok(0.0);
        }
        Ok((x / 3.0).sqrt() / PI * bessel_kv(1.0 / 3.0, z)?)
    } else {
        let s = (-x).sqrt();
        Ok(s / 3.0 * (bessel_jv(1.0 / 3.0, z)? + bessel_jv(-1.0 / 3.0, z)?))
    }
}

/// Airy Bi(x) via Bessel van orde 1/3.
pub fn airy_bi(x: f64) -> Result<f64, Error> {
    if x == 0.0 {
        return Ok(BI0);
    }
    let z = 2.0 / 3.0 * x.abs().powf(1.5);
    if x > 0.0 {
        Ok((x / 3.0).sqrt() * (bessel_iv(-1.0 / 3.0, z)? + bessel_iv(1.0 / 3.0, z)?))
    } else {
        Ok(((-x) / 3.0).sqrt() * (bessel_jv(-1.0 / 3.0, z)? - bessel_jv(1.0 / 3.0, z)?))
    }
}
// ---------------------------------------------------------------------------------------------
// Verdelingen (dichtheid d, verdelingsfunctie p, kwantiel q)

pub fn pnorm(z: f64) -> f64 {
    0.5 * erfc(-z / std::f64::consts::SQRT_2)
}

pub fn dnorm(z: f64) -> f64 {
    (-0.5 * z * z).exp() / (2.0 * PI).sqrt()
}

/// Inverse van de standaardnormale verdelingsfunctie (Acklam, verfijnd met één Halley-stap).
pub fn qnorm(p: f64) -> f64 {
    const A: [f64; 6] = [-3.969683028665376e1, 2.209460984245205e2, -2.759285104469687e2, 1.383577518672690e2, -3.066479806614716e1, 2.506628277459239];
    const B: [f64; 5] = [-5.447609879822406e1, 1.615858368580409e2, -1.556989798598866e2, 6.680131188771972e1, -1.328068155288572e1];
    const C: [f64; 6] = [-7.784894002430293e-3, -3.223964580411365e-1, -2.400758277161838, -2.549732539343734, 4.374664141464968, 2.938163982698783];
    const D: [f64; 4] = [7.784695709041462e-3, 3.224671290700398e-1, 2.445134137142996, 3.754408661907416];
    let pl = 0.02425;
    let x = if p < pl {
        let q = (-2.0 * p.ln()).sqrt();
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5]) / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else if p <= 1.0 - pl {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    } else {
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        -(((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5]) / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    };
    let e = pnorm(x) - p;
    let u = e * (2.0 * PI).sqrt() * (x * x / 2.0).exp();
    x - u / (1.0 + x * u / 2.0)
}

/// Kwantiel door de verdelingsfunctie `cdf` (stijgend) om te keren op [lo, hi] (bisectie).
pub fn invert(cdf: impl Fn(f64) -> f64, p: f64, mut lo: f64, mut hi: f64) -> f64 {
    // grenzen vergroten tot p ertussen ligt
    while cdf(hi) < p && hi < 1e300 {
        hi = if hi > 0.0 { hi * 2.0 } else { 1.0 };
    }
    while cdf(lo) > p && lo > -1e300 {
        lo = if lo < 0.0 { lo * 2.0 } else { -1.0 };
    }
    for _ in 0..300 {
        let mid = 0.5 * (lo + hi);
        if mid <= lo || mid >= hi {
            break;
        }
        if cdf(mid) < p {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Student-t verdelingsfunctie.
pub fn pt(x: f64, d: f64) -> f64 {
    let ib = beta_inc(d / 2.0, 0.5, d / (d + x * x));
    if x >= 0.0 {
        1.0 - 0.5 * ib
    } else {
        0.5 * ib
    }
}

pub fn dt(x: f64, d: f64) -> f64 {
    (ln_gamma((d + 1.0) / 2.0) - ln_gamma(d / 2.0)).exp() / (d * PI).sqrt() * (1.0 + x * x / d).powf(-(d + 1.0) / 2.0)
}

pub fn pchisq(x: f64, d: f64) -> f64 {
    gamma_p(d / 2.0, x / 2.0)
}

pub fn dchisq(x: f64, d: f64) -> f64 {
    if x < 0.0 {
        return 0.0;
    }
    ((d / 2.0 - 1.0) * x.ln() - x / 2.0 - (d / 2.0) * 2f64.ln() - ln_gamma(d / 2.0)).exp()
}

pub fn pf(x: f64, d1: f64, d2: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    beta_inc(d1 / 2.0, d2 / 2.0, d1 * x / (d1 * x + d2))
}

pub fn df(x: f64, d1: f64, d2: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    ((d1 * x).powf(d1) * d2.powf(d2) / (d1 * x + d2).powf(d1 + d2)).sqrt() / (x * beta(d1 / 2.0, d2 / 2.0))
}

pub fn dbinom(k: f64, n: f64, p: f64) -> f64 {
    if k < 0.0 || k > n {
        return 0.0;
    }
    (ln_gamma(n + 1.0) - ln_gamma(k + 1.0) - ln_gamma(n - k + 1.0) + k * p.ln() + (n - k) * (1.0 - p).ln()).exp()
}

pub fn pbinom(k: f64, n: f64, p: f64) -> f64 {
    let k = k.floor();
    if k < 0.0 {
        0.0
    } else if k >= n {
        1.0
    } else {
        beta_inc(n - k, k + 1.0, 1.0 - p)
    }
}

pub fn dpois(k: f64, l: f64) -> f64 {
    if k < 0.0 {
        return 0.0;
    }
    (k * l.ln() - l - ln_gamma(k + 1.0)).exp()
}

pub fn ppois(k: f64, l: f64) -> f64 {
    let k = k.floor();
    if k < 0.0 {
        0.0
    } else {
        gamma_q(k + 1.0, l)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol * (1.0 + b.abs())
    }

    // Referentiewaarden: Abramowitz & Stegun en standaardtabellen (bewust uitgeschreven).
    #[test]
    #[allow(clippy::approx_constant)]
    fn gamma_en_erf() {
        assert!(close(gamma(0.5).unwrap(), PI.sqrt(), 1e-14));
        assert!(close(gamma(5.0).unwrap(), 24.0, 1e-15));
        assert!(close(gamma(-0.5).unwrap(), -2.0 * PI.sqrt(), 1e-13));
        assert!(close(erf(1.0), 0.842_700_792_949_714_9, 1e-14));
        assert!(close(erfc(2.0), 0.004_677_734_981_047_266, 1e-13));
        assert!(close(erf(-0.5), -0.520_499_877_813_046_5, 1e-14));
        assert!(close(digamma(1.0).unwrap(), -0.577_215_664_901_532_9, 1e-13));
        assert!(close(beta(2.0, 3.0), 1.0 / 12.0, 1e-14));
    }

    #[test]
    fn bessel_gebroken_orde_en_airy() {
        // halftallige orde in gesloten vorm
        let s = (2.0 / PI).sqrt();
        assert!(close(bessel_jv(0.5, 1.0).unwrap(), s * 1f64.sin(), 1e-10));
        assert!(close(bessel_jv(-0.5, 2.0).unwrap(), s / 2f64.sqrt() * 2f64.cos(), 1e-10));
        assert!(close(bessel_yv(0.5, 1.0).unwrap(), -s * 1f64.cos(), 1e-10));
        assert!(close(bessel_iv(0.5, 1.0).unwrap(), s * 1f64.sinh(), 1e-10));
        assert!(close(bessel_kv(0.5, 1.0).unwrap(), (PI / 2.0).sqrt() * (-1f64).exp(), 1e-10));
        assert!(close(bessel_jv(2.0, 3.0).unwrap(), bessel_j(2, 3.0), 1e-14));
        // bolfuncties: j1(x) = sin x/x² − cos x/x, y0(x) = −cos x/x
        let x = 2.5f64;
        assert!(close(sph_j(1, x).unwrap(), x.sin() / (x * x) - x.cos() / x, 1e-10));
        assert!(close(sph_j(0, 0.0).unwrap(), 1.0, 1e-15));
        assert!(close(sph_y(0, x).unwrap(), -x.cos() / x, 1e-10));
        // Airy (Abramowitz & Stegun, tabel 10.11)
        assert!(close(airy_ai(1.0).unwrap(), 0.135_292_416_312_881_4, 1e-9));
        assert!(close(airy_bi(1.0).unwrap(), 1.207_423_594_952_871, 1e-9));
        assert!(close(airy_ai(-2.0).unwrap(), 0.227_407_428_201_528_1, 1e-9));
        assert!(close(airy_bi(-2.0).unwrap(), -0.412_302_587_956_398_2, 1e-9));
        assert!(close(airy_ai(1e-9).unwrap(), 0.355_028_053_6, 1e-8));
    }

    #[test]
    fn bessel() {
        assert!(close(bessel_j(0, 1.0), 0.765_197_686_557_966_6, 1e-14));
        assert!(close(bessel_j(1, 1.0), 0.440_050_585_744_933_5, 1e-14));
        assert!(close(bessel_j(2, 3.0), 0.486_091_260_585_891_1, 1e-14));
        assert!(close(bessel_y(0, 1.0).unwrap(), 0.088_256_964_215_676_96, 1e-9));
        assert!(close(bessel_y(1, 1.0).unwrap(), -0.781_212_821_300_288_7, 1e-9));
        assert!(close(bessel_i(0, 1.0), 1.266_065_877_752_008_4, 1e-14));
        assert!(close(bessel_i(1, 1.0), 0.565_159_103_992_485_0, 1e-14));
        assert!(close(bessel_k(0, 1.0).unwrap(), 0.421_024_438_240_708_3, 1e-9));
        assert!(close(bessel_k(1, 1.0).unwrap(), 0.601_907_230_197_234_6, 1e-9));
    }

    #[test]
    fn verdelingen() {
        assert!(close(qnorm(0.975), 1.959_963_984_540_054, 1e-13));
        assert!(close(pnorm(1.0), 0.841_344_746_068_542_9, 1e-14));
        // onafhankelijk: verdelingsfunctie = ½ + ∫₀² dichtheid
        let via_integraal = 0.5 + numeric::integrate(&mut |x| Ok(dt(x, 10.0)), 0.0, 2.0).unwrap();
        assert!(close(pt(2.0, 10.0), via_integraal, 1e-10), "{} {}", pt(2.0, 10.0), via_integraal);
        let chi = numeric::integrate(&mut |x| Ok(dchisq(x, 3.0)), 0.0, 4.0).unwrap();
        assert!(close(pchisq(4.0, 3.0), chi, 1e-10));
        let f = numeric::integrate(&mut |x| Ok(df(x, 3.0, 7.0)), 0.0, 2.5).unwrap();
        assert!(close(pf(2.5, 3.0, 7.0), f, 1e-9));
        assert!(close(invert(|x| pt(x, 10.0), 0.975, -10.0, 10.0), 2.228_138_851_986_274, 1e-10));
        assert!(close(invert(|x| pchisq(x, 2.0), 0.95, 0.0, 10.0), 5.991_464_547_107_979, 1e-10));
        assert!(close(invert(|x| pf(x, 2.0, 10.0), 0.95, 0.0, 10.0), 4.102_821_015_130_399, 1e-9));
        assert!(close(pbinom(3.0, 10.0, 0.5), 176.0 / 1024.0, 1e-13));
        assert!(close(dpois(2.0, 3.0), 4.5 * (-3f64).exp(), 1e-14));
        assert!(close(ppois(2.0, 3.0), 8.5 * (-3f64).exp(), 1e-13));
    }
}

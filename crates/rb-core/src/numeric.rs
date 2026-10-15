//! Numeriek integreren en differentiëren op SI-waarden (zonder eenheden; die regelt de evaluator).

use crate::error::Error;

type Func<'a> = dyn FnMut(f64) -> Result<f64, Error> + 'a;

// Gauss-Kronrod 7/15 (knooppunten en gewichten uit QUADPACK, in de oorspronkelijke precisie).
#[allow(clippy::excessive_precision)]
const XGK: [f64; 8] = [
    0.991455371120812639206854697526329,
    0.949107912342758524526189684047851,
    0.864864423359769072789712788640926,
    0.741531185599394439863864773280788,
    0.586087235467691130294144845693013,
    0.405845151377397166906606412076961,
    0.207784955007898467600689403773245,
    0.000000000000000000000000000000000,
];
#[allow(clippy::excessive_precision)]
const WGK: [f64; 8] = [
    0.022935322010529224963732008058970,
    0.063092092629978553290700663189204,
    0.104790010322250183839876322541518,
    0.140653259715525918745189590510238,
    0.169004726639267902826583426598550,
    0.190350578064785409913256402421014,
    0.204432940075298892414161999234649,
    0.209482141084727828012999174891714,
];
#[allow(clippy::excessive_precision)]
const WG: [f64; 4] = [
    0.129484966168869693270611432679082,
    0.279705391489276667901467771423780,
    0.381830050505118944950369775488975,
    0.417959183673469387755102040816327,
];

const MAX_EVALS: usize = 500_000;
const REL_TOL: f64 = 1e-10;

struct Quad<'a, 'b> {
    f: &'a mut Func<'b>,
    evals: usize,
}

struct Piece {
    value: f64,
    err: f64,
    abs: f64,
}

impl Quad<'_, '_> {
    fn eval(&mut self, x: f64) -> Result<f64, Error> {
        self.evals += 1;
        if self.evals > MAX_EVALS {
            return Err(Error::Domain(
                "de integraal convergeert niet (te veel functie-evaluaties); \
                 controleer of de functie ergens oneindig wordt"
                    .to_string(),
            ));
        }
        let y = (self.f)(x)?;
        if !y.is_finite() {
            return Err(Error::Domain(format!(
                "de functie onder de integraal is niet eindig bij x = {x} (SI)"
            )));
        }
        Ok(y)
    }

    fn gk15(&mut self, a: f64, b: f64) -> Result<Piece, Error> {
        let c = 0.5 * (a + b);
        let h = 0.5 * (b - a);
        let fc = self.eval(c)?;
        let mut resk = fc * WGK[7];
        let mut resg = fc * WG[3];
        let mut resabs = fc.abs() * WGK[7];
        for j in 0..7 {
            let x = h * XGK[j];
            let (f1, f2) = (self.eval(c - x)?, self.eval(c + x)?);
            resk += WGK[j] * (f1 + f2);
            resabs += WGK[j] * (f1.abs() + f2.abs());
            if j % 2 == 1 {
                resg += WG[j / 2] * (f1 + f2);
            }
        }
        Ok(Piece {
            value: resk * h,
            err: ((resk - resg) * h).abs(),
            abs: resabs * h.abs(),
        })
    }

    fn adapt(&mut self, a: f64, b: f64, whole: Piece, tol: f64, depth: u32) -> Result<f64, Error> {
        let roundoff = 50.0 * f64::EPSILON * whole.abs;
        if whole.err <= tol || whole.err <= roundoff || depth >= 50 {
            return Ok(whole.value);
        }
        let m = 0.5 * (a + b);
        if m <= a || m >= b {
            return Ok(whole.value);
        }
        let left = self.gk15(a, m)?;
        let right = self.gk15(m, b)?;
        Ok(self.adapt(a, m, left, tol / 2.0, depth + 1)? + self.adapt(m, b, right, tol / 2.0, depth + 1)?)
    }

    fn finite(&mut self, a: f64, b: f64) -> Result<f64, Error> {
        let whole = self.gk15(a, b)?;
        let tol = (REL_TOL * whole.value.abs()).max(1e-12 * whole.abs);
        self.adapt(a, b, whole, tol, 0)
    }
}

/// ∫ f(x) dx van `a` tot `b`. Grenzen mogen ±∞ zijn.
pub fn integrate(f: &mut Func<'_>, a: f64, b: f64) -> Result<f64, Error> {
    if a.is_nan() || b.is_nan() {
        return Err(Error::Domain("een integratiegrens is geen getal".to_string()));
    }
    if a == b {
        return Ok(0.0);
    }
    if a > b {
        return Ok(-integrate(f, b, a)?);
    }
    match (a.is_infinite(), b.is_infinite()) {
        (false, false) => Quad { f, evals: 0 }.finite(a, b),
        // [a, ∞): x = a + t/(1-t), dx = dt/(1-t)²
        (false, true) => {
            let mut g = |t: f64| -> Result<f64, Error> {
                let u = 1.0 - t;
                Ok(f(a + t / u)? / (u * u))
            };
            Quad { f: &mut g, evals: 0 }.finite(0.0, 1.0)
        }
        // (-∞, b]: x = b - t/(1-t)
        (true, false) => {
            let mut g = |t: f64| -> Result<f64, Error> {
                let u = 1.0 - t;
                Ok(f(b - t / u)? / (u * u))
            };
            Quad { f: &mut g, evals: 0 }.finite(0.0, 1.0)
        }
        (true, true) => Ok(integrate(f, f64::NEG_INFINITY, 0.0)? + integrate(f, 0.0, f64::INFINITY)?),
    }
}

fn binomial(n: u32, k: u32) -> f64 {
    (0..k).fold(1.0, |acc, i| acc * (n - i) as f64 / (i + 1) as f64)
}

/// De `n`-de afgeleide van f in x (n = 1 ... 5): centrale differenties met Richardson-extrapolatie
/// (methode van Ridders).
pub fn derivative(f: &mut Func<'_>, x: f64, n: u32) -> Result<f64, Error> {
    if !(1..=5).contains(&n) {
        return Err(Error::Domain("de orde van de afgeleide moet 1 tot en met 5 zijn".to_string()));
    }
    // centrale differentie van orde n: Σ (-1)^k C(n,k) f(x + (n/2 - k) h) / h^n
    let stencil: Vec<(f64, f64)> = (0..=n)
        .map(|k| {
            let sign = if k % 2 == 0 { 1.0 } else { -1.0 };
            (n as f64 / 2.0 - k as f64, sign * binomial(n, k))
        })
        .collect();
    let mut diff = |h: f64| -> Result<f64, Error> {
        let mut s = 0.0;
        for &(off, c) in &stencil {
            let y = f(x + off * h)?;
            if !y.is_finite() {
                return Err(Error::Domain(format!(
                    "de functie is niet eindig vlak bij x = {x} (SI); afgeleide niet te bepalen"
                )));
            }
            s += c * y;
        }
        Ok(s / h.powi(n as i32))
    };

    const NTAB: usize = 12;
    const CON: f64 = 1.4;
    const CON2: f64 = CON * CON;
    const SAFE: f64 = 2.0;
    let mut h = if x != 0.0 { 0.1 * x.abs() } else { 0.1 };
    let mut a = [[0.0f64; NTAB]; NTAB];
    a[0][0] = diff(h)?;
    let mut err = f64::MAX;
    let mut ans = a[0][0];
    for i in 1..NTAB {
        h /= CON;
        a[0][i] = diff(h)?;
        let mut fac = CON2;
        for j in 1..=i {
            a[j][i] = (a[j - 1][i] * fac - a[j - 1][i - 1]) / (fac - 1.0);
            fac *= CON2;
            let errt = (a[j][i] - a[j - 1][i])
                .abs()
                .max((a[j][i] - a[j - 1][i - 1]).abs());
            if errt <= err {
                err = errt;
                ans = a[j][i];
            }
        }
        if (a[i][i] - a[i - 1][i - 1]).abs() >= SAFE * err {
            break;
        }
    }
    Ok(ans)
}

// ---------------------------------------------------------------------------------------------
// Kleine lineaire stelsels en kleinste kwadraten

/// Los A·x = b op (A is n×n, rij voor rij) met Gauss-eliminatie en pivotering. `None` als A
/// (bijna) singulier is.
pub fn solve_dense(mut a: Vec<f64>, n: usize, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let scale = a.iter().fold(0.0f64, |s, v| s.max(v.abs()));
    if scale == 0.0 || !scale.is_finite() {
        return None;
    }
    for k in 0..n {
        let p = (k..n).max_by(|&i, &j| a[i * n + k].abs().total_cmp(&a[j * n + k].abs()))?;
        if a[p * n + k].abs() <= scale * 1e-15 * n as f64 {
            return None;
        }
        if p != k {
            for j in 0..n {
                a.swap(k * n + j, p * n + j);
            }
            b.swap(k, p);
        }
        for i in k + 1..n {
            let f = a[i * n + k] / a[k * n + k];
            for j in k..n {
                a[i * n + j] -= f * a[k * n + j];
            }
            b[i] -= f * b[k];
        }
    }
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let s: f64 = (i + 1..n).map(|j| a[i * n + j] * x[j]).sum();
        x[i] = (b[i] - s) / a[i * n + i];
    }
    Some(x)
}

/// Kleinste-kwadratenoplossing van A·c ≈ y (A is m×n, m ≥ n) met Householder-QR.
pub fn least_squares(a: &[f64], m: usize, n: usize, y: &[f64]) -> Result<Vec<f64>, Error> {
    if m < n {
        return Err(Error::Domain(format!(
            "te weinig gegevens: {m} punten voor {n} onbekende coëfficiënten"
        )));
    }
    let mut r = a.to_vec();
    let mut q_ty = y.to_vec();
    for k in 0..n {
        let norm = (k..m).map(|i| r[i * n + k].powi(2)).sum::<f64>().sqrt();
        if norm == 0.0 {
            continue;
        }
        let alpha = if r[k * n + k] > 0.0 { -norm } else { norm };
        let mut v: Vec<f64> = (k..m).map(|i| r[i * n + k]).collect();
        v[0] -= alpha;
        let vn = v.iter().map(|x| x * x).sum::<f64>().sqrt();
        if vn == 0.0 {
            continue;
        }
        v.iter_mut().for_each(|x| *x /= vn);
        for j in 0..n {
            let d: f64 = (k..m).map(|i| v[i - k] * r[i * n + j]).sum();
            for i in k..m {
                r[i * n + j] -= 2.0 * v[i - k] * d;
            }
        }
        let d: f64 = (k..m).map(|i| v[i - k] * q_ty[i]).sum();
        for i in k..m {
            q_ty[i] -= 2.0 * v[i - k] * d;
        }
    }
    let scale = (0..n).fold(0.0f64, |s, i| s.max(r[i * n + i].abs()));
    let mut c = vec![0.0; n];
    for i in (0..n).rev() {
        if r[i * n + i].abs() <= scale * 1e-13 {
            return Err(Error::Domain(
                "de basisfuncties zijn (bijna) lineair afhankelijk voor deze gegevens".to_string(),
            ));
        }
        let s: f64 = (i + 1..n).map(|j| r[i * n + j] * c[j]).sum();
        c[i] = (q_ty[i] - s) / r[i * n + i];
    }
    Ok(c)
}

// ---------------------------------------------------------------------------------------------
// Oplossers

type VecFunc<'a> = dyn FnMut(&[f64]) -> Result<Vec<f64>, Error> + 'a;
type ScalarFunc<'a> = dyn FnMut(&[f64]) -> Result<f64, Error> + 'a;

/// Levenberg–Marquardt: minimaliseer Σ r_k(x)². Geeft (x, Σ r²). Werkt het best met
/// geschaalde variabelen (orde 1).
pub fn levenberg_marquardt(f: &mut VecFunc<'_>, x0: Vec<f64>) -> Result<(Vec<f64>, f64), Error> {
    let n = x0.len();
    let mut x = x0;
    let mut r = f(&x)?;
    let m = r.len();
    let cost = |r: &[f64]| r.iter().map(|v| v * v).sum::<f64>();
    let mut c = cost(&r);
    let mut lambda = 1e-3;
    for _ in 0..500 {
        if c <= 1e-30 {
            break;
        }
        // Jacobiaan met voorwaartse differenties
        let mut jac = vec![0.0; m * n];
        for j in 0..n {
            let h = 1e-7 * x[j].abs().max(1.0);
            let mut xh = x.clone();
            xh[j] += h;
            let rh = f(&xh)?;
            for i in 0..m {
                jac[i * n + j] = (rh[i] - r[i]) / h;
            }
        }
        let mut jtj = vec![0.0; n * n];
        let mut jtr = vec![0.0; n];
        for i in 0..m {
            for a in 0..n {
                jtr[a] += jac[i * n + a] * r[i];
                for b in 0..n {
                    jtj[a * n + b] += jac[i * n + a] * jac[i * n + b];
                }
            }
        }
        let mut improved = false;
        let mut step_small = false;
        while lambda < 1e16 {
            let mut a = jtj.clone();
            for k in 0..n {
                a[k * n + k] += lambda * jtj[k * n + k].max(1e-12);
            }
            let delta = match solve_dense(a, n, jtr.iter().map(|v| -v).collect()) {
                Some(d) => d,
                None => {
                    lambda *= 10.0;
                    continue;
                }
            };
            let xn: Vec<f64> = x.iter().zip(&delta).map(|(a, b)| a + b).collect();
            let rn = match f(&xn) {
                Ok(rn) if rn.iter().all(|v| v.is_finite()) => rn,
                _ => {
                    lambda *= 10.0;
                    continue;
                }
            };
            let cn = cost(&rn);
            if cn < c {
                let dn = delta.iter().map(|v| v * v).sum::<f64>().sqrt();
                let xnorm = xn.iter().map(|v| v * v).sum::<f64>().sqrt();
                step_small = dn <= 1e-15 * (xnorm + 1e-15) || (c - cn) <= 1e-15 * c;
                x = xn;
                r = rn;
                c = cn;
                lambda = (lambda / 3.0).max(1e-15);
                improved = true;
                break;
            }
            lambda *= 4.0;
        }
        if !improved || step_small {
            break;
        }
    }
    Ok((x, c))
}

/// Nelder–Mead (simplexmethode) voor het minimum van f, vanaf x0 met beginstap `step` per variabele.
pub fn nelder_mead(f: &mut ScalarFunc<'_>, x0: &[f64], step: &[f64]) -> Result<Vec<f64>, Error> {
    let n = x0.len();
    let mut pts: Vec<Vec<f64>> = vec![x0.to_vec()];
    for j in 0..n {
        let mut p = x0.to_vec();
        p[j] += step[j];
        pts.push(p);
    }
    let mut vals = pts.iter().map(|p| f(p)).collect::<Result<Vec<_>, _>>()?;
    let max_evals = 4000 * (n + 1);
    let mut evals = n + 1;
    while evals < max_evals {
        let mut order: Vec<usize> = (0..=n).collect();
        order.sort_by(|&a, &b| vals[a].total_cmp(&vals[b]));
        pts = order.iter().map(|&k| pts[k].clone()).collect();
        vals = order.iter().map(|&k| vals[k]).collect();
        let fspread = (vals[n] - vals[0]).abs();
        let xspread = (1..=n)
            .flat_map(|k| (0..n).map(move |j| (k, j)))
            .map(|(k, j)| (pts[k][j] - pts[0][j]).abs())
            .fold(0.0f64, f64::max);
        if fspread <= 1e-15 * vals[0].abs().max(1e-300) && xspread <= 1e-11 || xspread <= 1e-13 {
            break;
        }
        let centroid: Vec<f64> = (0..n).map(|j| pts[..n].iter().map(|p| p[j]).sum::<f64>() / n as f64).collect();
        let along = |t: f64| -> Vec<f64> { (0..n).map(|j| centroid[j] + t * (pts[n][j] - centroid[j])).collect() };
        let xr = along(-1.0);
        let fr = f(&xr)?;
        evals += 1;
        if fr < vals[0] {
            let xe = along(-2.0);
            let fe = f(&xe)?;
            evals += 1;
            if fe < fr {
                pts[n] = xe;
                vals[n] = fe;
            } else {
                pts[n] = xr;
                vals[n] = fr;
            }
        } else if fr < vals[n - 1] {
            pts[n] = xr;
            vals[n] = fr;
        } else {
            let (xc, fc) = if fr < vals[n] {
                let xc = along(-0.5);
                let fc = f(&xc)?;
                (xc, fc)
            } else {
                let xc = along(0.5);
                let fc = f(&xc)?;
                (xc, fc)
            };
            evals += 1;
            if fc < vals[n].min(fr) {
                pts[n] = xc;
                vals[n] = fc;
            } else {
                // krimpen naar het beste punt
                for k in 1..=n {
                    pts[k] = (0..n).map(|j| pts[0][j] + 0.5 * (pts[k][j] - pts[0][j])).collect();
                    vals[k] = f(&pts[k])?;
                }
                evals += n;
            }
        }
    }
    let best = (0..=n).min_by(|&a, &b| vals[a].total_cmp(&vals[b])).unwrap_or(0);
    Ok(pts[best].clone())
}

/// Nulpunt van f op [a, b] met de methode van Brent (f(a) en f(b) moeten een verschillend teken hebben).
pub fn brent(f: &mut dyn FnMut(f64) -> Result<f64, Error>, mut a: f64, mut b: f64) -> Result<f64, Error> {
    let mut fa = f(a)?;
    let mut fb = f(b)?;
    if fa == 0.0 {
        return Ok(a);
    }
    if fb == 0.0 {
        return Ok(b);
    }
    if fa.signum() == fb.signum() {
        return Err(Error::Domain(
            "root: de functie heeft aan beide grenzen hetzelfde teken; kies grenzen waartussen een nulpunt ligt".to_string(),
        ));
    }
    let (mut c, mut fc) = (a, fa);
    let mut d = b - a;
    let mut e = d;
    for _ in 0..200 {
        if fb.signum() == fc.signum() {
            c = a;
            fc = fa;
            d = b - a;
            e = d;
        }
        if fc.abs() < fb.abs() {
            a = b;
            b = c;
            c = a;
            fa = fb;
            fb = fc;
            fc = fa;
        }
        let tol = 2.0 * f64::EPSILON * b.abs() + 1e-300;
        let m = 0.5 * (c - b);
        if m.abs() <= tol || fb == 0.0 {
            return Ok(b);
        }
        if e.abs() >= tol && fa.abs() > fb.abs() {
            let s = fb / fa;
            let (mut p, mut q);
            if a == c {
                p = 2.0 * m * s;
                q = 1.0 - s;
            } else {
                let qq = fa / fc;
                let r = fb / fc;
                p = s * (2.0 * m * qq * (qq - r) - (b - a) * (r - 1.0));
                q = (qq - 1.0) * (r - 1.0) * (s - 1.0);
            }
            if p > 0.0 {
                q = -q;
            } else {
                p = -p;
            }
            if 2.0 * p < (3.0 * m * q - (tol * q).abs()).min((e * q).abs()) {
                e = d;
                d = p / q;
            } else {
                d = m;
                e = m;
            }
        } else {
            d = m;
            e = m;
        }
        a = b;
        fa = fb;
        b += if d.abs() > tol { d } else { tol * m.signum() };
        fb = f(b)?;
    }
    Ok(b)
}

/// Nulpunt van f vanaf een startwaarde: eerst een tekenwissel zoeken rond x0, dan Brent.
pub fn root_from_guess(f: &mut dyn FnMut(f64) -> Result<f64, Error>, x0: f64) -> Result<f64, Error> {
    let f0 = f(x0)?;
    if f0 == 0.0 {
        return Ok(x0);
    }
    // secant-stappen; als dat een tekenwissel oplevert, maakt Brent het af
    let mut h = if x0 != 0.0 { 1e-3 * x0.abs() } else { 1e-3 };
    let (mut x1, mut f1) = (x0, f0);
    for _ in 0..60 {
        let x2 = x1 + h;
        let f2 = match f(x2) {
            Ok(v) if v.is_finite() => v,
            _ => {
                h = -h / 2.0;
                continue;
            }
        };
        if f2 == 0.0 {
            return Ok(x2);
        }
        if f2.signum() != f1.signum() {
            return brent(f, x1.min(x2), x1.max(x2));
        }
        // secant-schatting; anders verder uitwaaieren
        let slope = (f2 - f1) / (x2 - x1);
        let next = if slope != 0.0 && slope.is_finite() { x2 - f2 / slope } else { x2 + 2.0 * h };
        h = next - x2;
        if !h.is_finite() || h == 0.0 {
            h = 2.0 * (x2 - x1);
        }
        // niet te ver springen
        let limit = 10.0 * x2.abs().max(1.0);
        if h.abs() > limit {
            h = limit * h.signum();
        }
        x1 = x2;
        f1 = f2;
    }
    Err(Error::Domain(
        "root: geen nulpunt gevonden vanaf deze startwaarde; probeer een andere startwaarde of root(f, x, a, b) met grenzen".to_string(),
    ))
}

// ---------------------------------------------------------------------------------------------
// Differentiaalvergelijkingen y' = D(x, y)

type OdeFunc<'a> = dyn FnMut(f64, &[f64]) -> Result<Vec<f64>, Error> + 'a;

fn axpy(y: &[f64], h: f64, ks: &[(&[f64], f64)]) -> Vec<f64> {
    (0..y.len())
        .map(|i| y[i] + h * ks.iter().map(|(k, c)| c * k[i]).sum::<f64>())
        .collect()
}

/// Klassieke Runge-Kutta (orde 4) met vaste stap; geeft de oplossing in `n + 1` punten.
pub fn rk4(f: &mut OdeFunc<'_>, y0: &[f64], x0: f64, x1: f64, n: usize) -> Result<Vec<Vec<f64>>, Error> {
    let h = (x1 - x0) / n as f64;
    let mut out = vec![y0.to_vec()];
    let mut y = y0.to_vec();
    for k in 0..n {
        let x = x0 + h * k as f64;
        let k1 = f(x, &y)?;
        let k2 = f(x + h / 2.0, &axpy(&y, h, &[(&k1, 0.5)]))?;
        let k3 = f(x + h / 2.0, &axpy(&y, h, &[(&k2, 0.5)]))?;
        let k4 = f(x + h, &axpy(&y, h, &[(&k3, 1.0)]))?;
        y = axpy(&y, h, &[(&k1, 1.0 / 6.0), (&k2, 1.0 / 3.0), (&k3, 1.0 / 3.0), (&k4, 1.0 / 6.0)]);
        out.push(y.clone());
    }
    Ok(out)
}

/// Dormand–Prince 5(4) met adaptieve stap; geeft de oplossing in `n + 1` gelijk verdeelde punten.
pub fn dopri(f: &mut OdeFunc<'_>, y0: &[f64], x0: f64, x1: f64, n: usize, rtol: f64) -> Result<Vec<Vec<f64>>, Error> {
    const C: [f64; 7] = [0.0, 0.2, 0.3, 0.8, 8.0 / 9.0, 1.0, 1.0];
    const A: [[f64; 6]; 7] = [
        [0.0; 6],
        [0.2, 0.0, 0.0, 0.0, 0.0, 0.0],
        [3.0 / 40.0, 9.0 / 40.0, 0.0, 0.0, 0.0, 0.0],
        [44.0 / 45.0, -56.0 / 15.0, 32.0 / 9.0, 0.0, 0.0, 0.0],
        [19372.0 / 6561.0, -25360.0 / 2187.0, 64448.0 / 6561.0, -212.0 / 729.0, 0.0, 0.0],
        [9017.0 / 3168.0, -355.0 / 33.0, 46732.0 / 5247.0, 49.0 / 176.0, -5103.0 / 18656.0, 0.0],
        [35.0 / 384.0, 0.0, 500.0 / 1113.0, 125.0 / 192.0, -2187.0 / 6784.0, 11.0 / 84.0],
    ];
    const B5: [f64; 7] = [35.0 / 384.0, 0.0, 500.0 / 1113.0, 125.0 / 192.0, -2187.0 / 6784.0, 11.0 / 84.0, 0.0];
    const B4: [f64; 7] = [5179.0 / 57600.0, 0.0, 7571.0 / 16695.0, 393.0 / 640.0, -92097.0 / 339200.0, 187.0 / 2100.0, 1.0 / 40.0];
    let dim = y0.len();
    let mut out = vec![y0.to_vec()];
    let mut y = y0.to_vec();
    let mut x = x0;
    let span = x1 - x0;
    let mut h = span / (n as f64 * 4.0);
    let mut steps = 0usize;
    for k in 1..=n {
        let target = x0 + span * k as f64 / n as f64;
        while (target - x) * span.signum() > 1e-14 * span.abs() {
            steps += 1;
            if steps > 2_000_000 {
                return Err(Error::Domain("de differentiaalvergelijking vraagt te veel stappen (stijf probleem?)".to_string()));
            }
            if (x + h - target) * span.signum() > 0.0 {
                h = target - x;
            }
            let mut ks: Vec<Vec<f64>> = Vec::with_capacity(7);
            for s in 0..7 {
                let yi: Vec<f64> = (0..dim).map(|i| y[i] + h * (0..s).map(|j| A[s][j] * ks[j][i]).sum::<f64>()).collect();
                ks.push(f(x + C[s] * h, &yi)?);
            }
            let y5: Vec<f64> = (0..dim).map(|i| y[i] + h * (0..7).map(|s| B5[s] * ks[s][i]).sum::<f64>()).collect();
            let err = (0..dim)
                .map(|i| {
                    let e = h * (0..7).map(|s| (B5[s] - B4[s]) * ks[s][i]).sum::<f64>();
                    let sc = rtol * y[i].abs().max(y5[i].abs()) + rtol * 1e-3;
                    (e / sc).powi(2)
                })
                .sum::<f64>()
                .sqrt()
                / (dim as f64).sqrt();
            if !err.is_finite() {
                h /= 10.0;
                continue;
            }
            if err <= 1.0 {
                x += h;
                y = y5;
            }
            let fac = if err == 0.0 { 5.0 } else { (0.9 * err.powf(-0.2)).clamp(0.2, 5.0) };
            h *= fac;
            if h.abs() < 1e-14 * span.abs() {
                return Err(Error::Domain("de stapgrootte wordt te klein (singuliere differentiaalvergelijking?)".to_string()));
            }
        }
        x = target;
        out.push(y.clone());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn oplossers() {
        // Rosenbrock met Levenberg–Marquardt: r = (1-x, 10(y-x²))
        let (x, c) = levenberg_marquardt(&mut |v| Ok(vec![1.0 - v[0], 10.0 * (v[1] - v[0] * v[0])]), vec![-1.2, 1.0]).unwrap();
        assert!((x[0] - 1.0).abs() < 1e-8 && (x[1] - 1.0).abs() < 1e-8 && c < 1e-16, "{x:?}");
        // Nelder–Mead op een kwadratische vorm
        let x = nelder_mead(&mut |v| Ok((v[0] - 3.0).powi(2) + 2.0 * (v[1] + 1.0).powi(2)), &[0.0, 0.0], &[0.5, 0.5]).unwrap();
        assert!((x[0] - 3.0).abs() < 1e-6 && (x[1] + 1.0).abs() < 1e-6, "{x:?}");
        // Brent en startwaarde
        let r = brent(&mut |x| Ok(x * x - 2.0), 0.0, 2.0).unwrap();
        assert!((r - 2f64.sqrt()).abs() < 1e-15);
        let r = root_from_guess(&mut |x| Ok(x.cos() - x), 1.0).unwrap();
        assert!((r - 0.739_085_133_215_160_6).abs() < 1e-14);
        // kleinste kwadraten: rechte door (0,1), (1,3), (2,5)
        let c = least_squares(&[1.0, 0.0, 1.0, 1.0, 1.0, 2.0], 3, 2, &[1.0, 3.0, 5.0]).unwrap();
        assert!((c[0] - 1.0).abs() < 1e-14 && (c[1] - 2.0).abs() < 1e-14);
    }

    #[test]
    fn differentiaalvergelijkingen() {
        // y' = -y, y(0) = 1 → e^-x
        let sol = rk4(&mut |_, y| Ok(vec![-y[0]]), &[1.0], 0.0, 1.0, 100).unwrap();
        assert!((sol[100][0] - (-1f64).exp()).abs() < 1e-9);
        // harmonische oscillator met Dormand–Prince: y'' = -y → cos
        let sol = dopri(&mut |_, y| Ok(vec![y[1], -y[0]]), &[1.0, 0.0], 0.0, 2.0 * PI, 4, 1e-10).unwrap();
        assert!((sol[4][0] - 1.0).abs() < 1e-8 && (sol[2][0] + 1.0).abs() < 1e-8, "{:?}", sol);
    }

    #[test]
    fn klein_stelsel() {
        let x = solve_dense(vec![2.0, 1.0, 1.0, 3.0], 2, vec![3.0, 5.0]).unwrap();
        assert!((x[0] - 0.8).abs() < 1e-15 && (x[1] - 1.4).abs() < 1e-15);
        assert!(solve_dense(vec![1.0, 2.0, 2.0, 4.0], 2, vec![1.0, 2.0]).is_none());
    }

    fn int(mut f: impl FnMut(f64) -> f64, a: f64, b: f64) -> f64 {
        integrate(&mut |x| Ok(f(x)), a, b).unwrap()
    }

    fn der(mut f: impl FnMut(f64) -> f64, x: f64, n: u32) -> f64 {
        derivative(&mut |x| Ok(f(x)), x, n).unwrap()
    }

    #[test]
    fn integralen() {
        assert!((int(|x| x * x, 0.0, 3.0) - 9.0).abs() < 1e-12);
        assert!((int(f64::sin, 0.0, PI) - 2.0).abs() < 1e-12);
        // rond nul: absolute tolerantie
        assert!(int(f64::sin, 0.0, 2.0 * PI).abs() < 1e-12);
        // singulier aan de rand: ∫ 1/√x van 0 tot 1 = 2
        assert!((int(|x| 1.0 / x.sqrt(), 0.0, 1.0) - 2.0).abs() < 1e-8);
        // oneindige grenzen
        assert!((int(|x| (-x * x).exp(), f64::NEG_INFINITY, f64::INFINITY) - PI.sqrt()).abs() < 1e-10);
        assert!((int(|x| 1.0 / (1.0 + x * x), 0.0, f64::INFINITY) - PI / 2.0).abs() < 1e-10);
        // omgekeerde grenzen
        assert!((int(|x| x, 1.0, 0.0) + 0.5).abs() < 1e-14);
    }

    #[test]
    fn afgeleiden() {
        assert!((der(|x| x.powi(3), 2.0, 1) - 12.0).abs() < 1e-9);
        assert!((der(f64::sin, 0.0, 1) - 1.0).abs() < 1e-10);
        assert!((der(|x| x.powi(3), 2.0, 2) - 12.0).abs() < 1e-7);
        assert!((der(|x| x.powi(3), 2.0, 3) - 6.0).abs() < 1e-5);
        assert!((der(f64::exp, 1.0, 4) - 1f64.exp()).abs() < 1e-3);
    }
}

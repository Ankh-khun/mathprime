//! Lineaire algebra op matrices met eenheden.
//!
//! **Eenheden.** Een vierkante matrix waarmee je kunt rekenen heeft elementen met eenheid r_i·c_j
//! (een "rij-eenheid" maal een "kolom-eenheid"), bijvoorbeeld een stijfheidsmatrix met N/m en N.
//! De determinant heeft dan eenheid Π r_i · Π c_j en de inverse eenheid 1/(c_i·r_j). Eigenwaarden
//! vragen bovendien dat r_i·c_i voor alle i gelijk is (dat is de eenheid van de eigenwaarden).
//! Ontbindingen die alleen bij één eenheid zinvol zijn (SVD, QR, Cholesky, ...) vragen dat alle
//! elementen dezelfde eenheid hebben.
//!
//! Intern wordt gerekend met complexe getallen ([`C`]) op de SI-waarden.

use crate::error::Error;
use crate::value::{Complex, Matrix, Value};
use rb_units::{Dim, Quantity, UnitError};
use std::ops::{Add, Div, Mul, Neg, Sub};

fn dom(msg: impl Into<String>) -> Error {
    Error::Domain(msg.into())
}

/// Een complex getal zonder eenheid, voor de numerieke kern.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub(crate) struct C {
    pub re: f64,
    pub im: f64,
}

impl C {
    pub const fn new(re: f64, im: f64) -> C {
        C { re, im }
    }
    pub const ZERO: C = C::new(0.0, 0.0);
    pub const ONE: C = C::new(1.0, 0.0);
    pub fn abs(self) -> f64 {
        self.re.hypot(self.im)
    }
    pub fn conj(self) -> C {
        C::new(self.re, -self.im)
    }
    pub fn sqrt(self) -> C {
        let r = self.abs().sqrt();
        let t = self.im.atan2(self.re) / 2.0;
        C::new(r * t.cos(), r * t.sin())
    }
    pub fn scale(self, f: f64) -> C {
        C::new(self.re * f, self.im * f)
    }
}

impl Add for C {
    type Output = C;
    fn add(self, o: C) -> C {
        C::new(self.re + o.re, self.im + o.im)
    }
}
impl Sub for C {
    type Output = C;
    fn sub(self, o: C) -> C {
        C::new(self.re - o.re, self.im - o.im)
    }
}
impl Mul for C {
    type Output = C;
    fn mul(self, o: C) -> C {
        C::new(self.re * o.re - self.im * o.im, self.re * o.im + self.im * o.re)
    }
}
impl Div for C {
    type Output = C;
    fn div(self, o: C) -> C {
        let d = o.re * o.re + o.im * o.im;
        C::new((self.re * o.re + self.im * o.im) / d, (self.im * o.re - self.re * o.im) / d)
    }
}
impl Neg for C {
    type Output = C;
    fn neg(self) -> C {
        C::new(-self.re, -self.im)
    }
}

/// De SI-waarden van een matrix, rij voor rij.
fn values(m: &Matrix) -> Vec<C> {
    m.elems().into_iter().map(|z| C::new(z.re, z.im)).collect()
}

/// Bouw een matrix uit SI-waarden en een eenheid per element.
fn build(rows: usize, cols: usize, vals: &[C], dim: impl Fn(usize, usize) -> Dim) -> Matrix {
    let elems = (0..rows * cols)
        .map(|k| {
            let (i, j) = (k / cols, k % cols);
            Complex::new(vals[k].re, vals[k].im, dim(i, j))
        })
        .collect();
    Matrix::from_complex(rows, cols, elems)
}

fn require_square(m: &Matrix, what: &str) -> Result<(), Error> {
    if m.is_square() && !m.is_empty() {
        Ok(())
    } else {
        Err(dom(format!("{what} vraagt een vierkante matrix (deze is {})", m.shape())))
    }
}

fn require_real(m: &Matrix, what: &str) -> Result<(), Error> {
    if m.is_complex() {
        Err(dom(format!("{what} werkt (nog) niet met een complexe matrix")))
    } else {
        Ok(())
    }
}

/// Alle elementen moeten dezelfde eenheid hebben; geeft die eenheid.
fn common_dim(m: &Matrix, what: &str) -> Result<Dim, Error> {
    m.common_dim().ok_or_else(|| {
        dom(format!("{what} werkt alleen als alle elementen dezelfde eenheid hebben"))
    })
}

fn singular() -> Error {
    dom("de matrix is singulier (determinant 0): er is geen inverse of eenduidige oplossing")
}

fn overflow() -> Error {
    UnitError::Overflow.into()
}

/// De eenheidsstructuur r_i·c_j van een matrix (zie de moduledocumentatie).
pub(crate) fn dim_structure(m: &Matrix) -> Result<(Vec<Dim>, Vec<Dim>), Error> {
    let r: Vec<Dim> = (0..m.rows).map(|i| m.get(i, 0).dim).collect();
    let d00 = m.get(0, 0).dim;
    let c: Vec<Dim> = (0..m.cols)
        .map(|j| m.get(0, j).dim.checked_div(d00).ok_or_else(overflow))
        .collect::<Result<_, _>>()?;
    for i in 0..m.rows {
        for j in 0..m.cols {
            if Some(m.get(i, j).dim) != r[i].checked_mul(c[j]) {
                return Err(dom(
                    "de eenheden in deze matrix passen niet bij elkaar \
                     (element (i, j) moet de eenheid 'rij i' × 'kolom j' hebben)",
                ));
            }
        }
    }
    Ok((r, c))
}

// ---------------------------------------------------------------------------------------------
// LU, determinant, inverse, stelsels

/// LU-decompositie met rijpivotering. Geeft (LU, permutatie, teken) of `None` als de matrix
/// singulier is. Rij i van P·A is rij `perm[i]` van A.
fn lu(a: &[C], n: usize) -> Option<(Vec<C>, Vec<usize>, f64)> {
    let mut a = a.to_vec();
    let scale = a.iter().fold(0.0f64, |s, v| s.max(v.abs()));
    if scale == 0.0 || !scale.is_finite() {
        return None;
    }
    let mut perm: Vec<usize> = (0..n).collect();
    let mut sign = 1.0;
    for k in 0..n {
        let p = (k..n).max_by(|&i, &j| a[i * n + k].abs().total_cmp(&a[j * n + k].abs()))?;
        if a[p * n + k].abs() <= scale * 1e-14 * n as f64 {
            return None;
        }
        if p != k {
            for j in 0..n {
                a.swap(k * n + j, p * n + j);
            }
            perm.swap(k, p);
            sign = -sign;
        }
        for i in k + 1..n {
            let f = a[i * n + k] / a[k * n + k];
            a[i * n + k] = f;
            for j in k + 1..n {
                let akj = a[k * n + j];
                a[i * n + j] = a[i * n + j] - f * akj;
            }
        }
    }
    Some((a, perm, sign))
}

fn lu_solve(a: &[C], perm: &[usize], b: &[C]) -> Vec<C> {
    let n = perm.len();
    let mut x: Vec<C> = perm.iter().map(|&p| b[p]).collect();
    for i in 0..n {
        for j in 0..i {
            x[i] = x[i] - a[i * n + j] * x[j];
        }
    }
    for i in (0..n).rev() {
        for j in i + 1..n {
            x[i] = x[i] - a[i * n + j] * x[j];
        }
        x[i] = x[i] / a[i * n + i];
    }
    x
}

pub fn det(m: &Matrix) -> Result<Value, Error> {
    require_square(m, "de determinant")?;
    let (r, c) = dim_structure(m)?;
    let mut dim = Dim::NONE;
    for d in r.iter().chain(&c) {
        dim = dim.checked_mul(*d).ok_or_else(overflow)?;
    }
    let n = m.rows;
    let v = match lu(&values(m), n) {
        None => C::ZERO,
        Some((a, _, sign)) => (0..n).fold(C::new(sign, 0.0), |acc, i| acc * a[i * n + i]),
    };
    Ok(Value::from(Complex::new(v.re, v.im, dim)))
}

pub fn inverse(m: &Matrix) -> Result<Matrix, Error> {
    require_square(m, "de inverse")?;
    let (r, c) = dim_structure(m)?;
    let n = m.rows;
    let (a, perm, _) = lu(&values(m), n).ok_or_else(singular)?;
    let mut out = vec![C::ZERO; n * n];
    for j in 0..n {
        let mut e = vec![C::ZERO; n];
        e[j] = C::ONE;
        for (i, v) in lu_solve(&a, &perm, &e).into_iter().enumerate() {
            out[i * n + j] = v;
        }
    }
    let mut dims = vec![Dim::NONE; n * n];
    for i in 0..n {
        for j in 0..n {
            dims[i * n + j] = c[i]
                .checked_mul(r[j])
                .and_then(|d| Dim::NONE.checked_div(d))
                .ok_or_else(overflow)?;
        }
    }
    Ok(build(n, n, &out, |i, j| dims[i * n + j]))
}

/// Los A·x = b op. Geeft x als kolomvector.
pub fn lsolve(a: &Matrix, b: &Matrix) -> Result<Matrix, Error> {
    require_square(a, "lsolve")?;
    if !b.is_vector() || b.len() != a.rows {
        return Err(dom(format!(
            "lsolve: b moet een vector met {} elementen zijn (nu {})",
            a.rows,
            b.shape()
        )));
    }
    let (r, c) = dim_structure(a)?;
    // b_k moet eenheid s·r_k hebben; dan heeft x_i eenheid s/c_i
    let s = b.data[0].dim.checked_div(r[0]).ok_or_else(overflow)?;
    for k in 0..a.rows {
        if b.data[k].dim.checked_div(r[k]) != Some(s) {
            return Err(dom("lsolve: de eenheden van b passen niet bij de matrix"));
        }
    }
    let (lu_a, perm, _) = lu(&values(a), a.rows).ok_or_else(singular)?;
    let x = lu_solve(&lu_a, &perm, &values(b));
    let dims = c
        .iter()
        .map(|ci| s.checked_div(*ci).ok_or_else(overflow))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(build(a.rows, 1, &x, |i, _| dims[i]))
}

/// `lu(M)` zoals in Mathcad: augment(P, L, U) met P·M = L·U.
pub fn lu_decomp(m: &Matrix) -> Result<Matrix, Error> {
    require_square(m, "lu")?;
    let d = common_dim(m, "lu")?;
    let n = m.rows;
    let (a, perm, _) = lu(&values(m), n).ok_or_else(singular)?;
    let mut out = vec![C::ZERO; n * 3 * n];
    let mut dims = vec![Dim::NONE; n * 3 * n];
    let w = 3 * n;
    for i in 0..n {
        out[i * w + perm[i]] = C::ONE;
        for j in 0..n {
            if j < i {
                out[i * w + n + j] = a[i * n + j];
            } else {
                if j == i {
                    out[i * w + n + j] = C::ONE;
                }
                out[i * w + 2 * n + j] = a[i * n + j];
            }
            dims[i * w + 2 * n + j] = d;
        }
    }
    Ok(build(n, w, &out, |i, j| dims[i * w + j]))
}

// ---------------------------------------------------------------------------------------------
// Eigenwaarden en eigenvectoren

/// De eenheid van de eigenwaarden: r_i·c_i moet voor alle i gelijk zijn.
fn eig_dim(m: &Matrix, what: &str) -> Result<(Dim, Vec<Dim>), Error> {
    require_square(m, what)?;
    let (r, c) = dim_structure(m)?;
    let d = r[0].checked_mul(c[0]).ok_or_else(overflow)?;
    for i in 0..m.rows {
        if r[i].checked_mul(c[i]) != Some(d) {
            return Err(dom(format!(
                "{what}: de diagonaal moet overal dezelfde eenheid hebben"
            )));
        }
    }
    Ok((d, c))
}

fn is_real_symmetric(m: &Matrix) -> bool {
    if m.is_complex() || !m.is_square() {
        return false;
    }
    let n = m.rows;
    let scale = m.data.iter().fold(0.0f64, |s, q| s.max(q.value.abs()));
    (0..n).all(|i| {
        (0..i).all(|j| {
            let (a, b) = (m.get(i, j), m.get(j, i));
            a.dim == b.dim && (a.value - b.value).abs() <= 1e-12 * scale
        })
    })
}

/// Jacobi-methode voor een reële symmetrische matrix: (eigenwaarden, eigenvectoren als kolommen).
fn jacobi(mut a: Vec<f64>, n: usize) -> (Vec<f64>, Vec<f64>) {
    let mut v = vec![0.0; n * n];
    for i in 0..n {
        v[i * n + i] = 1.0;
    }
    let total: f64 = a.iter().map(|x| x * x).sum();
    for _ in 0..100 {
        let off: f64 = (0..n)
            .flat_map(|i| (0..n).filter(move |&j| j != i).map(move |j| (i, j)))
            .map(|(i, j)| a[i * n + j] * a[i * n + j])
            .sum();
        if off <= 1e-30 * total || total == 0.0 {
            break;
        }
        for p in 0..n {
            for q in p + 1..n {
                let apq = a[p * n + q];
                if apq.abs() < 1e-300 {
                    continue;
                }
                let theta = (a[q * n + q] - a[p * n + p]) / (2.0 * apq);
                let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
                let t = if theta == 0.0 { 1.0 } else { t };
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                for k in 0..n {
                    let (akp, akq) = (a[k * n + p], a[k * n + q]);
                    a[k * n + p] = c * akp - s * akq;
                    a[k * n + q] = s * akp + c * akq;
                }
                for k in 0..n {
                    let (apk, aqk) = (a[p * n + k], a[q * n + k]);
                    a[p * n + k] = c * apk - s * aqk;
                    a[q * n + k] = s * apk + c * aqk;
                }
                for k in 0..n {
                    let (vkp, vkq) = (v[k * n + p], v[k * n + q]);
                    v[k * n + p] = c * vkp - s * vkq;
                    v[k * n + q] = s * vkp + c * vkq;
                }
            }
        }
    }
    ((0..n).map(|i| a[i * n + i]).collect(), v)
}

/// Breng een matrix naar Hessenberg-vorm (eliminatie met pivotering, zoals `elmhes`).
fn hessenberg(a: &mut [C], n: usize) {
    for m in 1..n.saturating_sub(1) {
        let mut x = C::ZERO;
        let mut piv = m;
        for j in m..n {
            if a[j * n + m - 1].abs() > x.abs() {
                x = a[j * n + m - 1];
                piv = j;
            }
        }
        if piv != m {
            for j in m - 1..n {
                a.swap(piv * n + j, m * n + j);
            }
            for j in 0..n {
                a.swap(j * n + piv, j * n + m);
            }
        }
        if x.abs() != 0.0 {
            for i in m + 1..n {
                let mut y = a[i * n + m - 1];
                if y.abs() != 0.0 {
                    y = y / x;
                    a[i * n + m - 1] = y;
                    for j in m..n {
                        let amj = a[m * n + j];
                        a[i * n + j] = a[i * n + j] - y * amj;
                    }
                    for j in 0..n {
                        let aji = a[j * n + i];
                        a[j * n + m] = a[j * n + m] + y * aji;
                    }
                }
            }
        }
    }
    // de vermenigvuldigers onder de subdiagonaal zijn geen deel van H
    for i in 2..n {
        for j in 0..i - 1 {
            a[i * n + j] = C::ZERO;
        }
    }
}

/// Eigenwaarden van een (complexe) matrix: Hessenberg-vorm en daarna QR met Wilkinson-shifts.
fn qr_eigenvalues(mut h: Vec<C>, n: usize) -> Result<Vec<C>, Error> {
    hessenberg(&mut h, n);
    let mut eig = vec![C::ZERO; n];
    let mut hi = n; // actief blok: rijen/kolommen lo..hi
    let mut iter = 0;
    let norm = h.iter().fold(0.0f64, |s, v| s.max(v.abs())).max(f64::MIN_POSITIVE);
    while hi > 0 {
        if hi == 1 {
            eig[0] = h[0];
            break;
        }
        let m = hi - 1;
        // zoek de kleinste lo zodat het blok lo..=m geen verwaarloosbare subdiagonaal heeft
        let mut lo = m;
        while lo > 0 {
            let s = h[(lo - 1) * n + lo - 1].abs() + h[lo * n + lo].abs();
            if h[lo * n + lo - 1].abs() <= f64::EPSILON * if s == 0.0 { norm } else { s } {
                h[lo * n + lo - 1] = C::ZERO;
                break;
            }
            lo -= 1;
        }
        if lo == m {
            eig[m] = h[m * n + m];
            hi -= 1;
            iter = 0;
            continue;
        }
        iter += 1;
        if iter > 60 * n {
            return Err(dom("de eigenwaarden convergeren niet"));
        }
        // Wilkinson-shift: de eigenwaarde van het onderste 2×2-blok die het dichtst bij h[m][m] ligt
        let (a, b, c, d) = (h[(m - 1) * n + m - 1], h[(m - 1) * n + m], h[m * n + m - 1], h[m * n + m]);
        let tr = a + d;
        let disc = ((a - d) * (a - d) + C::new(4.0, 0.0) * b * c).sqrt();
        let l1 = (tr + disc).scale(0.5);
        let l2 = (tr - disc).scale(0.5);
        let mut mu = if (l1 - d).abs() < (l2 - d).abs() { l1 } else { l2 };
        if iter % 11 == 10 {
            // uitzonderlijke shift om cycli te doorbreken
            mu = mu + C::new(h[m * n + m - 1].abs(), 0.0);
        }
        for k in lo..=m {
            h[k * n + k] = h[k * n + k] - mu;
        }
        let mut rots = Vec::with_capacity(m - lo);
        for k in lo..m {
            let (x, y) = (h[k * n + k], h[(k + 1) * n + k]);
            let r = x.abs().hypot(y.abs());
            let (c, s) = if r == 0.0 { (C::ONE, C::ZERO) } else { (x.scale(1.0 / r), y.scale(1.0 / r)) };
            for j in k..=m {
                let (p, q) = (h[k * n + j], h[(k + 1) * n + j]);
                h[k * n + j] = c.conj() * p + s.conj() * q;
                h[(k + 1) * n + j] = -s * p + c * q;
            }
            rots.push((c, s));
        }
        for (idx, k) in (lo..m).enumerate() {
            let (c, s) = rots[idx];
            for i in lo..=(k + 1).min(m) {
                let (p, q) = (h[i * n + k], h[i * n + k + 1]);
                h[i * n + k] = p * c + q * s;
                h[i * n + k + 1] = -p * s.conj() + q * c.conj();
            }
        }
        for k in lo..=m {
            h[k * n + k] = h[k * n + k] + mu;
        }
    }
    Ok(eig)
}

fn sort_eig(e: &mut [C]) {
    e.sort_by(|x, y| x.re.total_cmp(&y.re).then(x.im.total_cmp(&y.im)));
}

/// Maak complexe eigenwaarden met een verwaarloosbaar imaginair deel reëel.
fn clean(e: &mut [C], scale: f64) {
    for z in e {
        if z.im.abs() <= 1e-12 * scale.max(z.re.abs()) {
            z.im = 0.0;
        }
    }
}

/// Eigenwaarden, oplopend gesorteerd (op reëel deel, dan imaginair deel), en de eenheid ervan.
fn eigen(m: &Matrix) -> Result<(Vec<C>, Dim), Error> {
    let (d, _) = eig_dim(m, "eigenwaarden")?;
    let n = m.rows;
    let mut e = if is_real_symmetric(m) {
        let (vals, _) = jacobi(m.data.iter().map(|q| q.value).collect(), n);
        vals.into_iter().map(|v| C::new(v, 0.0)).collect()
    } else {
        qr_eigenvalues(values(m), n)?
    };
    let scale = values(m).iter().fold(0.0f64, |s, v| s.max(v.abs()));
    clean(&mut e, scale);
    sort_eig(&mut e);
    Ok((e, d))
}

pub fn eigenvals(m: &Matrix) -> Result<Matrix, Error> {
    let (e, d) = eigen(m)?;
    Ok(build(e.len(), 1, &e, |_, _| d))
}

/// Eigenvector bij eigenwaarde `lambda` (SI) met inverse iteratie, genormeerd op lengte 1.
fn inverse_iteration(a: &[C], n: usize, lambda: C) -> Result<Vec<C>, Error> {
    let scale = a.iter().fold(0.0f64, |s, v| s.max(v.abs())).max(f64::MIN_POSITIVE);
    let shift = lambda + C::new(scale * 1e-10, scale * 1e-10);
    let mut b = a.to_vec();
    for i in 0..n {
        b[i * n + i] = b[i * n + i] - shift;
    }
    let (f, perm, _) = lu(&b, n).ok_or_else(|| dom("eigenvector: inverse iteratie mislukt"))?;
    let mut x = vec![C::ONE; n];
    for _ in 0..4 {
        x = lu_solve(&f, &perm, &x);
        let len = x.iter().map(|z| z.abs() * z.abs()).sum::<f64>().sqrt();
        if !len.is_finite() || len == 0.0 {
            return Err(dom("eigenvector: inverse iteratie mislukt"));
        }
        x = x.into_iter().map(|z| z.scale(1.0 / len)).collect();
    }
    // vaste fase: de grootste component reëel en positief
    let big = x.iter().copied().max_by(|p, q| p.abs().total_cmp(&q.abs())).unwrap_or(C::ONE);
    let phase = big.conj().scale(1.0 / big.abs());
    Ok(x.into_iter().map(|z| z * phase).collect())
}

/// Eigenvectoren (genormeerd op lengte 1) als kolommen, in de volgorde van `eigenvals`.
pub fn eigenvecs(m: &Matrix) -> Result<Matrix, Error> {
    let (_, c) = eig_dim(m, "eigenvecs")?;
    if c.iter().any(|d| *d != c[0]) {
        return Err(dom("eigenvecs: alle kolommen moeten dezelfde eenheid hebben"));
    }
    let n = m.rows;
    let mut cols: Vec<Vec<C>> = Vec::with_capacity(n);
    if is_real_symmetric(m) {
        let (vals, v) = jacobi(m.data.iter().map(|q| q.value).collect(), n);
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&p, &q| vals[p].total_cmp(&vals[q]));
        for &k in &order {
            let mut col: Vec<C> = (0..n).map(|i| C::new(v[i * n + k], 0.0)).collect();
            let big = col.iter().copied().max_by(|p, q| p.abs().total_cmp(&q.abs())).unwrap_or(C::ONE);
            if big.re < 0.0 {
                col = col.into_iter().map(|z| -z).collect();
            }
            cols.push(col);
        }
    } else {
        let (e, _) = eigen(m)?;
        let a = values(m);
        for lambda in e {
            cols.push(inverse_iteration(&a, n, lambda)?);
        }
    }
    let mut vals = vec![C::ZERO; n * n];
    for (j, col) in cols.iter().enumerate() {
        for i in 0..n {
            vals[i * n + j] = col[i];
        }
    }
    Ok(build(n, n, &vals, |_, _| Dim::NONE))
}

/// Eigenvector bij een gegeven eigenwaarde (zoals Mathcads `eigenvec(M, z)`).
pub fn eigenvec(m: &Matrix, z: Complex) -> Result<Matrix, Error> {
    let (d, c) = eig_dim(m, "eigenvec")?;
    if c.iter().any(|x| *x != c[0]) {
        return Err(dom("eigenvec: alle kolommen moeten dezelfde eenheid hebben"));
    }
    if z.dim != d {
        return Err(UnitError::Mismatch {
            op: "eigenvec (eigenwaarde en matrix)",
            left: z.dim,
            right: d,
        }
        .into());
    }
    let x = inverse_iteration(&values(m), m.rows, C::new(z.re, z.im))?;
    Ok(build(m.rows, 1, &x, |_, _| Dim::NONE))
}

// ---------------------------------------------------------------------------------------------
// Cholesky, SVD, QR, normen

pub fn cholesky(m: &Matrix) -> Result<Matrix, Error> {
    require_square(m, "cholesky")?;
    require_real(m, "cholesky")?;
    let d = common_dim(m, "cholesky")?;
    let ld = d.root(2).ok_or(UnitError::NoRoot { dim: d, n: 2 })?;
    if !is_real_symmetric(m) {
        return Err(dom("cholesky vraagt een symmetrische matrix"));
    }
    let n = m.rows;
    let a: Vec<f64> = m.data.iter().map(|q| q.value).collect();
    let mut l = vec![0.0; n * n];
    for j in 0..n {
        let s: f64 = (0..j).map(|k| l[j * n + k] * l[j * n + k]).sum();
        let diag = a[j * n + j] - s;
        if diag <= 0.0 {
            return Err(dom("cholesky vraagt een positief-definiete matrix"));
        }
        l[j * n + j] = diag.sqrt();
        for i in j + 1..n {
            let s: f64 = (0..j).map(|k| l[i * n + k] * l[j * n + k]).sum();
            l[i * n + j] = (a[i * n + j] - s) / l[j * n + j];
        }
    }
    let vals: Vec<C> = l.into_iter().map(|v| C::new(v, 0.0)).collect();
    Ok(build(n, n, &vals, |_, _| ld))
}

/// SVD met eenzijdige Jacobi: A (m×n, m ≥ n) = U·diag(s)·Vᵀ. s aflopend.
fn svd_raw(a: &[f64], m: usize, n: usize) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let mut u = a.to_vec();
    let mut v = vec![0.0; n * n];
    for i in 0..n {
        v[i * n + i] = 1.0;
    }
    for _ in 0..60 {
        let mut rotated = false;
        for p in 0..n {
            for q in p + 1..n {
                let (mut alpha, mut beta, mut gamma) = (0.0, 0.0, 0.0);
                for i in 0..m {
                    let (up, uq) = (u[i * n + p], u[i * n + q]);
                    alpha += up * up;
                    beta += uq * uq;
                    gamma += up * uq;
                }
                if gamma.abs() <= 1e-15 * (alpha * beta).sqrt() || gamma == 0.0 {
                    continue;
                }
                rotated = true;
                let zeta = (beta - alpha) / (2.0 * gamma);
                let t = zeta.signum() / (zeta.abs() + (1.0 + zeta * zeta).sqrt());
                let t = if zeta == 0.0 { 1.0 } else { t };
                let c = 1.0 / (1.0 + t * t).sqrt();
                let s = c * t;
                for i in 0..m {
                    let (up, uq) = (u[i * n + p], u[i * n + q]);
                    u[i * n + p] = c * up - s * uq;
                    u[i * n + q] = s * up + c * uq;
                }
                for i in 0..n {
                    let (vp, vq) = (v[i * n + p], v[i * n + q]);
                    v[i * n + p] = c * vp - s * vq;
                    v[i * n + q] = s * vp + c * vq;
                }
            }
        }
        if !rotated {
            break;
        }
    }
    let mut s: Vec<f64> = (0..n).map(|j| (0..m).map(|i| u[i * n + j].powi(2)).sum::<f64>().sqrt()).collect();
    for j in 0..n {
        if s[j] > 0.0 {
            for i in 0..m {
                u[i * n + j] /= s[j];
            }
        }
    }
    // aflopend sorteren
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&p, &q| s[q].total_cmp(&s[p]));
    let u2 = (0..m * n).map(|k| u[(k / n) * n + order[k % n]]).collect();
    let v2 = (0..n * n).map(|k| v[(k / n) * n + order[k % n]]).collect();
    s = order.iter().map(|&k| s[k]).collect();
    (u2, s, v2)
}

fn real_values(m: &Matrix) -> Vec<f64> {
    m.data.iter().map(|q| q.value).collect()
}

/// Singuliere waarden (aflopend) en hun eenheid.
fn singular_values(m: &Matrix, what: &str) -> Result<(Vec<f64>, Dim), Error> {
    require_real(m, what)?;
    let d = common_dim(m, what)?;
    let (s, rows, cols) = if m.rows >= m.cols {
        (real_values(m), m.rows, m.cols)
    } else {
        (real_values(&m.transpose()), m.cols, m.rows)
    };
    let (_, sv, _) = svd_raw(&s, rows, cols);
    Ok((sv, d))
}

pub fn svds(m: &Matrix) -> Result<Matrix, Error> {
    let (s, d) = singular_values(m, "svds")?;
    let vals: Vec<C> = s.iter().map(|&v| C::new(v, 0.0)).collect();
    Ok(build(s.len(), 1, &vals, |_, _| d))
}

/// `svd(A)` zoals in Mathcad: U (m×n) met daaronder V (n×n), zodat A = U·diag(svds(A))·Vᵀ.
pub fn svd(m: &Matrix) -> Result<Matrix, Error> {
    require_real(m, "svd")?;
    common_dim(m, "svd")?;
    if m.rows < m.cols {
        return Err(dom("svd vraagt minstens evenveel rijen als kolommen; gebruik svd(transpose(A))"));
    }
    let (rows, n) = (m.rows, m.cols);
    let (u, _, v) = svd_raw(&real_values(m), rows, n);
    let mut vals: Vec<C> = u.into_iter().map(|x| C::new(x, 0.0)).collect();
    vals.extend(v.into_iter().map(|x| C::new(x, 0.0)));
    Ok(build(rows + n, n, &vals, |_, _| Dim::NONE))
}

pub fn rank(m: &Matrix) -> Result<usize, Error> {
    let (s, _) = singular_values(m, "rank")?;
    let tol = s.first().copied().unwrap_or(0.0) * m.rows.max(m.cols) as f64 * f64::EPSILON * 10.0;
    Ok(s.iter().filter(|&&x| x > tol).count())
}

pub fn cond2(m: &Matrix) -> Result<f64, Error> {
    let (s, _) = singular_values(m, "cond2")?;
    let min = *s.last().unwrap_or(&0.0);
    if min == 0.0 {
        return Ok(f64::INFINITY);
    }
    Ok(s[0] / min)
}

pub fn norm2(m: &Matrix) -> Result<Quantity, Error> {
    let (s, d) = singular_values(m, "norm2")?;
    Ok(Quantity::new(s[0], d))
}

/// `norm1` (grootste kolomsom) of `normi` (grootste rijsom) van |a_ij|.
pub fn norm_1_inf(m: &Matrix, rows: bool) -> Result<Quantity, Error> {
    let d = common_dim(m, if rows { "normi" } else { "norm1" })?;
    let (outer, inner) = if rows { (m.rows, m.cols) } else { (m.cols, m.rows) };
    let best = (0..outer)
        .map(|o| {
            (0..inner)
                .map(|k| {
                    let (i, j) = if rows { (o, k) } else { (k, o) };
                    m.cget_rc(i, j).abs()
                })
                .sum::<f64>()
        })
        .fold(0.0f64, f64::max);
    Ok(Quantity::new(best, d))
}

/// `qr(A)` zoals in Mathcad: augment(Q, R) met A = Q·R (Householder).
pub fn qr(m: &Matrix) -> Result<Matrix, Error> {
    require_real(m, "qr")?;
    let d = common_dim(m, "qr")?;
    let (rows, cols) = (m.rows, m.cols);
    let mut r = real_values(m);
    let mut q = vec![0.0; rows * rows];
    for i in 0..rows {
        q[i * rows + i] = 1.0;
    }
    for k in 0..cols.min(rows.saturating_sub(1)) {
        let norm = (k..rows).map(|i| r[i * cols + k].powi(2)).sum::<f64>().sqrt();
        if norm == 0.0 {
            continue;
        }
        let alpha = if r[k * cols + k] > 0.0 { -norm } else { norm };
        let mut v: Vec<f64> = (k..rows).map(|i| r[i * cols + k]).collect();
        v[0] -= alpha;
        let vn = v.iter().map(|x| x * x).sum::<f64>().sqrt();
        if vn == 0.0 {
            continue;
        }
        v.iter_mut().for_each(|x| *x /= vn);
        for j in 0..cols {
            let dot: f64 = (k..rows).map(|i| v[i - k] * r[i * cols + j]).sum();
            for i in k..rows {
                r[i * cols + j] -= 2.0 * v[i - k] * dot;
            }
        }
        for i in 0..rows {
            let dot: f64 = (k..rows).map(|l| q[i * rows + l] * v[l - k]).sum();
            for l in k..rows {
                q[i * rows + l] -= 2.0 * dot * v[l - k];
            }
        }
    }
    let w = rows + cols;
    let mut vals = vec![C::ZERO; rows * w];
    for i in 0..rows {
        for j in 0..rows {
            vals[i * w + j] = C::new(q[i * rows + j], 0.0);
        }
        for j in 0..cols {
            let v = if i > j { 0.0 } else { r[i * cols + j] };
            vals[i * w + rows + j] = C::new(v, 0.0);
        }
    }
    Ok(build(rows, w, &vals, |_, j| if j < rows { Dim::NONE } else { d }))
}

// ---------------------------------------------------------------------------------------------
// Polynomen

/// Nulpunten van c0 + c1·x + c2·x² + ... (coëfficiënten oplopend, zoals in Mathcad), via de
/// eigenwaarden van de companion-matrix.
pub fn polyroots(v: &Matrix) -> Result<Matrix, Error> {
    if !v.is_vector() {
        return Err(dom("polyroots verwacht een vector met coëfficiënten (oplopend: c0, c1, c2, ...)"));
    }
    let coef = v.elems();
    let n = coef
        .iter()
        .rposition(|z| z.re != 0.0 || z.im != 0.0)
        .ok_or_else(|| dom("polyroots: alle coëfficiënten zijn 0"))?;
    if n == 0 {
        return Err(dom("polyroots: een constant polynoom heeft geen nulpunten"));
    }
    // eenheid van x: dim(c_k) = dim(c_n) · x^(n-k)
    let cn = coef[n].dim;
    let xdim = (0..n)
        .find(|&k| coef[k].re != 0.0 || coef[k].im != 0.0)
        .map(|k| {
            coef[k]
                .dim
                .checked_div(cn)
                .and_then(|d| d.root((n - k) as u32))
                .ok_or_else(|| dom("polyroots: de eenheden van de coëfficiënten passen niet bij elkaar"))
        })
        .transpose()?
        .unwrap_or(Dim::NONE);
    for (k, z) in coef.iter().enumerate().take(n) {
        let bare_zero = z.re == 0.0 && z.im == 0.0 && z.dim.is_none();
        if !bare_zero && xdim.checked_powi((n - k) as i32).and_then(|p| cn.checked_mul(p)) != Some(z.dim) {
            return Err(dom("polyroots: de eenheden van de coëfficiënten passen niet bij elkaar"));
        }
    }
    let lead = C::new(coef[n].re, coef[n].im);
    let a: Vec<C> = coef[..n].iter().map(|z| C::new(z.re, z.im) / lead).collect();
    let mut comp = vec![C::ZERO; n * n];
    for j in 0..n {
        comp[j] = -a[n - 1 - j];
    }
    for i in 1..n {
        comp[i * n + i - 1] = C::ONE;
    }
    let mut roots = qr_eigenvalues(comp, n)?;
    let scale = a.iter().fold(1.0f64, |s, z| s.max(z.abs()));
    clean(&mut roots, scale);
    sort_eig(&mut roots);
    Ok(build(n, 1, &roots, |_, _| xdim))
}

// ---------------------------------------------------------------------------------------------
// Fouriertransformatie

fn fft_pow2(a: &mut [C], inverse: bool) {
    let n = a.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            a.swap(i, j);
        }
    }
    let sign = if inverse { 1.0 } else { -1.0 };
    let mut len = 2;
    while len <= n {
        let ang = sign * 2.0 * std::f64::consts::PI / len as f64;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let w = C::new((ang * k as f64).cos(), (ang * k as f64).sin());
                let u = a[start + k];
                let v = a[start + k + len / 2] * w;
                a[start + k] = u + v;
                a[start + k + len / 2] = u - v;
            }
        }
        len <<= 1;
    }
}

/// DFT van willekeurige lengte: radix-2 als n een macht van 2 is, anders Bluestein.
fn dft(x: &[C], inverse: bool) -> Vec<C> {
    let n = x.len();
    if n.is_power_of_two() {
        let mut a = x.to_vec();
        fft_pow2(&mut a, inverse);
        return a;
    }
    // Bluestein: jk = (j² + k² − (k−j)²)/2, dus de DFT is een convolutie met een 'chirp'
    let sign = if inverse { 1.0 } else { -1.0 };
    let w: Vec<C> = (0..n)
        .map(|t| {
            let ang = sign * std::f64::consts::PI * ((t * t) % (2 * n)) as f64 / n as f64;
            C::new(ang.cos(), ang.sin())
        })
        .collect();
    let m = (2 * n - 1).next_power_of_two();
    let mut a = vec![C::ZERO; m];
    let mut b = vec![C::ZERO; m];
    for j in 0..n {
        a[j] = x[j] * w[j];
    }
    b[0] = w[0].conj();
    for t in 1..n {
        b[t] = w[t].conj();
        b[m - t] = w[t].conj();
    }
    fft_pow2(&mut a, false);
    fft_pow2(&mut b, false);
    for k in 0..m {
        a[k] = a[k] * b[k];
    }
    fft_pow2(&mut a, true);
    (0..n).map(|k| w[k] * a[k].scale(1.0 / m as f64)).collect()
}

/// `fft(v)`: X_k = Σ v_j·e^(−2πi·jk/n) (zonder normering); `ifft` is de inverse (met 1/n).
pub fn fft(m: &Matrix, inverse: bool) -> Result<Matrix, Error> {
    let what = if inverse { "ifft" } else { "fft" };
    if !m.is_vector() {
        return Err(dom(format!("{what} verwacht een vector")));
    }
    let d = common_dim(m, what)?;
    let mut out = dft(&values(m), inverse);
    if inverse {
        let f = 1.0 / out.len() as f64;
        out.iter_mut().for_each(|z| *z = z.scale(f));
    }
    // afrondingsruis (bijv. 1e-16i bij een reëel resultaat) weghalen
    let scale = out.iter().fold(0.0f64, |s, z| s.max(z.abs()));
    for z in &mut out {
        if z.re.abs() < 1e-14 * scale {
            z.re = 0.0;
        }
        if z.im.abs() < 1e-14 * scale {
            z.im = 0.0;
        }
    }
    Ok(build(m.rows, m.cols, &out, |_, _| d))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fourier() {
        // sin(2π·3j/16): X_3 = -8i, X_13 = 8i, de rest 0
        let v = Matrix::column((0..16).map(|j| Quantity::scalar((2.0 * std::f64::consts::PI * 3.0 * j as f64 / 16.0).sin())).collect());
        let x = fft(&v, false).unwrap();
        assert!((x.cget(3).im + 8.0).abs() < 1e-12 && (x.cget(13).im - 8.0).abs() < 1e-12);
        assert!((0..16).filter(|&k| k != 3 && k != 13).all(|k| x.cget(k).abs() < 1e-12));
        // Bluestein (lengte 6): terug naar het origineel
        let v = m(6, 1, &[1.0, 2.0, 0.5, -1.0, 3.0, 4.0]);
        let back = fft(&fft(&v, false).unwrap(), true).unwrap();
        for k in 0..6 {
            assert!((back.get(k, 0).value - v.get(k, 0).value).abs() < 1e-12);
        }
        assert!(!back.is_complex());
        // DFT van lengte 3 met de hand: [1, 2, 3] → X1 = -1.5 + 0.866i
        let x = fft(&m(3, 1, &[1.0, 2.0, 3.0]), false).unwrap();
        assert!((x.cget(1).re + 1.5).abs() < 1e-12 && (x.cget(1).im - 3f64.sqrt() / 2.0).abs() < 1e-12);
    }

    fn m(rows: usize, cols: usize, v: &[f64]) -> Matrix {
        Matrix::new(rows, cols, v.iter().map(|&x| Quantity::scalar(x)).collect())
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9 * (1.0 + b.abs())
    }

    #[test]
    fn determinant_en_inverse() {
        let a = m(3, 3, &[2.0, 1.0, 1.0, 1.0, 3.0, 2.0, 1.0, 0.0, 0.0]);
        match det(&a).unwrap() {
            Value::Num(q) => assert!(close(q.value, -1.0)),
            v => panic!("{v:?}"),
        }
        let inv = inverse(&a).unwrap();
        let id = crate::value::matmul(&a, &inv).unwrap();
        for i in 0..3 {
            for j in 0..3 {
                assert!(close(id.get(i, j).value, if i == j { 1.0 } else { 0.0 }));
            }
        }
        assert!(inverse(&m(2, 2, &[1.0, 2.0, 2.0, 4.0])).is_err());
    }

    #[test]
    fn eigenwaarden_symmetrisch_en_algemeen() {
        let e = eigenvals(&m(2, 2, &[2.0, 1.0, 1.0, 2.0])).unwrap();
        assert!(close(e.get(0, 0).value, 1.0) && close(e.get(1, 0).value, 3.0));
        // rotatiematrix: eigenwaarden ±i
        let e = eigenvals(&m(2, 2, &[0.0, -1.0, 1.0, 0.0])).unwrap();
        assert!(e.is_complex());
        let (z0, z1) = (e.cget(0), e.cget(1));
        assert!(close(z0.re, 0.0) && close(z0.im, -1.0) && close(z1.im, 1.0), "{z0:?} {z1:?}");
        // niet-symmetrisch met reële eigenwaarden 2, 3, 5 (bovendriehoeks)
        let e = eigenvals(&m(3, 3, &[5.0, 4.0, 2.0, 0.0, 3.0, 1.0, 0.0, 0.0, 2.0])).unwrap();
        let got: Vec<f64> = (0..3).map(|k| e.get(k, 0).value).collect();
        assert!(close(got[0], 2.0) && close(got[1], 3.0) && close(got[2], 5.0), "{got:?}");
        assert!(!e.is_complex());
    }

    #[test]
    fn eigenvectoren_voldoen_aan_a_v_is_lambda_v() {
        for a in [
            m(3, 3, &[4.0, 1.0, 0.0, 1.0, 3.0, 1.0, 0.0, 1.0, 2.0]),
            m(3, 3, &[1.0, 2.0, 3.0, 0.0, 4.0, 5.0, 1.0, 0.0, 6.0]),
        ] {
            let e = eigenvals(&a).unwrap();
            let v = eigenvecs(&a).unwrap();
            for k in 0..3 {
                let lambda = e.cget(k);
                for i in 0..3 {
                    let mut av = C::ZERO;
                    for j in 0..3 {
                        let x = v.cget_rc(j, k);
                        av = av + C::new(a.get(i, j).value, 0.0) * C::new(x.re, x.im);
                    }
                    let x = v.cget_rc(i, k);
                    let lv = C::new(lambda.re, lambda.im) * C::new(x.re, x.im);
                    assert!((av - lv).abs() < 1e-8, "k={k} i={i} {av:?} {lv:?}");
                }
            }
        }
    }

    #[test]
    fn svd_qr_cholesky_en_lu_reconstrueren_de_matrix() {
        let a = m(3, 2, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let s = svds(&a).unwrap();
        // σ² zijn de eigenwaarden van AᵀA = [35, 44; 44, 56]: 90.7355 en 0.264505
        assert!(close(s.get(0, 0).value.powi(2), 45.5 + (45.5f64.powi(2) - 24.0).sqrt()));
        let usv = svd(&a).unwrap();
        for i in 0..3 {
            for j in 0..2 {
                let rec: f64 = (0..2).map(|k| usv.get(i, k).value * s.get(k, 0).value * usv.get(3 + j, k).value).sum();
                assert!(close(rec, a.get(i, j).value));
            }
        }
        let qr_m = qr(&a).unwrap();
        for i in 0..3 {
            for j in 0..2 {
                let rec: f64 = (0..3).map(|k| qr_m.get(i, k).value * qr_m.get(k, 3 + j).value).sum();
                assert!(close(rec, a.get(i, j).value));
            }
        }
        let spd = m(2, 2, &[4.0, 2.0, 2.0, 3.0]);
        let l = cholesky(&spd).unwrap();
        assert!(close(l.get(0, 0).value, 2.0) && close(l.get(1, 0).value, 1.0) && close(l.get(1, 1).value, 2f64.sqrt()));
        assert_eq!(rank(&m(3, 3, &[1.0, 2.0, 3.0, 2.0, 4.0, 6.0, 1.0, 0.0, 1.0])).unwrap(), 2);
        let plu = lu_decomp(&m(2, 2, &[1.0, 2.0, 3.0, 4.0])).unwrap();
        // P wisselt de rijen (pivot 3), L = [1, 0; 1/3, 1], U = [3, 4; 0, 2/3]
        assert!(close(plu.get(0, 1).value, 1.0) && close(plu.get(1, 2).value, 1.0 / 3.0));
        assert!(close(plu.get(1, 5).value, 2.0 / 3.0));
    }

    #[test]
    fn polyroots_reeel_en_complex() {
        // (x-1)(x-2)(x-3) = -6 + 11x - 6x² + x³
        let r = polyroots(&m(4, 1, &[-6.0, 11.0, -6.0, 1.0])).unwrap();
        let got: Vec<f64> = (0..3).map(|k| r.get(k, 0).value).collect();
        assert!(close(got[0], 1.0) && close(got[1], 2.0) && close(got[2], 3.0), "{got:?}");
        assert!(!r.is_complex());
        // x² + 1: ±i
        let r = polyroots(&m(3, 1, &[1.0, 0.0, 1.0])).unwrap();
        assert!(close(r.cget(0).im, -1.0) && close(r.cget(1).im, 1.0));
    }
}

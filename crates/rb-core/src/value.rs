//! Waarden: reële en complexe grootheden, matrices en bereiken.
//!
//! Elk element van een matrix is een eigen [`Quantity`], dus een vector mag `[1 m, 2 s]` zijn, zoals
//! in Mathcad. Rekenen controleert de eenheden per element. Een matrix kan complexe elementen hebben
//! (het imaginaire deel heeft dezelfde eenheid als het reële deel van dat element).
//! Lineaire algebra (det, inverse, eigenwaarden, ...) staat in [`crate::linalg`].

use crate::ast::{BinOp, CmpOp};
use crate::error::Error;
use crate::linalg;
use rb_units::{Dim, Quantity, UnitError};

fn dom(msg: impl Into<String>) -> Error {
    Error::Domain(msg.into())
}

fn range_err() -> Error {
    dom("een bereikvariabele (zoals i := 1..10) kan hier niet als geheel worden gebruikt; \
         gebruik hem in een resultaatregel, een definitie of als index")
}

// ---------------------------------------------------------------------------------------------
// Complexe getallen

/// Een complexe grootheid: (re + im·i) in SI-eenheden met één dimensie.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Complex {
    pub re: f64,
    pub im: f64,
    pub dim: Dim,
}

impl Complex {
    pub const fn new(re: f64, im: f64, dim: Dim) -> Self {
        Complex { re, im, dim }
    }

    pub fn from_q(q: Quantity) -> Self {
        Complex::new(q.value, 0.0, q.dim)
    }

    pub fn abs(&self) -> f64 {
        self.re.hypot(self.im)
    }

    pub fn arg(&self) -> f64 {
        self.im.atan2(self.re)
    }

    pub fn conj(self) -> Self {
        Complex::new(self.re, -self.im, self.dim)
    }

    pub fn is_real(&self) -> bool {
        self.im == 0.0
    }

    pub fn neg(self) -> Self {
        Complex::new(-self.re, -self.im, self.dim)
    }

    fn same_dim(self, o: Self, op: &'static str) -> Result<(), Error> {
        if self.dim == o.dim {
            Ok(())
        } else {
            Err(UnitError::Mismatch {
                op,
                left: self.dim,
                right: o.dim,
            }
            .into())
        }
    }

    pub fn add(self, o: Self) -> Result<Self, Error> {
        self.same_dim(o, "optellen")?;
        Ok(Complex::new(self.re + o.re, self.im + o.im, self.dim))
    }

    pub fn sub(self, o: Self) -> Result<Self, Error> {
        self.same_dim(o, "aftrekken")?;
        Ok(Complex::new(self.re - o.re, self.im - o.im, self.dim))
    }

    fn mul_raw(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
        (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0)
    }

    fn div_raw(a: (f64, f64), b: (f64, f64)) -> Result<(f64, f64), Error> {
        let d = b.0 * b.0 + b.1 * b.1;
        if d == 0.0 {
            return Err(dom("deling door nul"));
        }
        Ok(((a.0 * b.0 + a.1 * b.1) / d, (a.1 * b.0 - a.0 * b.1) / d))
    }

    pub fn mul(self, o: Self) -> Result<Self, Error> {
        let dim = self.dim.checked_mul(o.dim).ok_or(UnitError::Overflow)?;
        let (re, im) = Self::mul_raw((self.re, self.im), (o.re, o.im));
        Ok(Complex::new(re, im, dim))
    }

    pub fn div(self, o: Self) -> Result<Self, Error> {
        let dim = self.dim.checked_div(o.dim).ok_or(UnitError::Overflow)?;
        let (re, im) = Self::div_raw((self.re, self.im), (o.re, o.im))?;
        Ok(Complex::new(re, im, dim))
    }

    /// e^z (z dimensieloos).
    pub fn exp(self) -> Self {
        let r = self.re.exp();
        Complex::new(r * self.im.cos(), r * self.im.sin(), Dim::NONE)
    }

    /// Hoofdwaarde van ln z (z dimensieloos).
    pub fn ln(self) -> Self {
        Complex::new(self.abs().ln(), self.arg(), Dim::NONE)
    }

    /// z^n voor een geheel getal n (ook met eenheid).
    pub fn powi(self, n: i32) -> Result<Self, Error> {
        let dim = self.dim.checked_powi(n).ok_or(UnitError::Overflow)?;
        let mut base = (self.re, self.im);
        let mut acc = (1.0, 0.0);
        let mut k = n.unsigned_abs();
        while k > 0 {
            if k & 1 == 1 {
                acc = Self::mul_raw(acc, base);
            }
            base = Self::mul_raw(base, base);
            k >>= 1;
        }
        if n < 0 {
            acc = Self::div_raw((1.0, 0.0), acc)?;
        }
        Ok(Complex::new(acc.0, acc.1, dim))
    }

    /// Hoofdwaarde van de n-de machtswortel (ook met eenheid, als de eenheid dat toelaat).
    pub fn root(self, n: u32) -> Result<Self, Error> {
        let dim = self.dim.root(n).ok_or(UnitError::NoRoot { dim: self.dim, n })?;
        let r = self.abs().powf(1.0 / n as f64);
        let t = self.arg() / n as f64;
        Ok(Complex::new(r * t.cos(), r * t.sin(), dim))
    }

    /// z^w voor dimensieloze z en w (hoofdwaarde).
    pub fn powc(self, w: Complex) -> Self {
        if self.re == 0.0 && self.im == 0.0 {
            return if w.re > 0.0 {
                Complex::new(0.0, 0.0, Dim::NONE)
            } else {
                Complex::new(f64::NAN, f64::NAN, Dim::NONE)
            };
        }
        let l = self.ln();
        let (re, im) = Self::mul_raw((w.re, w.im), (l.re, l.im));
        Complex::new(re, im, Dim::NONE).exp()
    }

    /// z^x voor een reële, dimensieloze exponent.
    fn pow_real(self, x: f64) -> Result<Self, Error> {
        if x.fract() == 0.0 && x.abs() <= 1e6 {
            return self.powi(x as i32);
        }
        if self.dim.is_none() {
            return Ok(self.powc(Complex::new(x, 0.0, Dim::NONE)));
        }
        let inv = 1.0 / x;
        if inv.is_finite() && (1.0..=100.0).contains(&inv) && (inv - inv.round()).abs() < 1e-9 {
            return self.root(inv.round() as u32);
        }
        Err(UnitError::NonIntegerPower { dim: self.dim }.into())
    }
}

// ---------------------------------------------------------------------------------------------
// Matrices

/// Een matrix (of vector) van grootheden, rij voor rij opgeslagen. `im` bevat de imaginaire delen
/// (in de eenheid van het element) als de matrix complex is; anders `None`.
#[derive(Debug, Clone, PartialEq)]
pub struct Matrix {
    pub rows: usize,
    pub cols: usize,
    pub data: Vec<Quantity>,
    pub im: Option<Vec<f64>>,
}

impl Matrix {
    pub fn new(rows: usize, cols: usize, data: Vec<Quantity>) -> Matrix {
        assert_eq!(data.len(), rows * cols, "matrixgrootte klopt niet");
        Matrix {
            rows,
            cols,
            data,
            im: None,
        }
    }

    /// Een matrix uit complexe elementen; zonder imaginaire delen wordt het een reële matrix.
    pub fn from_complex(rows: usize, cols: usize, elems: Vec<Complex>) -> Matrix {
        assert_eq!(elems.len(), rows * cols, "matrixgrootte klopt niet");
        let data = elems.iter().map(|z| Quantity::new(z.re, z.dim)).collect();
        let im = elems.iter().any(|z| z.im != 0.0).then(|| elems.iter().map(|z| z.im).collect());
        Matrix { rows, cols, data, im }
    }

    /// Een kolomvector.
    pub fn column(data: Vec<Quantity>) -> Matrix {
        Matrix::new(data.len(), 1, data)
    }

    pub fn identity(n: usize) -> Matrix {
        let mut data = vec![Quantity::scalar(0.0); n * n];
        for i in 0..n {
            data[i * n + i] = Quantity::scalar(1.0);
        }
        Matrix::new(n, n, data)
    }

    pub fn is_complex(&self) -> bool {
        self.im.is_some()
    }

    /// Het reële deel van element (r, c).
    pub fn get(&self, r: usize, c: usize) -> Quantity {
        self.data[r * self.cols + c]
    }

    /// Element `k` (rij voor rij) als complex getal.
    pub fn cget(&self, k: usize) -> Complex {
        let q = self.data[k];
        Complex::new(q.value, self.im.as_ref().map_or(0.0, |im| im[k]), q.dim)
    }

    /// Element (r, c) als complex getal.
    pub fn cget_rc(&self, r: usize, c: usize) -> Complex {
        self.cget(r * self.cols + c)
    }

    /// Alle elementen als complexe getallen.
    pub fn elems(&self) -> Vec<Complex> {
        (0..self.len()).map(|k| self.cget(k)).collect()
    }

    pub fn set(&mut self, r: usize, c: usize, q: Quantity) {
        self.set_c(r, c, Complex::from_q(q));
    }

    pub fn set_c(&mut self, r: usize, c: usize, z: Complex) {
        let k = r * self.cols + c;
        self.data[k] = Quantity::new(z.re, z.dim);
        if z.im != 0.0 {
            let n = self.data.len();
            self.im.get_or_insert_with(|| vec![0.0; n])[k] = z.im;
        } else if let Some(im) = &mut self.im {
            im[k] = 0.0;
        }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn is_vector(&self) -> bool {
        self.rows == 1 || self.cols == 1
    }

    pub fn is_square(&self) -> bool {
        self.rows == self.cols
    }

    pub fn shape(&self) -> String {
        format!("{}×{}", self.rows, self.cols)
    }

    /// Transponeren (zonder complex toevoegen, zoals in Mathcad).
    pub fn transpose(&self) -> Matrix {
        let mut elems = Vec::with_capacity(self.len());
        for c in 0..self.cols {
            for r in 0..self.rows {
                elems.push(self.cget_rc(r, c));
            }
        }
        Matrix::from_complex(self.cols, self.rows, elems)
    }

    /// Vergroot de matrix tot (minstens) `rows × cols`; nieuwe plaatsen krijgen `fill`.
    pub fn grow(&mut self, rows: usize, cols: usize, fill: Quantity) {
        let (nr, nc) = (rows.max(self.rows), cols.max(self.cols));
        if (nr, nc) == (self.rows, self.cols) {
            return;
        }
        let mut elems = vec![Complex::from_q(fill); nr * nc];
        for r in 0..self.rows {
            for c in 0..self.cols {
                elems[r * nc + c] = self.cget_rc(r, c);
            }
        }
        *self = Matrix::from_complex(nr, nc, elems);
    }

    /// De gemeenschappelijke dimensie van alle elementen, als die er is.
    pub fn common_dim(&self) -> Option<Dim> {
        let d = self.data.first()?.dim;
        self.data.iter().all(|q| q.dim == d).then_some(d)
    }

    /// Een reële functie element voor element toepassen (fout bij een complexe matrix).
    pub fn map(&self, mut f: impl FnMut(Quantity) -> Result<Quantity, Error>) -> Result<Matrix, Error> {
        if self.is_complex() {
            return Err(dom("deze bewerking werkt (nog) niet met een complexe matrix"));
        }
        let data = self.data.iter().map(|&q| f(q)).collect::<Result<Vec<_>, _>>()?;
        Ok(Matrix::new(self.rows, self.cols, data))
    }

    /// Een functie op complexe elementen toepassen.
    pub fn cmap(&self, f: impl FnMut(Complex) -> Result<Complex, Error>) -> Result<Matrix, Error> {
        let elems = self.elems().into_iter().map(f).collect::<Result<Vec<_>, _>>()?;
        Ok(Matrix::from_complex(self.rows, self.cols, elems))
    }

    fn czip(
        &self,
        o: &Matrix,
        opname: &str,
        mut f: impl FnMut(Complex, Complex) -> Result<Complex, Error>,
    ) -> Result<Matrix, Error> {
        if (self.rows, self.cols) != (o.rows, o.cols) {
            return Err(dom(format!(
                "matrices van verschillende grootte bij {opname}: {} en {}",
                self.shape(),
                o.shape()
            )));
        }
        let elems = (0..self.len())
            .map(|k| f(self.cget(k), o.cget(k)))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Matrix::from_complex(self.rows, self.cols, elems))
    }
}

// ---------------------------------------------------------------------------------------------
// Bereiken

/// Een bereik zoals `1..10` of `0 m, 0.5 m .. 3 m`: `n` waarden vanaf `start` met stap `step` (SI).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RangeVal {
    pub start: Quantity,
    pub step: f64,
    pub n: usize,
}

impl RangeVal {
    pub const MAX: usize = 1_000_000;

    /// Maak een bereik van `start` tot en met `end`, met als tweede waarde `second` (standaard ±1).
    pub fn new(start: Quantity, second: Option<Quantity>, end: Quantity) -> Result<RangeVal, Error> {
        start.same_dim(end, "een bereik (begin en einde)")?;
        let step = match second {
            Some(s) => {
                start.same_dim(s, "een bereik (begin en volgende waarde)")?;
                s.value - start.value
            }
            None if end.value >= start.value => 1.0,
            None => -1.0,
        };
        if step == 0.0 || !step.is_finite() || !start.value.is_finite() || !end.value.is_finite() {
            return Err(dom("de stap van een bereik moet eindig en ongelijk aan 0 zijn"));
        }
        let span = (end.value - start.value) / step;
        if span < -1e-9 {
            return Err(dom(
                "dit bereik loopt de verkeerde kant op: de tweede waarde ligt niet tussen begin en einde",
            ));
        }
        let n = (span + 1e-9).floor() + 1.0;
        if n > Self::MAX as f64 {
            return Err(dom(format!("een bereik mag hoogstens {} waarden hebben", Self::MAX)));
        }
        Ok(RangeVal {
            start,
            step,
            n: n as usize,
        })
    }

    pub fn get(&self, k: usize) -> Quantity {
        Quantity::new(self.start.value + self.step * k as f64, self.start.dim)
    }

    pub fn last(&self) -> Quantity {
        self.get(self.n - 1)
    }
}

// ---------------------------------------------------------------------------------------------
// Waarden

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Num(Quantity),
    Complex(Complex),
    Matrix(Matrix),
    Range(RangeVal),
    /// Tekst (`"abc"`), zoals een string in Mathcad.
    Str(String),
}

impl From<Quantity> for Value {
    fn from(q: Quantity) -> Value {
        Value::Num(q)
    }
}

impl From<Matrix> for Value {
    fn from(m: Matrix) -> Value {
        Value::Matrix(m)
    }
}

impl From<Complex> for Value {
    fn from(z: Complex) -> Value {
        Value::Complex(z).normalize()
    }
}

impl Value {
    pub fn scalar(v: f64) -> Value {
        Value::Num(Quantity::scalar(v))
    }

    pub fn bool(b: bool) -> Value {
        Value::scalar(if b { 1.0 } else { 0.0 })
    }

    /// Korte omschrijving voor foutmeldingen.
    pub fn kind(&self) -> &'static str {
        match self {
            Value::Num(_) => "getal",
            Value::Complex(_) => "complex getal",
            Value::Matrix(m) if m.is_vector() => "vector",
            Value::Matrix(_) => "matrix",
            Value::Range(_) => "bereik",
            Value::Str(_) => "tekst",
        }
    }

    /// Bevat de waarde een NaN?
    pub fn has_nan(&self) -> bool {
        match self {
            Value::Num(q) => q.value.is_nan(),
            Value::Complex(c) => c.re.is_nan() || c.im.is_nan(),
            Value::Matrix(m) => {
                m.data.iter().any(|q| q.value.is_nan()) || m.im.as_ref().is_some_and(|im| im.iter().any(|v| v.is_nan()))
            }
            Value::Range(_) | Value::Str(_) => false,
        }
    }

    /// Zijn alle getallen eindig?
    pub fn is_finite(&self) -> bool {
        match self {
            Value::Num(q) => q.value.is_finite(),
            Value::Complex(c) => c.re.is_finite() && c.im.is_finite(),
            Value::Matrix(m) => {
                m.data.iter().all(|q| q.value.is_finite()) && m.im.as_ref().is_none_or(|im| im.iter().all(|v| v.is_finite()))
            }
            Value::Range(_) | Value::Str(_) => true,
        }
    }

    /// De reële grootheid, of een fout als dit iets anders is.
    pub fn as_quantity(&self, what: &str) -> Result<Quantity, Error> {
        match self {
            Value::Num(q) => Ok(*q),
            Value::Range(_) => Err(range_err()),
            other => Err(dom(format!("{what} moet een reëel getal zijn, geen {}", other.kind()))),
        }
    }

    /// Een (eventueel complex) getal, of een fout als dit een matrix of bereik is.
    pub fn as_complex(&self, what: &str) -> Result<Complex, Error> {
        match self {
            Value::Num(q) => Ok(Complex::from_q(*q)),
            Value::Complex(z) => Ok(*z),
            Value::Range(_) => Err(range_err()),
            other => Err(dom(format!("{what} moet een getal zijn, geen {}", other.kind()))),
        }
    }

    fn scalar_c(&self) -> Option<Complex> {
        match self {
            Value::Num(q) => Some(Complex::from_q(*q)),
            Value::Complex(c) => Some(*c),
            _ => None,
        }
    }

    /// Een complex getal met imaginair deel 0 wordt een gewoon getal.
    pub fn normalize(self) -> Value {
        match self {
            Value::Complex(c) if c.im == 0.0 => Value::Num(Quantity::new(c.re, c.dim)),
            v => v,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Rekenen

pub fn neg(v: &Value) -> Result<Value, Error> {
    Ok(match v {
        Value::Num(q) => Value::Num(q.neg()),
        Value::Complex(c) => Value::Complex(c.neg()),
        Value::Matrix(m) => Value::Matrix(m.cmap(|z| Ok(z.neg()))?),
        Value::Range(_) => return Err(range_err()),
        Value::Str(_) => return Err(str_err()),
    })
}

/// Rekenen met tekst kan niet (zoals in Mathcad); gebruik concat om tekst samen te voegen.
pub fn str_err() -> Error {
    dom("met tekst kun je niet rekenen; gebruik concat(a, b) om tekst samen te voegen")
}

pub fn binop(op: BinOp, a: &Value, b: &Value) -> Result<Value, Error> {
    if matches!(a, Value::Range(_)) || matches!(b, Value::Range(_)) {
        return Err(range_err());
    }
    if matches!(a, Value::Str(_)) || matches!(b, Value::Str(_)) {
        return Err(str_err());
    }
    match op {
        BinOp::Add => add_sub(a, b, false),
        BinOp::Sub => add_sub(a, b, true),
        BinOp::Mul => mul(a, b),
        BinOp::Div => div(a, b),
        BinOp::Pow => pow(a, b),
    }
}

fn add_sub(a: &Value, b: &Value, sub: bool) -> Result<Value, Error> {
    let opname = if sub { "aftrekken" } else { "optellen" };
    let f = |x: Complex, y: Complex| if sub { x.sub(y) } else { x.add(y) };
    Ok(match (a, b) {
        (Value::Num(x), Value::Num(y)) => Value::Num(if sub { x.sub(*y)? } else { x.add(*y)? }),
        (Value::Matrix(m), Value::Matrix(n)) => Value::Matrix(m.czip(n, opname, f)?),
        (Value::Matrix(m), y) => {
            let y = y.scalar_c().expect("getal");
            Value::Matrix(m.cmap(|x| f(x, y))?)
        }
        (x, Value::Matrix(n)) => {
            let x = x.scalar_c().expect("getal");
            Value::Matrix(n.cmap(|y| f(x, y))?)
        }
        _ => Value::Complex(f(a.scalar_c().unwrap(), b.scalar_c().unwrap())?).normalize(),
    })
}

fn mul(a: &Value, b: &Value) -> Result<Value, Error> {
    Ok(match (a, b) {
        (Value::Num(x), Value::Num(y)) => Value::Num(x.mul(*y)?),
        (Value::Matrix(m), Value::Matrix(n)) => matmul_or_dot(m, n)?,
        (Value::Matrix(m), y) => {
            let y = y.scalar_c().expect("getal");
            Value::Matrix(m.cmap(|x| x.mul(y))?)
        }
        (x, Value::Matrix(n)) => {
            let x = x.scalar_c().expect("getal");
            Value::Matrix(n.cmap(|y| x.mul(y))?)
        }
        _ => Value::Complex(a.scalar_c().unwrap().mul(b.scalar_c().unwrap())?).normalize(),
    })
}

fn div(a: &Value, b: &Value) -> Result<Value, Error> {
    Ok(match (a, b) {
        (_, Value::Num(y)) if y.value == 0.0 => return Err(dom("deling door nul")),
        (Value::Num(x), Value::Num(y)) => Value::Num(x.div(*y)?),
        (_, Value::Matrix(_)) => {
            return Err(dom("delen door een matrix kan niet; gebruik inverse(M) of M^-1"))
        }
        (Value::Matrix(m), y) => {
            let y = y.scalar_c().expect("getal");
            Value::Matrix(m.cmap(|x| x.div(y))?)
        }
        _ => Value::Complex(a.scalar_c().unwrap().div(b.scalar_c().unwrap())?).normalize(),
    })
}

fn pow(a: &Value, b: &Value) -> Result<Value, Error> {
    match (a, b) {
        (Value::Num(x), Value::Num(y)) => {
            // negatief grondtal met een gebroken exponent: complexe hoofdwaarde, zoals in Mathcad
            if x.value < 0.0 && y.dim.is_none() && y.value.fract() != 0.0 {
                return Ok(Value::Complex(Complex::from_q(*x).pow_real(y.value)?).normalize());
            }
            Ok(Value::Num(x.pow(*y)?))
        }
        (Value::Complex(z), Value::Num(y)) => {
            let e = y.dimensionless_value("de exponent")?;
            Ok(Value::Complex(z.pow_real(e)?).normalize())
        }
        (Value::Num(_) | Value::Complex(_), Value::Complex(w)) => {
            let z = a.scalar_c().unwrap();
            if !z.dim.is_none() || !w.dim.is_none() {
                return Err(dom(
                    "een complexe exponent kan alleen bij een dimensieloos grondtal en een dimensieloze exponent",
                ));
            }
            Ok(Value::Complex(z.powc(*w)).normalize())
        }
        (Value::Matrix(m), Value::Num(y)) => matrix_pow(m, *y),
        (Value::Matrix(_), _) => Err(dom("een matrix kan alleen tot een reële, gehele macht worden verheven")),
        (_, Value::Matrix(_)) => Err(dom("een exponent kan geen vector of matrix zijn")),
        _ => Err(range_err()),
    }
}

fn matrix_pow(m: &Matrix, y: Quantity) -> Result<Value, Error> {
    let e = y.dimensionless_value("de exponent")?;
    if !m.is_square() {
        return Err(dom(format!(
            "alleen een vierkante matrix kan tot een macht worden verheven (deze is {})",
            m.shape()
        )));
    }
    if e.fract() != 0.0 || e.abs() > 1e6 {
        return Err(dom("een matrix kan alleen tot een gehele macht worden verheven"));
    }
    let n = e as i64;
    let base = if n < 0 { linalg::inverse(m)? } else { m.clone() };
    let mut k = n.unsigned_abs();
    if k == 0 {
        return Ok(Value::Matrix(Matrix::identity(m.rows)));
    }
    let mut acc: Option<Matrix> = None;
    let mut b = base;
    loop {
        if k & 1 == 1 {
            acc = Some(match acc {
                None => b.clone(),
                Some(a) => matmul(&a, &b)?,
            });
        }
        k >>= 1;
        if k == 0 {
            break;
        }
        b = matmul(&b, &b)?;
    }
    Ok(Value::Matrix(acc.unwrap()))
}

/// Matrixproduct; een 1×1-uitkomst (rij maal kolom) wordt een gewoon getal.
fn matmul_or_dot(m: &Matrix, n: &Matrix) -> Result<Value, Error> {
    if m.cols == n.rows {
        let r = matmul(m, n)?;
        if r.rows == 1 && r.cols == 1 {
            return Ok(Value::from(r.cget(0)));
        }
        return Ok(Value::Matrix(r));
    }
    // twee vectoren met dezelfde oriëntatie: inproduct, zoals in Mathcad
    if m.is_vector() && n.is_vector() && m.len() == n.len() {
        return dot(m, n);
    }
    Err(dom(format!(
        "kan een {} matrix niet met een {} matrix vermenigvuldigen \
         (het aantal kolommen links moet gelijk zijn aan het aantal rijen rechts)",
        m.shape(),
        n.shape()
    )))
}

pub fn matmul(m: &Matrix, n: &Matrix) -> Result<Matrix, Error> {
    let mut elems = Vec::with_capacity(m.rows * n.cols);
    for i in 0..m.rows {
        for j in 0..n.cols {
            let mut acc = m.cget_rc(i, 0).mul(n.cget_rc(0, j))?;
            for k in 1..m.cols {
                acc = acc.add(m.cget_rc(i, k).mul(n.cget_rc(k, j))?)?;
            }
            elems.push(acc);
        }
    }
    Ok(Matrix::from_complex(m.rows, n.cols, elems))
}

/// Inproduct van twee vectoren met evenveel elementen (zonder complex toevoegen, zoals in Mathcad).
pub fn dot(m: &Matrix, n: &Matrix) -> Result<Value, Error> {
    if m.len() != n.len() || m.is_empty() {
        return Err(dom("het inproduct vraagt twee vectoren met evenveel elementen"));
    }
    let mut acc = m.cget(0).mul(n.cget(0))?;
    for k in 1..m.len() {
        acc = acc.add(m.cget(k).mul(n.cget(k))?)?;
    }
    Ok(Value::from(acc))
}

fn approx_eq(a: f64, b: f64) -> bool {
    a == b || (a - b).abs() <= 1e-12 * a.abs().max(b.abs())
}

fn approx_eq_c(x: Complex, y: Complex) -> bool {
    x.dim == y.dim && approx_eq(x.re, y.re) && approx_eq(x.im, y.im)
}

/// Vergelijk twee waarden. `==` en `!=` gebruiken een relatieve tolerantie van 1e-12, zodat
/// `0.1 + 0.2 == 0.3` waar is.
pub fn compare(op: CmpOp, a: &Value, b: &Value) -> Result<bool, Error> {
    let equality = matches!(op, CmpOp::Eq | CmpOp::Ne);
    let eq = match (a, b) {
        (Value::Range(_), _) | (_, Value::Range(_)) => return Err(range_err()),
        // tekst: gelijk of niet, en alfabetisch ordenen
        (Value::Str(x), Value::Str(y)) => {
            return Ok(match op {
                CmpOp::Lt => x < y,
                CmpOp::Le => x <= y,
                CmpOp::Gt => x > y,
                CmpOp::Ge => x >= y,
                CmpOp::Eq => x == y,
                CmpOp::Ne => x != y,
            })
        }
        (Value::Str(_), _) | (_, Value::Str(_)) if equality => return Ok(op == CmpOp::Ne),
        (Value::Num(x), Value::Num(y)) => {
            // een kale 0 is in elke eenheid 0: `0 < L` mag, ook als L een lengte is
            let bare_zero = |q: &Quantity| q.dim.is_none() && q.value == 0.0;
            if !bare_zero(x) && !bare_zero(y) {
                x.same_dim(*y, "vergelijken")?;
            }
            let (x, y) = (x.value, y.value);
            return Ok(match op {
                CmpOp::Lt => x < y,
                CmpOp::Le => x <= y,
                CmpOp::Gt => x > y,
                CmpOp::Ge => x >= y,
                CmpOp::Eq => approx_eq(x, y),
                CmpOp::Ne => !approx_eq(x, y),
            });
        }
        _ if !equality => {
            return Err(dom(format!(
                "alleen reële getallen kun je ordenen met <, ≤, > of ≥ (niet een {} en een {})",
                a.kind(),
                b.kind()
            )))
        }
        (Value::Matrix(m), Value::Matrix(n)) => {
            (m.rows, m.cols) == (n.rows, n.cols) && (0..m.len()).all(|k| approx_eq_c(m.cget(k), n.cget(k)))
        }
        (Value::Matrix(_), _) | (_, Value::Matrix(_)) => false,
        _ => {
            let (x, y) = (a.scalar_c().unwrap(), b.scalar_c().unwrap());
            x.same_dim(y, "vergelijken")?;
            approx_eq_c(x, y)
        }
    };
    Ok(if op == CmpOp::Eq { eq } else { !eq })
}

/// Waar of onwaar: een dimensieloos getal is waar als het niet 0 is (zoals in Mathcad).
pub fn truth(v: &Value) -> Result<bool, Error> {
    match v {
        Value::Num(q) if q.dim.is_none() => Ok(q.value != 0.0),
        Value::Num(q) => Err(dom(format!(
            "een voorwaarde moet dimensieloos zijn (een vergelijking geeft 1 of 0), maar deze heeft eenheid {}",
            q.dim.describe()
        ))),
        Value::Range(_) => Err(range_err()),
        other => Err(dom(format!("een voorwaarde moet een getal zijn, geen {}", other.kind()))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complexe_machten() {
        let i = Complex::new(0.0, 1.0, Dim::NONE);
        let sq = i.powi(2).unwrap();
        assert_eq!((sq.re, sq.im), (-1.0, 0.0));
        // (-8)^(1/3) = 1 + 1.732i (hoofdwaarde)
        let r = Complex::from_q(Quantity::scalar(-8.0)).pow_real(1.0 / 3.0).unwrap();
        assert!((r.re - 1.0).abs() < 1e-12 && (r.im - 3f64.sqrt()).abs() < 1e-12);
    }

    #[test]
    fn complexe_matrix_rekenen() {
        let i = Complex::new(0.0, 1.0, Dim::NONE);
        let one = Complex::new(1.0, 0.0, Dim::NONE);
        let m = Matrix::from_complex(2, 2, vec![one, i, i.neg(), one]);
        assert!(m.is_complex());
        // [1, i; -i, 1]² = [2, 2i; -2i, 2]
        let sq = matmul(&m, &m).unwrap();
        assert_eq!(sq.cget(1), Complex::new(0.0, 2.0, Dim::NONE));
        assert_eq!(sq.cget(0), Complex::new(2.0, 0.0, Dim::NONE));
        // reële uitkomst wordt weer een reële matrix
        let conj = m.cmap(|z| Ok(z.conj())).unwrap();
        let p = matmul(&m, &conj.transpose()).unwrap();
        assert!(p.data.iter().all(|q| q.value.is_finite()));
        assert!(m.map(|q| Ok(q)).is_err());
    }

    #[test]
    fn bereik() {
        let r = RangeVal::new(Quantity::scalar(0.0), Some(Quantity::scalar(0.1)), Quantity::scalar(1.0)).unwrap();
        assert_eq!(r.n, 11);
        assert!((r.last().value - 1.0).abs() < 1e-12);
        let r = RangeVal::new(Quantity::scalar(5.0), None, Quantity::scalar(1.0)).unwrap();
        assert_eq!((r.n, r.step), (5, -1.0));
        assert!(RangeVal::new(Quantity::scalar(0.0), Some(Quantity::scalar(-1.0)), Quantity::scalar(3.0)).is_err());
    }
}

use crate::ast::{BStmt, BinOp, BindOp, Expr, Stmt, SymOp};
use crate::builtins;
use crate::error::Error;
use crate::numeric;
use crate::parser::parse_stmt;
use crate::render::{format_rows, prefix_lines, render_value, Display};
use crate::value::{self, Matrix, RangeVal, Value};
use rb_units::{
    format_number, format_quantity, has_auto_name, lookup_offset_unit, lookup_unit, unit_base,
    Dim, Quantity,
};
use std::collections::{BTreeSet, HashMap};

mod solve;

const MAX_DEPTH: usize = 200;
/// Maximaal aantal herhalingen van een som, product of definitie over bereiken.
const MAX_ITER: usize = 1_000_000;
/// Maximaal aantal rijen in een resultaattabel.
const MAX_ROWS: usize = 1000;
const TEMPERATURE: Dim = Dim::new(0, 0, 0, 0, 1, 0, 0);
/// Maximaal aantal lusdoorgangen in één programmablok.
const MAX_STEPS: usize = 10_000_000;

/// Hoe een (deel van een) programmablok eindigt.
enum Flow {
    /// Gewoon doorgelopen; met de waarde van de laatste regel (als die er is).
    Normal(Option<Value>),
    Break,
    Continue,
    Return(Value),
}

/// Een door de gebruiker gedefinieerde functie, bijvoorbeeld `f(x, y) := x^2 + y`.
#[derive(Debug, Clone, PartialEq)]
pub struct UserFn {
    pub params: Vec<String>,
    pub body: Expr,
}

/// Het resultaat van een definitie.
#[derive(Debug, Clone, PartialEq)]
pub enum Defined {
    Var {
        name: String,
        value: Value,
        /// De eenheid waarin de waarde bij voorkeur getoond wordt (zie [`Display::Hint`]).
        hint: Option<String>,
    },
    Fn {
        name: String,
        func: UserFn,
    },
}

impl Defined {
    pub fn name(&self) -> &str {
        match self {
            Defined::Var { name, .. } | Defined::Fn { name, .. } => name,
        }
    }
}

/// Het resultaat van het uitvoeren van een regel.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Defined(Defined),
    /// Meer definities tegelijk: de functies van een stelsel (`[x; y] := odesolve([x; y], t, b)`).
    DefinedAll(Vec<Defined>),
    Value {
        value: Value,
        lhs_src: String,
        display: Option<Display>,
        bare: bool,
    },
    /// Een resultaatregel met een bereikvariabele: één rij per waarde, al opgemaakt.
    Table {
        lhs_src: String,
        rows: Vec<String>,
        bare: bool,
    },
    /// Symbolisch uitgerekend (→): de uitkomst is een uitdrukking.
    Symbolic {
        lhs_src: String,
        result: Expr,
    },
}

type Locals = HashMap<String, Value>;

/// De rekenomgeving: variabelen, functies en weergave-eenheden.
#[derive(Debug, Clone, Default)]
pub struct Env {
    vars: HashMap<String, Value>,
    funcs: HashMap<String, UserFn>,
    hints: HashMap<String, String>,
    /// Toestand van de toevalsgenerator; per regel opnieuw gezaaid (zie [`Env::eval_stmt`]).
    rng: std::cell::Cell<u64>,
    /// De beperkingen van het Given-blok voor de regel die nu wordt uitgerekend.
    constraints: Vec<Expr>,
    /// Bestanden lezen en schrijven (READCSV, WRITEEXCEL, ...), als het werkblad dat aanbiedt.
    io: Option<Io>,
}

/// Bestandsfuncties (READCSV, WRITEEXCEL, ...). De rekenkern kent zelf geen bestanden; het werkblad
/// geeft een implementatie mee (zie `rb-sheet/src/io.rs`), met paden relatief aan het blad.
pub trait FileIo: Send + Sync {
    /// `None` als `name` geen bestandsfunctie is.
    fn call(&self, name: &str, args: &[Value]) -> Option<Result<Value, Error>>;
}

#[derive(Clone)]
pub struct Io(pub std::sync::Arc<dyn FileIo>);

impl std::fmt::Debug for Io {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Io")
    }
}

/// SplitMix64: klein, snel en goed genoeg voor simulaties in een rekenblad.
fn next_random(state: &std::cell::Cell<u64>) -> f64 {
    let mut z = state.get().wrapping_add(0x9E37_79B9_7F4A_7C15);
    state.set(z);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

/// Ingebouwde constanten: `pi`/`π`, `e`, `inf`/`∞` en de instelling `ORIGIN` (eerste index, 0).
pub fn constant_value(name: &str) -> Option<Quantity> {
    match name {
        "pi" | "π" => Some(Quantity::scalar(std::f64::consts::PI)),
        "e" => Some(Quantity::scalar(std::f64::consts::E)),
        "inf" | "∞" => Some(Quantity::scalar(f64::INFINITY)),
        "ORIGIN" => Some(Quantity::scalar(0.0)),
        _ => None,
    }
}

fn dom(msg: impl Into<String>) -> Error {
    Error::Domain(msg.into())
}

/// Controleer het resultaat van een bewerking: NaN is altijd fout, ∞ alleen als het niet al in de
/// invoer zat (`1 / 0` en `exp(1000)` zijn fout, `∞ + 1` niet).
fn checked(r: Value, inputs_finite: bool) -> Result<Value, Error> {
    if r.has_nan() || (inputs_finite && !r.is_finite()) {
        return Err(dom(
            "ongeldig resultaat (bijvoorbeeld wortel uit een negatief getal, ln van 0 of een overflow)",
        ));
    }
    Ok(r)
}

fn is_pure_number(e: &Expr) -> bool {
    match e {
        Expr::Num(_) => true,
        Expr::Neg(x) => is_pure_number(x),
        Expr::Bin(_, a, b) => is_pure_number(a) && is_pure_number(b),
        Expr::Matrix(rows) => rows.iter().flatten().all(is_pure_number),
        _ => false,
    }
}

/// Splits een product/quotiënt in factoren; `true` betekent: in de noemer.
fn flatten_product<'a>(e: &'a Expr, div: bool, out: &mut Vec<(bool, &'a Expr)>) {
    match e {
        Expr::Bin(BinOp::Mul, a, b) => {
            flatten_product(a, div, out);
            flatten_product(b, div, out);
        }
        Expr::Bin(BinOp::Div, a, b) => {
            flatten_product(a, div, out);
            flatten_product(b, !div, out);
        }
        _ => out.push((div, e)),
    }
}

/// De waarde van een eenheidstekst zoals `kN/m` of `kg/m^3` (alleen eenheden, geen variabelen).
pub(crate) fn unit_text_value(text: &str) -> Option<Quantity> {
    let e = crate::parser::parse_expr_str(text).ok()?;
    match Env::new().eval_expr(&e).ok()? {
        Value::Num(q) => Some(q),
        _ => None,
    }
}

/// Twee weergave-eenheden samenvoegen bij optellen: gelijk blijft gelijk, °C + Δ°C blijft °C.
fn merge_hints(a: Option<String>, b: Option<String>) -> Option<String> {
    match (a, b) {
        (Some(x), Some(y)) if x == y => Some(x),
        (Some(x), Some(y)) => {
            if y == format!("Δ{x}") {
                Some(x)
            } else if x == format!("Δ{y}") {
                Some(y)
            } else {
                None
            }
        }
        (Some(x), None) | (None, Some(x)) => Some(x),
        (None, None) => None,
    }
}

fn index_of(v: &Value, origin: i64) -> Result<usize, Error> {
    let q = v.as_quantity("een index")?;
    let x = q.dimensionless_value("een index")?;
    let k = x.round();
    if (x - k).abs() > 1e-9 {
        return Err(dom(format!("een index moet een geheel getal zijn, niet {x}")));
    }
    let k = k as i64 - origin;
    if k < 0 {
        return Err(dom(format!(
            "index {} ligt onder ORIGIN ({origin})",
            k + origin
        )));
    }
    Ok(k as usize)
}

/// Zet element `ks` (0-gebaseerd) van een vector of matrix; die groeit mee en wordt aangevuld met
/// nullen in de eenheid van `q`. Een rijvector groeit naar rechts, al het andere naar beneden.
fn set_element(m: &mut Option<Matrix>, ks: &[usize], q: value::Complex) -> Result<(), Error> {
    if ks.iter().any(|&k| k >= 100_000) {
        return Err(dom("een index groter dan 100000 is waarschijnlijk een vergissing"));
    }
    let fill = Quantity::new(0.0, q.dim);
    let mat = m.get_or_insert_with(|| Matrix::new(0, 1, Vec::new()));
    let (r, c) = match ks[..] {
        [k] if mat.rows == 1 && mat.cols > 1 => (0, k),
        [k] => (k, 0),
        [r, c] => (r, c),
        _ => unreachable!("de parser staat 1 of 2 indices toe"),
    };
    if mat.rows == 0 {
        *mat = Matrix::new(0, if ks.len() == 2 { c + 1 } else { 1 }, Vec::new());
    }
    mat.grow(r + 1, c + 1, fill);
    mat.set_c(r, c, q);
    Ok(())
}

fn out_of_range(k: usize, len: usize, origin: i64, what: &str) -> Error {
    dom(format!(
        "index {} valt buiten de {what} (geldige indices {} t/m {})",
        k as i64 + origin,
        origin,
        len as i64 - 1 + origin
    ))
}

impl Env {
    pub fn new() -> Env {
        Env::default()
    }

    /// Geef de omgeving bestandsfuncties (READCSV, WRITEEXCEL, ...).
    pub fn set_io(&mut self, io: std::sync::Arc<dyn FileIo>) {
        self.io = Some(Io(io));
    }

    /// Voer een definitie door in de omgeving.
    pub fn apply(&mut self, d: Defined) {
        match d {
            Defined::Var { name, value, hint } => {
                match hint {
                    Some(h) => self.hints.insert(name.clone(), h),
                    None => self.hints.remove(&name),
                };
                self.vars.insert(name, value);
            }
            Defined::Fn { name, func } => {
                self.funcs.insert(name, func);
            }
        }
    }

    pub fn set_var(&mut self, name: &str, value: impl Into<Value>) {
        self.vars.insert(name.to_string(), value.into());
        self.hints.remove(name);
    }

    pub fn get_var(&self, name: &str) -> Option<&Value> {
        self.vars.get(name)
    }

    /// De weergave-eenheid van een variabele, als die er is.
    pub fn hint(&self, name: &str) -> Option<&str> {
        self.hints.get(name).map(String::as_str)
    }

    /// Een variabele als tekst, in zijn eigen weergave-eenheid.
    pub fn render_var(&self, name: &str) -> Option<String> {
        let v = self.vars.get(name)?;
        let d = self.hints.get(name).cloned().map(Display::Hint);
        Some(render_value(v, &d).unwrap_or_else(|e| format!("✗ {e}")))
    }

    /// Namen van alle variabelen, gesorteerd.
    pub fn var_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.vars.keys().cloned().collect();
        v.sort();
        v
    }

    pub fn fn_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.funcs.keys().cloned().collect();
        v.sort();
        v
    }

    pub fn eval_expr(&self, e: &Expr) -> Result<Value, Error> {
        self.eval(e, &Locals::new(), 0)
    }

    fn is_variable(&self, n: &str, loc: &Locals) -> bool {
        loc.contains_key(n) || self.vars.contains_key(n)
    }

    fn lookup(&self, n: &str, loc: &Locals) -> Option<Value> {
        if let Some(v) = loc.get(n) {
            return Some(v.clone());
        }
        if let Some(v) = self.vars.get(n) {
            return Some(v.clone());
        }
        if let Some(q) = constant_value(n) {
            return Some(Value::Num(q));
        }
        lookup_unit(n).map(|(f, d)| Value::Num(Quantity::new(f, d)))
    }

    /// De eerste index (ORIGIN): 0, tenzij het blad `ORIGIN := 1` definieert.
    fn origin(&self, loc: &Locals) -> Result<i64, Error> {
        let v = self.lookup("ORIGIN", loc).unwrap_or(Value::scalar(0.0));
        let x = v.as_quantity("ORIGIN")?.dimensionless_value("ORIGIN")?;
        if x.fract() != 0.0 || x.abs() > 1e9 {
            return Err(dom("ORIGIN moet een geheel getal zijn"));
        }
        Ok(x as i64)
    }

    /// Een temperatuurschaal met nulpunt (`°C`), als `e` daarnaar verwijst en geen variabele is.
    fn offset_unit(&self, e: &Expr, loc: &Locals) -> Option<(f64, f64)> {
        match e {
            Expr::Var(n) if !self.is_variable(n, loc) => lookup_offset_unit(n),
            _ => None,
        }
    }

    fn eval(&self, e: &Expr, loc: &Locals, depth: usize) -> Result<Value, Error> {
        // Deze functie is recursief en blijft daarom klein; de zeldzame gevallen staan in aparte
        // functies, zodat diepe recursie (eigen functies) niet te veel stack kost.
        match e {
            Expr::Num(v) => Ok(Value::scalar(*v)),
            Expr::Var(n) => match self.lookup(n, loc) {
                Some(v) => Ok(v),
                None => Err(self.unknown_name(n)),
            },
            Expr::Bin(op, a, b) => self.eval_bin(*op, a, b, loc, depth),
            Expr::Call(name, args) => self.call(name, args, loc, depth),
            Expr::Neg(x) => value::neg(&self.eval(x, loc, depth)?),
            Expr::Not(x) => Ok(Value::bool(!value::truth(&self.eval(x, loc, depth)?)?)),
            Expr::And(a, b) => Ok(Value::bool(
                value::truth(&self.eval(a, loc, depth)?)? && value::truth(&self.eval(b, loc, depth)?)?,
            )),
            Expr::Or(a, b) => Ok(Value::bool(
                value::truth(&self.eval(a, loc, depth)?)? || value::truth(&self.eval(b, loc, depth)?)?,
            )),
            _ => self.eval_other(e, loc, depth),
        }
    }

    #[inline(never)]
    fn unknown_name(&self, n: &str) -> Error {
        if lookup_offset_unit(n).is_some() {
            dom(format!(
                "{n} is een temperatuurschaal met een nulpunt: zet er een getal voor (20 {n}), \
                 of gebruik Δ{} voor een temperatuurverschil",
                n.trim_start_matches("deg").trim_start_matches('°')
            ))
        } else {
            Error::Unknown(n.to_string())
        }
    }

    #[inline(never)]
    fn eval_bin(&self, op: BinOp, a: &Expr, b: &Expr, loc: &Locals, depth: usize) -> Result<Value, Error> {
        if op == BinOp::Mul {
            if let Some((factor, offset)) = self.offset_unit(b, loc) {
                return self.to_kelvin(a, factor, offset, loc, depth);
            }
        }
        let x = self.eval(a, loc, depth)?;
        let y = self.eval(b, loc, depth)?;
        let finite = x.is_finite() && y.is_finite();
        checked(value::binop(op, &x, &y)?, finite)
    }

    /// `20 °C` is 293.15 K: het getal wordt verschoven, niet alleen geschaald.
    #[inline(never)]
    fn to_kelvin(&self, a: &Expr, factor: f64, offset: f64, loc: &Locals, depth: usize) -> Result<Value, Error> {
        let to_kelvin = |q: Quantity| -> Result<Quantity, Error> {
            if !q.dim.is_none() {
                return Err(dom("voor °C of °F moet een getal zonder eenheid staan, bijvoorbeeld 20 °C"));
            }
            Ok(Quantity::new(q.value * factor + offset, TEMPERATURE))
        };
        match self.eval(a, loc, depth)? {
            Value::Num(q) => Ok(Value::Num(to_kelvin(q)?)),
            Value::Matrix(m) => Ok(Value::Matrix(m.map(to_kelvin)?)),
            other => Err(dom(format!("een temperatuur kan geen {} zijn", other.kind()))),
        }
    }

    #[inline(never)]
    fn eval_other(&self, e: &Expr, loc: &Locals, depth: usize) -> Result<Value, Error> {
        match e {
            Expr::Imag(v) => Ok(Value::Complex(value::Complex::new(0.0, *v, Dim::NONE)).normalize()),
            Expr::Str(s) => Ok(Value::Str(s.clone())),
            Expr::Cmp(ops, xs) => {
                let mut left = self.eval(&xs[0], loc, depth)?;
                for (k, op) in ops.iter().enumerate() {
                    let right = self.eval(&xs[k + 1], loc, depth)?;
                    if !value::compare(*op, &left, &right)? {
                        return Ok(Value::bool(false));
                    }
                    left = right;
                }
                Ok(Value::bool(true))
            }
            Expr::Index(base, idx) => self.eval_index(base, idx, loc, depth),
            Expr::Matrix(rows) => {
                let mut data = Vec::new();
                for x in rows.iter().flatten() {
                    match self.eval(x, loc, depth)? {
                        Value::Num(q) => data.push(value::Complex::from_q(q)),
                        Value::Complex(z) => data.push(z),
                        Value::Matrix(_) => {
                            return Err(dom(
                                "een matrix als element van een matrix kan niet; gebruik augment of stack",
                            ))
                        }
                        Value::Range(_) => {
                            return Err(dom("een bereik kan geen element van een matrix zijn"))
                        }
                        Value::Str(_) => {
                            return Err(dom("tekst kan (nog) geen element van een matrix zijn"))
                        }
                    }
                }
                Ok(Value::Matrix(Matrix::from_complex(rows.len(), rows[0].len(), data)))
            }
            Expr::Bind { op, body, var, args } => self.eval_bind(*op, body, var, args, loc, depth),
            Expr::Interp(table, arg) => {
                let x = self.eval(arg, loc, depth)?.as_quantity("het argument")?;
                solve::interp_table(table, x)
            }
            Expr::Block(stmts) => {
                let mut scope = loc.clone();
                let mut steps = 0usize;
                match self.exec(stmts, &mut scope, depth, &mut steps)? {
                    Flow::Return(v) | Flow::Normal(Some(v)) => Ok(v),
                    Flow::Normal(None) => Err(dom("het programmablok geeft geen waarde (de laatste regel is een lus zonder resultaat)")),
                    Flow::Break | Flow::Continue => Err(dom("'break' of 'continue' buiten een lus")),
                }
            }
            Expr::Range { start, second, end } => {
                let q = |e: &Expr| -> Result<Quantity, Error> {
                    self.eval(e, loc, depth)?.as_quantity("de grenzen van een bereik")
                };
                let s = match second {
                    Some(x) => Some(q(x)?),
                    None => None,
                };
                Ok(Value::Range(RangeVal::new(q(start)?, s, q(end)?)?))
            }
            Expr::Num(_) | Expr::Var(_) | Expr::Bin(..) | Expr::Call(..) | Expr::Neg(_) | Expr::Not(_)
            | Expr::And(..) | Expr::Or(..) => self.eval(e, loc, depth),
        }
    }

    /// Voer de regels van een programmablok uit. `scope` zijn de lokale variabelen; `steps` telt
    /// lusdoorgangen over het hele blok.
    fn exec(&self, stmts: &[BStmt], scope: &mut Locals, depth: usize, steps: &mut usize) -> Result<Flow, Error> {
        let mut last = None;
        for s in stmts {
            match s {
                BStmt::Assign { name, index, value } => {
                    let v = self.eval(value, scope, depth)?;
                    match index {
                        None => {
                            scope.insert(name.clone(), v.clone());
                        }
                        Some(idx) => {
                            let origin = self.origin(scope)?;
                            let ks = idx
                                .iter()
                                .map(|i| index_of(&self.eval(i, scope, depth)?, origin))
                                .collect::<Result<Vec<_>, _>>()?;
                            let q = v.as_complex("een element")?;
                            let mut m = match self.lookup(name, scope) {
                                Some(Value::Matrix(m)) => Some(m),
                                _ => None,
                            };
                            set_element(&mut m, &ks, q)?;
                            scope.insert(name.clone(), Value::Matrix(m.expect("element gezet")));
                        }
                    }
                    last = Some(v);
                }
                BStmt::If { branches, els } => {
                    let mut taken = None;
                    for (c, body) in branches {
                        if value::truth(&self.eval(c, scope, depth)?)? {
                            taken = Some(body);
                            break;
                        }
                    }
                    if let Some(body) = taken.or(els.as_ref()) {
                        match self.exec(body, scope, depth, steps)? {
                            Flow::Normal(v) => last = v,
                            flow => return Ok(flow),
                        }
                    }
                }
                BStmt::For { var, iter, body } => {
                    let items: Vec<Value> = match self.eval(iter, scope, depth)? {
                        Value::Range(r) => (0..r.n).map(|k| Value::Num(r.get(k))).collect(),
                        Value::Matrix(m) => m.elems().into_iter().map(Value::from).collect(),
                        v @ (Value::Num(_) | Value::Complex(_)) => vec![v],
                        // een tekst teken voor teken
                        Value::Str(s) => s.chars().map(|c| Value::Str(c.to_string())).collect(),
                    };
                    for item in items {
                        Self::count_step(steps)?;
                        scope.insert(var.clone(), item);
                        match self.exec(body, scope, depth, steps)? {
                            Flow::Normal(v) => last = v.or(last),
                            Flow::Continue => {}
                            Flow::Break => break,
                            r @ Flow::Return(_) => return Ok(r),
                        }
                    }
                }
                BStmt::While { cond, body } => {
                    while value::truth(&self.eval(cond, scope, depth)?)? {
                        Self::count_step(steps)?;
                        match self.exec(body, scope, depth, steps)? {
                            Flow::Normal(v) => last = v.or(last),
                            Flow::Continue => {}
                            Flow::Break => break,
                            r @ Flow::Return(_) => return Ok(r),
                        }
                    }
                }
                BStmt::Break => return Ok(Flow::Break),
                BStmt::Continue => return Ok(Flow::Continue),
                BStmt::Return(e) => return Ok(Flow::Return(self.eval(e, scope, depth)?)),
                BStmt::Expr(e) => last = Some(self.eval(e, scope, depth)?),
            }
        }
        Ok(Flow::Normal(last))
    }

    fn count_step(steps: &mut usize) -> Result<(), Error> {
        *steps += 1;
        if *steps > MAX_STEPS {
            return Err(dom(format!(
                "te veel herhalingen (meer dan {MAX_STEPS}); loopt een while-lus oneindig door?"
            )));
        }
        Ok(())
    }

    #[inline(never)]
    fn eval_index(&self, base: &Expr, idx: &[Expr], loc: &Locals, depth: usize) -> Result<Value, Error> {
        let b = self.eval(base, loc, depth)?;
        let origin = self.origin(loc)?;
        let ks = idx
            .iter()
            .map(|i| index_of(&self.eval(i, loc, depth)?, origin))
            .collect::<Result<Vec<_>, _>>()?;
        let m = match &b {
            Value::Matrix(m) => m,
            other => {
                return Err(dom(format!(
                    "alleen een vector of matrix heeft een index, dit is een {}",
                    other.kind()
                )))
            }
        };
        match ks[..] {
            [k] if m.is_vector() => {
                if k < m.len() {
                    Ok(Value::from(m.cget(k)))
                } else {
                    Err(out_of_range(k, m.len(), origin, "vector"))
                }
            }
            [_] => Err(dom(format!(
                "dit is een {} matrix: gebruik twee indices, M[rij, kolom]",
                m.shape()
            ))),
            [r, c] => {
                if r >= m.rows {
                    return Err(out_of_range(r, m.rows, origin, "rijen"));
                }
                if c >= m.cols {
                    return Err(out_of_range(c, m.cols, origin, "kolommen"));
                }
                Ok(Value::from(m.cget_rc(r, c)))
            }
            _ => unreachable!("de parser staat 1 of 2 indices toe"),
        }
    }

    fn eval_bind(
        &self,
        op: BindOp,
        body: &Expr,
        var: &str,
        args: &[Expr],
        loc: &Locals,
        depth: usize,
    ) -> Result<Value, Error> {
        let mut scope = loc.clone();
        match op {
            BindOp::Root => self.eval_root(body, var, args, loc, depth),
            BindOp::OdeSolve => Err(dom("odesolve hoort in een definitie onder een Given-blok: y := odesolve(t, eindwaarde)")),
            BindOp::Sum | BindOp::Prod => {
                let what = if op == BindOp::Sum { "sum" } else { "prod" };
                let bound = |e: &Expr| -> Result<i64, Error> {
                    let x = self
                        .eval(e, loc, depth)?
                        .as_quantity("een grens")?
                        .dimensionless_value(&format!("een grens van {what}"))?;
                    if x.fract() != 0.0 || x.abs() > 1e15 {
                        return Err(dom(format!("{what}: de grenzen moeten gehele getallen zijn")));
                    }
                    Ok(x as i64)
                };
                let (a, b) = (bound(&args[0])?, bound(&args[1])?);
                if b >= a && (b - a) as u64 >= MAX_ITER as u64 {
                    return Err(dom(format!("{what}: hoogstens {MAX_ITER} termen")));
                }
                let mut acc: Option<Value> = None;
                for k in a..=b {
                    scope.insert(var.to_string(), Value::scalar(k as f64));
                    let v = self.eval(body, &scope, depth)?;
                    acc = Some(match acc {
                        None => v,
                        Some(s) => {
                            let r = if op == BindOp::Sum {
                                value::binop(BinOp::Add, &s, &v)?
                            } else {
                                value::binop(BinOp::Mul, &s, &v)?
                            };
                            checked(r, true)?
                        }
                    });
                }
                // lege som is 0, leeg product is 1
                Ok(acc.unwrap_or_else(|| Value::scalar(if op == BindOp::Sum { 0.0 } else { 1.0 })))
            }
            BindOp::Integral | BindOp::Deriv => {
                let what = if op == BindOp::Integral { "integral" } else { "deriv" };
                let q = |e: &Expr, d: &str| -> Result<Quantity, Error> {
                    self.eval(e, loc, depth)?.as_quantity(&format!("{d} van {what}"))
                };
                // de dimensie van x volgt uit de grenzen (of het punt); ∞ past bij elke eenheid
                let (xdim, a, b, order) = if op == BindOp::Integral {
                    let (a, b) = (q(&args[0], "de ondergrens")?, q(&args[1], "de bovengrens")?);
                    let dim = if a.value.is_infinite() && a.dim.is_none() {
                        b.dim
                    } else {
                        a.dim
                    };
                    for g in [a, b] {
                        if g.dim != dim && !(g.value.is_infinite() && g.dim.is_none()) {
                            a.same_dim(b, "de integratiegrenzen")?;
                        }
                    }
                    (dim, a.value, b.value, 0)
                } else {
                    let x = q(&args[0], "het punt")?;
                    let n = match args.get(1) {
                        None => 1,
                        Some(e) => {
                            let n = q(e, "de orde")?.dimensionless_value("de orde van de afgeleide")?;
                            if n.fract() != 0.0 || !(1.0..=5.0).contains(&n) {
                                return Err(dom("de orde van de afgeleide moet 1 tot en met 5 zijn"));
                            }
                            n as u32
                        }
                    };
                    (x.dim, x.value, 0.0, n)
                };
                let mut fdim: Option<Dim> = None;
                let mut f = |x: f64| -> Result<f64, Error> {
                    scope.insert(var.to_string(), Value::Num(Quantity::new(x, xdim)));
                    let y = match self.eval(body, &scope, depth)? {
                        Value::Num(y) => y,
                        other => {
                            return Err(dom(format!(
                                "{what}: de uitdrukking moet een reëel getal geven, geen {}",
                                other.kind()
                            )))
                        }
                    };
                    match fdim {
                        None => fdim = Some(y.dim),
                        Some(d) if d != y.dim => {
                            return Err(dom(format!("{what}: de eenheid van de uitdrukking verandert onderweg")))
                        }
                        _ => {}
                    }
                    Ok(y.value)
                };
                let r = if op == BindOp::Integral {
                    numeric::integrate(&mut f, a, b)?
                } else {
                    numeric::derivative(&mut f, a, order)?
                };
                let fdim = fdim.unwrap_or(Dim::NONE);
                let dim = if op == BindOp::Integral {
                    fdim.checked_mul(xdim)
                } else {
                    xdim.checked_powi(order as i32).and_then(|xd| fdim.checked_div(xd))
                }
                .ok_or(rb_units::UnitError::Overflow)?;
                checked(Value::Num(Quantity::new(r, dim)), true)
            }
        }
    }

    /// `if(voorwaarde, dan, anders)` of `if(v1, w1, v2, w2, ..., anders)`. Alleen de gekozen tak
    /// wordt uitgerekend.
    fn eval_if(&self, args: &[Expr], loc: &Locals, depth: usize) -> Result<Value, Error> {
        if args.len() < 3 || args.len() % 2 == 0 {
            return Err(Error::Arity {
                name: "if".to_string(),
                expected: "3, 5, 7, ... (voorwaarde, waarde, ..., anders)".to_string(),
                got: args.len(),
            });
        }
        for pair in args[..args.len() - 1].chunks(2) {
            if value::truth(&self.eval(&pair[0], loc, depth)?)? {
                return self.eval(&pair[1], loc, depth);
            }
        }
        self.eval(&args[args.len() - 1], loc, depth)
    }

    fn call(&self, name: &str, args: &[Expr], loc: &Locals, depth: usize) -> Result<Value, Error> {
        if let Some(f) = self.funcs.get(name) {
            if depth >= MAX_DEPTH {
                return Err(Error::Depth);
            }
            if f.params.len() != args.len() {
                return Err(Error::Arity {
                    name: name.to_string(),
                    expected: f.params.len().to_string(),
                    got: args.len(),
                });
            }
            let mut scope = Locals::new();
            for (p, a) in f.params.iter().zip(args) {
                scope.insert(p.clone(), self.eval(a, loc, depth)?);
            }
            return checked(self.eval(&f.body, &scope, depth + 1)?, false);
        }
        self.call_other(name, args, loc, depth)
    }

    #[inline(never)]
    fn call_other(&self, name: &str, args: &[Expr], loc: &Locals, depth: usize) -> Result<Value, Error> {
        if name == "if" && !self.is_variable(name, loc) {
            return self.eval_if(args, loc, depth);
        }
        if !self.is_variable(name, loc) {
            if let Some(r) = self.call_with_functions(name, args, loc, depth) {
                return r;
            }
        }
        if matches!(name, "rnd" | "runif" | "rnorm") && !self.is_variable(name, loc) {
            return self.random(name, args, loc, depth);
        }
        let vals = args
            .iter()
            .map(|a| self.eval(a, loc, depth))
            .collect::<Result<Vec<_>, _>>()?;
        let finite = vals.iter().all(Value::is_finite);
        if !self.is_variable(name, loc) {
            if let Some(io) = &self.io {
                if let Some(r) = io.0.call(name, &vals) {
                    return r;
                }
            }
            if let Some(r) = builtins::call(name, &vals, self.origin(loc)?) {
                return checked(r?, finite);
            }
        }
        // `x(y + 1)` is gewoon x · (y + 1) als x geen functie maar een waarde (of eenheid) is.
        if let Some(q) = self.lookup(name, loc) {
            if vals.len() == 1 {
                return checked(value::binop(BinOp::Mul, &q, &vals[0])?, finite);
            }
            return Err(Error::NotAFunction(name.to_string()));
        }
        Err(Error::Unknown(name.to_string()))
    }

    // -----------------------------------------------------------------------------------------
    // Bereiken: impliciete iteratie zoals in Mathcad

    /// Alle vrije namen, ook die in de bodies van aangeroepen eigen functies.
    fn deep_names(&self, exprs: &[&Expr]) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        for e in exprs {
            e.collect_names(&BTreeSet::new(), &mut out);
        }
        let mut seen_fns = BTreeSet::new();
        let mut todo: Vec<String> = out.iter().filter(|n| self.funcs.contains_key(*n)).cloned().collect();
        while let Some(f) = todo.pop() {
            if !seen_fns.insert(f.clone()) {
                continue;
            }
            let func = &self.funcs[&f];
            let bound: BTreeSet<String> = func.params.iter().cloned().collect();
            let mut inner = BTreeSet::new();
            func.body.collect_names(&bound, &mut inner);
            for n in inner {
                if self.funcs.contains_key(&n) {
                    todo.push(n.clone());
                }
                out.insert(n);
            }
        }
        out
    }

    /// De bereikvariabelen waarvan deze uitdrukkingen (direct of via een functie) afhangen.
    fn range_vars(&self, exprs: &[&Expr]) -> Vec<(String, RangeVal)> {
        self.deep_names(exprs)
            .into_iter()
            .filter_map(|n| match self.vars.get(&n) {
                Some(Value::Range(r)) => Some((n, *r)),
                _ => None,
            })
            .collect()
    }

    /// Roep `f` aan voor elke combinatie van waarden van de bereiken (de laatste loopt het snelst).
    fn iterate(
        &self,
        ranges: &[(String, RangeVal)],
        mut f: impl FnMut(&Locals, &[usize]) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let mut total: usize = 1;
        for (_, r) in ranges {
            total = total.checked_mul(r.n).filter(|&t| t <= MAX_ITER).ok_or_else(|| {
                dom(format!("te veel combinaties van bereikwaarden (hoogstens {MAX_ITER})"))
            })?;
        }
        let mut idx = vec![0usize; ranges.len()];
        let mut loc = Locals::new();
        for _ in 0..total {
            for (k, (name, r)) in ranges.iter().enumerate() {
                loc.insert(name.clone(), Value::Num(r.get(idx[k])));
            }
            f(&loc, &idx)?;
            for k in (0..idx.len()).rev() {
                idx[k] += 1;
                if idx[k] < ranges[k].1.n {
                    break;
                }
                idx[k] = 0;
            }
        }
        Ok(())
    }

    fn binding_label(ranges: &[(String, RangeVal)], loc: &Locals) -> String {
        ranges
            .iter()
            .map(|(n, _)| match &loc[n] {
                Value::Num(q) => format!("{n} = {}", format_quantity(*q)),
                _ => n.clone(),
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// De weergave-eenheid die uit een uitdrukking volgt (zie [`Display::Hint`]).
    fn hint_of(&self, e: &Expr) -> Option<String> {
        if let Some(h) = self.unit_chain_hint(e) {
            return h;
        }
        match e {
            Expr::Var(n) => {
                if self.vars.contains_key(n) {
                    self.hints.get(n).cloned()
                } else if lookup_offset_unit(n).is_some() {
                    Some(n.clone())
                } else {
                    unit_base(n).map(|u| u.name.to_string())
                }
            }
            Expr::Neg(x) => self.hint_of(x),
            Expr::Bin(BinOp::Add | BinOp::Sub, a, b) => merge_hints(self.hint_of(a), self.hint_of(b)),
            Expr::Bin(BinOp::Mul, a, b) => {
                if is_pure_number(a) {
                    self.hint_of(b)
                } else if is_pure_number(b) {
                    self.hint_of(a)
                } else {
                    None
                }
            }
            Expr::Bin(BinOp::Div, a, b) if is_pure_number(b) => self.hint_of(a),
            Expr::Call(f, args) if !self.funcs.contains_key(f) => match f.as_str() {
                "abs" | "min" | "max" | "hypot" | "mean" | "median" | "sum" => {
                    args.iter().map(|a| self.hint_of(a)).reduce(merge_hints).flatten()
                }
                _ => None,
            },
            Expr::Index(base, _) => self.hint_of(base),
            _ => None,
        }
    }

    /// Een getal maal samengestelde eenheden, zoals `3 kN/m` of `7850 kg/m^3`: de hint is de
    /// eenheid zoals getypt. `None` als `e` niet zo'n keten is (of maar één losse eenheid heeft;
    /// dat regelt [`Env::hint_of`] zelf), `Some(None)` als de automatische weergave al een eigen
    /// naam heeft (`N·m`, `Pa`, `W`) en dus beter is.
    fn unit_chain_hint(&self, e: &Expr) -> Option<Option<String>> {
        if !matches!(e, Expr::Bin(BinOp::Mul | BinOp::Div, ..)) {
            return None;
        }
        let mut factors = Vec::new();
        flatten_product(e, false, &mut factors);
        // exponent per eenheid, in volgorde van eerste voorkomen (m · m → m^2)
        let mut units: Vec<(String, f64)> = Vec::new();
        for (div, f) in factors {
            if is_pure_number(f) {
                continue;
            }
            let (name, p) = self.unit_factor(f)?;
            let p = if div { -p } else { p };
            match units.iter_mut().find(|(n, _)| *n == name) {
                Some((_, q)) => *q += p,
                None => units.push((name, p)),
            }
        }
        units.retain(|(_, p)| *p != 0.0);
        if units.len() == 1 && units[0].1 == 1.0 {
            return None;
        }
        let pow = |n: &str, p: f64| if p == 1.0 { n.to_string() } else { format!("{n}^{}", format_number(p)) };
        let num: Vec<String> = units.iter().filter(|u| u.1 > 0.0).map(|(n, p)| pow(n, *p)).collect();
        let den: Vec<String> = units.iter().filter(|u| u.1 < 0.0).map(|(n, p)| pow(n, -p)).collect();
        if num.is_empty() && den.is_empty() {
            return Some(None);
        }
        let num_text = if num.is_empty() { "1".to_string() } else { num.join("·") };
        let text = match den.len() {
            0 => num_text,
            1 => format!("{num_text}/{}", den[0]),
            _ => format!("{num_text}/({})", den.join("·")),
        };
        let q = unit_text_value(&text)?;
        if q.dim.is_none() || has_auto_name(q.dim) {
            return Some(None);
        }
        Some(Some(text))
    }

    /// `kN` of `m^3` als (eenheid, exponent), als dit een eenheid is (en geen variabele).
    fn unit_factor(&self, f: &Expr) -> Option<(String, f64)> {
        let unit = |e: &Expr| match e {
            Expr::Var(n) if !self.vars.contains_key(n) && lookup_unit(n).is_some() => Some(n.clone()),
            _ => None,
        };
        match f {
            Expr::Var(_) => Some((unit(f)?, 1.0)),
            Expr::Bin(BinOp::Pow, b, p) if is_pure_number(p) => {
                let v = self.eval_expr(p).ok()?.as_quantity("").ok()?.value;
                Some((unit(b)?, v))
            }
            _ => None,
        }
    }

    fn display_for(&self, expr: &Expr, unit: &Option<(Expr, String)>) -> Result<Option<Display>, Error> {
        let Some((ue, text)) = unit else {
            return Ok(self.hint_of(expr).map(Display::Hint));
        };
        if matches!(ue, Expr::Num(_)) {
            return Err(dom(
                "rechts van '=' staat een getal. Gebruik ':=' om een waarde toe te kennen, \
                 of zet er een eenheid neer (bijvoorbeeld '= kN')",
            ));
        }
        if let Expr::Var(n) = ue {
            if let Some((factor, offset)) = self.offset_unit(ue, &Locals::new()) {
                return Ok(Some(Display::Offset {
                    name: n.clone(),
                    factor,
                    offset,
                }));
            }
        }
        let u = match self.unit_quantity(ue) {
            Some(u) => u,
            None => match self.eval_expr(ue)? {
                Value::Num(u) => u,
                other => return Err(dom(format!("de weergave-eenheid moet een eenheid zijn, geen {}", other.kind()))),
            },
        };
        if u.value == 0.0 {
            return Err(dom("de weergave-eenheid heeft waarde 0"));
        }
        Ok(Some(Display::Unit {
            unit: u,
            text: text.clone(),
        }))
    }

    /// `x := ...` met een bereikvariabele rechts: een vector (één bereik) of matrix (twee bereiken).
    fn define_over_ranges(&self, body: &Expr, ranges: &[(String, RangeVal)]) -> Result<Value, Error> {
        if ranges.len() > 2 {
            return Err(dom(
                "een definitie over meer dan twee bereiken tegelijk kan niet (het resultaat zou meer dan 2-dimensionaal zijn)",
            ));
        }
        let rows = ranges[0].1.n;
        let cols = ranges.get(1).map_or(1, |r| r.1.n);
        let mut data = Vec::with_capacity(rows * cols);
        self.iterate(ranges, |loc, _| {
            match self.eval(body, loc, 0) {
                Ok(Value::Num(q)) => data.push(value::Complex::from_q(q)),
                Ok(Value::Complex(z)) => data.push(z),
                Ok(other) => {
                    return Err(dom(format!(
                        "bij {}: elk element moet een getal zijn, geen {}",
                        Self::binding_label(ranges, loc),
                        other.kind()
                    )))
                }
                Err(e) => return Err(dom(format!("bij {}: {e}", Self::binding_label(ranges, loc)))),
            }
            Ok(())
        })?;
        Ok(Value::Matrix(Matrix::from_complex(rows, cols, data)))
    }

    /// `v[i] := ...` of `M[i, j] := ...`, eventueel over bereiken. De vector of matrix groeit mee.
    fn define_elements(&self, name: &str, idx: &[Expr], body: &Expr) -> Result<Value, Error> {
        let mut exprs: Vec<&Expr> = vec![body];
        exprs.extend(idx.iter());
        let ranges = self.range_vars(&exprs);
        let origin = self.origin(&Locals::new())?;
        let mut m: Option<Matrix> = match self.vars.get(name) {
            Some(Value::Matrix(m)) => Some(m.clone()),
            _ => None,
        };
        self.iterate(&ranges, |loc, _| {
            let at = || Self::binding_label(&ranges, loc);
            let wrap = |e: Error| if ranges.is_empty() { e } else { dom(format!("bij {}: {e}", at())) };
            let ks = idx
                .iter()
                .map(|i| index_of(&self.eval(i, loc, 0)?, origin))
                .collect::<Result<Vec<_>, _>>()
                .map_err(wrap)?;
            let q = self.eval(body, loc, 0).map_err(wrap)?.as_complex("een element").map_err(wrap)?;
            set_element(&mut m, &ks, q).map_err(wrap)
        })?;
        Ok(Value::Matrix(m.expect("minstens één element gezet")))
    }

    /// `rnd(x)`, `runif(m, a, b)` en `rnorm(m, μ, σ)`.
    fn random(&self, name: &str, args: &[Expr], loc: &Locals, depth: usize) -> Result<Value, Error> {
        let vals = args
            .iter()
            .map(|a| self.eval(a, loc, depth)?.as_quantity("een argument"))
            .collect::<Result<Vec<_>, _>>()?;
        let want = if name == "rnd" { 1 } else { 3 };
        if vals.len() != want {
            return Err(Error::Arity {
                name: name.to_string(),
                expected: want.to_string(),
                got: vals.len(),
            });
        }
        if name == "rnd" {
            return Ok(Value::Num(Quantity::new(vals[0].value * next_random(&self.rng), vals[0].dim)));
        }
        let m = vals[0].dimensionless_value("het aantal")?;
        if m.fract() != 0.0 || !(1.0..=1e6).contains(&m) {
            return Err(dom(format!("{name}: het aantal moet een geheel getal van 1 tot en met 1000000 zijn")));
        }
        vals[1].same_dim(vals[2], "de parameters van de verdeling")?;
        let (p, q) = (vals[1].value, vals[2].value);
        let data = (0..m as usize)
            .map(|_| {
                let v = if name == "runif" {
                    p + (q - p) * next_random(&self.rng)
                } else {
                    // Box-Muller
                    let (u1, u2) = (1.0 - next_random(&self.rng), next_random(&self.rng));
                    p + q * (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
                };
                Quantity::new(v, vals[1].dim)
            })
            .collect();
        Ok(Value::Matrix(Matrix::column(data)))
    }

    /// Zaai de toevalsgenerator voor deze regel: uit de regel zelf en de variabele `SEED` (als die
    /// bestaat). Daardoor geeft herberekenen van één regel dezelfde getallen als het hele blad.
    fn seed_rng(&self, stmt: &Stmt) {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut feed = |bytes: &[u8]| {
            for b in bytes {
                h ^= *b as u64;
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
        };
        feed(format!("{stmt:?}").as_bytes());
        if let Some(Value::Num(q)) = self.vars.get("SEED") {
            feed(&q.value.to_le_bytes());
        }
        self.rng.set(h);
    }

    /// Een weergave-eenheid uitrekenen zoals Mathcad: achter `=` staat altijd een eenheid, dus een naam
    /// is daar eerst een eenheid, ook als er een variabele met die naam is (`a := F / m = m/s^2` met
    /// een massa `m`). Geeft `None` als de uitdrukking niet alleen uit eenheden en getallen bestaat.
    fn unit_quantity(&self, e: &Expr) -> Option<Quantity> {
        match e {
            Expr::Var(n) => lookup_unit(n).map(|(f, d)| Quantity::new(f, d)),
            Expr::Num(v) => Some(Quantity::scalar(*v)),
            Expr::Bin(op, a, b) => {
                let (a, b) = (self.unit_quantity(a)?, self.unit_quantity(b)?);
                match op {
                    BinOp::Mul => a.mul(b).ok(),
                    BinOp::Div => a.div(b).ok(),
                    BinOp::Pow => a.pow(b).ok(),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Hoe een definitie met `= [eenheid]` erachter getoond wordt: de waarde en de weergave.
    pub fn show_defined(&self, d: &Defined, unit: &Option<(Expr, String)>) -> Result<(Value, Option<Display>), Error> {
        let Defined::Var { value, hint, .. } = d else {
            return Err(dom("een functie kan niet meteen getoond worden"));
        };
        let display = match unit {
            None => hint.clone().map(Display::Hint),
            Some(_) => self.display_for(&Expr::Var(String::new()), unit)?,
        };
        Ok((value.clone(), display))
    }

    /// Voor de pijl →: vul bekende waarden in (getallen zonder eenheid) en schrijf eigen functies uit,
    /// zoals Mathcad. Namen in `keep` (de onbekende van solve) en namen met een eenheid blijven
    /// symbolen.
    fn symbolic_inline(&self, e: &Expr, keep: &[String], depth: usize) -> Result<Expr, Error> {
        if depth > 30 {
            return Err(dom("eigen functies roepen elkaar te diep aan om symbolisch uit te schrijven"));
        }
        let go = |x: &Expr| self.symbolic_inline(x, keep, depth);
        Ok(match e {
            Expr::Var(n) if !keep.contains(n) => match self.vars.get(n) {
                Some(Value::Num(q)) if q.dim.is_none() => Expr::Num(q.value),
                // met eenheid: getal maal eenheid (in de eenheid van de definitie), zoals Mathcad
                // met "units in symbolics"; de eenheden zijn daarna gewone symbolen
                Some(Value::Num(q)) => self.quantity_expr(*q, self.hints.get(n).map(String::as_str)).unwrap_or_else(|| e.clone()),
                _ => e.clone(),
            },
            Expr::Neg(x) => Expr::Neg(Box::new(go(x)?)),
            Expr::Bin(op, a, b) => Expr::Bin(*op, Box::new(go(a)?), Box::new(go(b)?)),
            Expr::Cmp(ops, xs) => Expr::Cmp(ops.clone(), xs.iter().map(&go).collect::<Result<_, _>>()?),
            Expr::Call(f, args) => {
                let args: Vec<Expr> = args.iter().map(&go).collect::<Result<_, _>>()?;
                match self.funcs.get(f) {
                    Some(uf) if uf.params.len() == args.len() && !matches!(uf.body, Expr::Block(_) | Expr::Interp(..)) => {
                        // eerst de parameters vervangen door tijdelijke namen, dan door de argumenten
                        let mut body = uf.body.clone();
                        for (k, p) in uf.params.iter().enumerate() {
                            body = crate::symbolic::substitute(&body, p, &Expr::Var(format!("\u{0}{k}")));
                        }
                        for (k, a) in args.iter().enumerate() {
                            body = crate::symbolic::substitute(&body, &format!("\u{0}{k}"), a);
                        }
                        self.symbolic_inline(&body, keep, depth + 1)?
                    }
                    _ => Expr::Call(f.clone(), args),
                }
            }
            Expr::Bind { op, body, var, args } => {
                let mut inner = keep.to_vec();
                inner.push(var.clone());
                Expr::Bind {
                    op: *op,
                    body: Box::new(self.symbolic_inline(body, &inner, depth)?),
                    var: var.clone(),
                    args: args.iter().map(&go).collect::<Result<_, _>>()?,
                }
            }
            _ => e.clone(),
        })
    }

    /// Een grootheid als `getal * eenheid` voor de pijl: in de eenheid van de definitie (`hint`), anders
    /// automatisch (`kN`, `N/m`). `None` bij eenheden met een nulpunt (°C) of als het niet lukt.
    fn quantity_expr(&self, q: Quantity, hint: Option<&str>) -> Option<Expr> {
        // een hint is een eenheidsfamilie (`N`: het voorvoegsel kiest de weergave) of een keten (`kN/m`)
        let text = match hint.map(|h| (h, rb_units::named_scale(h, q.value.abs(), q.dim))) {
            Some((_, Some((_, named)))) => named,
            Some((h, None)) if unit_text_value(h).is_some_and(|u| u.dim == q.dim) => h.to_string(),
            _ => rb_units::auto_scale(q.value.abs(), q.dim).1,
        };
        if text.is_empty() || text.contains('°') {
            return None;
        }
        let u = unit_text_value(&text).filter(|u| u.dim == q.dim && u.value != 0.0)?;
        // het getal vooraan in de keten (`(20 * kN) * m`), zodat het als `20 kN · m` getoond wordt
        fn lead(e: Expr, v: f64) -> Expr {
            match e {
                Expr::Bin(op @ (BinOp::Mul | BinOp::Div), a, b) => Expr::Bin(op, Box::new(lead(*a, v)), b),
                // `1/s`: het getal komt in plaats van de 1
                Expr::Num(one) if one == 1.0 => Expr::Num(v),
                unit => Expr::Bin(BinOp::Mul, Box::new(Expr::Num(v)), Box::new(unit)),
            }
        }
        Some(lead(crate::parser::parse_expr_str(&text).ok()?, q.value / u.value))
    }

    /// Staan er alleen nog getallen en eenheden in (`10 kN * 2 m`, `1 m + 50 cm`)? Dan is het één
    /// grootheid: uitrekenen en netjes tonen (`20 kN·m`, `1.5 m`).
    fn fold_units(&self, e: &Expr) -> Option<Expr> {
        let names = e.names();
        let is_unit = |n: &String| !self.vars.contains_key(n) && !self.funcs.contains_key(n) && lookup_unit(n).is_some();
        if names.is_empty() || !names.iter().all(is_unit) {
            return None;
        }
        match self.eval_expr(e).ok()? {
            Value::Num(q) if q.dim.is_none() => Some(Expr::Num(q.value)),
            Value::Num(q) => self.quantity_expr(q, None),
            _ => None,
        }
    }

    fn eval_symbolic(&self, expr: &Expr, op: &SymOp) -> Result<Expr, Error> {
        use crate::symbolic as sym;
        let keep: Vec<String> = match op {
            SymOp::Solve(x) | SymOp::Substitute(x, _) => vec![x.clone()],
            _ => Vec::new(),
        };
        let e = self.symbolic_inline(expr, &keep, 0)?;
        let r = match op {
            SymOp::Evaluate | SymOp::Simplify => sym::simplify(&e),
            SymOp::Expand => sym::expand(&e),
            SymOp::Factor => sym::factor(&e),
            SymOp::Solve(x) => {
                let (l, r) = match &e {
                    Expr::Cmp(ops, xs) if ops.as_slice() == [crate::ast::CmpOp::Eq] => (xs[0].clone(), xs[1].clone()),
                    other => (other.clone(), Expr::Num(0.0)),
                };
                sym::solve(&l, &r, x).map(|s| if s.len() == 1 { s[0].clone() } else { Expr::Matrix(s.into_iter().map(|v| vec![v]).collect()) })
            }
            SymOp::Substitute(x, v) => {
                let v = self.symbolic_inline(v, &[], 0)?;
                sym::simplify(&sym::substitute(&e, x, &v))
            }
        };
        let r = r.map_err(dom)?;
        if matches!(op, SymOp::Evaluate | SymOp::Simplify | SymOp::Substitute(..) | SymOp::Solve(_)) {
            if let Some(q) = self.fold_units(&r) {
                return Ok(q);
            }
        }
        Ok(r)
    }

    /// Voer één geparseerde regel uit zonder de omgeving te wijzigen.
    pub fn eval_stmt(&self, stmt: &Stmt) -> Result<Outcome, Error> {
        self.seed_rng(stmt);
        match stmt {
            Stmt::Symbolic { expr, op, lhs_src } => Ok(Outcome::Symbolic { lhs_src: lhs_src.clone(), result: self.eval_symbolic(expr, op)? }),
            Stmt::Define {
                name,
                params: Some(p),
                body,
                ..
            } => Ok(Outcome::Defined(Defined::Fn {
                name: name.clone(),
                func: UserFn {
                    params: p.clone(),
                    body: body.clone(),
                },
            })),
            Stmt::Define {
                name,
                index: Some(idx),
                body,
                ..
            } => Ok(Outcome::Defined(Defined::Var {
                name: name.clone(),
                value: self.define_elements(name, idx, body)?,
                hint: None,
            })),
            Stmt::Define {
                name,
                body: Expr::Bind { op: BindOp::OdeSolve, var, args, body },
                ..
            } => {
                let names = crate::ast::ode_names(body).unwrap_or_else(|| vec![name.clone()]);
                let funcs = self.odesolve(&names, var, &args[0])?;
                let mut defs: Vec<Defined> = names.into_iter().zip(funcs).map(|(name, func)| Defined::Fn { name, func }).collect();
                Ok(if defs.len() == 1 { Outcome::Defined(defs.remove(0)) } else { Outcome::DefinedAll(defs) })
            }
            Stmt::Define { name, body, .. } => {
                let ranges = self.range_vars(&[body]);
                let (value, hint) = if ranges.is_empty() || matches!(body, Expr::Range { .. }) {
                    (self.eval_expr(body)?, self.hint_of(body))
                } else {
                    (self.define_over_ranges(body, &ranges)?, None)
                };
                Ok(Outcome::Defined(Defined::Var {
                    name: name.clone(),
                    value,
                    hint,
                }))
            }
            Stmt::Eval {
                expr,
                lhs_src,
                unit,
                bare,
            } => {
                let display = self.display_for(expr, unit)?;
                let ranges = self.range_vars(&[expr]);
                let whole_range = matches!(expr, Expr::Var(n) if ranges.len() == 1 && ranges[0].0 == *n);
                if ranges.is_empty() || whole_range {
                    return Ok(Outcome::Value {
                        value: self.eval_expr(expr)?,
                        lhs_src: lhs_src.clone(),
                        display,
                        bare: *bare,
                    });
                }
                let mut rows = Vec::new();
                let total: usize = ranges.iter().map(|(_, r)| r.n).product();
                if total > MAX_ROWS {
                    return Err(dom(format!(
                        "dit resultaat heeft {total} rijen; hoogstens {MAX_ROWS} kunnen getoond worden. \
                         Maak het bereik kleiner, of definieer er een vector mee (y := ...)"
                    )));
                }
                self.iterate(&ranges, |loc, _| {
                    let right = match self.eval(expr, loc, 0).and_then(|v| render_value(&v, &display)) {
                        Ok(t) => prefix_lines(&format!("{lhs_src} = "), &t),
                        Err(e) => format!("✗ {e}"),
                    };
                    rows.push((Self::binding_label(&ranges, loc), right));
                    Ok(())
                })?;
                Ok(Outcome::Table {
                    lhs_src: lhs_src.clone(),
                    rows: format_rows(rows),
                    bare: *bare,
                })
            }
        }
    }

    /// Voer één regel tekst uit (REPL-stijl): definities worden doorgevoerd, resultaten als tekst teruggegeven.
    pub fn run_line(&mut self, line: &str) -> Result<Option<String>, Error> {
        let stmt = parse_stmt(line, true)?;
        match self.eval_stmt(&stmt)? {
            Outcome::Defined(d) => {
                self.apply(d);
                Ok(None)
            }
            Outcome::DefinedAll(ds) => {
                for d in ds {
                    self.apply(d);
                }
                Ok(None)
            }
            Outcome::Value {
                value,
                lhs_src,
                display,
                bare,
            } => {
                let text = render_value(&value, &display)?;
                if bare {
                    Ok(Some(text))
                } else {
                    Ok(Some(prefix_lines(&format!("{lhs_src} = "), &text)))
                }
            }
            Outcome::Table { rows, .. } => Ok(Some(rows.join("\n"))),
            Outcome::Symbolic { lhs_src, result } => Ok(Some(format!("{lhs_src} → {}", crate::symbolic::print(&result)))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(lines: &[&str]) -> Vec<String> {
        crate::with_stack(|| {
            let mut env = Env::new();
            let mut out = Vec::new();
            for l in lines {
                match env.run_line(l) {
                    Ok(Some(t)) => out.push(t),
                    Ok(None) => {}
                    Err(e) => out.push(format!("FOUT: {e}")),
                }
            }
            out
        })
    }

    fn one(line: &str) -> String {
        run(&[line]).remove(0)
    }

    fn last(lines: &[&str]) -> String {
        run(lines).pop().unwrap()
    }

    #[test]
    fn rekenen_met_voorrang() {
        assert_eq!(one("1 + 2 * 3"), "7");
        assert_eq!(one("2^3^2"), "512");
        assert_eq!(one("-2^2"), "-4");
        assert_eq!(one("(1 + 2) * 3"), "9");
        assert_eq!(one("10 / 4"), "2.5");
    }

    #[test]
    fn eenheden_en_weergave() {
        assert_eq!(one("5 kN + 300 N"), "5.3 kN");
        assert_eq!(one("36 km/hr = m/s"), "36 km/hr = 10 m/s");
        assert_eq!(one("2 m * 3 m"), "6 m^2");
        assert_eq!(one("100 N * 2 m"), "200 N·m");
        assert_eq!(one("1500 N * 1 m = kN*m"), "1500 N * 1 m = 1.5 kN*m");
        assert_eq!(one("sin(30 deg)"), "0.5");
        assert_eq!(one("1 hp = kW"), "1 hp = 0.7457 kW");
    }

    #[test]
    fn dimensie_fouten() {
        assert!(one("1 m + 1 s").starts_with("FOUT: eenheden komen niet overeen bij optellen"));
        assert!(one("sin(2 m)").starts_with("FOUT: het argument van sin moet dimensieloos zijn"));
        assert!(one("1 m = s").contains("weergeven in deze eenheid"));
        assert!(one("5 m = 5").contains("':='"));
    }

    #[test]
    fn variabelen_en_functies() {
        let out = run(&[
            "a := 3 m",
            "b := 4 m",
            "c := hypot(a, b)",
            "c =",
            "f(x, y) := x^2 + y",
            "f(3, 1) =",
            "f(2 m, 1 m^2) =",
        ]);
        assert_eq!(out, vec!["c = 5 m", "f(3, 1) = 10", "f(2 m, 1 m^2) = 5 m^2"]);
    }

    #[test]
    fn variabele_gaat_voor_eenheid() {
        let out = run(&["m := 5", "m * 2"]);
        assert_eq!(out, vec!["10"]);
    }

    #[test]
    fn impliciet_vermenigvuldigen_met_haakjes() {
        let out = run(&["x := 3", "x(2 + 1)", "2(x + 1)", "2 pi"]);
        assert_eq!(out, vec!["9", "8", "6.28319"]);
    }

    #[test]
    fn fouten_zijn_duidelijk() {
        assert!(one("1 / 0").contains("deling door nul"));
        assert!(one("ln(0)").contains("bestaat niet"));
        assert!(one("log(-1)").contains("groter dan 0"));
        assert!(one("foo + 1").contains("onbekende naam 'foo'"));
        assert!(one("sin(1, 2)").contains("verwacht 1 argument"));
        assert!(one("sqrt(4 m)").contains("machtswortel"));
        assert!(one("exp(1000)").contains("overflow"));
    }

    #[test]
    fn recursie_loopt_niet_vast() {
        let out = run(&["f(x) := f(x)", "f(1)"]);
        assert!(out[0].contains("te diep"), "{out:?}");
        // echte recursie via if, tot vlak onder de grens
        let out = run(&[
            "fac(n) := if(n <= 1, 1, n * fac(n - 1))",
            "fac(10) =",
            "diep(n) := if(n == 0, 0, 1 + diep(n - 1))",
            "diep(199) =",
            "diep(250) =",
        ]);
        assert_eq!(out[0], "fac(10) = 3628800");
        assert_eq!(out[1], "diep(199) = 199");
        assert!(out[2].contains("te diep"), "{out:?}");
    }

    #[test]
    fn machten_en_wortels_met_eenheden() {
        assert_eq!(one("sqrt(9 m^2)"), "3 m");
        assert_eq!(one("(4 m^2)^0.5"), "2 m");
        assert_eq!(one("root(27 m^3, 3)"), "3 m");
        assert!(one("(4 m)^0.5").starts_with("FOUT"));
    }

    // --- vergelijkingen, logica en if

    #[test]
    fn vergelijkingen_geven_1_of_0() {
        assert_eq!(one("3 < 4"), "1");
        assert_eq!(one("3 m > 4 m"), "0");
        assert_eq!(one("0.1 + 0.2 == 0.3"), "1");
        assert_eq!(one("1 != 1"), "0");
        assert_eq!(one("0 < 0.5 ≤ 1"), "1");
        assert_eq!(one("0 < 1.5 ≤ 1"), "0");
        assert_eq!(one("1 < 2 and 2 < 1"), "0");
        assert_eq!(one("1 < 2 or 2 < 1"), "1");
        assert_eq!(one("not 1 > 2"), "1");
        assert!(one("1 m < 2 s").contains("vergelijken"));
        // een kale 0 past bij elke eenheid; een andere kale waarde niet
        assert_eq!(one("0 < 5 mm < 1 m"), "1");
        assert!(one("1 < 5 mm").contains("vergelijken"));
        assert!(one("not 2 m").contains("dimensieloos"));
    }

    #[test]
    fn if_en_stuksgewijze_functies() {
        let out = run(&[
            "f(x) := if(x < 0, 0, x^2)",
            "f(-2) =",
            "f(3) =",
            // meerdere takken
            "g(x) := if(x < 0, -1, x == 0, 0, 1)",
            "g(-5) =",
            "g(0) =",
            "g(7) =",
            // alleen de gekozen tak wordt uitgerekend
            "h(x) := if(x == 0, 1, sin(x)/x)",
            "h(0) =",
        ]);
        assert_eq!(out, vec!["f(-2) = 0", "f(3) = 9", "g(-5) = -1", "g(0) = 0", "g(7) = 1", "h(0) = 1"]);
        assert!(one("if(1, 2)").contains("verwacht 3, 5, 7"));
    }

    // --- complexe getallen

    #[test]
    fn complexe_getallen() {
        assert_eq!(one("sqrt(-4)"), "2i");
        assert_eq!(one("(1 + 2i) * (3 - 1i)"), "5 + 5i");
        assert_eq!(one("abs(3 + 4i)"), "5");
        assert_eq!(one("1i^2"), "-1");
        assert_eq!(one("(2 + 1i) m"), "(2 + 1i) m");
        assert_eq!(one("Re(3 - 4j)"), "3");
        assert_eq!(one("Im(3 - 4j)"), "-4");
        assert_eq!(one("conj(3 - 4j)"), "3 + 4i");
        assert_eq!(one("arg(1i) = deg"), "arg(1i) = 90 deg");
        assert_eq!(one("ln(-1)"), "3.14159i");
        assert_eq!(one("e^(1i pi)"), "-1");
        assert_eq!(one("(-8)^(1/3)"), "1 + 1.73205i");
        assert_eq!(one("sqrt(-9 m^2)"), "3i m");
        assert!(one("1i < 2").contains("ordenen"));
    }

    // --- bereiken en impliciete iteratie

    #[test]
    fn bereiken_tonen_en_itereren() {
        assert_eq!(last(&["i := 1..4", "i ="]), "i = 1, 2, 3, 4");
        assert_eq!(last(&["x := 0, 0.25 .. 1", "x ="]), "x = 0, 0.25, 0.5, 0.75, 1");
        assert_eq!(last(&["k := 3..1", "k ="]), "k = 3, 2, 1");
        assert_eq!(
            last(&["n := 0..100", "n ="]),
            "n = 0, 1, 2, …, 99, 100  (101 waarden)"
        );
        let t = last(&["x := 0 m, 1 m .. 2 m", "x^2 ="]);
        assert_eq!(
            t.lines().collect::<Vec<_>>(),
            vec!["x = 0 m  →  x^2 = 0 m^2", "x = 1 m  →  x^2 = 1 m^2", "x = 2 m  →  x^2 = 4 m^2"]
        );
    }

    #[test]
    fn bereik_via_functie_en_fouten_per_rij() {
        let t = last(&["x := 0..2", "f(t) := 1 / t", "f(x) ="]);
        let rows: Vec<&str> = t.lines().collect();
        assert!(rows[0].contains("✗ deling door nul"), "{t}");
        assert!(rows[2].ends_with("f(x) = 0.5"), "{t}");
    }

    #[test]
    fn definitie_over_bereik_geeft_vector_of_matrix() {
        assert_eq!(last(&["i := 0..3", "v := i^2", "v ="]), "v = ⎡ 0 ⎤\n    ⎢ 1 ⎥\n    ⎢ 4 ⎥\n    ⎣ 9 ⎦");
        assert_eq!(
            last(&["i := 0..1", "j := 0..2", "T := 10 i + j", "T ="]),
            "T = ⎡  0   1   2 ⎤\n    ⎣ 10  11  12 ⎦"
        );
    }

    #[test]
    fn elementdefinitie_zoals_in_mathcad() {
        // v[i] := ... vult een vector, M[i, j] := ... een matrix
        assert_eq!(last(&["i := 0..2", "v[i] := 2 i", "v[2] ="]), "v[2] = 4");
        assert_eq!(
            last(&["i := 0..1", "j := 0..1", "M[i, j] := δ(i, j) + 1", "M ="]),
            "M = ⎡ 2  1 ⎤\n    ⎣ 1  2 ⎦"
        );
        // losse elementen, de vector groeit en vult aan met nullen
        assert_eq!(last(&["w[3] := 5 m", "w ="]), "w = ⎡ 0 ⎤\n    ⎢ 0 ⎥ m\n    ⎢ 0 ⎥\n    ⎣ 5 ⎦");
        // ORIGIN := 1
        assert_eq!(last(&["ORIGIN := 1", "u := [10, 20, 30]", "u[1] ="]), "u[1] = 10");
        assert!(last(&["u := [10, 20, 30]", "u[3] ="]).contains("buiten de vector"));
    }

    // --- vectoren en matrices

    #[test]
    fn matrices_rekenen() {
        assert_eq!(one("[1, 2; 3, 4] * [5; 6]"), "⎡ 17 ⎤\n⎣ 39 ⎦");
        assert_eq!(one("[1, 2, 3] * [4; 5; 6]"), "32");
        assert_eq!(one("[1; 2; 3] * [4; 5; 6]"), "32");
        assert_eq!(one("[1, 2] + [3, 4]"), "[4, 6]");
        assert_eq!(one("2 * [1, 2] m"), "[2, 4] m");
        assert_eq!(one("det([2, 1; 1, 3])"), "5");
        assert_eq!(one("[2, 1; 1, 3]^-1 * [2, 1; 1, 3]"), "⎡ 1  0 ⎤\n⎣ 0  1 ⎦");
        assert_eq!(one("transpose([1, 2; 3, 4])"), "⎡ 1  3 ⎤\n⎣ 2  4 ⎦");
        assert_eq!(one("cross([1, 0, 0], [0, 1, 0])"), "[0, 0, 1]");
        assert_eq!(one("abs([3, 4] m)"), "5 m");
        assert_eq!(one("sqrt([4, 9])"), "[2, 3]");
        assert_eq!(one("max([3, 9, 2], 4)"), "9");
        assert_eq!(one("mean([1, 2, 3, 4])"), "2.5");
        assert_eq!(one("rows([1; 2; 3])"), "3");
        assert!(one("[1, 2] + [1, 2, 3]").contains("verschillende grootte"));
        assert!(one("[1, 2] / [1, 2]").contains("inverse"));
        assert!(one("[1, 2; 2, 4]^-1").contains("singulier"));
    }

    #[test]
    fn stelsel_met_eenheden_oplossen() {
        // veersysteem: K·u = F met K in N/mm en F in kN
        let out = run(&[
            "K := [2, -1; -1, 2] N/mm",
            "F := [0; 1] kN",
            "u := lsolve(K, F)",
            "u = mm",
            "det(K) = (N/mm)^2",
            "K^-1 * F = mm",
        ]);
        assert_eq!(
            out,
            vec![
                "u = ⎡ 333.333 ⎤ mm\n    ⎣ 666.667 ⎦",
                "det(K) = 3 (N/mm)^2",
                "K^-1 * F = ⎡ 333.333 ⎤ mm\n           ⎣ 666.667 ⎦"
            ]
        );
    }

    #[test]
    fn complexe_matrices() {
        assert_eq!(one("[1, 2i; 3, 4] * [1; 1i]"), "⎡     -1 ⎤\n⎣ 3 + 4i ⎦");
        assert_eq!(one("det([1, 1i; 1i, 1])"), "2");
        assert_eq!(one("[1, 1i; 1i, 1]^-1 * [1, 1i; 1i, 1]"), "⎡ 1  0 ⎤\n⎣ 0  1 ⎦");
        assert_eq!(one("conj([1 + 1i, 2])"), "[1 - 1i, 2]");
        assert_eq!(one("abs([3i, 4])"), "5");
        assert_eq!(last(&["v := [1, 1i]", "v[1] ="]), "v[1] = 1i");
        assert_eq!(last(&["w[1] := 2i", "w ="]), "w = ⎡  0 ⎤\n    ⎣ 2i ⎦");
        assert!(one("sqrt([1i, 4])").contains("complexe matrix"));
    }

    #[test]
    fn eigenwaarden_en_ontbindingen() {
        assert_eq!(one("eigenvals([2, 1; 1, 2])"), "⎡ 1 ⎤\n⎣ 3 ⎦");
        assert_eq!(one("eigenvals([0, -1; 1, 0])"), "⎡ -1i ⎤\n⎣  1i ⎦");
        assert_eq!(
            last(&["A := [4, 1, 0; 1, 3, 1; 0, 1, 2]", "V := eigenvecs(A)", "norm(A * V - V * diag(eigenvals(A))) < 1e-12 ="]),
            "norm(A * V - V * diag(eigenvals(A))) < 1e-12 = 1"
        );
        assert_eq!(one("svds([3, 0; 0, 4])"), "⎡ 4 ⎤\n⎣ 3 ⎦");
        assert_eq!(one("rank([1, 2; 2, 4])"), "1");
        // L = [2, 0; 1, √2], dus L² = [4, 0; 2 + √2, 2]
        assert_eq!(one("cholesky([4, 2; 2, 3])^2"), "⎡       4  0 ⎤\n⎣ 3.41421  2 ⎦");
        assert_eq!(one("polyroots([-6, 11, -6, 1])"), "⎡ 1 ⎤\n⎢ 2 ⎥\n⎣ 3 ⎦");
        assert_eq!(one("polyroots([1, 0, 1])"), "⎡ -1i ⎤\n⎣  1i ⎦");
        assert_eq!(one("polyroots([-4 m^2, 0, 1])"), "⎡ -2 ⎤ m\n⎣  2 ⎦");
    }

    #[test]
    fn eigenfrequenties_van_twee_massas_met_eenheden() {
        // ω² = k/m · (3 ∓ √5)/2 = 190.983 en 1309.02 1/s²; f = ω/2π = 2.19947 en 5.75828 Hz
        let out = run(&[
            "k := 1000 N/m",
            "m.1 := 2 kg",
            "K := [2 k, -k; -k, k]",
            "M := [m.1, 0 kg; 0 kg, m.1]",
            "λ := eigenvals(M^-1 * K)",
            "λ = 1/s^2",
            "sqrt(λ) / (2 π) = Hz",
        ]);
        assert_eq!(
            out,
            vec![
                "λ = ⎡ 190.983 ⎤ 1/s^2\n    ⎣ 1309.02 ⎦",
                "sqrt(λ) / (2 π) = ⎡ 2.19947 ⎤ Hz\n                  ⎣ 5.75828 ⎦"
            ]
        );
    }

    #[test]
    fn sorteren_en_opzoeken() {
        assert_eq!(one("csort([3, 1; 1, 2; 2, 3], 0)"), "⎡ 1  2 ⎤\n⎢ 2  3 ⎥\n⎣ 3  1 ⎦");
        assert_eq!(one("rsort([3, 1, 2; 9, 8, 7], 0)"), "⎡ 1  2  3 ⎤\n⎣ 8  7  9 ⎦");
        assert_eq!(one("vlookup(2, [1, 10; 2, 20; 3, 30], 1)"), "[20]");
        assert_eq!(one("match(20, [10, 20, 30])"), "[1]");
        assert_eq!(one("lookup(2, [1, 2, 3], [10, 20, 30] N)"), "[20] N");
        assert!(one("match(5, [1, 2])").contains("komt niet voor"));
    }

    #[test]
    fn speciale_functies_en_verdelingen() {
        // referentiewaarden uit Abramowitz & Stegun en standaardtabellen
        assert_eq!(one("erf(1)"), "0.842701");
        assert_eq!(one("Γ(5)"), "24");
        assert_eq!(one("Γ(0.5)^2"), "3.14159");
        assert_eq!(one("J0(1)"), "0.765198");
        assert_eq!(one("Jn(2, 3)"), "0.486091");
        assert_eq!(one("K1(1)"), "0.601907");
        assert_eq!(one("J0([0, 1])"), "[1, 0.765198]");
        assert_eq!(one("pnorm(1.96)"), "0.975002");
        assert_eq!(one("qt(0.975, 10)"), "2.22814");
        assert_eq!(one("qchisq(0.95, 2)"), "5.99146");
        assert_eq!(one("pbinom(3, 10, 0.5)"), "0.171875");
        // eenheden: dichtheid in 1/m, kwantiel in mm
        assert_eq!(one("dnorm(0 m, 0 m, 2 m)"), "0.199471 1/m");
        assert_eq!(one("qnorm(0.975, 10 mm, 2 mm)"), "13.9199 mm");
        assert_eq!(one("pexp(10 s, 0.1 / s)"), "0.632121");
        assert!(one("qnorm(1.5)").contains("tussen 0 en 1"));
        assert!(one("dnorm(1 m, 0 s, 1 s)").contains("eenheden"));
    }

    #[test]
    fn statistiek_en_toeval() {
        assert_eq!(one("corr([1, 2, 3], [3, 2, 1])"), "-1");
        assert_eq!(one("cvar([1, 2, 3], [2, 4, 6] m)"), "1.33333 m");
        assert_eq!(one("percentile([1, 2, 3, 4], 0.5)"), "2.5");
        assert_eq!(one("hist(2, [1, 2, 3, 4])"), "⎡ 2 ⎤\n⎣ 2 ⎦");
        assert_eq!(one("hist([0, 10, 20] mm, [5, 15, 15, 25] mm)"), "⎡ 1 ⎤\n⎣ 2 ⎦");
        assert_eq!(one("abs(mean(runif(10000, 0, 1)) - 0.5) < 0.02"), "1");
        assert_eq!(one("abs(Stdev(rnorm(10000, 0 m, 2 m)) - 2 m) < 0.1 m"), "1");
        // dezelfde regel geeft altijd dezelfde getallen
        let a = run(&["x := rnd(1)", "x ="]);
        let b = run(&["y := 2", "x := rnd(1)", "x ="]);
        assert_eq!(a, b);
        let c = run(&["SEED := 7", "x := rnd(1)", "x ="]);
        assert_ne!(a, c);
    }

    #[test]
    fn root_zoals_in_mathcad() {
        assert_eq!(one("root(x^2 - 2, x, 0, 2)"), "1.41421");
        assert_eq!(last(&["x := 1", "root(cos(x) - x, x) ="]), "root(cos(x) - x, x) = 0.739085");
        assert_eq!(one("root(x^2 - 2 m^2, x, 0 m, 2 m)"), "1.41421 m");
        // knikbelasting: kleinste wortel van tan(z) = z voorbij π (4.49341)
        assert_eq!(one("root(tan(z) - z, z, 4, 4.6)"), "4.49341");
        assert!(one("root(x^2 + 1, x, 0, 2)").contains("hetzelfde teken"));
        assert!(one("root(q^2 - 2, q)").contains("startwaarde"));
        // met een getal als tweede argument blijft het de n-de machtswortel
        assert_eq!(one("root(27 m^3, 3)"), "3 m");
    }

    #[test]
    fn regressie_met_eigen_functies() {
        let out = run(&[
            "F(x) := [1; x^2]",
            "linfit([0, 1, 2, 3], [2, 5, 14, 29], F) =",
            // vrije val: y = y0 + ½ g t² met eenheden
            "G(t) := [1; t^2]",
            "c := linfit([0, 1, 2, 3] s, [5, 9.905, 24.62, 49.145] m, G)",
            "c[1] = m/s^2",
            "M(x, u) := u[0] * exp(-u[1] * x)",
            "genfit([0, 1, 2, 3, 4], [5, 5 exp(-0.5), 5 exp(-1), 5 exp(-1.5), 5 exp(-2)], [1; 1], M) =",
        ]);
        assert_eq!(out[0], "linfit([0, 1, 2, 3], [2, 5, 14, 29], F) = ⎡ 2 ⎤\n                                          ⎣ 3 ⎦");
        assert_eq!(out[1], "c[1] = 4.905 m/s^2");
        assert!(out[2].contains("⎡   5 ⎤") && out[2].contains("⎣ 0.5 ⎦"), "{}", out[2]);
    }

    #[test]
    fn differentiaalvergelijkingen_met_rkfixed() {
        let out = run(&[
            "D(x, y) := -y",
            "rkfixed(1, 0, 1, 100, D)[100, 1] =",
            // remming: v' = -v / τ met eenheden
            "R(t, v) := -v / (2 s)",
            "rkfixed(10 m/s, 0 s, 2 s, 50, R)[50, 1] =",
            // harmonische oscillator als stelsel
            "H(t, y) := [y[1]; -y[0]]",
            "Rkadapt([1; 0], 0, π, 10, H)[10, 1] =",
        ]);
        assert_eq!(
            out,
            vec![
                "rkfixed(1, 0, 1, 100, D)[100, 1] = 0.367879",
                "rkfixed(10 m/s, 0 s, 2 s, 50, R)[50, 1] = 3.67879 m/s",
                "Rkadapt([1; 0], 0, π, 10, H)[10, 1] = -1"
            ]
        );
        assert!(last(&["E(x, y) := [y; y]", "rkfixed(1, 0, 1, 10, E) ="]).contains("evenveel componenten"));
    }

    // --- sommen, integralen en afgeleiden

    #[test]
    fn som_en_product() {
        assert_eq!(one("sum(i^2, i, 1, 10)"), "385");
        assert_eq!(one("Σ(1/k^2, k, 1, 1000) = "), "Σ(1/k^2, k, 1, 1000) = 1.64393");
        assert_eq!(one("prod(k, k, 1, 5)"), "120");
        assert_eq!(one("5!"), "120");
        assert_eq!(one("sum(i, i, 5, 1)"), "0");
        assert_eq!(one("sum([1, 2, 3] m)"), "6 m");
        // de gebonden i botst niet met een bereik i
        assert_eq!(last(&["i := 1..3", "sum(i, i, 1, 4) ="]), "sum(i, i, 1, 4) = 10");
    }

    #[test]
    fn integralen_met_eenheden() {
        assert_eq!(one("integral(x^2, x, 0, 3)"), "9");
        assert_eq!(one("∫(exp(-x^2), x, -∞, ∞)^2"), "3.14159");
        // arbeid van een veer: ∫ k x dx van 0 tot 20 mm met k = 5 N/mm → 1 N·m
        assert_eq!(
            last(&["k := 5 N/mm", "W := integral(k x, x, 0 mm, 20 mm)", "W = J"]),
            "W = 1 J"
        );
        // traagheidsmoment van een rechthoek: ∫ b y² dy = b h³/12
        assert_eq!(
            last(&["b := 50 mm", "h := 120 mm", "integral(b y^2, y, -h/2, h/2) = cm^4"]),
            "integral(b y^2, y, -h/2, h/2) = 720 cm^4"
        );
        assert!(one("integral(x, x, 0 m, 1 s)").contains("integratiegrenzen"));
    }

    #[test]
    fn afgeleiden_met_eenheden() {
        assert_eq!(one("deriv(x^3, x, 2)"), "12");
        assert_eq!(one("deriv(sin(x), x, 0)"), "1");
        assert_eq!(one("deriv(x^3, x, 2, 2)"), "12");
        // snelheid uit een positie: s(t) = 5 m/s² · t² → v(2 s) = 20 m/s
        assert_eq!(last(&["s(t) := 5 m/s^2 * t^2", "deriv(s(t), t, 2 s) ="]), "deriv(s(t), t, 2 s) = 20 m/s");
    }

    // --- programmablokken

    #[test]
    fn programmablok_met_lus_en_continue() {
        let out = run(&[
            "som(n) := {\n    s := 0\n    for k in 1..n\n        if mod(k, 2) == 0\n            continue\n        s := s + k\n    return s\n}",
            "som(10) =",
        ]);
        assert_eq!(out, vec!["som(10) = 25"]);
    }

    #[test]
    fn programmablok_while_en_laatste_regel() {
        let out = run(&[
            "wortel(a) := {\n  x := a\n  while abs(x^2 - a) > 1e-12 * a\n    x := (x + a/x) / 2\n  x\n}",
            "wortel(2) =",
            "fib(n) := {\n  a := 0\n  b := 1\n  for k in 1..n\n    c := a + b\n    a := b\n    b := c\n  a\n}",
            "fib(10) =",
        ]);
        assert_eq!(out, vec!["wortel(2) = 1.41421", "fib(10) = 55"]);
    }

    #[test]
    fn programmablok_return_uit_lus_en_else_if() {
        let out = run(&[
            "eerste(v) := {\n  for k in 0..length(v) - 1\n    if v[k] < 0\n      return k\n  -1\n}",
            "eerste([3, 4, -2, 5]) =",
            "eerste([3, 4]) =",
            "teken(x) := {\n  if x < 0\n    r := -1\n  else if x == 0\n    r := 0\n  else\n    r := 1\n  r\n}",
            "teken(-3) =",
            "teken(0) =",
            "teken(8) =",
            // `if(...)` aan het begin van een regel zonder ingesprongen regel eronder is de functie
            "g(x) := {\n  if(x > 0, x, -x)\n}",
            "g(-4) =",
        ]);
        assert_eq!(
            out,
            vec!["eerste([3, 4, -2, 5]) = 2", "eerste([3, 4]) = -1", "teken(-3) = -1", "teken(0) = 0", "teken(8) = 1", "g(-4) = 4"]
        );
    }

    #[test]
    fn programmablok_lokale_vector_eenheden_en_scope() {
        let out = run(&[
            "kwadraten(n) := {\n  for k in 0..n-1\n    w[k] := k^2\n  w\n}",
            "kwadraten(3) =",
            "arbeid(F, s) := {\n  W ← F * s\n  W\n}",
            "arbeid(2 kN, 3 m) =",
            "t := 5",
            "f(x) := {\n  t := x * 2\n  t\n}",
            "f(1) =",
            "t =",
            "x := {\n  a := 2  // commentaar mag\n\n  a^2\n}",
            "x =",
        ]);
        assert_eq!(
            out,
            vec![
                "kwadraten(3) = ⎡ 0 ⎤\n               ⎢ 1 ⎥\n               ⎣ 4 ⎦",
                "arbeid(2 kN, 3 m) = 6 kN·m",
                "f(1) = 2",
                "t = 5",
                "x = 4"
            ]
        );
    }

    #[test]
    fn programmablok_fouten() {
        assert!(one("f(x) := {\n  else\n    1\n}").contains("'else' zonder"));
        assert!(one("f(x) := {\n  a := 1\n      b := 2\n}").contains("springt verder in"));
        assert!(one("f(x) := {\n  for k in 1..3\n}").contains("ingesprongen regel"));
        assert!(one("f(x) := {\n  a := 1\n  a").contains("niet afgesloten"));
        assert!(one("f(x) := {\n  g(y) := y\n}").contains("buiten het blok"));
        assert!(last(&["f(x) := {\n  for k in 1..3\n    k\n  break\n}", "f(1) ="]).contains("buiten een lus"));
    }

    // --- temperaturen en weergave-eenheden

    #[test]
    fn temperaturen() {
        assert_eq!(one("20 °C = K"), "20 °C = 293.15 K");
        assert_eq!(one("293.15 K = °C"), "293.15 K = 20 °C");
        assert_eq!(one("98.6 °F = °C"), "98.6 °F = 37 °C");
        assert_eq!(one("-40 °C = °F"), "-40 °C = -40 °F");
        assert_eq!(last(&["T := 20 °C", "T ="]), "T = 20 °C");
        assert_eq!(last(&["T := 20 °C", "T + 5 Δ°C ="]), "T + 5 Δ°C = 25 °C");
        assert_eq!(last(&["T.1 := 20 °C", "T.2 := 80 °C", "T.2 - T.1 = Δ°C"]), "T.2 - T.1 = 60 Δ°C");
        assert!(one("°C * 2").contains("nulpunt"));
        // 2 * °C is hetzelfde als 2 °C: een temperatuur
        assert_eq!(one("2 * °C = K"), "2 * °C = 275.15 K");
        assert!(one("20 m °C").contains("getal zonder eenheid"));
    }

    #[test]
    fn weergave_eenheid_volgt_de_definitie() {
        assert_eq!(last(&["E := 5 J", "E ="]), "E = 5 J");
        assert_eq!(last(&["E := 5000 J", "E ="]), "E = 5 kJ");
        assert_eq!(last(&["E := 5 J", "2 E + 1 J ="]), "2 E + 1 J = 11 J");
        assert_eq!(last(&["n := 1450 rpm", "n ="]), "n = 1450 rpm");
        assert_eq!(last(&["t := 90 min", "t ="]), "t = 90 min");
        assert_eq!(last(&["p := 2 bar", "p ="]), "p = 2 bar");
        // een moment blijft N·m, ook in kN·m
        assert_eq!(last(&["M := 1500 N*m", "M ="]), "M = 1.5 kN·m");
        // vermenigvuldigen met een andere grootheid: weer automatisch
        assert_eq!(last(&["E := 5 J", "E / (2 s) ="]), "E / (2 s) = 2.5 W");
        // expliciete eenheid gaat altijd voor
        assert_eq!(last(&["E := 5 J", "E = N*m"]), "E = 5 N*m");
        // samengestelde eenheden zonder eigen naam: zoals getypt (in plaats van kg/s^2)
        assert_eq!(last(&["q := 3 kN/m", "q ="]), "q = 3 kN/m");
        assert_eq!(last(&["q := 3 kN/m", "2 q ="]), "2 q = 6 kN/m");
        assert_eq!(last(&["ρ := 7850 kg/m^3", "ρ ="]), "ρ = 7850 kg/m^3");
        assert_eq!(last(&["ω := 10 rad/s", "ω ="]), "ω = 10 rad/s");
        assert_eq!(
            last(&["K := [4, 1; 1, 3] kN/m", "K ="]),
            "K = ⎡ 4  1 ⎤ kN/m\n    ⎣ 1  3 ⎦"
        );
        // een variabele in de keten: geen hint (q · L is geen 'getal maal eenheid')
        assert_eq!(last(&["q := 3 kN/m", "L := 2 m", "q * L ="]), "q * L = 6 kN");
    }
}

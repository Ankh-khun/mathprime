//! Functies die een functie of uitdrukking als argument krijgen: `root`, `linfit`, `genfit`,
//! `rkfixed`/`Rkadapt`, solve blocks (`Find`, `Minerr`, `Minimize`, `Maximize`) en `odesolve`.
//!
//! Onbekenden en parameters worden intern geschaald (gedeeld door hun startwaarde, of 1 in SI als
//! die 0 is), zodat de oplossers met getallen van orde 1 werken, wat de eenheid ook is.

use super::*;
use crate::ast::{CmpOp, Table};
use crate::fit;
use std::sync::Arc;

/// Soort beperking in een Given-blok.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Eq,
    /// links ≤ rechts
    Le,
    /// links ≥ rechts
    Ge,
    Ne,
}

fn scale_of(x: f64) -> f64 {
    if x != 0.0 && x.is_finite() {
        x.abs()
    } else {
        1.0
    }
}

/// Een waarde als (SI-getal, eenheid); complexe getallen en matrices geven een fout.
fn real(v: Value, what: &str) -> Result<Quantity, Error> {
    v.as_quantity(what)
}

/// Waarden van een vector of los getal, met eenheid per element.
fn components(v: &Value, what: &str) -> Result<Vec<Quantity>, Error> {
    match v {
        Value::Num(q) => Ok(vec![*q]),
        Value::Matrix(m) if m.is_vector() && !m.is_complex() => Ok(m.data.clone()),
        other => Err(dom(format!("{what} moet een reëel getal of een reële vector zijn, geen {}", other.kind()))),
    }
}

fn vector_value(qs: Vec<Quantity>) -> Value {
    if qs.len() == 1 {
        Value::Num(qs[0])
    } else {
        Value::Matrix(Matrix::column(qs))
    }
}

impl Env {
    /// Geef de beperkingen van het Given-blok door aan de volgende oplosregel (`Find`, ...).
    pub fn set_constraints(&mut self, constraints: Vec<Expr>) {
        self.constraints = constraints;
    }

    /// Roep een eigen functie aan met al uitgerekende argumenten.
    pub(super) fn call_user(&self, name: &str, vals: Vec<Value>, depth: usize) -> Result<Value, Error> {
        let f = self
            .funcs
            .get(name)
            .ok_or_else(|| dom(format!("'{name}' is geen eigen functie (definieer bijvoorbeeld {name}(x) := ...)")))?;
        if f.params.len() != vals.len() {
            return Err(Error::Arity {
                name: name.to_string(),
                expected: f.params.len().to_string(),
                got: vals.len(),
            });
        }
        if depth >= MAX_DEPTH {
            return Err(Error::Depth);
        }
        let scope: Locals = f.params.iter().cloned().zip(vals).collect();
        self.eval(&f.body, &scope, depth + 1)
    }

    /// De naam van een eigen functie die als argument is meegegeven (`linfit(vx, vy, F)`).
    fn fn_arg<'a>(&self, e: &'a Expr, what: &str) -> Result<&'a str, Error> {
        match e {
            Expr::Var(n) if self.funcs.contains_key(n) => Ok(n),
            _ => Err(dom(format!(
                "{what}: dit argument moet de naam van een eigen functie zijn (bijvoorbeeld F, met F(x) := ...)"
            ))),
        }
    }

    /// Speciale functies met een functie als argument. `None`: geen van deze namen.
    pub(super) fn call_with_functions(&self, name: &str, args: &[Expr], loc: &Locals, depth: usize) -> Option<Result<Value, Error>> {
        Some(match name {
            "linfit" => self.linfit(args, loc, depth),
            "genfit" => self.genfit(args, loc, depth),
            "rkfixed" | "Rkadapt" | "Bulstoer" | "Radau" | "Stiffb" | "Stiffr" => self.ode_fixed(name, args, loc, depth),
            "sbval" => self.sbval(args, loc, depth),
            "bvalfit" => self.bvalfit(args, loc, depth),
            "Find" | "Minerr" => self.find(name, args, loc, depth),
            "Minimize" | "Maximize" => self.minimize(name, args, loc, depth),
            _ => return None,
        })
    }

    fn want(name: &str, args: &[Expr], n: usize) -> Result<(), Error> {
        if args.len() == n {
            Ok(())
        } else {
            Err(Error::Arity {
                name: name.to_string(),
                expected: n.to_string(),
                got: args.len(),
            })
        }
    }

    // -----------------------------------------------------------------------------------------
    // Regressie met eigen functies

    /// `linfit(vx, vy, F)`: coëfficiënten c zodat Σ c_j·F(x)_j zo goed mogelijk door de punten gaat.
    fn linfit(&self, args: &[Expr], loc: &Locals, depth: usize) -> Result<Value, Error> {
        Self::want("linfit", args, 3)?;
        let (vx, vy) = (self.eval(&args[0], loc, depth)?, self.eval(&args[1], loc, depth)?);
        let (x, dx, y, dy) = fit::xy("linfit", &vx, &vy, 1, false)?;
        let f = self.fn_arg(&args[2], "linfit")?;
        let mut a = Vec::new();
        let mut dims: Option<Vec<Dim>> = None;
        for &xi in &x {
            let row = components(&self.call_user(f, vec![Value::Num(Quantity::new(xi, dx))], depth)?, "F(x)")?;
            let d: Vec<Dim> = row.iter().map(|q| q.dim).collect();
            match &dims {
                None => dims = Some(d),
                Some(prev) if *prev != d => return Err(dom("linfit: de eenheden van F(x) veranderen per punt")),
                _ => {}
            }
            a.extend(row.iter().map(|q| q.value));
        }
        let dims = dims.expect("minstens één punt");
        let c = numeric::least_squares(&a, x.len(), dims.len(), &y)?;
        let qs = c
            .iter()
            .zip(&dims)
            .map(|(v, d)| Ok(Quantity::new(*v, dy.checked_div(*d).ok_or(rb_units::UnitError::Overflow)?)))
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(Value::Matrix(Matrix::column(qs)))
    }

    /// `genfit(vx, vy, vg, F)`: parameters u (startwaarden vg) zodat F(x, u) zo goed mogelijk door
    /// de punten gaat (Levenberg–Marquardt).
    fn genfit(&self, args: &[Expr], loc: &Locals, depth: usize) -> Result<Value, Error> {
        Self::want("genfit", args, 4)?;
        let (vx, vy) = (self.eval(&args[0], loc, depth)?, self.eval(&args[1], loc, depth)?);
        let (x, dx, y, dy) = fit::xy("genfit", &vx, &vy, 1, false)?;
        let guess = components(&self.eval(&args[2], loc, depth)?, "vg")?;
        let f = self.fn_arg(&args[3], "genfit")?;
        let scales: Vec<f64> = guess.iter().map(|q| scale_of(q.value)).collect();
        let ys = y.iter().fold(0.0f64, |m, v| m.max(v.abs())).max(f64::MIN_POSITIVE);
        let params = |p: &[f64]| -> Value {
            let qs: Vec<Quantity> = p.iter().zip(&guess).zip(&scales).map(|((v, g), s)| Quantity::new(v * s, g.dim)).collect();
            Value::Matrix(Matrix::column(qs))
        };
        let mut resid = |p: &[f64]| -> Result<Vec<f64>, Error> {
            let u = params(p);
            x.iter()
                .zip(&y)
                .map(|(&xi, &yi)| {
                    let m = real(self.call_user(f, vec![Value::Num(Quantity::new(xi, dx)), u.clone()], depth)?, "F(x, u)")?;
                    if m.dim != dy {
                        return Err(dom("genfit: F(x, u) heeft niet dezelfde eenheid als vy"));
                    }
                    Ok((m.value - yi) / ys)
                })
                .collect()
        };
        let p0: Vec<f64> = guess.iter().zip(&scales).map(|(g, s)| g.value / s).collect();
        let (p, _) = numeric::levenberg_marquardt(&mut resid, p0)?;
        match params(&p) {
            Value::Matrix(m) => Ok(Value::Matrix(m)),
            v => Ok(v),
        }
    }

    // -----------------------------------------------------------------------------------------
    // Differentiaalvergelijkingen

    /// Los y' = D(x, y) op van x1 tot x2 in n stappen; geeft de toestand in elk punt (SI-getallen).
    /// `method`: "rkfixed" (RK4), "Radau" (impliciet, stijf) of anders Dormand–Prince.
    #[allow(clippy::too_many_arguments)]
    fn ode_path(&self, name: &str, d: &str, x1: Quantity, x2: f64, y0: &[Quantity], as_vector: bool, n: usize, method: &str, depth: usize) -> Result<Vec<Vec<f64>>, Error> {
        let ydims: Vec<Dim> = y0.iter().map(|q| q.dim).collect();
        let want: Vec<Dim> = ydims
            .iter()
            .map(|yd| yd.checked_div(x1.dim).ok_or(rb_units::UnitError::Overflow))
            .collect::<Result<_, _>>()?;
        let mut rhs = |x: f64, y: &[f64]| -> Result<Vec<f64>, Error> {
            let qs: Vec<Quantity> = y.iter().zip(&ydims).map(|(v, d)| Quantity::new(*v, *d)).collect();
            let yv = if as_vector { Value::Matrix(Matrix::column(qs)) } else { vector_value(qs) };
            let out = components(&self.call_user(d, vec![Value::Num(Quantity::new(x, x1.dim)), yv], depth)?, "D(x, y)")?;
            if out.len() != y.len() {
                return Err(dom(format!("{name}: D(x, y) moet evenveel componenten geven als y ({})", y.len())));
            }
            for (q, w) in out.iter().zip(&want) {
                if q.dim != *w {
                    return Err(dom(format!(
                        "{name}: D(x, y) moet de eenheid y/x hebben, maar een component heeft {}",
                        q.dim.describe()
                    )));
                }
            }
            Ok(out.iter().map(|q| q.value).collect())
        };
        let start: Vec<f64> = y0.iter().map(|q| q.value).collect();
        match method {
            "rkfixed" => numeric::rk4(&mut rhs, &start, x1.value, x2, n),
            "Radau" => numeric::radau(&mut rhs, &start, x1.value, x2, n, 1e-8),
            _ => numeric::dopri(&mut rhs, &start, x1.value, x2, n, 1e-10),
        }
    }

    /// `rkfixed(y, x1, x2, n, D)`, `Rkadapt`, `Bulstoer`, `Radau`, en `Stiffb`/`Stiffr(y, x1, x2, n, D, J)`:
    /// matrix met x in kolom 0 en y in de volgende kolommen. De Jacobiaan J van Stiffb/Stiffr moet een
    /// functie zijn maar wordt niet gebruikt: Radau bepaalt hem zelf numeriek.
    fn ode_fixed(&self, name: &str, args: &[Expr], loc: &Locals, depth: usize) -> Result<Value, Error> {
        let stiff = matches!(name, "Stiffb" | "Stiffr");
        Self::want(name, args, if stiff { 6 } else { 5 })?;
        let init = self.eval(&args[0], loc, depth)?;
        // een beginwaarde als vector ([1]) geeft D ook een vector, zoals in Mathcad (y[0] werkt dan)
        let as_vector = matches!(init, Value::Matrix(_));
        let y0 = components(&init, "de beginwaarde y")?;
        let x1 = real(self.eval(&args[1], loc, depth)?, "x1")?;
        let x2 = real(self.eval(&args[2], loc, depth)?, "x2")?;
        x1.same_dim(x2, "de grenzen van de differentiaalvergelijking")?;
        let n = real(self.eval(&args[3], loc, depth)?, "het aantal punten")?.dimensionless_value("het aantal punten")?;
        if n.fract() != 0.0 || !(1.0..=100_000.0).contains(&n) {
            return Err(dom(format!("{name}: het aantal punten moet een geheel getal van 1 tot en met 100000 zijn")));
        }
        let n = n as usize;
        let d = self.fn_arg(&args[4], name)?;
        if stiff {
            self.fn_arg(&args[5], name)?;
        }
        let method = match name {
            "rkfixed" => "rkfixed",
            "Radau" | "Stiffb" | "Stiffr" => "Radau",
            _ => "dopri",
        };
        let sol = self.ode_path(name, d, x1, x2.value, &y0, as_vector, n, method, depth)?;
        let cols = 1 + y0.len();
        let mut data = Vec::with_capacity((n + 1) * cols);
        for (k, row) in sol.iter().enumerate() {
            data.push(Quantity::new(x1.value + (x2.value - x1.value) * k as f64 / n as f64, x1.dim));
            data.extend(row.iter().zip(&y0).map(|(v, q)| Quantity::new(*v, q.dim)));
        }
        Ok(Value::Matrix(Matrix::new(n + 1, cols, data)))
    }

    /// Getallen als waarde, in dezelfde vorm en eenheden als `like` (los getal of vector).
    fn shaped(vals: &[f64], like: &[Quantity], as_vector: bool) -> Value {
        let qs: Vec<Quantity> = vals.iter().zip(like).map(|(v, q)| Quantity::new(*v, q.dim)).collect();
        if as_vector {
            Value::Matrix(Matrix::column(qs))
        } else {
            vector_value(qs)
        }
    }

    /// Start in x1 met load(x1, v) en integreer naar x2; geeft de eindtoestand als waarde.
    #[allow(clippy::too_many_arguments)]
    fn shoot(&self, name: &str, d: &str, load: &str, x1: Quantity, x2: f64, v: Value, depth: usize) -> Result<Value, Error> {
        let start = self.call_user(load, vec![Value::Num(x1), v], depth)?;
        let as_vector = matches!(start, Value::Matrix(_));
        let y0 = components(&start, &format!("{name}: de uitkomst van load"))?;
        let path = self.ode_path(name, d, x1, x2, &y0, as_vector, 1, "dopri", depth)?;
        Ok(Self::shaped(&path[1], &y0, as_vector))
    }

    /// Los residu(onbekenden) = 0 op met Levenberg–Marquardt, geschaald met de startwaarden. Elk residu
    /// wordt geschaald met zijn grootte in de startwaarde en in een iets verschoven startwaarde (in de
    /// startwaarde alleen kan het toevallig bijna 0 zijn).
    fn shooting_solve(name: &str, guess: &[Quantity], resid: &mut dyn FnMut(&[f64]) -> Result<Vec<Quantity>, Error>) -> Result<Vec<f64>, Error> {
        let scales: Vec<f64> = guess.iter().map(|q| scale_of(q.value)).collect();
        let r0 = resid(&guess.iter().map(|q| q.value).collect::<Vec<_>>())?;
        let r1 = resid(&guess.iter().zip(&scales).map(|(q, s)| q.value + 0.1 * s).collect::<Vec<_>>())?;
        let rs: Vec<f64> = r0.iter().zip(&r1).map(|(a, b)| scale_of(a.value.abs().max(b.value.abs()))).collect();
        let mut f = |p: &[f64]| -> Result<Vec<f64>, Error> {
            let x: Vec<f64> = p.iter().zip(&scales).map(|(a, s)| a * s).collect();
            Ok(resid(&x)?.iter().zip(&rs).map(|(q, s)| q.value / s).collect())
        };
        let p0: Vec<f64> = guess.iter().zip(&scales).map(|(q, s)| q.value / s).collect();
        let (p, cost) = numeric::levenberg_marquardt(&mut f, p0)?;
        if cost > 1e-16 {
            return Err(dom(format!(
                "{name}: geen oplossing gevonden (restfout {:.3e}); probeer andere startwaarden",
                cost.sqrt()
            )));
        }
        Ok(p.iter().zip(&scales).map(|(a, s)| a * s).collect())
    }

    /// `sbval(v, x1, x2, D, load, score)`: de ontbrekende beginwaarden v in x1, zodat score(x2, y(x2)) = 0.
    fn sbval(&self, args: &[Expr], loc: &Locals, depth: usize) -> Result<Value, Error> {
        let name = "sbval";
        Self::want(name, args, 6)?;
        let guess_v = self.eval(&args[0], loc, depth)?;
        let as_vector = matches!(guess_v, Value::Matrix(_));
        let guess = components(&guess_v, "de startwaarden v")?;
        let x1 = real(self.eval(&args[1], loc, depth)?, "x1")?;
        let x2 = real(self.eval(&args[2], loc, depth)?, "x2")?;
        x1.same_dim(x2, "de grenzen van sbval")?;
        let (d, load, score) = (self.fn_arg(&args[3], name)?, self.fn_arg(&args[4], name)?, self.fn_arg(&args[5], name)?);
        let mut resid = |x: &[f64]| -> Result<Vec<Quantity>, Error> {
            let end = self.shoot(name, d, load, x1, x2.value, Self::shaped(x, &guess, as_vector), depth)?;
            let r = components(&self.call_user(score, vec![Value::Num(x2), end], depth)?, "sbval: de uitkomst van score")?;
            if r.len() != guess.len() {
                return Err(dom(format!("sbval: score moet evenveel elementen geven als v ({})", guess.len())));
            }
            Ok(r)
        };
        let sol = Self::shooting_solve(name, &guess, &mut resid)?;
        Ok(Self::shaped(&sol, &guess, as_vector))
    }

    /// `bvalfit(v1, v2, x1, x2, xf, D, load1, load2, score)`: schiet vanuit x1 en vanuit x2 naar xf en zoek
    /// v1 en v2 zodat score(xf, y) van beide kanten gelijk is. Geeft de startwaarden v1 in x1.
    fn bvalfit(&self, args: &[Expr], loc: &Locals, depth: usize) -> Result<Value, Error> {
        let name = "bvalfit";
        Self::want(name, args, 9)?;
        let (g1v, g2v) = (self.eval(&args[0], loc, depth)?, self.eval(&args[1], loc, depth)?);
        let (vec1, vec2) = (matches!(g1v, Value::Matrix(_)), matches!(g2v, Value::Matrix(_)));
        let (g1, g2) = (components(&g1v, "de startwaarden v1")?, components(&g2v, "de startwaarden v2")?);
        let x1 = real(self.eval(&args[2], loc, depth)?, "x1")?;
        let x2 = real(self.eval(&args[3], loc, depth)?, "x2")?;
        let xf = real(self.eval(&args[4], loc, depth)?, "xf")?;
        x1.same_dim(x2, "de grenzen van bvalfit")?;
        x1.same_dim(xf, "het aansluitpunt xf")?;
        let d = self.fn_arg(&args[5], name)?;
        let (load1, load2, score) = (self.fn_arg(&args[6], name)?, self.fn_arg(&args[7], name)?, self.fn_arg(&args[8], name)?);
        let guess: Vec<Quantity> = g1.iter().chain(&g2).copied().collect();
        let k = g1.len();
        let mut resid = |x: &[f64]| -> Result<Vec<Quantity>, Error> {
            let a = self.shoot(name, d, load1, x1, xf.value, Self::shaped(&x[..k], &g1, vec1), depth)?;
            let b = self.shoot(name, d, load2, x2, xf.value, Self::shaped(&x[k..], &g2, vec2), depth)?;
            let sa = components(&self.call_user(score, vec![Value::Num(xf), a], depth)?, "bvalfit: de uitkomst van score")?;
            let sb = components(&self.call_user(score, vec![Value::Num(xf), b], depth)?, "bvalfit: de uitkomst van score")?;
            if sa.len() != guess.len() || sb.len() != guess.len() {
                return Err(dom(format!(
                    "bvalfit: score moet evenveel elementen geven als v1 en v2 samen ({})",
                    guess.len()
                )));
            }
            Ok(sa.iter().zip(&sb).map(|(p, q)| Quantity::new(p.value - q.value, p.dim)).collect())
        };
        let sol = Self::shooting_solve(name, &guess, &mut resid)?;
        Ok(Self::shaped(&sol[..k], &g1, vec1))
    }

    /// `y := odesolve(t, b)` en `[x; y] := odesolve([x; y], t, b)`: los de differentiaalvergelijking(en) in
    /// het Given-blok op, van het beginpunt tot b. Staan alle voorwaarden in één punt, dan is dat een
    /// beginwaardeprobleem; staan ze in meer punten (`y(0) == 0`, `y(L) == 0`), dan een randwaardeprobleem,
    /// opgelost met de schietmethode: de ontbrekende beginwaarden worden gezocht tot de andere voorwaarden
    /// kloppen. Geeft per functie een tabel (kubische Hermite-interpolatie in 1000 stappen).
    pub(super) fn odesolve(&self, fnames: &[String], tvar: &str, end: &Expr) -> Result<Vec<UserFn>, Error> {
        if self.constraints.is_empty() {
            return Err(dom("odesolve hoort onder een Given-blok met de differentiaalvergelijking en de beginvoorwaarden"));
        }
        let m = fnames.len();
        // `y''` → (welke functie, orde)
        let which = |n: &str| -> Option<(usize, usize)> {
            let base = n.trim_end_matches('\'');
            fnames.iter().position(|f| f == base).map(|i| (i, n.len() - base.len()))
        };
        let label = |i: usize, k: usize| format!("{}{}", fnames[i], "'".repeat(k));
        // de vergelijkingen bevatten y^(k)(t); de rest zijn voorwaarden y^(k)(p) == waarde
        let mut odes: Vec<Expr> = Vec::new();
        let mut orders = vec![0usize; m];
        let mut conds: Vec<(usize, usize, Quantity, Quantity)> = Vec::new();
        for c in &self.constraints {
            let (l, r) = match c {
                Expr::Cmp(ops, xs) if ops.len() == 1 && ops[0] == CmpOp::Eq => (&xs[0], &xs[1]),
                _ => return Err(dom("odesolve: elke beperking moet een gelijkheid met == zijn")),
            };
            let mut found = Vec::new();
            find_calls(c, &which, tvar, &mut found);
            if found.iter().any(|f| f.2) {
                if m == 1 && !odes.is_empty() {
                    return Err(dom("odesolve: er mag maar één differentiaalvergelijking in het blok staan"));
                }
                for &(i, k, on_t) in &found {
                    if on_t {
                        orders[i] = orders[i].max(k);
                    }
                }
                // een kale 0 aan één kant past bij elke eenheid: dan is de andere kant het residu
                let (l, r) = (substitute(l, &which, tvar), substitute(r, &which, tvar));
                odes.push(match (&l, &r) {
                    (_, Expr::Num(z)) if *z == 0.0 => l,
                    (Expr::Num(z), _) if *z == 0.0 => r,
                    _ => Expr::Bin(BinOp::Sub, Box::new(l), Box::new(r)),
                });
                continue;
            }
            let (call, other) = match (l, r) {
                (Expr::Call(n, a), o) | (o, Expr::Call(n, a)) if which(n).is_some() && a.len() == 1 => ((n, &a[0]), o),
                _ => return Err(dom(format!("odesolve: een beginvoorwaarde schrijf je als {f}(0) == 1 of {f}'(0) == 0", f = fnames[0]))),
            };
            let (i, k) = which(call.0).unwrap();
            let at = real(self.eval_expr(call.1)?, "het beginpunt")?;
            let val = real(self.eval_expr(other)?, "een beginwaarde")?;
            conds.push((i, k, at, val));
        }
        if odes.is_empty() {
            return Err(dom(format!("odesolve: geen differentiaalvergelijking met {}'({tvar}) gevonden", fnames[0])));
        }
        if let Some(i) = orders.iter().position(|&n| n == 0) {
            let what = if m == 1 { "de vergelijking bevat" } else { "de vergelijkingen bevatten" };
            return Err(dom(format!("odesolve: {what} geen afgeleide van {}", fnames[i])));
        }
        if odes.len() != m {
            return Err(dom(format!("odesolve: {m} functies vragen {m} differentiaalvergelijkingen, het blok heeft er {}", odes.len())));
        }
        let b = real(self.eval_expr(end)?, "de eindwaarde")?;
        // het beginpunt: bij een randwaardeprobleem het punt het verst van de eindwaarde
        let a = conds
            .iter()
            .map(|c| c.2)
            .max_by(|p, q| (p.value - b.value).abs().total_cmp(&(q.value - b.value).abs()))
            .ok_or_else(|| dom("odesolve: beginvoorwaarden ontbreken"))?;
        b.same_dim(a, "odesolve (beginpunt en eindwaarde)")?;
        let tdim = a.dim;
        // de toestand: per functie y, y', .. tot de orde − 1
        let offs: Vec<usize> = orders.iter().scan(0, |s, n| Some(std::mem::replace(s, *s + n))).collect();
        let total: usize = orders.iter().sum();
        let same = |p: f64| (p - a.value).abs() <= 1e-12 * a.value.abs().max(1e-300);
        let (lo, hi) = (a.value.min(b.value), a.value.max(b.value));
        let tol = 1e-9 * (hi - lo);
        let mut fixed: Vec<Option<f64>> = vec![None; total];
        let mut targets: Vec<(usize, f64, f64)> = Vec::new();
        let mut ydims: Vec<Option<Dim>> = vec![None; m];
        let tpow = |k: usize| tdim.checked_powi(k as i32).ok_or(rb_units::UnitError::Overflow);
        for &(i, k, at, val) in &conds {
            if k >= orders[i] {
                return Err(dom(format!("odesolve: een voorwaarde voor de hoogste afgeleide ({}) kan niet", label(i, k))));
            }
            at.same_dim(a, "odesolve (de punten van de voorwaarden)")?;
            let yd = val.dim.checked_mul(tpow(k)?).ok_or(rb_units::UnitError::Overflow)?;
            // een kale 0 past bij elke eenheid (w''(0) == 0 bij een balk in meter)
            let bare_zero = val.value == 0.0 && val.dim == Dim::NONE;
            match ydims[i] {
                _ if bare_zero => {}
                Some(d) if d != yd => {
                    let want = d.checked_div(tpow(k)?).ok_or(rb_units::UnitError::Overflow)?;
                    return Err(dom(format!(
                        "odesolve: de beginwaarde van {} heeft eenheid {}, verwacht {}",
                        label(i, k),
                        val.dim.describe(),
                        want.describe()
                    )));
                }
                _ => ydims[i] = Some(yd),
            }
            let idx = offs[i] + k;
            if same(at.value) {
                if fixed[idx].replace(val.value).is_some() {
                    return Err(dom(format!("odesolve: twee voorwaarden voor {}({})", label(i, k), format_quantity(at))));
                }
            } else if at.value < lo - tol || at.value > hi + tol {
                return Err(dom(format!(
                    "odesolve: de voorwaarde voor {}({}) ligt buiten het bereik van {} tot {}",
                    label(i, k),
                    format_quantity(at),
                    format_quantity(a),
                    format_quantity(b)
                )));
            } else {
                targets.push((idx, at.value, val.value));
            }
        }
        // per functie evenveel voorwaarden als de orde
        for i in 0..m {
            let count = conds.iter().filter(|c| c.0 == i).count();
            if count > orders[i] {
                return Err(dom(format!("odesolve: te veel voorwaarden voor {} (de vergelijking is van orde {})", fnames[i], orders[i])));
            }
            if count < orders[i] {
                let k = (0..orders[i]).find(|&k| fixed[offs[i] + k].is_none()).unwrap_or(0);
                return Err(dom(format!("odesolve: de beginvoorwaarde voor {}({}) ontbreekt", label(i, k), format_quantity(a))));
            }
        }
        let ydims: Vec<Dim> = ydims.into_iter().map(|d| d.unwrap_or(Dim::NONE)).collect();
        let deriv_dim = |i: usize, k: usize| -> Result<Dim, Error> { Ok(ydims[i].checked_div(tpow(k)?).ok_or(rb_units::UnitError::Overflow)?) };
        let mut dims = Vec::with_capacity(total);
        for i in 0..m {
            for k in 0..orders[i] {
                dims.push(deriv_dim(i, k)?);
            }
        }
        let top_dims: Vec<Dim> = (0..m).map(|i| deriv_dim(i, orders[i])).collect::<Result<_, _>>()?;

        // de vergelijkingen uitrekenen bij t, de toestand en de hoogste afgeleiden
        let mut base = Locals::new();
        let residual = |t: f64, s: &[f64], tops: &[f64], base: &mut Locals| -> Result<Vec<f64>, Error> {
            base.insert(tvar.to_string(), Value::Num(Quantity::new(t, tdim)));
            for i in 0..m {
                for k in 0..orders[i] {
                    base.insert(format!("__y{i}_{k}"), Value::Num(Quantity::new(s[offs[i] + k], dims[offs[i] + k])));
                }
                base.insert(format!("__y{i}_{}", orders[i]), Value::Num(Quantity::new(tops[i], top_dims[i])));
            }
            odes.iter().map(|e| Ok(real(self.eval(e, base, 0)?, "de differentiaalvergelijking")?.value)).collect()
        };
        let singular = || {
            if m == 1 {
                dom(format!("odesolve: de vergelijking hangt niet af van {}", label(0, orders[0])))
            } else {
                let tops: Vec<String> = (0..m).map(|i| label(i, orders[i])).collect();
                dom(format!("odesolve: de vergelijkingen leggen {} niet vast", tops.join(", ")))
            }
        };
        // de hoogste afgeleiden uit de vergelijkingen halen: Newton (meestal lineair: één stap)
        let mut rhs = |t: f64, s: &[f64]| -> Result<Vec<f64>, Error> {
            let mut tops = vec![0.0; m];
            let mut r = residual(t, s, &tops, &mut base)?;
            for _ in 0..30 {
                let mut jac = vec![0.0; m * m];
                for j in 0..m {
                    let h = tops[j].abs().max(1.0) * 1e-6;
                    let mut tp = tops.clone();
                    tp[j] += h;
                    let rj = residual(t, s, &tp, &mut base)?;
                    for q in 0..m {
                        jac[q * m + j] = (rj[q] - r[q]) / h;
                    }
                }
                let size: Vec<f64> = (0..m).map(|q| r[q].abs() + (0..m).map(|j| (jac[q * m + j] * tops[j]).abs()).sum::<f64>()).collect();
                let delta = numeric::solve_dense(jac, m, r.iter().map(|v| -v).collect()).ok_or_else(singular)?;
                if delta.iter().any(|d| !d.is_finite()) {
                    return Err(singular());
                }
                for (t, d) in tops.iter_mut().zip(&delta) {
                    *t += d;
                }
                r = residual(t, s, &tops, &mut base)?;
                if r.iter().zip(&size).all(|(v, s)| v.abs() <= 1e-9 * s) {
                    break;
                }
            }
            let mut d = vec![0.0; total];
            for i in 0..m {
                for k in 0..orders[i] {
                    d[offs[i] + k] = if k + 1 < orders[i] { s[offs[i] + k + 1] } else { tops[i] };
                }
            }
            Ok(d)
        };
        let mut start: Vec<f64> = fixed.iter().map(|v| v.unwrap_or(0.0)).collect();

        // randwaardeprobleem: de onbekende beginwaarden zoeken (Newton op de schietfout)
        if !targets.is_empty() {
            let unknown: Vec<usize> = (0..total).filter(|&i| fixed[i].is_none()).collect();
            let mut mags: Vec<f64> = targets.iter().map(|t| t.2.abs()).collect();
            let mut shoot = |s0: &[f64], mags: &mut Vec<f64>| -> Result<Vec<f64>, Error> {
                let mut at: Vec<(f64, Vec<f64>)> = Vec::new();
                let mut out = Vec::with_capacity(targets.len());
                for (q, &(idx, p, v)) in targets.iter().enumerate() {
                    if !at.iter().any(|(x, _)| *x == p) {
                        let sol = numeric::dopri(&mut rhs, s0, a.value, p, 1, 1e-10)?;
                        at.push((p, sol.last().cloned().unwrap_or_default()));
                    }
                    let y = at.iter().find(|(x, _)| *x == p).map(|(_, s)| s[idx]).unwrap_or(f64::NAN);
                    mags[q] = mags[q].max(y.abs());
                    out.push(y - v);
                }
                Ok(out)
            };
            let base_start = start.clone();
            let with = |u: &[f64]| {
                let mut s = base_start.clone();
                for (j, &i) in unknown.iter().enumerate() {
                    s[i] = u[j];
                }
                s
            };
            let n = unknown.len();
            let mut u = vec![0.0; n];
            let mut g = shoot(&with(&u), &mut mags)?;
            let worst = |g: &[f64], mags: &[f64]| g.iter().zip(mags).map(|(e, m)| e.abs() / m.max(1e-300)).fold(0.0, f64::max);
            let norm = |g: &[f64], mags: &[f64]| g.iter().zip(mags).map(|(e, m)| (e / m.max(1e-300)).powi(2)).sum::<f64>();
            let mut done = worst(&g, &mags) <= 1e-9;
            for _ in 0..40 {
                if done {
                    break;
                }
                let mut jac = vec![0.0; n * n];
                for j in 0..n {
                    let h = u[j].abs().max(1.0) * 1e-4;
                    let mut up = u.clone();
                    up[j] += h;
                    let gp = shoot(&with(&up), &mut mags)?;
                    for q in 0..n {
                        jac[q * n + j] = (gp[q] - g[q]) / h;
                    }
                }
                let delta = numeric::solve_dense(jac, n, g.iter().map(|v| -v).collect())
                    .filter(|d| d.iter().all(|x| x.is_finite()))
                    .ok_or_else(|| dom("odesolve: de randvoorwaarden leggen de oplossing niet vast"))?;
                // gedempte stap: halveren tot de fout kleiner wordt
                let mut lambda = 1.0;
                loop {
                    let un: Vec<f64> = u.iter().zip(&delta).map(|(x, d)| x + lambda * d).collect();
                    match shoot(&with(&un), &mut mags) {
                        Ok(gn) if norm(&gn, &mags) < norm(&g, &mags) || lambda < 1e-3 => {
                            u = un;
                            g = gn;
                            break;
                        }
                        _ if lambda < 1e-3 => break,
                        _ => lambda /= 2.0,
                    }
                }
                done = worst(&g, &mags) <= 1e-9;
            }
            if !done {
                return Err(dom("odesolve: de randvoorwaarden zijn niet te halen (geen oplossing gevonden)"));
            }
            start = with(&u);
        }

        let steps = 1000;
        let sol = numeric::dopri(&mut rhs, &start, a.value, b.value, steps, 1e-10)?;
        let ts: Vec<f64> = (0..=steps).map(|k| a.value + (b.value - a.value) * k as f64 / steps as f64).collect();
        // de afgeleide in de knooppunten: uit de toestand, of bij eerste orde uit de vergelijking
        let firsts: Vec<Vec<f64>> = if orders.contains(&1) {
            ts.iter().zip(&sol).map(|(t, s)| rhs(*t, s)).collect::<Result<_, Error>>()?
        } else {
            Vec::new()
        };
        Ok((0..m)
            .map(|i| {
                let o = offs[i];
                let ys = sol.iter().map(|s| s[o]).collect();
                let ds = if orders[i] >= 2 { sol.iter().map(|s| s[o + 1]).collect() } else { firsts.iter().map(|d| d[o]).collect() };
                let table = Table { ts: ts.clone(), ys, ds, tdim, ydim: ydims[i] };
                UserFn {
                    params: vec![tvar.to_string()],
                    body: Expr::Interp(Arc::new(table), Box::new(Expr::Var(tvar.to_string()))),
                }
            })
            .collect())
    }

    // -----------------------------------------------------------------------------------------
    // root

    /// `root(f(x), x)` vanaf de huidige waarde van x, of `root(f(x), x, a, b)` tussen a en b.
    pub(super) fn eval_root(&self, body: &Expr, var: &str, args: &[Expr], loc: &Locals, depth: usize) -> Result<Value, Error> {
        let (x0, bracket) = if args.is_empty() {
            let g = self.lookup(var, loc).ok_or_else(|| {
                dom(format!("root: geef {var} eerst een startwaarde, bijvoorbeeld {var} := 1 (of gebruik root(f, {var}, a, b))"))
            })?;
            (real(g, "de startwaarde")?, None)
        } else {
            let a = real(self.eval(&args[0], loc, depth)?, "de ondergrens")?;
            let b = real(self.eval(&args[1], loc, depth)?, "de bovengrens")?;
            a.same_dim(b, "de grenzen van root")?;
            (a, Some(b))
        };
        let mut scope = loc.clone();
        let mut f = |x: f64| -> Result<f64, Error> {
            scope.insert(var.to_string(), Value::Num(Quantity::new(x, x0.dim)));
            Ok(real(self.eval(body, &scope, depth)?, "de functie in root")?.value)
        };
        let r = match bracket {
            Some(b) => numeric::brent(&mut f, x0.value.min(b.value), x0.value.max(b.value))?,
            None => numeric::root_from_guess(&mut f, x0.value)?,
        };
        Ok(Value::Num(Quantity::new(r, x0.dim)))
    }

    // -----------------------------------------------------------------------------------------
    // Solve blocks

    /// De onbekenden van `Find(x, y)` / `Minimize(f, x, y)` met hun startwaarden.
    fn unknowns(&self, name: &str, args: &[Expr], loc: &Locals) -> Result<Vec<(String, Quantity)>, Error> {
        if args.is_empty() {
            return Err(dom(format!("{name}: noem de onbekenden, bijvoorbeeld {name}(x, y)")));
        }
        args.iter()
            .map(|a| match a {
                Expr::Var(n) => {
                    let g = self.lookup(n, loc).filter(|_| self.is_variable(n, loc)).ok_or_else(|| {
                        dom(format!("{name}: geef {n} eerst een startwaarde boven het Given-blok, bijvoorbeeld {n} := 1"))
                    })?;
                    Ok((n.clone(), real(g, "een startwaarde")?))
                }
                _ => Err(dom(format!("{name}: de onbekenden moeten namen zijn"))),
            })
            .collect()
    }

    /// Termen van een beperking: (links − rechts, grootte, soort), in SI.
    fn constraint_terms(&self, e: &Expr, loc: &Locals, out: &mut Vec<(f64, f64, Kind)>) -> Result<(), Error> {
        match e {
            Expr::Cmp(ops, xs) => {
                let vals = xs
                    .iter()
                    .map(|x| real(self.eval(x, loc, 0)?, "een kant van een beperking"))
                    .collect::<Result<Vec<_>, _>>()?;
                for (k, op) in ops.iter().enumerate() {
                    let (l, r) = (vals[k], vals[k + 1]);
                    let bare_zero = |q: &Quantity| q.dim.is_none() && q.value == 0.0;
                    if !bare_zero(&l) && !bare_zero(&r) {
                        l.same_dim(r, "een beperking")?;
                    }
                    let kind = match op {
                        CmpOp::Eq => Kind::Eq,
                        CmpOp::Lt | CmpOp::Le => Kind::Le,
                        CmpOp::Gt | CmpOp::Ge => Kind::Ge,
                        CmpOp::Ne => Kind::Ne,
                    };
                    out.push((l.value - r.value, l.value.abs().max(r.value.abs()), kind));
                }
                Ok(())
            }
            Expr::And(a, b) => {
                self.constraint_terms(a, loc, out)?;
                self.constraint_terms(b, loc, out)
            }
            _ => Err(dom("een beperking in een Given-blok moet een vergelijking zijn, zoals x + y == 10 of x > 0")),
        }
    }

    /// Geschaalde residuen van alle beperkingen bij gegeven waarden van de onbekenden.
    fn residuals(&self, unknowns: &[(String, Quantity)], x: &[f64], loc: &Locals, scales: Option<&[f64]>) -> Result<(Vec<f64>, Vec<f64>), Error> {
        let mut scope = loc.clone();
        for ((n, g), v) in unknowns.iter().zip(x) {
            scope.insert(n.clone(), Value::Num(Quantity::new(*v, g.dim)));
        }
        let mut terms = Vec::new();
        for c in &self.constraints {
            self.constraint_terms(c, &scope, &mut terms)?;
        }
        let mags: Vec<f64> = terms.iter().map(|t| scale_of(t.1)).collect();
        let sc = scales.unwrap_or(&mags);
        let r = terms
            .iter()
            .zip(sc)
            .map(|((d, _, k), s)| {
                let v = match k {
                    Kind::Eq => *d,
                    Kind::Le => d.max(0.0),
                    Kind::Ge => (-d).max(0.0),
                    Kind::Ne => 0.0,
                };
                v / s
            })
            .collect();
        Ok((r, mags))
    }

    fn result_value(unknowns: &[(String, Quantity)], x: &[f64]) -> Value {
        vector_value(unknowns.iter().zip(x).map(|((_, g), v)| Quantity::new(*v, g.dim)).collect())
    }

    /// `Find(x, y)` en `Minerr(x, y)`: maak de beperkingen van het Given-blok waar.
    fn find(&self, name: &str, args: &[Expr], loc: &Locals, _depth: usize) -> Result<Value, Error> {
        if self.constraints.is_empty() {
            return Err(dom(format!(
                "{name} hoort onder een Given-blok: een regel 'Given', daaronder de vergelijkingen (met ==), dan {name}(...)"
            )));
        }
        let unknowns = self.unknowns(name, args, loc)?;
        let xs: Vec<f64> = unknowns.iter().map(|(_, g)| scale_of(g.value)).collect();
        let x0: Vec<f64> = unknowns.iter().map(|(_, g)| g.value).collect();
        let (_, mags) = self.residuals(&unknowns, &x0, loc, None)?;
        let mut f = |p: &[f64]| -> Result<Vec<f64>, Error> {
            let x: Vec<f64> = p.iter().zip(&xs).map(|(a, b)| a * b).collect();
            Ok(self.residuals(&unknowns, &x, loc, Some(&mags))?.0)
        };
        let p0: Vec<f64> = x0.iter().zip(&xs).map(|(a, b)| a / b).collect();
        let (p, _) = numeric::levenberg_marquardt(&mut f, p0)?;
        let x: Vec<f64> = p.iter().zip(&xs).map(|(a, b)| a * b).collect();
        if name == "Find" {
            // nauwkeurigheid beoordelen ten opzichte van de grootte in de oplossing
            let (r, _) = self.residuals(&unknowns, &x, loc, None)?;
            let worst = r.iter().fold(0.0f64, |m, v| m.max(v.abs()));
            if worst > 1e-8 {
                return Err(dom(format!(
                    "Find: geen oplossing gevonden (grootste relatieve restfout {worst:.2e}); \
                     probeer andere startwaarden, of Minerr voor de beste benadering"
                )));
            }
        }
        Ok(Self::result_value(&unknowns, &x))
    }

    /// `Minimize(f, x, y)` / `Maximize(f, x, y)`, eventueel met de beperkingen van een Given-blok.
    fn minimize(&self, name: &str, args: &[Expr], loc: &Locals, depth: usize) -> Result<Value, Error> {
        let fname = self.fn_arg(args.first().ok_or_else(|| dom(format!("{name}(f, x, ...): noem eerst de functie")))?, name)?;
        let unknowns = self.unknowns(name, &args[1..], loc)?;
        let sign = if name == "Maximize" { -1.0 } else { 1.0 };
        let xs: Vec<f64> = unknowns.iter().map(|(_, g)| scale_of(g.value)).collect();
        let x0: Vec<f64> = unknowns.iter().map(|(_, g)| g.value).collect();
        let objective = |x: &[f64]| -> Result<f64, Error> {
            let vals = unknowns.iter().zip(x).map(|((_, g), v)| Value::Num(Quantity::new(*v, g.dim))).collect();
            Ok(real(self.call_user(fname, vals, depth)?, "de functie")?.value)
        };
        let g0 = scale_of(objective(&x0)?);
        let has_constraints = !self.constraints.is_empty();
        let mags = if has_constraints { self.residuals(&unknowns, &x0, loc, None)?.1 } else { Vec::new() };
        let mut p: Vec<f64> = x0.iter().zip(&xs).map(|(a, b)| a / b).collect();
        let weights: &[f64] = if has_constraints { &[1e2, 1e4, 1e6, 1e8, 1e10] } else { &[0.0, 0.0] };
        for &mu in weights {
            let mut f = |q: &[f64]| -> Result<f64, Error> {
                let x: Vec<f64> = q.iter().zip(&xs).map(|(a, b)| a * b).collect();
                let mut v = sign * objective(&x)? / g0;
                if mu > 0.0 {
                    let (r, _) = self.residuals(&unknowns, &x, loc, Some(&mags))?;
                    v += mu * r.iter().map(|t| t * t).sum::<f64>();
                }
                Ok(v)
            };
            let step: Vec<f64> = p.iter().map(|v| 0.05 * v.abs().max(0.1)).collect();
            p = numeric::nelder_mead(&mut f, &p, &step)?;
        }
        let x: Vec<f64> = p.iter().zip(&xs).map(|(a, b)| a * b).collect();
        if has_constraints {
            let (r, _) = self.residuals(&unknowns, &x, loc, None)?;
            let worst = r.iter().fold(0.0f64, |m, v| m.max(v.abs()));
            if worst > 1e-6 {
                return Err(dom(format!("{name}: de beperkingen zijn niet te vervullen (restfout {worst:.2e})")));
            }
        }
        Ok(Self::result_value(&unknowns, &x))
    }
}

/// Zoek aanroepen y^(k)(...) van de functies en geef (welke functie, k, argument is precies t).
fn find_calls(e: &Expr, which: &dyn Fn(&str) -> Option<(usize, usize)>, tvar: &str, out: &mut Vec<(usize, usize, bool)>) {
    match e {
        Expr::Call(n, args) => {
            if let Some((i, k)) = which(n) {
                out.push((i, k, matches!(args.as_slice(), [Expr::Var(v)] if v == tvar)));
            }
            for a in args {
                find_calls(a, which, tvar, out);
            }
        }
        Expr::Neg(x) | Expr::Not(x) => find_calls(x, which, tvar, out),
        Expr::Bin(_, a, b) | Expr::And(a, b) | Expr::Or(a, b) => {
            find_calls(a, which, tvar, out);
            find_calls(b, which, tvar, out);
        }
        Expr::Cmp(_, xs) => xs.iter().for_each(|x| find_calls(x, which, tvar, out)),
        _ => {}
    }
}

/// Vervang y^(k)(t) van functie i door de interne variabele `__yi_k`.
fn substitute(e: &Expr, which: &dyn Fn(&str) -> Option<(usize, usize)>, tvar: &str) -> Expr {
    let s = |x: &Expr| Box::new(substitute(x, which, tvar));
    match e {
        Expr::Call(n, args) => match (which(n), args.as_slice()) {
            (Some((i, k)), [Expr::Var(v)]) if v == tvar => Expr::Var(format!("__y{i}_{k}")),
            _ => Expr::Call(n.clone(), args.iter().map(|a| substitute(a, which, tvar)).collect()),
        },
        Expr::Neg(x) => Expr::Neg(s(x)),
        Expr::Not(x) => Expr::Not(s(x)),
        Expr::Bin(op, a, b) => Expr::Bin(*op, s(a), s(b)),
        Expr::And(a, b) => Expr::And(s(a), s(b)),
        Expr::Or(a, b) => Expr::Or(s(a), s(b)),
        Expr::Cmp(ops, xs) => Expr::Cmp(ops.clone(), xs.iter().map(|x| substitute(x, which, tvar)).collect()),
        other => other.clone(),
    }
}

/// Kubische Hermite-interpolatie in een tabel uit `odesolve`.
pub(super) fn interp_table(t: &Table, x: Quantity) -> Result<Value, Error> {
    if x.dim != t.tdim {
        return Err(rb_units::UnitError::Mismatch {
            op: "de functie uit odesolve",
            left: x.dim,
            right: t.tdim,
        }
        .into());
    }
    let n = t.ts.len();
    let (lo, hi) = (t.ts[0].min(t.ts[n - 1]), t.ts[0].max(t.ts[n - 1]));
    let tol = 1e-9 * (hi - lo);
    if x.value < lo - tol || x.value > hi + tol {
        return Err(dom(format!(
            "de oplossing van odesolve bestaat alleen tussen {} en {}",
            format_quantity(Quantity::new(t.ts[0], t.tdim)),
            format_quantity(Quantity::new(t.ts[n - 1], t.tdim))
        )));
    }
    let step = (t.ts[n - 1] - t.ts[0]) / (n - 1) as f64;
    let k = (((x.value - t.ts[0]) / step).floor().max(0.0) as usize).min(n - 2);
    let h = t.ts[k + 1] - t.ts[k];
    let s = (x.value - t.ts[k]) / h;
    let (s2, s3) = (s * s, s * s * s);
    let y = (2.0 * s3 - 3.0 * s2 + 1.0) * t.ys[k]
        + (s3 - 2.0 * s2 + s) * h * t.ds[k]
        + (-2.0 * s3 + 3.0 * s2) * t.ys[k + 1]
        + (s3 - s2) * h * t.ds[k + 1];
    Ok(Value::Num(Quantity::new(y, t.ydim)))
}

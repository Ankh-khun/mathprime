//! Grafiekregio's.
//!
//! ```text
//! plot x = 0 ; 10 : sin(x), cos(x)            functies van x, 200 punten
//! plot t = 0 s ; 2 s ; 500 : u(t) = mm        500 punten, y-as in mm
//! plot vx : vy, vz                            vectoren (meetpunten) tegen elkaar
//! ```
//!
//! De uitkomst ([`PlotData`]) is al in de weergave-eenheden omgerekend, zodat elke weergave
//! (interface, SVG, PDF) hem direct kan tekenen.

use rb_core::{lex, parse_expr_str, Env, Error, Expr, Tok, Value};
use rb_units::{auto_scale, Dim, Quantity};
use std::collections::BTreeSet;

const DEFAULT_POINTS: f64 = 200.0;

#[derive(Debug, Clone, PartialEq)]
pub enum PlotX {
    /// `x = begin ; einde [; aantal]`
    Range {
        var: String,
        start: Expr,
        end: Expr,
        points: Option<Expr>,
    },
    /// Een vector met x-waarden.
    Data(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlotSpec {
    pub x: PlotX,
    /// De reeksen: (uitdrukking, tekst zoals getypt).
    pub series: Vec<(Expr, String)>,
    /// Gekozen eenheid voor de y-as: (uitdrukking, tekst).
    pub unit: Option<(Expr, String)>,
}

/// Eén lijn in een grafiek.
#[derive(Debug, Clone, PartialEq)]
pub struct Series {
    pub label: String,
    /// Punten in weergave-eenheden; NaN waar de uitdrukking niet uit te rekenen was.
    pub points: Vec<[f64; 2]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlotData {
    /// Bijvoorbeeld `t [s]`.
    pub x_label: String,
    /// Bijvoorbeeld `[mm]` (leeg voor dimensieloos).
    pub y_label: String,
    pub series: Vec<Series>,
}

impl PlotSpec {
    /// Namen die deze grafiek van buiten gebruikt (de variabele van de x-as telt niet mee).
    pub fn uses(&self) -> BTreeSet<String> {
        let none = BTreeSet::new();
        let mut out = BTreeSet::new();
        let mut bound = BTreeSet::new();
        match &self.x {
            PlotX::Range { var, start, end, points } => {
                start.collect_names(&none, &mut out);
                end.collect_names(&none, &mut out);
                if let Some(p) = points {
                    p.collect_names(&none, &mut out);
                }
                bound.insert(var.clone());
            }
            PlotX::Data(e) => e.collect_names(&none, &mut out),
        }
        for (e, _) in &self.series {
            e.collect_names(&bound, &mut out);
        }
        if let Some((u, _)) = &self.unit {
            u.collect_names(&none, &mut out);
        }
        out
    }
}

fn perr(msg: &str) -> Error {
    Error::Parse {
        pos: 0,
        msg: msg.to_string(),
    }
}

const SYNTAX: &str = "grafiek-syntax: plot x = begin ; einde : f(x), g(x)  of  plot vx : vy";

/// Splits `src` op tokens `sep` die niet tussen haakjes staan. Geeft de stukken als tekst.
fn split_top(src: &str, is_sep: impl Fn(&Tok) -> bool) -> Result<Vec<String>, Error> {
    let toks = lex(src)?;
    let mut depth = 0i32;
    let mut parts = Vec::new();
    let mut start = 0;
    for t in &toks {
        match t.tok {
            Tok::LParen | Tok::LBracket => depth += 1,
            Tok::RParen | Tok::RBracket => depth -= 1,
            ref k if depth == 0 && is_sep(k) => {
                parts.push(src[start..t.start].trim().to_string());
                start = t.end;
            }
            Tok::Eof => parts.push(src[start..].trim().to_string()),
            _ => {}
        }
    }
    Ok(parts)
}

/// Lees de rest van een regel na `plot `.
pub fn parse_plot(rest: &str) -> Result<PlotSpec, Error> {
    // kop en reeksen worden gescheiden door de eerste ':' die geen ':=' is
    let bytes = rest.as_bytes();
    let colon = (0..bytes.len())
        .find(|&i| bytes[i] == b':' && bytes.get(i + 1) != Some(&b'='))
        .ok_or_else(|| perr(SYNTAX))?;
    let (head, body) = (rest[..colon].trim(), rest[colon + 1..].trim());
    let x = match head.split_once('=') {
        Some((var, ranges)) => {
            let var = var.trim().to_string();
            if var.is_empty() || !var.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.') {
                return Err(perr("plot: de naam van de x-variabele is ongeldig"));
            }
            let parts: Vec<&str> = ranges.split(';').collect();
            if !(2..=3).contains(&parts.len()) {
                return Err(perr(SYNTAX));
            }
            PlotX::Range {
                var,
                start: parse_expr_str(parts[0])?,
                end: parse_expr_str(parts[1])?,
                points: parts.get(2).map(|p| parse_expr_str(p)).transpose()?,
            }
        }
        None => PlotX::Data(parse_expr_str(head)?),
    };
    // optioneel `= eenheid` aan het eind
    let pieces = split_top(body, |t| *t == Tok::Eq)?;
    let (series_src, unit) = match pieces.as_slice() {
        [s] => (s.clone(), None),
        [s, u] if !u.is_empty() => (s.clone(), Some((parse_expr_str(u)?, u.clone()))),
        _ => return Err(perr("plot: na '=' hoort één eenheid, bijvoorbeeld '= mm'")),
    };
    let series = split_top(&series_src, |t| *t == Tok::Comma)?
        .into_iter()
        .map(|s| {
            if s.is_empty() {
                return Err(perr("plot: lege reeks (twee komma's achter elkaar?)"));
            }
            Ok((parse_expr_str(&s)?, s))
        })
        .collect::<Result<Vec<_>, Error>>()?;
    if series.is_empty() {
        return Err(perr(SYNTAX));
    }
    Ok(PlotSpec { x, series, unit })
}

fn real_vector(v: Value, what: &str) -> Result<Vec<Quantity>, Error> {
    match v {
        Value::Matrix(m) if m.is_vector() && !m.is_complex() => Ok(m.data),
        Value::Num(q) => Ok(vec![q]),
        other => Err(Error::Domain(format!("plot: {what} moet een reële vector zijn, geen {}", other.kind()))),
    }
}

fn label(unit: &str) -> String {
    if unit.is_empty() {
        String::new()
    } else {
        format!("[{unit}]")
    }
}

/// Reken een grafiek uit.
pub fn eval_plot(env: &Env, spec: &PlotSpec) -> Result<PlotData, Error> {
    // x-waarden en per reeks de y-waarden (SI, met eenheid), NaN bij een fout
    let mut first_error: Option<Error> = None;
    let (xs, xdim, x_name, ys): (Vec<f64>, Dim, String, Vec<Vec<Option<Quantity>>>) = match &spec.x {
        PlotX::Range { var, start, end, points } => {
            let a = env.eval_expr(start)?.as_quantity("het begin van de x-as")?;
            let b = env.eval_expr(end)?.as_quantity("het einde van de x-as")?;
            a.same_dim(b, "de x-as (begin en einde)")?;
            let n = match points {
                Some(p) => env.eval_expr(p)?.as_quantity("het aantal punten")?.dimensionless_value("het aantal punten")?,
                None => DEFAULT_POINTS,
            };
            if n.fract() != 0.0 || !(2.0..=100_000.0).contains(&n) {
                return Err(Error::Domain("plot: het aantal punten moet een geheel getal van 2 tot en met 100000 zijn".into()));
            }
            let n = n as usize;
            let xs: Vec<f64> = (0..n).map(|k| a.value + (b.value - a.value) * k as f64 / (n - 1) as f64).collect();
            let mut env2 = env.clone();
            let mut ys = vec![Vec::with_capacity(n); spec.series.len()];
            for &x in &xs {
                env2.set_var(var, Quantity::new(x, a.dim));
                for (s, (e, _)) in spec.series.iter().enumerate() {
                    match env2.eval_expr(e).and_then(|v| v.as_quantity("een reeks")) {
                        Ok(q) => ys[s].push(Some(q)),
                        Err(err) => {
                            first_error.get_or_insert(err);
                            ys[s].push(None);
                        }
                    }
                }
            }
            (xs, a.dim, var.clone(), ys)
        }
        // een bereikvariabele op de x-as, zoals in Mathcad: `plot ω : T(ω)` met `ω := 0, 1 .. 300`
        PlotX::Data(Expr::Var(name)) if matches!(env.eval_expr(&Expr::Var(name.clone())), Ok(Value::Range(_))) => {
            let Ok(Value::Range(r)) = env.eval_expr(&Expr::Var(name.clone())) else { unreachable!() };
            if r.n > 100_000 {
                return Err(Error::Domain("plot: het bereik heeft meer dan 100000 waarden".into()));
            }
            let mut env2 = env.clone();
            let mut ys = vec![Vec::with_capacity(r.n); spec.series.len()];
            let mut xs = Vec::with_capacity(r.n);
            for k in 0..r.n {
                let x = r.get(k);
                xs.push(x.value);
                env2.set_var(name, x);
                for (s, (e, _)) in spec.series.iter().enumerate() {
                    match env2.eval_expr(e).and_then(|v| v.as_quantity("een reeks")) {
                        Ok(q) => ys[s].push(Some(q)),
                        Err(err) => {
                            first_error.get_or_insert(err);
                            ys[s].push(None);
                        }
                    }
                }
            }
            (xs, r.start.dim, name.clone(), ys)
        }
        PlotX::Data(e) => {
            let vx = real_vector(env.eval_expr(e)?, "de x-vector")?;
            let xdim = vx.first().map_or(Dim::NONE, |q| q.dim);
            if vx.iter().any(|q| q.dim != xdim) {
                return Err(Error::Domain("plot: alle x-waarden moeten dezelfde eenheid hebben".into()));
            }
            let mut ys = Vec::new();
            for (e, src) in &spec.series {
                let vy = real_vector(env.eval_expr(e)?, "een y-vector")?;
                if vy.len() != vx.len() {
                    return Err(Error::Domain(format!(
                        "plot: {src} heeft {} waarden, de x-vector {}",
                        vy.len(),
                        vx.len()
                    )));
                }
                ys.push(vy.into_iter().map(Some).collect());
            }
            let src = match e {
                Expr::Var(n) => n.clone(),
                _ => "x".to_string(),
            };
            (vx.iter().map(|q| q.value).collect(), xdim, src, ys)
        }
    };
    let ydim = ys.iter().flatten().flatten().map(|q| q.dim).next();
    let Some(ydim) = ydim else {
        return Err(first_error.unwrap_or_else(|| Error::Domain("plot: geen enkel punt uit te rekenen".into())));
    };
    if ys.iter().flatten().flatten().any(|q| q.dim != ydim) {
        return Err(Error::Domain("plot: alle reeksen moeten dezelfde eenheid hebben".into()));
    }
    // weergave-eenheden
    let xmag = xs.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    let (xdiv, xunit) = auto_scale(xmag, xdim);
    let (ydiv, yunit) = match &spec.unit {
        Some((u, text)) => {
            let uq = env.eval_expr(u)?.as_quantity("de eenheid van de y-as")?;
            uq.same_dim(Quantity::new(0.0, ydim), "de y-as (reeksen en eenheid)")?;
            if uq.value == 0.0 {
                return Err(Error::Domain("plot: de eenheid van de y-as is 0".into()));
            }
            (uq.value, text.clone())
        }
        None => {
            let ymag = ys.iter().flatten().flatten().fold(0.0f64, |m, q| m.max(q.value.abs()));
            auto_scale(ymag, ydim)
        }
    };
    let series = spec
        .series
        .iter()
        .zip(ys)
        .map(|((_, src), vals)| Series {
            label: src.clone(),
            points: xs
                .iter()
                .zip(vals)
                .map(|(x, y)| [x / xdiv, y.map_or(f64::NAN, |q| q.value / ydiv)])
                .collect(),
        })
        .collect();
    Ok(PlotData {
        x_label: format!("{x_name} {}", label(&xunit)).trim().to_string(),
        y_label: label(&yunit),
        series,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grafieksyntax() {
        let p = parse_plot("x = 0 ; 10 : sin(x), max(x, 1)").unwrap();
        assert_eq!(p.series.len(), 2);
        assert_eq!(p.series[1].1, "max(x, 1)");
        assert!(p.uses().contains("sin") && !p.uses().contains("x"));
        let p = parse_plot("t = 0 s ; 2 s ; 50 : u(t) = mm").unwrap();
        assert_eq!(p.unit.unwrap().1, "mm");
        assert!(matches!(parse_plot("vx : vy").unwrap().x, PlotX::Data(_)));
        assert!(parse_plot("x = 0 : sin(x)").is_err());
        assert!(parse_plot("x = 0 ; 1").is_err());
    }

    #[test]
    fn grafiek_uitrekenen_met_eenheden() {
        let env = Env::new();
        let p = parse_plot("t = 0 s ; 2 s ; 3 : 5 mm * t / s = mm").unwrap();
        let d = eval_plot(&env, &p).unwrap();
        assert_eq!(d.x_label, "t [s]");
        assert_eq!(d.y_label, "[mm]");
        assert_eq!(d.series[0].points, vec![[0.0, 0.0], [1.0, 5.0], [2.0, 10.0]]);
        // reeksen met verschillende eenheden in één grafiek kan niet
        let p = parse_plot("t = 0 s ; 2 s ; 3 : 5 mm * t / s, t / s").unwrap();
        assert!(eval_plot(&env, &p).unwrap_err().to_string().contains("dezelfde eenheid"));
        let p = parse_plot("x = 0 ; 2 ; 3 : 1 / (x - 1)").unwrap();
        let d = eval_plot(&env, &p).unwrap();
        assert!(d.series[0].points[1][1].is_nan());
        assert_eq!(d.series[0].points[2], [2.0, 1.0]);
    }
}

//! Grafiekregio's.
//!
//! ```text
//! plot x = 0 ; 10 : sin(x), cos(x)            functies van x, 200 punten
//! plot t = 0 s ; 2 s ; 500 : u(t) = mm        500 punten, y-as in mm
//! plot vx : vy, vz                            vectoren (meetpunten) tegen elkaar
//! plot f = 10 ; 1e5 : H(f) | log xy            logaritmische assen (Bode)
//! plot x = 0 ; 10 : sin(x) | y = -2 ; 2         vaste grenzen van de y-as
//! ```
//!
//! Opties staan na de laatste `|`, gescheiden door komma's: `log x`, `log y`, `log xy`,
//! `x = a ; b` en `y = a ; b`. Grenzen zonder eenheid gelden in de eenheid van de as.
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
    /// Een tweede variabele (`y = c ; d`): een oppervlak z = f(x, y).
    pub y: Option<(String, Expr, Expr, Option<Expr>)>,
    /// Reeksen op de rechter y-as (na `;`) en hun eenheid.
    pub series2: Vec<(Expr, String)>,
    pub unit2: Option<(Expr, String)>,
    pub opts: PlotOptions,
}

/// Een oppervlak z = f(x, y) op een rooster, in weergave-eenheden.
#[derive(Debug, Clone, PartialEq)]
pub struct Surface {
    pub xs: Vec<f64>,
    pub ys: Vec<f64>,
    /// Rij voor rij: `z[j * xs.len() + i]` hoort bij (xs[i], ys[j]); NaN waar niet uit te rekenen.
    pub z: Vec<f64>,
    pub y_label: String,
    pub z_label: String,
    /// Hoogtelijnen in plaats van een 3D-oppervlak.
    pub contour: bool,
}

/// De opties na `|`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlotOptions {
    pub log_x: bool,
    pub log_y: bool,
    pub x_range: Option<(Expr, Expr)>,
    pub y_range: Option<(Expr, Expr)>,
    /// `polar`: x is de hoek θ, de reeksen zijn de straal r(θ).
    pub polar: bool,
    /// `contour`: een oppervlak als hoogtelijnen.
    pub contour: bool,
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
    pub log_x: bool,
    pub log_y: bool,
    /// Vaste grenzen in weergave-eenheden.
    pub x_range: Option<(f64, f64)>,
    pub y_range: Option<(f64, f64)>,
    /// Poolgrafiek: de punten zijn al (r cos θ, r sin θ); x en y even groot tekenen.
    pub polar: bool,
    /// Reeksen op de rechter y-as en het label van die as.
    pub series2: Vec<Series>,
    pub y2_label: String,
    /// Een oppervlak (`plot x = .., y = .. : f(x, y)`); dan zijn `series` leeg.
    pub surface: Option<Surface>,
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
        if let Some((var, start, end, points)) = &self.y {
            start.collect_names(&none, &mut out);
            end.collect_names(&none, &mut out);
            if let Some(p) = points {
                p.collect_names(&none, &mut out);
            }
            bound.insert(var.clone());
        }
        for (e, _) in self.series.iter().chain(&self.series2) {
            e.collect_names(&bound, &mut out);
        }
        for (u, _) in self.unit.iter().chain(&self.unit2) {
            u.collect_names(&none, &mut out);
        }
        for (a, b) in self.opts.x_range.iter().chain(&self.opts.y_range) {
            a.collect_names(&none, &mut out);
            b.collect_names(&none, &mut out);
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

/// Lees de opties na `|`: `log x`, `log y`, `log xy`, `x = a ; b`, `y = a ; b`.
/// `None` als het geen optielijst is (dan hoort de `|` bij een absolute waarde).
fn parse_options(src: &str) -> Option<Result<PlotOptions, Error>> {
    let src = src.trim();
    if src.is_empty() {
        return None;
    }
    let mut o = PlotOptions::default();
    for item in src.split(',') {
        let item = item.trim();
        if item == "polar" || item == "pool" {
            o.polar = true;
        } else if item == "contour" || item == "hoogtelijnen" {
            o.contour = true;
        } else if let Some(axes) = item.strip_prefix("log") {
            let axes = axes.trim();
            if axes.is_empty() || !axes.chars().all(|c| c == 'x' || c == 'y') {
                return None;
            }
            o.log_x |= axes.contains('x');
            o.log_y |= axes.contains('y');
        } else if let Some((axis, range)) = item.split_once('=') {
            let axis = axis.trim();
            if axis != "x" && axis != "y" {
                return None;
            }
            let Some((a, b)) = range.split_once(';') else {
                return Some(Err(perr("plot: een grens schrijf je als 'x = begin ; einde'")));
            };
            let r = match (parse_expr_str(a), parse_expr_str(b)) {
                (Ok(a), Ok(b)) => (a, b),
                (Err(e), _) | (_, Err(e)) => return Some(Err(e)),
            };
            if axis == "x" {
                o.x_range = Some(r);
            } else {
                o.y_range = Some(r);
            }
        } else {
            return None;
        }
    }
    Some(Ok(o))
}

/// `x = begin ; einde [; aantal]`: (naam, begin, einde, aantal).
fn parse_range(src: &str) -> Result<(String, Expr, Expr, Option<Expr>), Error> {
    let (var, ranges) = src.split_once('=').ok_or_else(|| perr(SYNTAX))?;
    let var = var.trim().to_string();
    if var.is_empty() || !var.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.') {
        return Err(perr("plot: de naam van de x-variabele is ongeldig"));
    }
    let parts: Vec<&str> = ranges.split(';').collect();
    if !(2..=3).contains(&parts.len()) {
        return Err(perr(SYNTAX));
    }
    Ok((var, parse_expr_str(parts[0])?, parse_expr_str(parts[1])?, parts.get(2).map(|p| parse_expr_str(p)).transpose()?))
}

/// Reeksen met een optionele eenheid: `f(x), g(x) = mm`.
fn parse_group(src: &str) -> Result<(Vec<(Expr, String)>, Option<(Expr, String)>), Error> {
    let pieces = split_top(src, |t| *t == Tok::Eq)?;
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
    Ok((series, unit))
}

/// Lees de rest van een regel na `plot `.
pub fn parse_plot(rest: &str) -> Result<PlotSpec, Error> {
    // kop en reeksen worden gescheiden door de eerste ':' die geen ':=' is
    let bytes = rest.as_bytes();
    let colon = (0..bytes.len())
        .find(|&i| bytes[i] == b':' && bytes.get(i + 1) != Some(&b'='))
        .ok_or_else(|| perr(SYNTAX))?;
    let (head, mut body) = (rest[..colon].trim(), rest[colon + 1..].trim());
    // opties na de laatste '|', als wat erna staat echt opties zijn
    let mut opts = PlotOptions::default();
    if let Some(bar) = body.rfind('|') {
        if let Some(o) = parse_options(&body[bar + 1..]) {
            opts = o?;
            body = body[..bar].trim();
        }
    }
    // twee variabelen (`x = .., y = ..`): een oppervlak
    let axes = split_top(head, |t| *t == Tok::Comma)?;
    let (x, y) = match axes.as_slice() {
        [one] if one.contains('=') => {
            let (var, start, end, points) = parse_range(one)?;
            (PlotX::Range { var, start, end, points }, None)
        }
        [one] => (PlotX::Data(parse_expr_str(one)?), None),
        [a, b] if a.contains('=') && b.contains('=') => {
            let (var, start, end, points) = parse_range(a)?;
            (PlotX::Range { var, start, end, points }, Some(parse_range(b)?))
        }
        _ => return Err(perr(SYNTAX)),
    };
    // `;` tussen twee groepen reeksen: de tweede groep hoort bij de rechter y-as
    let groups = split_top(body, |t| *t == Tok::Semicolon)?;
    let (series, unit, series2, unit2) = match groups.as_slice() {
        [g] => {
            let (s, u) = parse_group(g)?;
            (s, u, Vec::new(), None)
        }
        [g1, g2] => {
            let (s, u) = parse_group(g1)?;
            let (s2, u2) = parse_group(g2)?;
            (s, u, s2, u2)
        }
        _ => return Err(perr("plot: hooguit twee groepen reeksen (links en rechts), gescheiden door ';'")),
    };
    if y.is_some() && (series.len() != 1 || !series2.is_empty()) {
        return Err(perr("plot: een oppervlak heeft één functie: plot x = a ; b, y = c ; d : f(x, y)"));
    }
    if opts.polar && (y.is_some() || !series2.is_empty()) {
        return Err(perr("plot: polar hoort bij een gewone grafiek met één y-as"));
    }
    Ok(PlotSpec { x, y, series, unit, series2, unit2, opts })
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

/// Gelijk verdeelde waarden van `start` tot `end` (in SI) en hun eenheid.
fn axis_values(env: &Env, start: &Expr, end: &Expr, points: &Option<Expr>, default: f64, max: f64, log: bool) -> Result<(Vec<f64>, Dim), Error> {
    let a = env.eval_expr(start)?.as_quantity("het begin van de x-as")?;
    let b = env.eval_expr(end)?.as_quantity("het einde van de x-as")?;
    a.same_dim(b, "de x-as (begin en einde)")?;
    let n = match points {
        Some(p) => env.eval_expr(p)?.as_quantity("het aantal punten")?.dimensionless_value("het aantal punten")?,
        None => default,
    };
    if n.fract() != 0.0 || !(2.0..=max).contains(&n) {
        return Err(Error::Domain(format!("plot: het aantal punten moet een geheel getal van 2 tot en met {max} zijn")));
    }
    let n = n as usize;
    let xs: Vec<f64> = if log {
        // geometrisch verdeeld, zodat elke decade evenveel punten krijgt
        if !(a.value > 0.0 && b.value > 0.0) {
            return Err(Error::Domain("plot: een logaritmische x-as vraagt een begin en einde groter dan 0".into()));
        }
        let (la, lb) = (a.value.ln(), b.value.ln());
        (0..n).map(|k| (la + (lb - la) * k as f64 / (n - 1) as f64).exp()).collect()
    } else {
        (0..n).map(|k| a.value + (b.value - a.value) * k as f64 / (n - 1) as f64).collect()
    };
    Ok((xs, a.dim))
}

/// De weergave-eenheid van een groep reeksen: (dimensie, deler, eenheidstekst), of `None` als er geen
/// enkel punt is.
fn group_scale(env: &Env, ys: &[Vec<Option<Quantity>>], unit: &Option<(Expr, String)>, axis: &str) -> Result<Option<(Dim, f64, String)>, Error> {
    let Some(ydim) = ys.iter().flatten().flatten().map(|q| q.dim).next() else { return Ok(None) };
    if ys.iter().flatten().flatten().any(|q| q.dim != ydim) {
        return Err(Error::Domain(format!("plot: alle reeksen op de {axis} moeten dezelfde eenheid hebben")));
    }
    Ok(Some(match unit {
        Some((u, text)) => {
            let uq = env.eval_expr(u)?.as_quantity("de eenheid van de y-as")?;
            uq.same_dim(Quantity::new(0.0, ydim), "de y-as (reeksen en eenheid)")?;
            if uq.value == 0.0 {
                return Err(Error::Domain("plot: de eenheid van de y-as is 0".into()));
            }
            (ydim, uq.value, text.clone())
        }
        None => {
            let ymag = ys.iter().flatten().flatten().fold(0.0f64, |m, q| m.max(q.value.abs()));
            let (div, text) = auto_scale(ymag, ydim);
            (ydim, div, text)
        }
    }))
}

/// Reken een grafiek uit.
pub fn eval_plot(env: &Env, spec: &PlotSpec) -> Result<PlotData, Error> {
    if let Some(yaxis) = &spec.y {
        return eval_surface(env, spec, yaxis);
    }
    // alle reeksen: eerst de linker, dan de rechter y-as
    let all: Vec<&(Expr, String)> = spec.series.iter().chain(&spec.series2).collect();
    // x-waarden en per reeks de y-waarden (SI, met eenheid), NaN bij een fout
    let mut first_error: Option<Error> = None;
    let (xs, xdim, x_name, mut ys): (Vec<f64>, Dim, String, Vec<Vec<Option<Quantity>>>) = match &spec.x {
        PlotX::Range { var, start, end, points } => {
            let (xs, xdim) = axis_values(env, start, end, points, DEFAULT_POINTS, 100_000.0, spec.opts.log_x)?;
            let mut env2 = env.clone();
            let mut ys = vec![Vec::with_capacity(xs.len()); all.len()];
            for &x in &xs {
                env2.set_var(var, Quantity::new(x, xdim));
                for (s, (e, _)) in all.iter().enumerate() {
                    match env2.eval_expr(e).and_then(|v| v.as_quantity("een reeks")) {
                        Ok(q) => ys[s].push(Some(q)),
                        Err(err) => {
                            first_error.get_or_insert(err);
                            ys[s].push(None);
                        }
                    }
                }
            }
            (xs, xdim, var.clone(), ys)
        }
        // een bereikvariabele op de x-as, zoals in Mathcad: `plot ω : T(ω)` met `ω := 0, 1 .. 300`
        PlotX::Data(Expr::Var(name)) if matches!(env.eval_expr(&Expr::Var(name.clone())), Ok(Value::Range(_))) => {
            let Ok(Value::Range(r)) = env.eval_expr(&Expr::Var(name.clone())) else { unreachable!() };
            if r.n > 100_000 {
                return Err(Error::Domain("plot: het bereik heeft meer dan 100000 waarden".into()));
            }
            let mut env2 = env.clone();
            let mut ys = vec![Vec::with_capacity(r.n); all.len()];
            let mut xs = Vec::with_capacity(r.n);
            for k in 0..r.n {
                let x = r.get(k);
                xs.push(x.value);
                env2.set_var(name, x);
                for (s, (e, _)) in all.iter().enumerate() {
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
            for (e, src) in &all {
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
    let ys2 = ys.split_off(spec.series.len());
    let no_points = |e: Option<Error>| e.unwrap_or_else(|| Error::Domain("plot: geen enkel punt uit te rekenen".into()));
    let Some((ydim, ydiv, yunit)) = group_scale(env, &ys, &spec.unit, "y-as")? else { return Err(no_points(first_error)) };
    let right = if spec.series2.is_empty() {
        None
    } else {
        Some(group_scale(env, &ys2, &spec.unit2, "rechter y-as")?.ok_or_else(|| no_points(first_error.take()))?)
    };
    // weergave-eenheden van de x-as (bij een poolgrafiek: de hoek blijft in radialen)
    if spec.opts.polar && xdim != Dim::NONE {
        return Err(Error::Domain("plot: bij polar is de x-variabele een hoek (zonder eenheid, in radialen)".into()));
    }
    let xmag = xs.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    let (xdiv, xunit) = if spec.opts.polar { (ydiv, yunit.clone()) } else { auto_scale(xmag, xdim) };
    // vaste grenzen: met eenheid omgerekend, zonder eenheid al in weergave-eenheden
    let range = |r: &Option<(Expr, Expr)>, dim: Dim, div: f64, what: &'static str| -> Result<Option<(f64, f64)>, Error> {
        let Some((a, b)) = r else { return Ok(None) };
        let mut ends = [0.0; 2];
        for (k, e) in [a, b].into_iter().enumerate() {
            let q = env.eval_expr(e)?.as_quantity(what)?;
            ends[k] = if q.dim == Dim::NONE && dim != Dim::NONE {
                q.value
            } else {
                q.same_dim(Quantity::new(0.0, dim), what)?;
                q.value / div
            };
        }
        if !(ends[0] < ends[1]) {
            return Err(Error::Domain(format!("plot: {what}: het begin moet kleiner zijn dan het einde")));
        }
        Ok(Some((ends[0], ends[1])))
    };
    let x_range = range(&spec.opts.x_range, if spec.opts.polar { ydim } else { xdim }, xdiv, "de grenzen van de x-as")?;
    let y_range = range(&spec.opts.y_range, ydim, ydiv, "de grenzen van de y-as")?;
    for (log, r, axis) in [(spec.opts.log_x, x_range, "x"), (spec.opts.log_y, y_range, "y")] {
        if log && r.is_some_and(|(a, _)| a <= 0.0) {
            return Err(Error::Domain(format!("plot: een logaritmische {axis}-as kan niet bij 0 of lager beginnen")));
        }
    }
    let polar = spec.opts.polar;
    let make = |list: &[(Expr, String)], ys: Vec<Vec<Option<Quantity>>>, div: f64| -> Vec<Series> {
        list.iter()
            .zip(ys)
            .map(|((_, src), vals)| Series {
                label: src.clone(),
                points: xs
                    .iter()
                    .zip(vals)
                    .map(|(x, y)| {
                        let y = y.map_or(f64::NAN, |q| q.value / div);
                        // poolgrafiek: (r cos θ, r sin θ)
                        if polar {
                            [y * x.cos(), y * x.sin()]
                        } else {
                            [x / xdiv, y]
                        }
                    })
                    .collect(),
            })
            .collect()
    };
    let series = make(&spec.series, ys, ydiv);
    let (series2, y2_label) = match &right {
        Some((_, div2, unit2)) => (make(&spec.series2, ys2, *div2), label(unit2)),
        None => (Vec::new(), String::new()),
    };
    Ok(PlotData {
        x_label: if polar { label(&xunit) } else { format!("{x_name} {}", label(&xunit)).trim().to_string() },
        y_label: label(&yunit),
        series,
        log_x: spec.opts.log_x && !polar,
        log_y: spec.opts.log_y && !polar,
        x_range,
        y_range,
        polar,
        series2,
        y2_label,
        surface: None,
    })
}

/// Een oppervlak z = f(x, y) op een rooster (standaard 40 × 40 punten).
fn eval_surface(env: &Env, spec: &PlotSpec, yaxis: &(String, Expr, Expr, Option<Expr>)) -> Result<PlotData, Error> {
    let PlotX::Range { var: xvar, start, end, points } = &spec.x else {
        return Err(perr("plot: een oppervlak vraagt twee bereiken: plot x = a ; b, y = c ; d : f(x, y)"));
    };
    let (yvar, ystart, yend, ypoints) = yaxis;
    let (xs, xdim) = axis_values(env, start, end, points, 40.0, 300.0, false)?;
    let (ys, ydim) = axis_values(env, ystart, yend, ypoints, 40.0, 300.0, false)?;
    let f = &spec.series[0].0;
    let mut env2 = env.clone();
    let mut zs: Vec<Option<Quantity>> = Vec::with_capacity(xs.len() * ys.len());
    let mut first_error: Option<Error> = None;
    for &y in &ys {
        env2.set_var(yvar, Quantity::new(y, ydim));
        for &x in &xs {
            env2.set_var(xvar, Quantity::new(x, xdim));
            match env2.eval_expr(f).and_then(|v| v.as_quantity("het oppervlak")) {
                Ok(q) => zs.push(Some(q)),
                Err(err) => {
                    first_error.get_or_insert(err);
                    zs.push(None);
                }
            }
        }
    }
    let Some((_, zdiv, zunit)) = group_scale(env, std::slice::from_ref(&zs), &spec.unit, "z-as")? else {
        return Err(first_error.unwrap_or_else(|| Error::Domain("plot: geen enkel punt uit te rekenen".into())));
    };
    let mag = |v: &[f64]| v.iter().fold(0.0f64, |m, x| m.max(x.abs()));
    let (xdiv, xunit) = auto_scale(mag(&xs), xdim);
    let (ydiv, yunit) = auto_scale(mag(&ys), ydim);
    Ok(PlotData {
        x_label: format!("{xvar} {}", label(&xunit)).trim().to_string(),
        y_label: String::new(),
        series: Vec::new(),
        log_x: false,
        log_y: false,
        x_range: None,
        y_range: None,
        polar: false,
        series2: Vec::new(),
        y2_label: String::new(),
        surface: Some(Surface {
            xs: xs.iter().map(|x| x / xdiv).collect(),
            ys: ys.iter().map(|y| y / ydiv).collect(),
            z: zs.iter().map(|z| z.map_or(f64::NAN, |q| q.value / zdiv)).collect(),
            y_label: format!("{yvar} {}", label(&yunit)).trim().to_string(),
            z_label: format!("{} {}", spec.series[0].1, label(&zunit)).trim().to_string(),
            contour: spec.opts.contour,
        }),
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
    fn grafiekopties() {
        let p = parse_plot("f = 10 ; 1e5 : 1 / sqrt(1 + (f / 1000)^2) | log xy").unwrap();
        assert!(p.opts.log_x && p.opts.log_y && p.series.len() == 1);
        let p = parse_plot("x = -1 ; 1 : abs(x)").unwrap();
        assert_eq!(p.opts, PlotOptions::default());
        let p = parse_plot("x = -1 ; 1 : abs(x) * 2 = 1 | y = 0 ; 3, log x").unwrap();
        assert_eq!(p.series[0].1, "abs(x) * 2");
        assert!(p.unit.is_some() && p.opts.y_range.is_some() && p.opts.log_x && !p.opts.log_y);
        assert!(parse_plot("x = 0 ; 1 : x | y = 0").is_err());
    }

    #[test]
    fn pool_twee_assen_en_oppervlak() {
        let env = Env::new();
        // cirkel met straal 2 mm: alle punten op afstand 2 (in mm)
        let p = parse_plot("θ = 0 ; 2*π ; 9 : 2 mm | polar").unwrap();
        let d = eval_plot(&env, &p).unwrap();
        assert!(d.polar && d.x_label == "[mm]");
        assert!(d.series[0].points.iter().all(|q| ((q[0] * q[0] + q[1] * q[1]).sqrt() - 2.0).abs() < 1e-12));
        // kracht links in kN, verplaatsing rechts in mm
        let p = parse_plot("t = 0 s ; 2 s ; 3 : 3 kN * t / s ; 5 mm * t / s").unwrap();
        assert_eq!((p.series.len(), p.series2.len()), (1, 1));
        let d = eval_plot(&env, &p).unwrap();
        assert_eq!((d.y_label.as_str(), d.y2_label.as_str()), ("[kN]", "[mm]"));
        assert_eq!(d.series2[0].points[2], [2.0, 10.0]);
        // oppervlak z = x·y op 3 × 2 punten
        let p = parse_plot("x = 0 ; 2 ; 3, y = 0 ; 1 ; 2 : x * y | contour").unwrap();
        let d = eval_plot(&env, &p).unwrap();
        let s = d.surface.unwrap();
        assert!(s.contour && s.xs == vec![0.0, 1.0, 2.0] && s.ys == vec![0.0, 1.0]);
        assert_eq!(s.z, vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0]);
        // een oppervlak heeft één functie
        assert!(parse_plot("x = 0 ; 1, y = 0 ; 1 : x, y").is_err());
    }

    #[test]
    fn bodediagram() {
        // eerste-orde laagdoorlaatfilter met kantelfrequentie 1 kHz
        let env = Env::new();
        let p = parse_plot("f = 10 Hz ; 100 kHz ; 5 : 1 / sqrt(1 + (f / 1 kHz)^2) | log xy, y = 1e-3 ; 2").unwrap();
        let d = eval_plot(&env, &p).unwrap();
        assert!(d.log_x && d.log_y);
        assert_eq!(d.y_range, Some((1e-3, 2.0)));
        // een punt per decade: steeds dezelfde verhouding
        let xs: Vec<f64> = d.series[0].points.iter().map(|p| p[0]).collect();
        let ratio = xs[1] / xs[0];
        assert!((ratio - 10.0).abs() < 1e-9);
        assert!(xs.windows(2).all(|w| (w[1] / w[0] - ratio).abs() < 1e-9 * ratio));
        // bij de kantelfrequentie is de versterking 1/√2
        let mid = d.series[0].points[2][1];
        assert!((mid - 1.0 / 2f64.sqrt()).abs() < 1e-12);
        // logaritmisch vanaf 0 kan niet
        let p = parse_plot("x = 0 ; 10 : x | log x").unwrap();
        assert!(eval_plot(&env, &p).unwrap_err().to_string().contains("groter dan 0"));
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

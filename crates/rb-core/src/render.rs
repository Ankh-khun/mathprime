//! Tekstweergave van waarden: getallen met eenheid, complexe getallen, matrices en bereiken.

use crate::error::Error;
use crate::value::{Complex, Matrix, RangeVal, Value};
use rb_units::{
    auto_scale, format_in_unit, format_number, format_quantity, join_unit, lookup_offset_unit,
    named_scale, Dim, Quantity, UnitError,
};

const TEMPERATURE: Dim = Dim::new(0, 0, 0, 0, 1, 0, 0);

/// Hoe een resultaat getoond wordt.
#[derive(Debug, Clone, PartialEq)]
pub enum Display {
    /// Een door de gebruiker gekozen eenheid (`= kN`): de eenheid als grootheid en de tekst zoals getypt.
    Unit { unit: Quantity, text: String },
    /// Een temperatuurschaal met nulpunt (`= °C`).
    Offset { name: String, factor: f64, offset: f64 },
    /// Een eenheid die uit de definitie volgt (`E := 5 J` → `E =` toont joule). Valt terug op de
    /// automatische weergave als de dimensie niet past.
    Hint(String),
}

/// Kies deler en eenheidstekst voor waarden met dimensie `dim` en grootte `magnitude`.
fn scale_for(dim: Dim, magnitude: f64, display: &Option<Display>) -> Result<(f64, String), Error> {
    match display {
        Some(Display::Unit { unit, text }) => {
            if unit.dim != dim {
                return Err(UnitError::Mismatch {
                    op: "weergeven in deze eenheid",
                    left: dim,
                    right: unit.dim,
                }
                .into());
            }
            Ok((unit.value, text.trim().to_string()))
        }
        Some(Display::Offset { name, .. }) => Err(Error::Domain(format!(
            "{name} kan alleen bij een losse temperatuur; gebruik K of Δ{}",
            name.trim_start_matches("deg").trim_start_matches('°')
        ))),
        Some(Display::Hint(h)) => Ok(named_scale(h, magnitude, dim)
            .or_else(|| {
                // samengestelde eenheid zoals `kN/m`: precies zo tonen als getypt
                let u = crate::eval::unit_text_value(h)?;
                (u.dim == dim && u.value != 0.0).then(|| (u.value, h.clone()))
            })
            .unwrap_or_else(|| auto_scale(magnitude, dim))),
        None => Ok(auto_scale(magnitude, dim)),
    }
}

fn offset_text(q: Quantity, name: &str, factor: f64, offset: f64) -> Result<String, Error> {
    if q.dim != TEMPERATURE {
        return Err(UnitError::Mismatch {
            op: "weergeven in deze eenheid",
            left: q.dim,
            right: TEMPERATURE,
        }
        .into());
    }
    Ok(format!("{} {name}", format_number((q.value - offset) / factor)))
}

/// Tekstweergave van een resultaat, in de gevraagde eenheid of automatisch. Matrices met meer dan
/// één rij beslaan meerdere regels.
pub fn render_value(value: &Value, display: &Option<Display>) -> Result<String, Error> {
    match value {
        Value::Str(s) => match display {
            Some(Display::Unit { .. } | Display::Offset { .. }) => Err(Error::Domain("tekst heeft geen eenheid; laat het vakje achter '=' leeg".into())),
            _ => Ok(format!("\"{s}\"")),
        },
        Value::Num(q) => match display {
            Some(Display::Unit { unit, text }) => Ok(format_in_unit(*q, *unit, text)?),
            Some(Display::Offset { name, factor, offset }) => offset_text(*q, name, *factor, *offset),
            Some(Display::Hint(h)) => {
                if let Some((f, o)) = lookup_offset_unit(h) {
                    if q.dim == TEMPERATURE {
                        return offset_text(*q, h, f, o);
                    }
                }
                let (div, unit) = scale_for(q.dim, q.value, display)?;
                Ok(join_unit(&format_number(q.value / div), &unit))
            }
            None => Ok(format_quantity(*q)),
        },
        Value::Complex(c) => {
            let (div, unit) = scale_for(c.dim, c.abs(), display)?;
            Ok(complex_with_unit(c.re / div, c.im / div, &unit))
        }
        Value::Matrix(m) => render_matrix(m, display),
        Value::Range(r) => render_range(r, display),
    }
}

/// Getal en eenheid van een (complex) getal apart, zoals `render_value` ze toont: (`12.5`, `MPa`).
/// `None` voor matrices en bereiken (zie [`matrix_cells`]).
pub fn scalar_parts(value: &Value, display: &Option<Display>) -> Result<Option<(String, String)>, Error> {
    Ok(Some(match value {
        Value::Num(q) => {
            let offset = match display {
                Some(Display::Offset { name, factor, offset }) => Some((name.clone(), *factor, *offset)),
                Some(Display::Hint(h)) if q.dim == TEMPERATURE => lookup_offset_unit(h).map(|(f, o)| (h.clone(), f, o)),
                _ => None,
            };
            match offset {
                Some((name, f, o)) => {
                    // zelfde controle en tekst als offset_text
                    let t = offset_text(*q, &name, f, o)?;
                    let (n, u) = t.split_once(' ').unwrap_or((&t, ""));
                    (n.to_string(), u.to_string())
                }
                None => {
                    let (div, unit) = match display {
                        None => auto_scale(q.value, q.dim),
                        _ => scale_for(q.dim, q.value, display)?,
                    };
                    (format_number(q.value / div), unit)
                }
            }
        }
        Value::Complex(c) => {
            let (div, unit) = scale_for(c.dim, c.abs(), display)?;
            (complex_text(c.re / div, c.im / div), unit)
        }
        _ => return Ok(None),
    }))
}

/// `3 + 4i`, `-2i`, `1.5`. Een verwaarloosbaar klein deel (< 1e-12 van het andere) wordt weggelaten.
pub fn complex_text(re: f64, im: f64) -> String {
    let tiny = 1e-12 * re.abs().max(im.abs());
    if im.abs() <= tiny {
        return format_number(re);
    }
    if re.abs() <= tiny {
        return format!("{}i", format_number(im));
    }
    let sign = if im < 0.0 { "-" } else { "+" };
    format!("{} {sign} {}i", format_number(re), format_number(im.abs()))
}

fn complex_with_unit(re: f64, im: f64, unit: &str) -> String {
    let t = complex_text(re, im);
    if unit.is_empty() {
        t
    } else if t.contains(' ') {
        format!("({t}) {unit}")
    } else {
        format!("{t} {unit}")
    }
}

/// Weergave van een complexe grootheid (automatische eenheid).
pub fn format_complex(c: Complex) -> String {
    let (div, unit) = auto_scale(c.abs(), c.dim);
    complex_with_unit(c.re / div, c.im / div, &unit)
}

/// De opgemaakte cellen van een matrix (rij voor rij) en de gemeenschappelijke eenheid (leeg als die
/// er niet is of als de matrix dimensieloos is). Basis voor de tekstweergave en de formuleweergave.
#[derive(Debug, Clone, PartialEq)]
pub struct MatrixCells {
    pub rows: usize,
    pub cols: usize,
    pub cells: Vec<String>,
    pub unit: String,
}

pub fn matrix_cells(m: &Matrix, display: &Option<Display>) -> Result<MatrixCells, Error> {
    let elems = m.elems();
    let (cells, unit): (Vec<String>, String) = match m.common_dim() {
        Some(dim) => {
            let mag = elems.iter().fold(0.0f64, |a, z| a.max(z.abs()));
            let (div, unit) = scale_for(dim, mag, display)?;
            // afrondingsruis (zoals 1e-16 naast 1 na een inverse) tonen als 0, zoals in Mathcad
            let zero = 1e-13 * mag;
            let clip = |v: f64| if v.abs() < zero { 0.0 } else { v } / div;
            let cells = elems
                .iter()
                .map(|z| {
                    if m.is_complex() {
                        complex_text(clip(z.re), clip(z.im))
                    } else {
                        format_number(clip(z.re))
                    }
                })
                .collect();
            (cells, unit)
        }
        None => {
            if display.is_some() && !matches!(display, Some(Display::Hint(_))) {
                return Err(Error::Domain(
                    "de elementen hebben verschillende eenheden; toon ze zonder gekozen eenheid".to_string(),
                ));
            }
            (elems.iter().map(|z| format_complex(*z)).collect(), String::new())
        }
    };
    Ok(MatrixCells {
        rows: m.rows,
        cols: m.cols,
        cells,
        unit,
    })
}

fn render_matrix(m: &Matrix, display: &Option<Display>) -> Result<String, Error> {
    let MatrixCells { cells, unit, .. } = matrix_cells(m, display)?;
    if m.rows == 1 {
        return Ok(join_unit(&format!("[{}]", cells.join(", ")), &unit));
    }
    let widths: Vec<usize> = (0..m.cols)
        .map(|c| (0..m.rows).map(|r| cells[r * m.cols + c].chars().count()).max().unwrap_or(0))
        .collect();
    let mid = (m.rows - 1) / 2;
    let lines: Vec<String> = (0..m.rows)
        .map(|r| {
            let (l, rb) = match r {
                0 => ('⎡', '⎤'),
                _ if r == m.rows - 1 => ('⎣', '⎦'),
                _ => ('⎢', '⎥'),
            };
            let row: Vec<String> = (0..m.cols)
                .map(|c| format!("{:>w$}", cells[r * m.cols + c], w = widths[c]))
                .collect();
            let mut line = format!("{l} {} {rb}", row.join("  "));
            if r == mid && !unit.is_empty() {
                line.push(' ');
                line.push_str(&unit);
            }
            line
        })
        .collect();
    Ok(lines.join("\n"))
}

fn render_range(r: &RangeVal, display: &Option<Display>) -> Result<String, Error> {
    let mag = r.start.value.abs().max(r.last().value.abs());
    let (div, unit) = scale_for(r.start.dim, mag, display)?;
    let num = |k: usize| format_number(r.get(k).value / div);
    let list = if r.n <= 10 {
        (0..r.n).map(num).collect::<Vec<_>>().join(", ")
    } else {
        format!(
            "{}, {}, {}, …, {}, {}",
            num(0),
            num(1),
            num(2),
            num(r.n - 2),
            num(r.n - 1)
        )
    };
    let text = join_unit(&list, &unit);
    Ok(if r.n > 10 {
        format!("{text}  ({} waarden)", r.n)
    } else {
        text
    })
}

/// Zet `prefix` voor de eerste regel van `text` en spring de volgende regels even ver in, zodat
/// een matrix netjes onder zichzelf staat.
pub fn prefix_lines(prefix: &str, text: &str) -> String {
    let pad = " ".repeat(prefix.chars().count());
    let mut out = String::new();
    for (i, line) in text.lines().enumerate() {
        if i == 0 {
            out.push_str(prefix);
        } else {
            out.push('\n');
            out.push_str(&pad);
        }
        out.push_str(line);
    }
    out
}

/// Rijen van een tabel (sweep of resultaat over een bereik) uitlijnen: `links  →  rechts`.
pub fn format_rows(rows: Vec<(String, String)>) -> Vec<String> {
    let w = rows.iter().map(|(l, _)| l.chars().count()).max().unwrap_or(0);
    rows.into_iter()
        .map(|(l, r)| {
            let pad = " ".repeat(w - l.chars().count());
            format!("{l}{pad}  →  {r}")
        })
        .collect()
}

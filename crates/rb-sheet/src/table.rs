//! Tabellen (Ctrl+6): kolommen met een naam en een eenheid, die elk een kolomvector definiëren.
//!
//! ```text
//! tabel x [mm], F [kN] {
//!     0, 0
//!     10, 2.5
//! }
//! ```
//!
//! definieert `x := [0; 10] * (mm)` en `F := [0; 2.5] * (kN)`, zoals een tabel in Mathcad Prime.

use rb_core::{parse_stmt, Error, Stmt};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    pub name: String,
    /// De eenheid zoals getypt (`mm`, `kN/m`); leeg als de kolom geen eenheid heeft.
    pub unit: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableSpec {
    pub cols: Vec<Column>,
    /// De cellen zoals getypt, per rij.
    pub rows: Vec<Vec<String>>,
    /// Per kolom de definitie `naam := [..] * (eenheid)`.
    pub stmts: Vec<Stmt>,
}

fn err(msg: impl Into<String>) -> Error {
    Error::Parse { pos: 0, msg: msg.into() }
}

/// Splits op komma's die niet tussen haakjes staan.
fn split_top(s: &str) -> Vec<String> {
    let (mut out, mut cur, mut depth) = (Vec::new(), String::new(), 0i32);
    for c in s.chars() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                out.push(cur.trim().to_string());
                cur.clear();
                continue;
            }
            _ => {}
        }
        cur.push(c);
    }
    out.push(cur.trim().to_string());
    out
}

/// Begint deze regel een tabel (`tabel x [mm], F [kN] {`)?
pub fn opens_table(line: &str) -> bool {
    let l = line.trim();
    (l.starts_with("tabel ") || l.starts_with("table ")) && l.ends_with('{')
}

/// Lees een tabel: de kopregel, de rijen en de sluitende `}`.
pub fn parse_table(src: &str) -> Result<TableSpec, Error> {
    let mut lines = src.lines();
    let head = lines.next().unwrap_or_default().trim();
    let head = head
        .strip_prefix("tabel ")
        .or_else(|| head.strip_prefix("table "))
        .and_then(|h| h.trim_end().strip_suffix('{'))
        .ok_or_else(|| err("een tabel begint met 'tabel x [mm], F [kN] {'"))?;
    let mut cols = Vec::new();
    for c in split_top(head) {
        let (name, unit) = match c.split_once('[') {
            Some((n, u)) => (n.trim(), u.trim().strip_suffix(']').ok_or_else(|| err(format!("']' ontbreekt bij kolom '{c}'")))?.trim()),
            None => (c.trim(), ""),
        };
        if name.is_empty() || !name.chars().next().is_some_and(char::is_alphabetic) {
            return Err(err(format!("'{c}' is geen kolomnaam")));
        }
        cols.push(Column { name: name.to_string(), unit: unit.to_string() });
    }
    let mut rows = Vec::new();
    let mut closed = false;
    for l in lines {
        let l = l.trim();
        if l == "}" {
            closed = true;
            break;
        }
        if l.is_empty() {
            continue;
        }
        let cells = split_top(l);
        if cells.len() != cols.len() {
            return Err(err(format!("rij {} heeft {} cellen, de tabel heeft {} kolommen", rows.len() + 1, cells.len(), cols.len())));
        }
        rows.push(cells);
    }
    if !closed {
        return Err(err("de tabel wordt niet afgesloten met '}'"));
    }
    if rows.is_empty() {
        return Err(err("de tabel heeft nog geen rijen"));
    }
    let stmts = cols
        .iter()
        .enumerate()
        .map(|(k, c)| {
            let cells: Vec<&str> = rows.iter().map(|r| if r[k].is_empty() { "NaN" } else { r[k].as_str() }).collect();
            let unit = if c.unit.is_empty() { String::new() } else { format!(" * ({})", c.unit) };
            let src = format!("{} := [{}]{unit}", c.name, cells.join("; "));
            parse_stmt(&src, false).map_err(|e| err(format!("kolom {}: {e}", c.name)))
        })
        .collect::<Result<_, _>>()?;
    Ok(TableSpec { cols, rows, stmts })
}

impl TableSpec {
    pub fn defines(&self) -> Vec<String> {
        self.cols.iter().map(|c| c.name.clone()).collect()
    }

    pub fn uses(&self) -> BTreeSet<String> {
        self.stmts.iter().flat_map(|s| s.uses()).collect()
    }

    /// De bron van de tabel (zoals in het `.rkn`-bestand).
    pub fn source(&self) -> String {
        let head: Vec<String> =
            self.cols.iter().map(|c| if c.unit.is_empty() { c.name.clone() } else { format!("{} [{}]", c.name, c.unit) }).collect();
        let mut s = format!("tabel {} {{\n", head.join(", "));
        for r in &self.rows {
            s += &format!("    {}\n", r.join(", "));
        }
        s + "}"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabel_lezen() {
        let t = parse_table("tabel x [mm], F [kN] {\n  0, 0\n  10, 2.5\n}").unwrap();
        assert_eq!(t.defines(), vec!["x", "F"]);
        assert_eq!(t.rows, vec![vec!["0", "0"], vec!["10", "2.5"]]);
        assert_eq!(parse_table(&t.source()).unwrap(), t);
        assert!(parse_table("tabel a, b {\n 1\n}").unwrap_err().to_string().contains("1 cellen"));
        assert!(parse_table("tabel a {\n 1\n").is_err());
        // een cel mag een uitdrukking zijn, met komma's tussen haakjes
        let t = parse_table("tabel a, b {\n max(1, 2), 3\n}").unwrap();
        assert!(t.uses().contains("max"));
    }
}

//! MathPrime vergelijken met de uitkomsten die Mathcad zelf in een .mcdx bewaarde (`result.xml`).
//!
//! Uitkomsten met een eenheid bewaart Mathcad in SI (`39.015 joule`); die vergelijken we met de
//! SI-waarde van MathPrime, op volle precisie. Een uitkomst in een gekozen eenheid (`= mV`) bewaart
//! Mathcad als getal in die eenheid; die vergelijken we met het getal dat MathPrime toont.

use crate::Import;
use rb_core::{Stmt, Value};
use rb_sheet::{Kind, Output, Sheet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Gelijk,
    Anders,
    /// MathPrime geeft een fout.
    Fout,
    /// Niet te vergelijken: Mathcad rekende dit niet uit omdat het een premium-functie is (de
    /// gratis Mathcad Express kent bijvoorbeeld geen lsolve en solve blocks).
    Premium,
}

#[derive(Debug, Clone)]
pub struct Row {
    pub region: String,
    pub mathprime: String,
    pub mathcad: String,
    pub status: Status,
}

#[derive(Debug, Clone, Default)]
pub struct Report {
    pub rows: Vec<Row>,
}

impl Report {
    pub fn count(&self, s: Status) -> usize {
        self.rows.iter().filter(|r| r.status == s).count()
    }

    /// Een overzicht als Markdown-tabel (alleen de verschillen en fouten, tenzij `all`).
    pub fn markdown(&self, title: &str, all: bool) -> String {
        let mut s = format!(
            "## {title}\n\n{} vergeleken: {} gelijk, {} anders, {} met een fout in MathPrime, {} niet te vergelijken (premium-functie in Mathcad)\n\n",
            self.rows.len(),
            self.count(Status::Gelijk),
            self.count(Status::Anders),
            self.count(Status::Fout),
            self.count(Status::Premium)
        );
        let shown: Vec<&Row> = self.rows.iter().filter(|r| all || r.status != Status::Gelijk).collect();
        if !shown.is_empty() {
            s.push_str("| regio | MathPrime | Mathcad | |\n|---|---|---|---|\n");
            for r in shown {
                let cell = |t: &str| t.replace('|', "\\|").replace('\n', " ");
                let mark = match r.status {
                    Status::Gelijk => "✓",
                    Status::Anders => "≠",
                    Status::Fout => "✗",
                    Status::Premium => "—",
                };
                s.push_str(&format!("| `{}` | {} | {} | {mark} |\n", cell(&r.region), cell(&r.mathprime), cell(&r.mathcad)));
            }
        }
        s
    }
}

fn close(a: f64, b: f64, rel: f64) -> bool {
    (a - b).abs() <= rel * a.abs().max(b.abs()) + 1e-12
}

/// Het eerste getal in een uitkomst zoals MathPrime hem toont (`6.5 kN·m` → 6.5, `3.6·10⁷` → 3.6e7).
fn shown_number(t: &str) -> Option<f64> {
    let t = t.trim().replace('−', "-");
    let num: String = t.chars().take_while(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | 'e' | '+')).collect();
    num.parse().ok()
}

/// Heeft deze regel een gekozen weergave-eenheid (`= mV`)?
fn has_display_unit(k: &Kind) -> bool {
    match k {
        Kind::Math(Stmt::Eval { unit, .. }) => unit.is_some(),
        Kind::Math(Stmt::Define { show: Some(sh), .. }) => sh.unit.is_some(),
        _ => false,
    }
}

/// Reken een ingelezen blad door en vergelijk met de uitkomsten van Mathcad.
pub fn compare(imp: &Import) -> Report {
    let sheet = Sheet::load(&imp.source);
    let mut seen: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    let mut rows = Vec::new();
    for r in &imp.results {
        // dezelfde tekst kan vaker voorkomen: de n-de uitkomst hoort bij de n-de regio
        let nth = seen.entry(r.region.as_str()).or_insert(0);
        let Some(i) = sheet.regions().iter().enumerate().filter(|(_, x)| x.source == r.region).map(|(i, _)| i).nth(*nth) else { continue };
        *nth += 1;
        let mathcad = match (&r.error, &r.unit) {
            (Some(e), _) => format!("fout: {e}"),
            (None, Some(u)) => format!("{} {u} (SI)", r.value),
            (None, None) => format!("{}", r.value),
        };
        let row = |mathprime: String, status: Status| Row { region: r.region.clone(), mathprime, mathcad: mathcad.clone(), status };
        let premium = r.error.as_deref().is_some_and(|e| e.contains("premium feature"));
        match sheet.output(i) {
            _ if premium => rows.push(row(format!("{:?}", sheet.output(i)), Status::Premium)),
            // beide een fout: dat is gelijk
            Output::Error { message, .. } if r.error.is_some() => rows.push(row(format!("fout: {message}"), Status::Gelijk)),
            Output::Error { message, .. } => rows.push(row(message.clone(), Status::Fout)),
            _ if r.error.is_some() => rows.push(row(format!("{:?}", sheet.output(i)), Status::Anders)),
            Output::Value(text) => {
                let raw = sheet.result(i).and_then(|(v, _)| if let Value::Num(q) = v { Some(q.value) } else { None });
                let (ours, ok) = if has_display_unit(&sheet.regions()[i].kind) && r.unit.is_none() {
                    // in de gekozen eenheid: het getoonde getal (6 cijfers)
                    match shown_number(text) {
                        Some(x) => (text.clone(), close(x, r.value, 2e-5)),
                        None => (text.clone(), false),
                    }
                } else {
                    match raw {
                        Some(x) => (format!("{text}  [{x} SI]"), close(x, r.value, 1e-9)),
                        None => (text.clone(), false),
                    }
                };
                rows.push(row(ours, if ok { Status::Gelijk } else { Status::Anders }));
            }
            _ => {}
        }
    }
    Report { rows }
}

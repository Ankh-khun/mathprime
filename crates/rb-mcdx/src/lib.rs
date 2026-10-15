//! rb-mcdx: Mathcad Prime-bestanden (`.mcdx`) lezen en schrijven als MathPrime-werkblad.
//!
//! Een `.mcdx` is een ZIP-archief (Open Packaging Conventions) met onder meer:
//! * `mathcad/worksheet.xml`: de regio's met hun plek (`top`, `left` in 1/96 inch) en de formules
//!   als boom (`ml:define`, `ml:apply`, `ml:id`, ...);
//! * `mathcad/result.xml`: de uitkomsten zoals Mathcad ze het laatst berekende;
//! * `mathcad/xaml/*.XamlPackage`: de tekstregio's (zelf weer een ZIP met een WPF-FlowDocument);
//! * `mathcad/media/*`: afbeeldingen.
//!
//! Alleen lezen, zo goed mogelijk: wat niet overgenomen kan worden, komt als commentaar op zijn plek
//! in het blad en in [`Import::notes`].

pub mod compare;
mod image;
mod math;
mod math_out;
pub mod write;

pub use write::{export, Export};

pub use rb_zipxml::{xml, zip};

use std::collections::BTreeMap;
use xml::Node;

/// Het raster van het blad (zie `rb_export::doc::Grid`): breedte van een kolom en hoogte van een rij,
/// in punten.
#[derive(Debug, Clone, Copy)]
pub struct Grid {
    pub col_w: f32,
    pub row_h: f32,
}

/// Een uitkomst zoals Mathcad hem berekende, voor controle.
#[derive(Debug, Clone, PartialEq)]
pub struct MathcadResult {
    /// De regio in de bron (zonder positie).
    pub region: String,
    /// De waarde: in de eenheid die Mathcad toonde, of (als `unit` er is) in SI.
    pub value: f64,
    /// De SI-eenheid van een uitkomst met eenheid, zoals Mathcad hem bewaart (`joule`, `kilogram·meter`).
    pub unit: Option<String>,
    /// De foutmelding als Mathcad hier een fout gaf (dan is `value` geen getal).
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Import {
    /// Het werkblad als `.rkn`-tekst, met een positie voor elke regio.
    pub source: String,
    /// Wat niet (helemaal) overgenomen kon worden.
    pub notes: Vec<String>,
    pub results: Vec<MathcadResult>,
    /// Aantal overgenomen regio's.
    pub regions: usize,
}

struct Item {
    /// Plek in Mathcad (1/96 inch): bepaalt de volgorde van doorrekenen.
    top: f64,
    left: f64,
    row: usize,
    col: usize,
    /// Volgnummer, voor een vaste volgorde bij dezelfde plek.
    seq: usize,
    text: String,
}

struct Ctx<'a> {
    grid: Grid,
    files: &'a BTreeMap<String, Vec<u8>>,
    rels: BTreeMap<String, String>,
    results: BTreeMap<String, (f64, Option<String>, Option<String>)>,
    items: Vec<Item>,
    notes: Vec<String>,
    checks: Vec<MathcadResult>,
    helpers: usize,
}

/// De rekeninstellingen van een Mathcad-blad (`mathcad/settings/calculation.xml`).
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// Eerste index van vectoren en matrices (`array-origin`, Mathcad: ORIGIN).
    pub origin: i64,
    /// Zaad van de toevalsgetallen (`random-seed`).
    pub seed: f64,
    /// Eenhedenstelsel voor de weergave (`si`, `mks`, `cgs`, `us`, `none`).
    pub unit_system: String,
    /// Toleranties van de oplossers (Mathcad: TOL en CTOL).
    pub tol: f64,
    pub ctol: f64,
}

impl Default for Settings {
    /// De standaardwaarden van Mathcad Prime.
    fn default() -> Settings {
        Settings { origin: 0, seed: 1.0, unit_system: "si".into(), tol: 0.001, ctol: 0.001 }
    }
}

impl Settings {
    /// Lees `calculation.xml`; wat ontbreekt of onleesbaar is, houdt de standaardwaarde.
    pub fn parse(xml_src: &str) -> Settings {
        let mut s = Settings::default();
        let Ok(doc) = xml::parse(xml_src) else { return s };
        let mut found = Vec::new();
        doc.descendants("builtInVariables", &mut found);
        if let Some(b) = found.first() {
            let num = |k: &str| b.attr(k).and_then(|v| v.trim().parse::<f64>().ok());
            if let Some(v) = num("array-origin") {
                s.origin = v as i64;
            }
            if let Some(v) = num("random-seed") {
                s.seed = v;
            }
            if let Some(v) = num("convergence-tolerance") {
                s.tol = v;
            }
            if let Some(v) = num("constraint-tolerance") {
                s.ctol = v;
            }
        }
        let mut found = Vec::new();
        doc.descendants("currentUnitSystem", &mut found);
        if let Some(name) = found.first().and_then(|u| u.attr("name")) {
            s.unit_system = name.trim().to_ascii_lowercase();
        }
        s
    }

    /// Regels voor het MathPrime-blad (alleen waar het afwijkt van de standaard) en meldingen voor
    /// wat MathPrime niet overneemt.
    pub fn translate(&self) -> (Vec<String>, Vec<String>) {
        let d = Settings::default();
        let mut lines = Vec::new();
        let mut notes = Vec::new();
        if self.origin != d.origin {
            lines.push(format!("ORIGIN := {} // eerste index, uit de instellingen van Mathcad", self.origin));
        }
        if self.seed != d.seed {
            lines.push(format!("SEED := {} // zaad van de toevalsgetallen, uit Mathcad (de reeks zelf verschilt)", self.seed));
        }
        if self.unit_system != d.unit_system {
            notes.push(format!(
                "blad: eenhedenstelsel '{}' van Mathcad; MathPrime rekent hetzelfde, maar toont uitkomsten zonder gekozen eenheid in SI",
                self.unit_system
            ));
        }
        if self.tol != d.tol || self.ctol != d.ctol {
            notes.push(format!(
                "blad: toleranties TOL = {} en CTOL = {} van Mathcad; de oplossers van MathPrime gebruiken hun eigen nauwkeurigheid",
                self.tol, self.ctol
            ));
        }
        (lines, notes)
    }
}

/// Lees een `.mcdx`.
pub fn import(bytes: &[u8], grid: Grid) -> Result<Import, String> {
    let files = zip::read(bytes)?;
    let text = |name: &str| files.get(name).map(|b| String::from_utf8_lossy(b).into_owned());
    let ws = text("mathcad/worksheet.xml").ok_or("dit is geen Mathcad Prime-bestand (mathcad/worksheet.xml ontbreekt)")?;
    let ws = xml::parse(&ws)?;
    let mut rels = BTreeMap::new();
    if let Some(r) = text("mathcad/_rels/worksheet.xml.rels") {
        for rel in xml::parse(&r)?.elems() {
            if let (Some(id), Some(target)) = (rel.attr("Id"), rel.attr("Target")) {
                rels.insert(id.to_string(), target.trim_start_matches('/').to_string());
            }
        }
    }
    let mut results = BTreeMap::new();
    if let Some(r) = text("mathcad/result.xml") {
        for rd in xml::parse(&r)?.elems() {
            // een fout van Mathcad zelf (zoals "This variable is undefined")
            if let (Some(id), Some(errs)) = (rd.attr("result-id"), rd.child("engineErrors")) {
                let mut msgs = Vec::new();
                errs.descendants("resource-string", &mut msgs);
                let msg = msgs.first().map_or("fout".to_string(), |m| m.text().trim().to_string());
                results.insert(id.to_string(), (f64::NAN, None, Some(msg)));
                continue;
            }
            let (Some(id), Some(res)) = (rd.attr("result-id"), rd.child("result")) else { continue };
            // een getal (in de getoonde eenheid), of een getal met eenheid (in SI)
            if let Some(Ok(v)) = res.child("real").map(|n| n.text().trim().parse::<f64>()) {
                results.insert(id.to_string(), (v, None, None));
            } else if let Some(uv) = res.child("unitedValue") {
                let Some(Ok(v)) = uv.child("real").map(|n| n.text().trim().parse::<f64>()) else { continue };
                let mut refs = Vec::new();
                uv.descendants("unitReference", &mut refs);
                let unit: Vec<String> = refs
                    .iter()
                    .map(|u| {
                        let name = u.attr("unit").unwrap_or("?");
                        match (u.attr("power-numerator"), u.attr("power-denominator")) {
                            (Some("1"), Some("1")) | (None, None) => name.to_string(),
                            (Some(n), Some("1")) => format!("{name}^{n}"),
                            (Some(n), Some(d)) => format!("{name}^({n}/{d})"),
                            _ => name.to_string(),
                        }
                    })
                    .collect();
                results.insert(id.to_string(), (v, Some(unit.join("·")), None));
            }
        }
    }
    let mut cx = Ctx { grid, files: &files, rels, results, items: Vec::new(), notes: Vec::new(), checks: Vec::new(), helpers: 0 };
    // rekeninstellingen van het blad: als regels helemaal bovenaan (vóór alle regio's), want ze gelden
    // voor het hele blad
    let settings = text("mathcad/settings/calculation.xml").map_or_else(Settings::default, |s| Settings::parse(&s));
    let (lines, notes) = settings.translate();
    for (k, line) in lines.into_iter().enumerate() {
        cx.push((-100.0 + k as f64, 0.0), line);
    }
    cx.notes.extend(notes);
    if let Some(regions) = ws.child("regions") {
        cx.regions(regions, 0.0, 0.0);
    }
    // de volgorde van Mathcad (boven naar beneden, dan links naar rechts) moet blijven; vallen twee
    // regio's na afronding op dezelfde rij in de verkeerde volgorde, dan schuift de tweede een rij omlaag
    cx.items.sort_by(|a, b| (a.top, a.left, a.seq).partial_cmp(&(b.top, b.left, b.seq)).unwrap_or(std::cmp::Ordering::Equal));
    for k in 1..cx.items.len() {
        let (pr, pc) = (cx.items[k - 1].row, cx.items[k - 1].col);
        let it = &mut cx.items[k];
        if it.row < pr || (it.row == pr && it.col <= pc) {
            it.row = pr + 1;
        }
    }
    let source: String = cx.items.iter().map(|i| format!("@{},{} {}\n", i.col, i.row, i.text)).collect();
    Ok(Import { source, notes: cx.notes, results: cx.checks, regions: cx.items.len() })
}

impl Ctx<'_> {
    /// Rasterplek bij een positie in 1/96 inch.
    fn cell(&self, top: f64, left: f64) -> (usize, usize) {
        let pt = |v: f64| (v * 0.75) as f32;
        let col = (pt(left) / self.grid.col_w).round().max(0.0) as usize;
        let row = (pt(top) / self.grid.row_h).round().max(0.0) as usize + 1;
        (row, col)
    }

    fn push(&mut self, (top, left): (f64, f64), text: String) {
        let seq = self.items.len();
        let (row, col) = self.cell(top, left);
        self.items.push(Item { top, left, row, col, seq, text });
    }

    /// Definieert het blad (tot nu toe) deze naam?
    fn defines(&self, name: &str) -> bool {
        let head = format!("{name} :=");
        self.items.iter().any(|i| i.text.starts_with(&head))
    }

    fn note(&mut self, at: (f64, f64), what: String) {
        self.notes.push(format!("rij {}: {what}", self.cell(at.0, at.1).0));
        self.push(at, format!("// [uit Mathcad, niet overgenomen: {what}]"));
    }

    fn regions(&mut self, regions: &Node, top0: f64, left0: f64) {
        for r in regions.elems().filter(|n| n.local() == "region") {
            let num = |k: &str| r.attr(k).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
            let (top, left) = (top0 + num("top"), left0 + num("left"));
            let at = (top, left);
            let Some(content) = r.elems().next() else { continue };
            match content.local() {
                "math" => self.math(content, at),
                "text" => self.text(content, at),
                "plot" => self.plot(content, at),
                "solveblock" => self.solveblock(content, top, left),
                "picture" => self.picture(content, at),
                "pageBreak" => self.push(at, "pagebreak".to_string()),
                other => self.note(at, format!("regio van het type '{other}'")),
            }
        }
    }

    fn math(&mut self, m: &Node, at: (f64, f64)) {
        let Some(stmt) = m.elems().find(|n| !matches!(n.local(), "resultFormat" | "formattingOverride")) else { return };
        match math::statement(stmt, &mut self.helpers) {
            Ok(lines) => {
                for (k, line) in lines.into_iter().enumerate() {
                    if k == 0 {
                        if let Some((v, u, err)) = m.attr("resultRef").and_then(|r| self.results.get(r)) {
                            self.checks.push(MathcadResult { region: line.clone(), value: *v, unit: u.clone(), error: err.clone() });
                        }
                    }
                    // de regels na de eerste (hulpvariabele, elementen) eronder
                    self.push((at.0 + k as f64 * 20.0, at.1), line);
                }
            }
            Err(e) => self.note(at, e),
        }
    }

    fn text(&mut self, t: &Node, at: (f64, f64)) {
        let Some(target) = t.attr("item-idref").and_then(|id| self.rels.get(id)) else {
            return self.note(at, "tekst zonder inhoud".into());
        };
        let Some(pkg) = self.files.get(target) else { return self.note(at, format!("tekst {target} ontbreekt")) };
        let doc = zip::read(pkg).ok().and_then(|f| {
            f.iter().find(|(n, _)| n.to_ascii_lowercase().ends_with(".xaml")).map(|(_, b)| String::from_utf8_lossy(b).into_owned())
        });
        let Some(doc) = doc.and_then(|d| xml::parse(&d).ok()) else { return self.note(at, "tekst niet te lezen".into()) };
        let base = doc.attr("FontSize").and_then(|s| s.parse::<f64>().ok()).unwrap_or(14.67);
        let mut paras = Vec::new();
        doc.descendants("Paragraph", &mut paras);
        let mut top = at.0;
        for p in paras {
            let size = max_font(p).unwrap_or(base);
            let marker = if size >= 24.0 {
                "# "
            } else if size >= 18.5 {
                "## "
            } else {
                "\" "
            };
            for line in paragraph_text(p).lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                self.push((top, at.1), format!("{marker}{line}"));
                top += 20.0;
            }
        }
    }

    /// Een afbeelding: de PNG uit `mathcad/media` komt in de regio zelf (base64).
    fn picture(&mut self, pic: &Node, at: (f64, f64)) {
        let Some(img) = pic.elems().next() else { return self.note(at, "afbeelding zonder inhoud".into()) };
        let file = img.attr("item-idref").and_then(|id| self.rels.get(id)).and_then(|t| self.files.get(t));
        let Some(bytes) = file else { return self.note(at, "afbeelding (bestand ontbreekt)".into()) };
        let converted;
        let bytes: &[u8] = if bytes.starts_with(b"\x89PNG") {
            bytes
        } else if let Some((w, h, px)) = image::bmp_to_rgba(bytes) {
            converted = image::png(w, h, &px);
            &converted
        } else {
            return self.note(at, "afbeelding (alleen PNG en BMP worden overgenomen)".into());
        };
        let dim = |k: &str| img.attr(k).and_then(|v| v.parse::<f64>().ok());
        let (w, h) = match (dim("display-width"), dim("display-height")) {
            (Some(w), Some(h)) => (w * 0.75, h * 0.75),
            _ => (200.0, 150.0),
        };
        let data = rb_core::base64::encode(bytes);
        self.push(at, format!("image {:.1}x{:.1} data:image/png;base64,{data}", w, h));
    }

    fn plot(&mut self, p: &Node, at: (f64, f64)) {
        let mut xy = Vec::new();
        p.descendants("xyPlot", &mut xy);
        let Some(xy) = xy.first() else { return self.note(at, "grafiek van een onbekend type (alleen XY)".into()) };
        let equations = |axis: &str| -> Vec<(Option<String>, Option<String>)> {
            let mut eqs = Vec::new();
            if let Some(a) = xy.child("axes").and_then(|a| a.child(axis)) {
                let mut found = Vec::new();
                a.descendants("plotEquation", &mut found);
                for e in found {
                    let maths: Vec<&Node> = e.elems().filter(|n| n.local() == "math").collect();
                    let expr = maths.first().and_then(|m| m.elems().next()).and_then(|n| math::expr_text(n).ok());
                    let unit = maths.get(1).and_then(|m| m.elems().next()).and_then(|n| math::expr_text(n).ok());
                    eqs.push((expr, unit));
                }
            }
            eqs
        };
        let xs = equations("xAxis");
        let ys = equations("yAxis");
        let x = xs.iter().find_map(|(e, _)| e.clone());
        let series: Vec<String> = ys.iter().filter_map(|(e, _)| e.clone()).collect();
        // het bereik van de x-as, als het vast is (startValue/endValue met een getal)
        let limit = |k: &str| -> Option<String> {
            let mut found = Vec::new();
            xy.child("axes")?.child("xAxis")?.descendants(k, &mut found);
            let real = found.first()?.child("real")?;
            Some(real.text().trim().to_string())
        };
        match x {
            Some(x) if !series.is_empty() => {
                let unit = ys.iter().find_map(|(_, u)| u.clone());
                let unit = unit.map_or(String::new(), |u| format!(" = {u}"));
                // een losse naam met een vast bereik: plot x = begin ; eind (anders tekent Mathcad
                // over een bereikvariabele of vector)
                let is_name = x.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '\''));
                let head = match (limit("startValue"), limit("endValue")) {
                    (Some(lo), Some(hi)) if is_name && !self.defines(&x) => format!("{x} = {lo} ; {hi}"),
                    _ => x,
                };
                self.push(at, format!("plot {head} : {}{unit}", series.join(", ")));
            }
            _ => self.note(at, "grafiek zonder x- of y-uitdrukking".into()),
        }
    }

    fn solveblock(&mut self, sb: &Node, top: f64, left: f64) {
        let Some(regions) = sb.child("regions") else { return };
        // de beginwaarden eerst, dan Given, de vergelijkingen en de oplosser (zoals in MathPrime)
        let inner: Vec<&Node> = regions.elems().filter(|n| n.local() == "region").collect();
        let first_constraint = inner
            .iter()
            .filter(|r| r.attr("solve-block-category") == Some("constraint"))
            .filter_map(|r| r.attr("top").and_then(|v| v.parse::<f64>().ok()))
            .fold(f64::INFINITY, f64::min);
        // Given net boven de eerste vergelijking
        let given_top = if first_constraint.is_finite() { top + first_constraint - 0.01 } else { top };
        self.push((given_top, left), "Given".into());
        self.regions(regions, top, left);
    }
}

/// De grootste lettergrootte in een alinea (ook in de Runs).
fn max_font(p: &Node) -> Option<f64> {
    let mut best = p.attr("FontSize").and_then(|s| s.parse::<f64>().ok());
    for c in p.elems() {
        if let Some(v) = max_font(c) {
            best = Some(best.map_or(v, |b| b.max(v)));
        }
    }
    best
}

fn paragraph_text(p: &Node) -> String {
    let mut s = String::new();
    for c in &p.children {
        match c {
            xml::Child::Text(t) => s.push_str(t),
            xml::Child::Elem(n) if n.local() == "LineBreak" => s.push('\n'),
            xml::Child::Elem(n) => s.push_str(&paragraph_text(n)),
        }
    }
    s
}

#[cfg(test)]
mod tests;

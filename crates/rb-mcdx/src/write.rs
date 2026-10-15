//! Een MathPrime-blad opslaan als Mathcad Prime-bestand (`.mcdx`): de omkering van [`crate::import`].
//!
//! Het pakket heeft dezelfde onderdelen als een .mcdx dat Mathcad zelf schrijft. Formules, tekst,
//! koppen, afbeeldingen, grafieken en solve blocks worden echte Mathcad-regio's. Wat Mathcad niet
//! kent (programmablokken, sweeps, regels met een fout) komt als tekstregio met de bron erin, zodat
//! er niets verloren gaat; dat staat in [`Export::notes`].

use crate::math_out::{esc, expr, id, statement, unit, Names};
use crate::{zip, Grid};
use rb_core::{Expr, Stmt};
use rb_sheet::{parse_sheet, Kind, PlotX, Region, SOLVERS};
use std::collections::BTreeSet;

pub struct Export {
    pub bytes: Vec<u8>,
    /// Regio's die als tekst zijn opgeslagen (met de reden).
    pub notes: Vec<String>,
}

const NS: &str = r#"xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:ve="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:ws="http://schemas.mathsoft.com/worksheet50" xmlns:ml="http://schemas.mathsoft.com/math50" xmlns:u="http://schemas.mathsoft.com/units10" xmlns:p="http://schemas.mathsoft.com/provenance10" xmlns="http://schemas.mathsoft.com/worksheet50""#;
const XML: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>";
const REL_OFFICE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
/// Kleuren van de lijnen in een grafiek (zoals de reeksen in MathPrime).
const TRACE_COLORS: [&str; 6] = ["#FF1F77B4", "#FFD62728", "#FF2CA02C", "#FFFF7F0E", "#FF9467BD", "#FF8C564B"];

struct Out {
    grid: Grid,
    /// Wat op dit punt van het blad al gedefinieerd is (labels hangen af van de positie).
    names: Names,
    regions: String,
    next_id: usize,
    rels: Vec<(String, String, String)>, // (id, type, target)
    files: Vec<(String, Vec<u8>)>,
    notes: Vec<String>,
    texts: usize,
    images: usize,
}

/// Positie in Mathcad (1/96 inch) bij een rasterplek: de omkering van het inlezen.
fn place(grid: Grid, r: &Region) -> (f64, f64) {
    let top = (r.pos.row.saturating_sub(1)) as f64 * grid.row_h as f64 / 0.75;
    let left = r.pos.col as f64 * grid.col_w as f64 / 0.75;
    (top, left)
}

fn region_open(id: usize, top: f64, left: f64, extra: &str) -> String {
    format!("<region region-id=\"{id}\"{extra} top=\"{top:.2}\" left=\"{left:.2}\">")
}

impl Out {
    fn rel(&mut self, kind: &str, target: String) -> String {
        let id = format!("R{:016x}", 0x5a17_0000_0000_0000u64 + self.rels.len() as u64 + 1);
        self.rels.push((id.clone(), format!("{REL_OFFICE}/{kind}"), target));
        id
    }

    fn math(&mut self, at: (f64, f64), inner: &str) {
        let id = self.next_id;
        self.next_id += 1;
        self.regions.push_str(&region_open(id, at.0, at.1, ""));
        self.regions.push_str(&format!("<math>{inner}</math></region>"));
    }

    /// Een tekstregio: een eigen pakket (`XamlPackage`) met een WPF-document.
    fn text(&mut self, at: (f64, f64), text: &str, size: f64, color: &str) {
        self.texts += 1;
        let section = format!(
            "<Section xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\" xml:space=\"preserve\" TextAlignment=\"Left\" FontFamily=\"Tahoma\" FontSize=\"{size}\" Foreground=\"{color}\"><Paragraph><Run>{}</Run></Paragraph></Section>",
            esc(text)
        );
        let pkg = zip::write(&[
            (
                "[Content_Types].xml".into(),
                format!("\u{feff}{XML}<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"xaml\" ContentType=\"application/vnd.ms-wpf.xaml+xml\" /><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\" /></Types>").into_bytes(),
            ),
            (
                "_rels/.rels".into(),
                format!("\u{feff}{XML}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Type=\"http://schemas.microsoft.com/wpf/2005/10/xaml/entry\" Target=\"/Xaml/Document.xaml\" Id=\"R0000000000000001\" /></Relationships>").into_bytes(),
            ),
            ("Xaml/Document.xaml".into(), section.into_bytes()),
        ]);
        let target = format!("mathcad/xaml/FlowDocument{}.XamlPackage", self.texts);
        let rid = self.rel("flowDocument", format!("/{target}"));
        self.files.push((target, pkg));
        let width = (640.0 - at.1).max(200.0);
        let id = self.next_id;
        self.next_id += 1;
        self.regions.push_str(&region_open(id, at.0, at.1, &format!(" width=\"{width:.2}\"")));
        self.regions.push_str(&format!(
            "<text item-idref=\"{rid}\"><FlowDocument FontFamily=\"Tahoma\" FontSize=\"{size}\" Foreground=\"{color}\" Background=\"#00FFFFFF\" TextAlignment=\"Left\" xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\" /></text></region>"
        ));
    }

    /// Iets wat Mathcad niet kent: als tekst, met een melding.
    fn as_text(&mut self, at: (f64, f64), r: &Region, why: &str) {
        self.notes.push(format!("regel {}: {why}; als tekst opgeslagen", r.line));
        for (k, line) in r.source.lines().enumerate() {
            self.text((at.0 + k as f64 * 20.0, at.1), line, 13.0, "#FF555555");
        }
    }

    fn image(&mut self, at: (f64, f64), img: &rb_sheet::Image) {
        self.images += 1;
        let target = format!("mathcad/media/Image{}.png", self.images);
        let rid = self.rel("image", format!("/{target}"));
        self.files.push((target, img.png.to_vec()));
        let (w, h) = (img.width as f64 / 0.75, img.height as f64 / 0.75);
        let id = self.next_id;
        self.next_id += 1;
        self.regions.push_str(&region_open(id, at.0, at.1, &format!(" width=\"{w:.2}\" height=\"{h:.2}\"")));
        self.regions.push_str(&format!("<picture><png item-idref=\"{rid}\" display-width=\"{w:.2}\" display-height=\"{h:.2}\" /></picture></region>"));
    }

    /// Een XY-grafiek: x-uitdrukking, reeksen en (als die er is) de eenheid van de y-as.
    fn plot(&mut self, at: (f64, f64), spec: &rb_sheet::PlotSpec) -> Result<(), String> {
        let n = &self.names;
        let (x, domain) = match &spec.x {
            PlotX::Range { var, start, end, .. } => {
                let lim = |e: &Expr| -> String { if let Expr::Num(v) = e { format!("<ml:real>{v}</ml:real>") } else { "<ml:placeholder />".into() } };
                (id(var, Some("VARIABLE")), (lim(start), lim(end)))
            }
            PlotX::Data(e) => (expr(e, n)?, ("<ml:placeholder />".into(), "<ml:placeholder />".into())),
        };
        let yunit = match &spec.unit {
            Some((u, _)) => unit(u)?,
            None => "<ml:placeholder />".into(),
        };
        let mut traces = String::new();
        let mut yeq = String::new();
        let mut xeq = String::new();
        for (k, (e, _)) in spec.series.iter().enumerate() {
            traces.push_str(&format!(
                "<trace><traceStyle color=\"{}\" symbol=\"none\" line-weight=\"2\" line-style=\"Solid\">lines</traceStyle></trace>",
                TRACE_COLORS[k % TRACE_COLORS.len()]
            ));
            xeq.push_str(&format!("<plotEquation><math>{x}</math><math><ml:placeholder /></math></plotEquation>"));
            yeq.push_str(&format!("<plotEquation><math>{}</math><math>{yunit}</math></plotEquation>", expr(e, n)?));
        }
        let axis = |name: &str, eqs: &str, (lo, hi): &(String, String)| {
            format!(
                "<{name} rank=\"1\"><axisLine position=\"origin\" /><axisGrid><gridFrequency>11</gridFrequency><gridLabels display=\"true\" /><gridLines /><tickMarks display=\"true\" /></axisGrid><axisLabel /><markers /><plotEquations>{eqs}</plotEquations><xyDomain scale-type=\"linear\" auto-scale=\"true\"><startValue>{lo}</startValue><endValue>{hi}</endValue></xyDomain></{name}>"
            )
        };
        let none = ("<ml:placeholder />".to_string(), "<ml:placeholder />".to_string());
        let (w, h) = (430.0, 240.0);
        let id = self.next_id;
        self.next_id += 1;
        self.regions.push_str(&region_open(id, at.0, at.1, &format!(" width=\"{w}\" height=\"{h}\"")));
        self.regions.push_str(&format!(
            "<plot origin-positioning=\"true\"><xyPlot><title /><legend /><traces>{traces}</traces><graph-size width=\"{w}\" height=\"{h}\" /><axes>{}{}</axes></xyPlot></plot></region>",
            axis("xAxis", &xeq, &domain),
            axis("yAxis", &yeq, &none)
        ));
        Ok(())
    }
}

/// De namen die een solver oplost: `Find(x, y)` → x, y.
fn solver_unknowns(stmt: &Stmt) -> Vec<String> {
    fn walk(e: &Expr, out: &mut Vec<String>) {
        match e {
            Expr::Call(f, args) if SOLVERS.contains(&f.as_str()) && f != "odesolve" => {
                let skip = usize::from(matches!(f.as_str(), "Minimize" | "Maximize"));
                out.extend(args.iter().skip(skip).filter_map(|a| if let Expr::Var(n) = a { Some(n.clone()) } else { None }));
            }
            Expr::Index(b, _) => walk(b, out),
            _ => {}
        }
    }
    let mut out = Vec::new();
    match stmt {
        Stmt::Define { body, .. } => walk(body, &mut out),
        Stmt::Eval { expr, .. } => walk(expr, &mut out),
        Stmt::Symbolic { .. } => {}
    }
    out
}

/// Neem de definitie van een regio op: vanaf hier is die naam een variabele (of eigen functie),
/// daarvoor nog een eenheid (zoals `m` voor een massa `m := 4 kg` verderop).
fn learn(names: &mut Names, r: &Region) {
    if let Kind::Table(t) = &r.kind {
        names.defined.extend(t.defines());
    }
    if let Kind::Math(Stmt::Define { name, params, .. }) = &r.kind {
        names.defined.insert(name.clone());
        if params.is_some() {
            names.functions.insert(name.clone());
        }
    }
}

/// Sla de bron van een MathPrime-blad op als `.mcdx`.
pub fn export(source: &str, grid: Grid) -> Export {
    let regions = parse_sheet(source);
    let names = Names { defined: BTreeSet::new(), functions: BTreeSet::new() };
    let mut out = Out { grid, names, regions: String::new(), next_id: 0, rels: Vec::new(), files: Vec::new(), notes: Vec::new(), texts: 0, images: 0 };
    // solve blocks: Given, de vergelijkingen, de oplosser, en de beginwaarden direct boven Given
    let mut in_block = vec![false; regions.len()];
    let mut blocks: Vec<(usize, Vec<usize>, Vec<usize>)> = Vec::new(); // (Given, beginwaarden, rest)
    for (g, r) in regions.iter().enumerate() {
        if !matches!(r.kind, Kind::Given) {
            continue;
        }
        let Some(s) = (g + 1..regions.len()).find(|&k| matches!(&regions[k].kind, Kind::Math(st) if !solver_unknowns(st).is_empty())) else { continue };
        let unknowns = match &regions[s].kind {
            Kind::Math(st) => solver_unknowns(st),
            _ => Vec::new(),
        };
        let mut guesses = Vec::new();
        for k in (0..g).rev() {
            match &regions[k].kind {
                Kind::Math(Stmt::Define { name, params: None, index: None, .. }) if unknowns.contains(name) && !in_block[k] => guesses.insert(0, k),
                _ => break,
            }
        }
        let rest: Vec<usize> = (g + 1..=s).filter(|&k| matches!(regions[k].kind, Kind::Constraint(_)) || k == s).collect();
        for &k in guesses.iter().chain(&rest).chain(std::iter::once(&g)) {
            in_block[k] = true;
        }
        blocks.push((g, guesses, rest));
    }
    for (k, r) in regions.iter().enumerate() {
        let at = place(grid, r);
        if let Some((_, guesses, rest)) = blocks.iter().find(|b| b.0 == k) {
            solve_block(&mut out, &regions, at, guesses, rest);
            continue;
        }
        if in_block[k] {
            continue;
        }
        match &r.kind {
            Kind::Math(st) => match statement(st, &out.names) {
                Ok(xml) => out.math(at, &xml),
                Err(why) => out.as_text(at, r, &format!("Mathcad kent {why} niet")),
            },
            Kind::Formula(e) | Kind::Constraint(e) => match expr(e, &out.names) {
                Ok(xml) => out.math(at, &xml),
                Err(why) => out.as_text(at, r, &format!("Mathcad kent {why} niet")),
            },
            Kind::Text(t) => out.text(at, t, 14.666_666_666_666_7, "#FF000000"),
            Kind::Heading(h) => {
                let level = h.chars().take_while(|c| *c == '#').count();
                let size = if level <= 1 { 24.0 } else if level == 2 { 18.666_666_666_666_7 } else { 16.0 };
                out.text(at, h[level..].trim(), size, "#FF1F376E");
            }
            Kind::Comment(c) => out.text(at, c.trim_start_matches('/').trim(), 13.0, "#FF6E6E6E"),
            Kind::Image(img) => out.image(at, img),
            Kind::Plot(spec) => {
                if let Err(why) = out.plot(at, spec) {
                    out.as_text(at, r, &format!("grafiek met {why}"));
                }
            }
            // een tabel: per kolom een definitie `x := [..] mm`, naast elkaar
            Kind::Table(t) => {
                for (k, st) in t.stmts.iter().enumerate() {
                    match statement(st, &out.names) {
                        Ok(xml) => out.math((at.0, at.1 + 120.0 * k as f64), &xml),
                        Err(why) => {
                            out.as_text(at, r, &format!("Mathcad kent {why} niet"));
                            break;
                        }
                    }
                }
            }
            // zoals Mathcad: een eigen regio over de hele breedte
            Kind::PageBreak => {
                let id = out.next_id;
                out.next_id += 1;
                out.regions.push_str(&region_open(id, at.0, 0.0, " height=\"2\" actualWidth=\"816\" actualHeight=\"2\""));
                out.regions.push_str("<pageBreak /></region>");
            }
            Kind::Area { .. } | Kind::AreaEnd => out.as_text(at, r, "gebieden (area) schrijft MathPrime nog niet naar Mathcad"),
            Kind::HeaderFooter { .. } => out.as_text(at, r, "kop- en voetteksten schrijft MathPrime nog niet naar Mathcad"),
            Kind::Given => out.as_text(at, r, "Given zonder oplosser"),
            Kind::Sweep(_) => out.as_text(at, r, "een sweep bestaat niet in Mathcad"),
            Kind::Invalid { .. } => out.as_text(at, r, "de regel bevat een fout"),
        }
        learn(&mut out.names, r);
    }
    let worksheet = format!("{XML}<worksheet msg-id=\"NoMessage\" {NS}><regions>{}</regions></worksheet>", out.regions);
    let mut rels = String::new();
    for (id, kind, target) in &out.rels {
        rels.push_str(&format!("<Relationship Type=\"{kind}\" Target=\"{target}\" Id=\"{id}\" />"));
    }
    let mut files: Vec<(String, Vec<u8>)> = vec![
        ("[Content_Types].xml".into(), content_types().into_bytes()),
        ("_rels/.rels".into(), package_rels().into_bytes()),
        ("docProps/app.xml".into(), APP.as_bytes().to_vec()),
        ("docProps/core.xml".into(), format!("{XML}<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\"><cp:revision>1</cp:revision></cp:coreProperties>").into_bytes()),
        ("mathcad/worksheet.xml".into(), worksheet.into_bytes()),
        ("mathcad/header.xml".into(), format!("<header {NS}><regions /></header>").into_bytes()),
        ("mathcad/footer.xml".into(), format!("<footer {NS}><regions /></footer>").into_bytes()),
        ("mathcad/integration.xml".into(), format!("{XML}<regions xmlns=\"http://schemas.ptc.com/integration10\" />").into_bytes()),
        ("mathcad/result.xml".into(), format!("{XML}<resultsList xmlns:ml=\"http://schemas.mathsoft.com/math50\" xmlns:u=\"http://schemas.mathsoft.com/units10\" xmlns=\"http://schemas.mathsoft.com/result10\" />").into_bytes()),
        ("mathcad/settings/calculation.xml".into(), CALCULATION.as_bytes().to_vec()),
        ("mathcad/settings/presentation.xml".into(), PRESENTATION.as_bytes().to_vec()),
    ];
    if !out.rels.is_empty() {
        files.push((
            "mathcad/_rels/worksheet.xml.rels".into(),
            format!("{XML}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">{rels}</Relationships>").into_bytes(),
        ));
    }
    files.extend(out.files);
    Export { bytes: zip::write(&files), notes: out.notes }
}

/// Een solve block: Given (de rand), beginwaarden, vergelijkingen en de oplosser.
fn solve_block(out: &mut Out, regions: &[Region], at: (f64, f64), guesses: &[usize], rest: &[usize]) {
    // de beginwaarden staan boven Given: het blok begint bij de bovenste
    let top = guesses.iter().map(|&k| place(out.grid, &regions[k]).0).fold(at.0, f64::min);
    let mut inner = String::new();

    let order: Vec<(usize, bool)> = guesses.iter().map(|&k| (k, true)).chain(rest.iter().map(|&k| (k, false))).collect();
    for (k, guess) in order {
        let r = &regions[k];
        let (t, l) = place(out.grid, r);
        let category = match &r.kind {
            _ if guess => "guess-value",
            Kind::Constraint(_) => "constraint",
            _ => "solver",
        };
        let xml = match &r.kind {
            Kind::Math(st) => statement(st, &out.names),
            Kind::Constraint(e) => expr(e, &out.names),
            _ => Err("dit soort regio".into()),
        };
        match xml {
            Ok(xml) => {
                let id = out.next_id;
                out.next_id += 1;
                inner.push_str(&format!(
                    "<region region-id=\"{id}\" top=\"{:.2}\" left=\"{:.2}\" solve-block-category=\"{category}\"><math>{xml}</math></region>",
                    t - top,
                    (l - at.1).max(0.0)
                ));
            }
            Err(why) => out.notes.push(format!("regel {}: Mathcad kent {why} niet; weggelaten uit het solve block", r.line)),
        }
        learn(&mut out.names, r);
    }
    let id = out.next_id;
    out.next_id += 1;
    out.regions.push_str(&region_open(id, top, at.1, ""));
    out.regions.push_str(&format!("<solveblock><regions>{inner}</regions></solveblock></region>"));
}

fn content_types() -> String {
    let main = "application/vnd.openxmlformats-officedocument.mathprocessingml.mathcad";
    format!(
        "\u{feff}{XML}<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"xml\" ContentType=\"{main}.main+xml\" /><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\" /><Default Extension=\"png\" ContentType=\"image/png\" /><Default Extension=\"XamlPackage\" ContentType=\"application/zip\" /><Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\" /><Override PartName=\"/docProps/app.xml\" ContentType=\"application/mathcad.extended-properties+xml\" /><Override PartName=\"/mathcad/settings/presentation.xml\" ContentType=\"{main}.settings.presentation+xml\" /><Override PartName=\"/mathcad/settings/calculation.xml\" ContentType=\"{main}.settings.calculation+xml\" /><Override PartName=\"/mathcad/result.xml\" ContentType=\"{main}.result+xml\" /><Override PartName=\"/mathcad/integration.xml\" ContentType=\"{main}.integration+xml\" /></Types>"
    )
}

fn package_rels() -> String {
    format!(
        "\u{feff}{XML}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Type=\"{REL_OFFICE}/officeDocument\" Target=\"/mathcad/worksheet.xml\" Id=\"R0000000000000001\" /><Relationship Type=\"{REL_OFFICE}/officeDocument\" Target=\"/mathcad/header.xml\" Id=\"R0000000000000002\" /><Relationship Type=\"{REL_OFFICE}/officeDocument\" Target=\"/mathcad/footer.xml\" Id=\"R0000000000000003\" /><Relationship Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"/docProps/core.xml\" Id=\"R0000000000000004\" /></Relationships>"
    )
}

const APP: &str = r#"<?xml version="1.0" encoding="utf-8"?><properties xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns="http://schemas.mathsoft.com/extended-properties"><appVersion>10.0.0.0</appVersion><serializationVersion architecture="x64" Culture="nl-NL" UiCulture="nl-NL">10.0.0.0</serializationVersion><engineVersion>10.0.0.17</engineVersion></properties>"#;

const CALCULATION: &str = r#"<?xml version="1.0" encoding="utf-8"?><calculation cache-worksheet="true" xmlns="http://schemas.ptc.com/mathcad/settings/calculation10"><builtInVariables array-origin="0" convergence-tolerance="0.001" constraint-tolerance="0.001" prn-col-width="8" prn-precision="4" random-seed="1" /><calculationBehavior automatic-recalculation="true" strings-use-origin="false" matrix-strict-singularity-check="false" exact-boolean="true" units-in-symbolics="false" constants-in-symbolics="false" symbolic-engine="axiom" /><units><currentUnitSystem name="si" /></units></calculation>"#;

const PRESENTATION: &str = r#"<?xml version="1.0" encoding="utf-8"?><presentation xmlns:ws="http://schemas.mathsoft.com/worksheet50" xmlns="http://schemas.ptc.com/mathcad/settings/presentation10"><pageModel paper-code="A4" grid-size="Standard" orientation="Portrait" margin-type="Standard" spell-check-enabled="false" /></presentation>"#;

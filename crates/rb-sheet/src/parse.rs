//! Het tekstformaat van een werkblad (`.rkn`). Eén regel is één regio:
//!
//! ```text
//! # Kop                          kop (wordt letterlijk getoond)
//! " Tekstregio                   tekst (begint met een aanhalingsteken, zoals in Mathcad)
//! // commentaar                  commentaar (wordt letterlijk getoond)
//! a := 5 m                       definitie
//! a * 2 =                        resultaat
//! a * 2 = cm                     resultaat in gekozen eenheid
//! sweep x = 1 m ; 5 m ; 5 : x^2 =   parametersweep: 5 punten van 1 m tot 5 m
//! @40,3 b := 2 m                 regio op kolom 40, rij 3 (zie hieronder)
//! ```
//! Achter een wiskunderegel mag `// toelichting` staan.
//!
//! **Posities.** Net als in Mathcad heeft elke regio een plek op het blad, en wordt het blad van
//! boven naar beneden en daarbinnen van links naar rechts doorgerekend. Zonder `@` staat een regio
//! in kolom 0 van de rij met zijn regelnummer. Met `@kolom,rij` zet je hem ergens anders neer; zo
//! kan een definitie naast een andere staan, of eerder gelden dan zijn plek in het bestand. Een
//! grafische interface schrijft later voor elke regio een positie.

use rb_core::{parse_expr_str, parse_stmt, Error, Expr, Stmt};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq)]
pub struct SweepSpec {
    pub var: String,
    pub start: Expr,
    pub end: Expr,
    pub points: Expr,
    pub body: Stmt,
}

impl SweepSpec {
    /// Namen die deze sweep van buiten gebruikt (de sweepvariabele zelf telt niet mee).
    pub fn uses(&self) -> BTreeSet<String> {
        let none = BTreeSet::new();
        let mut out = BTreeSet::new();
        self.start.collect_names(&none, &mut out);
        self.end.collect_names(&none, &mut out);
        self.points.collect_names(&none, &mut out);
        for n in self.body.uses() {
            if n != self.var {
                out.insert(n);
            }
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Heading(String),
    Text(String),
    Comment(String),
    Math(Stmt),
    Sweep(SweepSpec),
    /// Een grafiek (`plot x = 0 ; 10 : sin(x)`).
    Plot(crate::plot::PlotSpec),
    /// `Given`: begin van een solve block.
    Given,
    /// Een beperking in een solve block, zoals `x + y == 10` of `x > 0`.
    Constraint(Expr),
    /// Een formule die alleen getoond wordt (buiten een solve block, zonder `:=` of `=`), zoals
    /// `P == 1/2 ρ v^3 A` als uitleg. Mathcad rekent die ook niet uit.
    Formula(Expr),
    /// Een afbeelding (`image 200x120 data:image/png;base64,...`; breedte en hoogte in punten).
    Image(Image),
    /// Een tabel (`tabel x [mm], F [kN] { ... }`): elke kolom definieert een kolomvector.
    Table(crate::table::TableSpec),
    /// Begin van een gebied (`area Titel`, ingeklapt: `area ingeklapt Titel`). Wat tussen begin en
    /// `area end` staat, wordt bij een ingeklapt gebied niet getoond maar wel doorgerekend.
    Area { title: String, collapsed: bool },
    /// `area end`
    AreaEnd,
    /// `pagebreak`: wat eronder staat, begint op een nieuwe pagina.
    PageBreak,
    /// `header ...` of `footer ...`: kop- of voettekst op elke pagina, met `{pagina}`, `{paginas}`,
    /// `{bestand}` en `{datum}`.
    HeaderFooter { text: String, footer: bool },
    /// Een regel die niet gelezen kon worden. `caret`: de fout hoort bij een positie in de bronregel.
    Invalid { err: Error, caret: bool },
}

/// Een afbeelding in het werkblad: de PNG-bestandsinhoud en de getoonde grootte.
#[derive(Debug, Clone)]
pub struct Image {
    /// Breedte en hoogte op de pagina, in punten.
    pub width: f32,
    pub height: f32,
    pub png: std::sync::Arc<Vec<u8>>,
    /// Kenmerk van de inhoud (voor caches in de interface).
    pub hash: u64,
}

impl PartialEq for Image {
    fn eq(&self, o: &Image) -> bool {
        self.hash == o.hash && self.width == o.width && self.height == o.height
    }
}

/// `image BxH data:image/png;base64,...` lezen.
fn parse_image(rest: &str) -> Result<Image, Error> {
    let bad = || parse_err("een afbeelding schrijf je als 'image 200x120 data:image/png;base64,...'");
    let (size, data) = rest.trim().split_once(' ').ok_or_else(bad)?;
    let (w, h) = size.split_once('x').ok_or_else(bad)?;
    let (width, height): (f32, f32) = (w.parse().map_err(|_| bad())?, h.parse().map_err(|_| bad())?);
    let b64 = data.trim().strip_prefix("data:image/png;base64,").ok_or_else(|| parse_err("alleen PNG-afbeeldingen worden ondersteund"))?;
    let png = rb_core::base64::decode(b64).ok_or_else(|| parse_err("de afbeelding is beschadigd (geen geldige base64)"))?;
    // FNV-1a
    let hash = png.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ *b as u64).wrapping_mul(0x0100_0000_01b3));
    Ok(Image { width, height, png: std::sync::Arc::new(png), hash })
}

/// Plek van een regio op het blad (in tekens: kolom en rij).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pos {
    pub row: usize,
    pub col: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Region {
    /// Regelnummer in het bestand (vanaf 1).
    pub line: usize,
    /// Plek op het blad; bepaalt de volgorde van doorrekenen.
    pub pos: Pos,
    /// Stond er een expliciete `@kolom,rij` voor?
    pub placed: bool,
    /// De regel zoals getypt (zonder omringende spaties en zonder `@kolom,rij`).
    pub source: String,
    pub kind: Kind,
}

impl Region {
    /// Maak een regio van een (ongetrimde) bronregel op regelnummer `line`.
    pub fn from_line(line: usize, text: &str) -> Region {
        let text = text.trim();
        let (pos, rest) = match split_pos(text) {
            Some(Ok((pos, rest))) => (Some(pos), rest),
            Some(Err(err)) => {
                return Region {
                    line,
                    pos: Pos { row: line, col: 0 },
                    placed: false,
                    source: text.to_string(),
                    kind: Kind::Invalid { err, caret: false },
                }
            }
            None => (None, text),
        };
        let source = rest.trim().to_string();
        let kind = parse_kind(&source);
        Region {
            line,
            pos: pos.unwrap_or(Pos { row: line, col: 0 }),
            placed: pos.is_some(),
            source,
            kind,
        }
    }
}

/// `@kolom,rij rest` → (positie, rest). `None` als de regel niet met `@` begint.
fn split_pos(text: &str) -> Option<Result<(Pos, &str), Error>> {
    let rest = text.strip_prefix('@')?;
    let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
    let (spec, tail) = rest.split_at(end);
    let parsed = spec.split_once(',').and_then(|(c, r)| {
        Some(Pos {
            col: c.trim().parse().ok()?,
            row: r.trim().parse().ok()?,
        })
    });
    Some(match parsed {
        Some(pos) if !tail.trim().is_empty() => Ok((pos, tail)),
        Some(_) => Err(parse_err("na de positie '@kolom,rij' ontbreekt de inhoud van de regio")),
        None => Err(parse_err(
            "een positie schrijf je als '@kolom,rij' met gehele getallen, bijvoorbeeld '@40,3 b := 2 m'",
        )),
    })
}

/// Lees een heel werkblad. Lege regels zijn geen regio's. De regio's staan in de volgorde waarin
/// ze worden doorgerekend: op positie (rij, dan kolom), bij gelijke positie op regelnummer.
pub fn parse_sheet(src: &str) -> Vec<Region> {
    // Kladblok en PowerShell zetten soms een BOM voor UTF-8-tekst
    let src = src.strip_prefix('\u{feff}').unwrap_or(src);
    let lines: Vec<&str> = src.lines().collect();
    let mut regions = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i];
        if l.trim().is_empty() {
            i += 1;
            continue;
        }
        // een programmablok of tabel: van `... := {` of `tabel ... {` tot en met een regel `}`
        if opens_block(l) || crate::table::opens_table(split_pos(l.trim()).and_then(Result::ok).map_or(l, |(_, rest)| rest)) {
            match lines[i + 1..].iter().position(|x| x.trim() == "}") {
                Some(k) => {
                    let end = i + 1 + k;
                    regions.push(Region::from_line(i + 1, &lines[i..=end].join("\n")));
                    i = end + 1;
                }
                None => {
                    regions.push(Region::from_line(i + 1, &lines[i..].join("\n")));
                    i = lines.len();
                }
            }
            continue;
        }
        regions.push(Region::from_line(i + 1, l));
        i += 1;
    }
    sort_regions(&mut regions);
    regions
}

/// Begint deze regel een programmablok (`f(x) := {`)?
fn opens_block(line: &str) -> bool {
    let code = line.find("//").map_or(line, |k| &line[..k]).trim_end();
    code.ends_with('{') && (code.contains(":=") || code.contains('←'))
}

/// Zet regio's in de volgorde van doorrekenen en herken solve blocks.
pub fn sort_regions(regions: &mut [Region]) {
    regions.sort_by_key(|r| (r.pos, r.line));
    mark_given_blocks(regions);
}

/// Namen van functies die een solve block afsluiten.
pub const SOLVERS: &[&str] = &["Find", "Minerr", "Minimize", "Maximize", "odesolve"];

/// Herken solve blocks: een regio `Given`, daaronder beperkingen (vergelijkingen zonder `=` of
/// `:=`), tot en met de eerste regio die `Find`, `Minerr`, `Minimize`, `Maximize` of `odesolve`
/// gebruikt. Wordt na elke wijziging opnieuw toegepast (de soort van een regel hangt af van de
/// regels erboven).
pub fn mark_given_blocks(regions: &mut [Region]) {
    let mut in_given = false;
    for r in regions.iter_mut() {
        let code = strip_comment(&r.source);
        if code.eq_ignore_ascii_case("given") {
            r.kind = Kind::Given;
            in_given = true;
            continue;
        }
        // een eerder herkende beperking of Given opnieuw beoordelen
        if matches!(r.kind, Kind::Given | Kind::Constraint(_) | Kind::Formula(_)) {
            r.kind = parse_kind(&r.source);
        }
        if !in_given {
            // een losse uitdrukking buiten een solve block: alleen tonen
            if let Kind::Invalid { .. } = r.kind {
                if let Ok(e) = parse_expr_str(code) {
                    r.kind = Kind::Formula(e);
                }
            }
            continue;
        }
        match &r.kind {
            Kind::Invalid { .. } => {
                if let Ok(e) = parse_expr_str(code) {
                    if matches!(e, Expr::Cmp(..) | Expr::And(..)) {
                        r.kind = Kind::Constraint(e);
                    }
                }
            }
            Kind::Math(stmt) => {
                if stmt.uses().iter().any(|n| SOLVERS.contains(&n.as_str())) || matches!(stmt, Stmt::Define { body: Expr::Bind { op: rb_core::BindOp::OdeSolve, .. }, .. }) {
                    in_given = false;
                }
            }
            _ => {}
        }
    }
}

/// Bepaal het soort regio van één (getrimde) regel.
/// Gebieden, pagina-einden en kop- en voetteksten. Een regel met `=` of `←` is een formule (zodat
/// `area := 5` gewoon een variabele blijft).
fn layout_kind(source: &str) -> Option<Kind> {
    let s = strip_comment(source).trim();
    if s.contains('=') || s.contains('←') || s.contains('\n') {
        return None;
    }
    let word = |w: &str| s == w || s.starts_with(&format!("{w} "));
    if word("pagebreak") {
        return (s == "pagebreak").then_some(Kind::PageBreak);
    }
    if word("area") {
        let rest = s["area".len()..].trim();
        if rest == "end" {
            return Some(Kind::AreaEnd);
        }
        let (collapsed, title) = match rest.strip_prefix("ingeklapt").or_else(|| rest.strip_prefix("collapsed")) {
            Some(t) if t.is_empty() || t.starts_with(' ') => (true, t.trim()),
            _ => (false, rest),
        };
        return Some(Kind::Area { title: title.to_string(), collapsed });
    }
    for (w, footer) in [("header", false), ("footer", true)] {
        if let Some(rest) = s.strip_prefix(w).filter(|r| r.starts_with(' ')) {
            let t = rest.trim();
            let t = t.strip_prefix('"').and_then(|t| t.strip_suffix('"')).unwrap_or(t);
            return Some(Kind::HeaderFooter { text: t.to_string(), footer });
        }
    }
    None
}

pub fn parse_kind(source: &str) -> Kind {
    if let Some(rest) = source.strip_prefix('"') {
        return Kind::Text(rest.trim().to_string());
    }
    if source.starts_with('#') {
        return Kind::Heading(source.to_string());
    }
    if source.starts_with("//") {
        return Kind::Comment(source.to_string());
    }
    if let Some(k) = layout_kind(source) {
        return k;
    }
    if source.starts_with("tabel ") || source.starts_with("table ") {
        return match crate::table::parse_table(source) {
            Ok(t) => Kind::Table(t),
            Err(err) => Kind::Invalid { err, caret: false },
        };
    }
    if source.contains('\n') {
        // programmablok: commentaar per regel haalt de blokparser weg; geen caret over meerdere regels
        return match parse_stmt(source, false) {
            Ok(s) => Kind::Math(s),
            Err(err) => Kind::Invalid { err, caret: false },
        };
    }
    if let Some(rest) = source.strip_prefix("image ") {
        return match parse_image(rest) {
            Ok(i) => Kind::Image(i),
            Err(err) => Kind::Invalid { err, caret: false },
        };
    }
    let math = strip_comment(source);
    if let Some(rest) = math.strip_prefix("plot ") {
        return match crate::plot::parse_plot(rest) {
            Ok(p) => Kind::Plot(p),
            Err(err) => Kind::Invalid { err, caret: false },
        };
    }
    if let Some(rest) = math.strip_prefix("sweep ") {
        return match parse_sweep(rest) {
            Ok(s) => Kind::Sweep(s),
            Err(err) => Kind::Invalid { err, caret: false },
        };
    }
    match parse_stmt(math, false) {
        Ok(s) => Kind::Math(s),
        Err(err) => Kind::Invalid { err, caret: true },
    }
}

fn strip_comment(s: &str) -> &str {
    match s.find("//") {
        Some(i) => s[..i].trim_end(),
        None => s,
    }
}

fn parse_err(msg: &str) -> Error {
    Error::Parse {
        pos: 0,
        msg: msg.to_string(),
    }
}

/// `x = begin ; einde ; aantal : uitdrukking =`
fn parse_sweep(rest: &str) -> Result<SweepSpec, Error> {
    const SYNTAX: &str = "sweep-syntax: sweep naam = begin ; einde ; aantal : uitdrukking =";
    // het scheidingsteken is de eerste ':' die geen ':=' is
    let bytes = rest.as_bytes();
    let mut colon = None;
    for (i, &b) in bytes.iter().enumerate() {
        if b == b':' && bytes.get(i + 1) != Some(&b'=') {
            colon = Some(i);
            break;
        }
    }
    let colon = colon.ok_or_else(|| parse_err(SYNTAX))?;
    let (header, body) = (&rest[..colon], &rest[colon + 1..]);
    let (var_part, ranges) = header.split_once('=').ok_or_else(|| parse_err(SYNTAX))?;
    let var = var_part.trim().to_string();
    if var.is_empty() || !var.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.') {
        return Err(parse_err("sweep: de naam van de sweepvariabele is ongeldig"));
    }
    let parts: Vec<&str> = ranges.split(';').collect();
    if parts.len() != 3 {
        return Err(parse_err(SYNTAX));
    }
    let start = parse_expr_str(parts[0])?;
    let end = parse_expr_str(parts[1])?;
    let points = parse_expr_str(parts[2])?;
    let body = parse_stmt(body.trim(), false)?;
    if !matches!(body, Stmt::Eval { .. }) {
        return Err(parse_err(
            "sweep: na de ':' moet een resultaatregel staan, bijvoorbeeld 'x^2 ='",
        ));
    }
    Ok(SweepSpec {
        var,
        start,
        end,
        points,
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soorten_regels() {
        let regions = parse_sheet("# Kop\n\n\" Tekst hier\n// opm\na := 5 m // toelichting\na =\n");
        let kinds: Vec<&str> = regions
            .iter()
            .map(|r| match &r.kind {
                Kind::Heading(_) => "kop",
                Kind::Text(_) => "tekst",
                Kind::Comment(_) => "opm",
                Kind::Math(Stmt::Define { .. }) => "def",
                Kind::Math(Stmt::Eval { .. }) => "eval",
                Kind::Math(Stmt::Symbolic { .. }) => "symbolisch",
                Kind::Sweep(_) => "sweep",
                Kind::Table(_) => "tabel",
                Kind::Area { .. } | Kind::AreaEnd | Kind::PageBreak | Kind::HeaderFooter { .. } => "opmaak",
                Kind::Plot(_) => "grafiek",
                Kind::Given => "given",
                Kind::Constraint(_) => "beperking",
                Kind::Formula(_) => "formule",
                Kind::Image(_) => "afbeelding",
                Kind::Invalid { .. } => "fout",
            })
            .collect();
        assert_eq!(kinds, vec!["kop", "tekst", "opm", "def", "eval"]);
        // regelnummers slaan lege regels over
        assert_eq!(regions[1].line, 3);
        // een BOM aan het begin van het bestand wordt genegeerd
        assert!(matches!(parse_sheet("\u{feff}a := 1\n")[0].kind, Kind::Math(_)));
    }

    #[test]
    fn posities_bepalen_de_volgorde() {
        // b staat op regel 3 maar wordt op rij 1 (rechts van a) geplaatst, en dus eerder doorgerekend
        let regions = parse_sheet("a := 1\nc := a + b\n@30,1 b := 2\n");
        let order: Vec<&str> = regions.iter().map(|r| r.source.as_str()).collect();
        assert_eq!(order, vec!["a := 1", "b := 2", "c := a + b"]);
        assert!(regions[1].placed);
        assert_eq!(regions[1].pos, Pos { row: 1, col: 30 });
        assert_eq!(regions[1].line, 3);
        assert!(matches!(Region::from_line(1, "@x,y a := 1").kind, Kind::Invalid { .. }));
        assert!(matches!(Region::from_line(1, "@1,2").kind, Kind::Invalid { .. }));
    }

    #[test]
    fn sweep_parsen() {
        match parse_kind("sweep x = 1 m ; 5 m ; 5 : x^2 = m^2") {
            Kind::Sweep(s) => {
                assert_eq!(s.var, "x");
                assert!(s.uses().is_empty() || s.uses().iter().all(|n| n == "m"));
            }
            k => panic!("{k:?}"),
        }
        assert!(matches!(parse_kind("sweep x = 1 ; 2 : x ="), Kind::Invalid { .. }));
        assert!(matches!(parse_kind("sweep x = 1 ; 2 ; 3 : y := 2"), Kind::Invalid { .. }));
    }
}

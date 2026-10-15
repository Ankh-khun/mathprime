//! Omzetten van Mathcad-formules, en (als de bestanden er zijn) echte bladen tegen de uitkomsten van
//! Mathcad zelf.

use super::*;

fn stmt(xml_src: &str) -> Vec<String> {
    let n = xml::parse(xml_src).unwrap();
    math::statement(&n, &mut 0).unwrap()
}

const SPAN: &str = r#"<Span xmlns="x" xmlns:pw="y">"#;

#[test]
fn definitie_met_subscript_en_eenheid() {
    let v = format!(r#"<ml:define><ml:id labels="VARIABLE">{SPAN}V<pw:Subscript>ref</pw:Subscript></Span></ml:id><ml:apply><ml:scale /><ml:real>5</ml:real><ml:id labels="UNIT">V</ml:id></ml:apply></ml:define>"#);
    assert_eq!(stmt(&v), vec!["V.ref := 5 V"]);
    // definiëren en meteen tonen in mV
    let v = format!(r#"<ml:define><ml:id>Step</ml:id><ml:eval><ml:apply><ml:div /><ml:id>{SPAN}V<pw:Subscript>ref</pw:Subscript></Span></ml:id><ml:apply><ml:pow /><ml:real>2</ml:real><ml:real>10</ml:real></ml:apply></ml:apply><ml:unitOverride><ml:id labels="UNIT">mV</ml:id></ml:unitOverride></ml:eval></ml:define>"#);
    assert_eq!(stmt(&v), vec!["Step := V.ref / 2^10 = mV"]);
}

#[test]
fn voorrang_en_haakjes() {
    // (a + b) * c en a - (b - c)
    let v = r#"<ml:eval><ml:apply><ml:mult /><ml:apply><ml:plus /><ml:id>a</ml:id><ml:id>b</ml:id></ml:apply><ml:id>c</ml:id></ml:apply><ml:unitOverride><ml:placeholder /></ml:unitOverride></ml:eval>"#;
    assert_eq!(stmt(v), vec!["(a + b) * c ="]);
    let v = r#"<ml:eval><ml:apply><ml:minus /><ml:id>a</ml:id><ml:apply><ml:minus /><ml:id>b</ml:id><ml:id>c</ml:id></ml:apply></ml:apply></ml:eval>"#;
    assert_eq!(stmt(v), vec!["a - (b - c) ="]);
}

#[test]
fn functies_integralen_en_bereiken() {
    let v = r#"<ml:define><ml:function><ml:id>f</ml:id><ml:boundVars><ml:id>x</ml:id><ml:id>y</ml:id></ml:boundVars></ml:function><ml:apply><ml:nthRoot /><ml:placeholder /><ml:apply><ml:plus /><ml:id>x</ml:id><ml:id>y</ml:id></ml:apply></ml:apply></ml:define>"#;
    assert_eq!(stmt(v), vec!["f(x, y) := sqrt(x + y)"]);
    let v = r#"<ml:define><ml:id>I</ml:id><ml:apply><ml:integral /><ml:lambda><ml:boundVars><ml:id>t</ml:id></ml:boundVars><ml:apply><ml:id labels="FUNCTION">sin</ml:id><ml:id>t</ml:id></ml:apply></ml:lambda><ml:lowerBound><ml:real>0</ml:real></ml:lowerBound><ml:upperBound><ml:id>π</ml:id></ml:upperBound></ml:apply></ml:define>"#;
    assert_eq!(stmt(v), vec!["I := integral(sin(t), t, 0, π)"]);
    let v = r#"<ml:define><ml:id>w</ml:id><ml:range><ml:sequence><ml:real>0</ml:real><ml:real>1</ml:real></ml:sequence><ml:real>300</ml:real></ml:range></ml:define>"#;
    assert_eq!(stmt(v), vec!["w := 0, 1 .. 300"]);
    // find → Find, vector links: hulpvariabele
    let v = r#"<ml:define><ml:matrix rows="2" cols="1"><ml:id>a</ml:id><ml:id>b</ml:id></ml:matrix><ml:eval><ml:apply><ml:id labels="KEYWORD">find</ml:id><ml:sequence><ml:id>x</ml:id><ml:id>y</ml:id></ml:sequence></ml:apply><ml:unitOverride><ml:placeholder /></ml:unitOverride></ml:eval></ml:define>"#;
    assert_eq!(stmt(v), vec!["opl.1 := Find(x, y)", "a := opl.1[0] =", "b := opl.1[1] ="]);
}

#[test]
fn matrix_kolom_voor_kolom() {
    let v = r#"<ml:define><ml:id>M</ml:id><ml:matrix rows="2" cols="2"><ml:real>1</ml:real><ml:real>3</ml:real><ml:real>2</ml:real><ml:real>4</ml:real></ml:matrix></ml:define>"#;
    assert_eq!(stmt(v), vec!["M := [1, 2; 3, 4]"]);
}

/// Mappen met echte Mathcad-bladen voor de vergelijkingstests: `MATHPRIME_MCDX_DIRS`, gescheiden door `;`
/// (bijvoorbeeld `F:\Bladen;F:\School`). Zonder die variabele worden deze tests overgeslagen.
fn real_sheet_dirs() -> Vec<String> {
    std::env::var("MATHPRIME_MCDX_DIRS").map(|v| v.split(';').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect()).unwrap_or_default()
}

/// Echte bladen (als ze op deze computer staan): inlezen, doorrekenen en vergelijken met wat Mathcad
/// zelf uitrekende. Draai met `cargo test -p rb-mcdx -- --nocapture` voor het overzicht.
#[test]
fn echte_bladen_tegen_mathcad() {
    let dirs = real_sheet_dirs();
    let grid = Grid { col_w: 6.4, row_h: 14.0 };
    for d in dirs {
        let Ok(entries) = std::fs::read_dir(d) else { continue };
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("mcdx") {
                continue;
            }
            let bytes = std::fs::read(&p).unwrap();
            let imp = import(&bytes, grid).unwrap_or_else(|err| panic!("{}: {err}", p.display()));
            let sheet = rb_sheet::Sheet::load(&imp.source);
            if let Some(dir) = std::env::var_os("MCDX_DUMP") {
                let out = std::path::Path::new(&dir).join(p.file_stem().unwrap()).with_extension("rkn");
                std::fs::write(out, &imp.source).unwrap();
            }
            let report = crate::compare::compare(&imp);
            let (same, differ) = (report.count(crate::compare::Status::Gelijk), report.count(crate::compare::Status::Anders));
            println!(
                "{}: {} regio's, {} fouten, {} niet overgenomen; uitkomsten gelijk {same}, anders {differ}",
                p.file_name().unwrap().to_string_lossy(),
                imp.regions,
                sheet.error_count(),
                imp.notes.len()
            );
            for r in report.rows.iter().filter(|r| r.status != crate::compare::Status::Gelijk).take(12) {
                println!("    {} → MathPrime {}, Mathcad {}", r.region, r.mathprime, r.mathcad);
            }
            assert_eq!(differ, 0, "{}: uitkomsten wijken af van Mathcad", p.display());
            if std::env::var_os("MCDX_FOUTEN").is_some() {
                for (i, r) in sheet.regions().iter().enumerate() {
                    if let rb_sheet::Output::Error { message, .. } = sheet.output(i) {
                        println!("    ✗ {}  →  {message}", r.source.lines().next().unwrap_or(""));
                    }
                }
                for n in &imp.notes {
                    println!("    • {n}");
                }
            }
        }
    }
}

#[test]
fn rekeninstellingen_lezen() {
    let s = Settings::parse(
        r#"<calculation><builtInVariables array-origin="1" convergence-tolerance="0.0001" constraint-tolerance="0.001" random-seed="7"/><units><currentUnitSystem name="us"/></units></calculation>"#,
    );
    assert_eq!(s, Settings { origin: 1, seed: 7.0, unit_system: "us".into(), tol: 0.0001, ctol: 0.001 });
    let (lines, notes) = s.translate();
    assert!(lines[0].starts_with("ORIGIN := 1"), "{lines:?}");
    assert!(lines[1].starts_with("SEED := 7"), "{lines:?}");
    assert_eq!(notes.len(), 2, "{notes:?}");
    // standaardwaarden: niets toevoegen
    let (lines, notes) = Settings::default().translate();
    assert!(lines.is_empty() && notes.is_empty());
    // onleesbaar of leeg: standaard
    assert_eq!(Settings::parse("geen xml"), Settings::default());
}

/// Een ZIP-archief zonder compressie (voor tests).
fn zip_stored(files: &[(&str, &str)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in files {
        let offset = out.len() as u32;
        let header = |sig: u32, central: bool| {
            let mut h = sig.to_le_bytes().to_vec();
            if central {
                h.extend_from_slice(&20u16.to_le_bytes()); // gemaakt door
            }
            h.extend_from_slice(&20u16.to_le_bytes()); // nodig om uit te pakken
            h.extend_from_slice(&[0, 0, 0, 0]); // vlaggen, methode 0 (stored)
            h.extend_from_slice(&[0, 0, 0, 0]); // tijd, datum
            h.extend_from_slice(&0u32.to_le_bytes()); // crc (de lezer controleert hem niet)
            h.extend_from_slice(&(data.len() as u32).to_le_bytes());
            h.extend_from_slice(&(data.len() as u32).to_le_bytes());
            h.extend_from_slice(&(name.len() as u16).to_le_bytes());
            h.extend_from_slice(&0u16.to_le_bytes()); // extra
            if central {
                h.extend_from_slice(&[0; 6]); // commentaar, schijf, interne attributen
                h.extend_from_slice(&0u32.to_le_bytes()); // externe attributen
                h.extend_from_slice(&offset.to_le_bytes());
            }
            h.extend_from_slice(name.as_bytes());
            h
        };
        out.extend(header(0x0403_4b50, false));
        out.extend_from_slice(data.as_bytes());
        central.extend(header(0x0201_4b50, true));
    }
    let cd_offset = out.len() as u32;
    let cd_size = central.len() as u32;
    out.extend(central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

#[test]
fn eerste_index_uit_mathcad_telt_mee() {
    // v := [10; 20; 30] en v[1] =: met ORIGIN 1 is dat 10, met de standaard (0) zou het 20 zijn
    let ws = r#"<worksheet><regions>
      <region top="20" left="20"><math><ml:define><ml:id>v</ml:id><ml:matrix rows="3" cols="1"><ml:real>10</ml:real><ml:real>20</ml:real><ml:real>30</ml:real></ml:matrix></ml:define></math></region>
      <region top="60" left="20"><math><ml:eval><ml:apply><ml:indexer /><ml:id>v</ml:id><ml:real>1</ml:real></ml:apply><ml:unitOverride><ml:placeholder /></ml:unitOverride></ml:eval></math></region>
    </regions></worksheet>"#;
    let calc = r#"<calculation><builtInVariables array-origin="1" random-seed="1" convergence-tolerance="0.001" constraint-tolerance="0.001"/><units><currentUnitSystem name="si"/></units></calculation>"#;
    let grid = Grid { col_w: 6.4, row_h: 14.0 };
    let with = |files: &[(&str, &str)]| {
        let imp = import(&zip_stored(files), grid).unwrap();
        let sheet = rb_sheet::Sheet::load(&imp.source);
        (imp, sheet.render())
    };
    let (imp, r) = with(&[("mathcad/worksheet.xml", ws), ("mathcad/settings/calculation.xml", calc)]);
    assert!(imp.source.lines().next().unwrap().contains("ORIGIN := 1"), "{}", imp.source);
    assert!(r.contains("v[1] = 10"), "{r}");
    // zonder instellingen: de standaard van Mathcad (0)
    let (imp, r) = with(&[("mathcad/worksheet.xml", ws)]);
    assert!(!imp.source.contains("ORIGIN"), "{}", imp.source);
    assert!(r.contains("v[1] = 20"), "{r}");
}

/// Alle formules met hun uitkomst (gesorteerd, zonder posities).
fn outcomes(src: &str) -> Vec<String> {
    let sheet = rb_sheet::Sheet::load(src);
    let mut v: Vec<String> = sheet
        .regions()
        .iter()
        .enumerate()
        .filter_map(|(i, r)| {
            // de formule zonder schrijfwijze (spaties, `*`, haakjes): `3 kN/m` = `3 kN / m` en
            // `0.0269 * 2 ω` = `0.0269 * 2 * ω`; de uitkomst hieronder vangt verschillen in betekenis
            let norm = |s: &str| match rb_core::lex(s) {
                // getallen als waarde (0.000200 = 0.0002), zonder `*` en haakjes
                Ok(toks) => toks
                    .iter()
                    .filter(|t| !matches!(t.tok, rb_core::Tok::Star | rb_core::Tok::LParen | rb_core::Tok::RParen))
                    .map(|t| format!("{:?}", t.tok))
                    .collect::<Vec<_>>()
                    .join(" "),
                Err(_) => s.to_string(),
            };
            let meaning = match &r.kind {
                rb_sheet::Kind::Math(st) => {
                    let _ = canon(st);
                    norm(&r.source)
                }
                rb_sheet::Kind::Formula(_) | rb_sheet::Kind::Constraint(_) | rb_sheet::Kind::Plot(_) => norm(&r.source),
                _ => return None,
            };
            Some(format!("{}  →  {meaning}  →  {}", r.source, format!("{:?}", sheet.output(i)).replace(' ', "")))
        })
        .map(|s| s.split("  →  ").skip(1).collect::<Vec<_>>().join("  →  "))
        .collect();
    v.sort();
    v
}

#[test]
fn opslaan_als_mcdx_en_terug() {
    let src = "\
# Kop
\" Een tekst met \"\"aanhalingstekens\"\"
// commentaar
L := 4 m
q := 3 kN/m
M.max := q * L^2 / 8 = kN*m
w(x) := q * x^2 / 2
w(2 m) = kN*m
A := [1, 2; 3, 4]
A[1, 0] =
i := 1..5
s := sum(k^2, k, 1, 10) =
I := integral(x^2, x, 0, 1) =
d := deriv(x^3, x, x) =
r := sqrt(16) + nthroot(27, 3) + abs(-2) =
b := 1 < 2 and not 3 == 4 =
t := concat(\"ab\", \"cd\") =
ORIGIN := 1
v := [10; 20; 30]
v[1] =
x := 1
y := 1
Given
x + y == 10
x - y == 2
opl := Find(x, y) =
P == F * v
plot z = 0 ; 10 : z^2, 2 * z
";
    let grid = Grid { col_w: 6.4, row_h: 14.0 };
    let out = export(src, grid);
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    let back = import(&out.bytes, grid).unwrap();
    assert_eq!(outcomes(&back.source), outcomes(src), "\n--- terug ingelezen:\n{}", back.source);
    // tekst, kop en grafiek komen ook terug
    assert!(back.source.contains("# Kop") && back.source.contains("aanhalingstekens") && back.source.contains("plot "), "{}", back.source);
}

#[test]
fn echte_bladen_heen_en_terug() {
    let grid = Grid { col_w: 6.4, row_h: 14.0 };
    for d in real_sheet_dirs() {
        let Ok(entries) = std::fs::read_dir(d) else { continue };
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("mcdx") {
                continue;
            }
            let first = import(&std::fs::read(&p).unwrap(), grid).unwrap();
            let out = export(&first.source, grid);
            let again = import(&out.bytes, grid).unwrap_or_else(|err| panic!("{}: {err}", p.display()));
            let (a, b) = (outcomes(&first.source), outcomes(&again.source));
            let lost: Vec<&String> = a.iter().filter(|x| !b.contains(x)).collect();
            println!("{}: {} formules, {} anders na heen en terug, {} als tekst", p.file_name().unwrap().to_string_lossy(), a.len(), lost.len(), out.notes.len());
            for l in lost.iter().take(6) {
                println!("    {l}");
            }
            for n in out.notes.iter().take(6) {
                println!("    • {n}");
            }
            assert!(lost.len() <= out.notes.len(), "{}: {} formules veranderden", p.display(), lost.len());
        }
    }
}

/// Een regel zonder de brontekst van eenheden en kopjes (die verschilt alleen in spaties).
fn canon(s: &rb_core::Stmt) -> rb_core::Stmt {
    use rb_core::{Show, Stmt};
    match s.clone() {
        Stmt::Eval { expr, unit, bare, .. } => Stmt::Eval { expr, lhs_src: String::new(), unit: unit.map(|(u, _)| (u, String::new())), bare },
        Stmt::Define { name, params, index, body, show } => Stmt::Define {
            name,
            params,
            index,
            body,
            show: show.map(|sh| Show { unit: sh.unit.map(|(u, _)| (u, String::new())), head_src: String::new() }),
        },
        Stmt::Symbolic { expr, op, .. } => Stmt::Symbolic { expr, op, lhs_src: String::new() },
    }
}

/// Lessen uit de vergelijking met Mathcad zelf (tools/mathcad-vergelijk.ps1).
#[test]
fn export_volgt_mathcad() {
    let grid = Grid { col_w: 6.4, row_h: 14.0 };
    let ws = |src: &str| {
        let out = export(src, grid);
        let files = zip::read(&out.bytes).unwrap();
        String::from_utf8(files["mathcad/worksheet.xml"].clone()).unwrap()
    };
    // `m` is een eenheid tot het blad een variabele m definieert (anders: "variable undefined")
    let x = ws("L := 3.5 m\nm := 4 kg\nF := m * 2 m/s^2 =\n");
    let first = &x[..x.find(">L<").unwrap() + 400];
    assert!(first.contains(r#"labels="UNIT">m</ml:id>"#), "{first}");
    // Mathcad kent geen inverse(): M^-1
    let x = ws("M := [2, 1; 1, 3]\nMi := inverse(M) =\n");
    assert!(x.contains("<ml:pow />") && !x.contains(">inverse<"), "{x}");
    // root is een sleutelwoord (zoals find)
    let x = ws("z := 1\nR := root(z^2 - 2, z) =\n");
    assert!(x.contains(r#"labels="KEYWORD">root</ml:id>"#), "{x}");
    // en M^-1 komt terug als inverse die hetzelfde uitrekent
    let back = import(&export("M := [2, 1; 1, 3]\nMi := inverse(M) =\n", grid).bytes, grid).unwrap();
    assert!(rb_sheet::Sheet::load(&back.source).render().contains("0.6"), "{}", back.source);
}

#[test]
fn symbolisch_heen_en_terug() {
    let grid = Grid { col_w: 6.4, row_h: 14.0 };
    let src = "(x + 1)^2 → expand\n\nx^2 - 5*x + 6 → factor\n\nx^2 - 4 == 0 → solve, x\n\na*x^2 + b → substitute, x = 2\n\nderiv(x^3, x, x) →\n";
    let out = export(src, grid);
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    let files = zip::read(&out.bytes).unwrap();
    let ws = String::from_utf8(files["mathcad/worksheet.xml"].clone()).unwrap();
    assert!(ws.contains("<ml:symEval>") && ws.contains("<ml:command>") && ws.contains(r#"labels="KEYWORD">solve</ml:id>"#), "{ws}");
    let back = import(&out.bytes, grid).unwrap();
    let a = rb_sheet::Sheet::load(src).render();
    let b = rb_sheet::Sheet::load(&back.source).render();
    let results = |s: &str| s.lines().filter_map(|l| l.split_once(" → ").map(|(_, r)| r.to_string())).collect::<Vec<_>>();
    assert_eq!(results(&a), results(&b), "{}", back.source);
    assert!(b.contains("[-2; 2]") && b.contains("4 * a + b"), "{b}");
}

#[test]
fn tabel_naar_mathcad() {
    let grid = Grid { col_w: 6.4, row_h: 14.0 };
    let src = "tabel x [mm], F [kN] {\n    0, 0\n    10, 2.5\n}\n\nsum(F) * x[1] = kN*m\n";
    let out = export(src, grid);
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    let back = import(&out.bytes, grid).unwrap();
    let a = rb_sheet::Sheet::load(src).render();
    let b = rb_sheet::Sheet::load(&back.source).render();
    assert!(a.contains("= 0.025 kN") && b.contains("= 0.025 kN"), "{a}\n---\n{b}");
}

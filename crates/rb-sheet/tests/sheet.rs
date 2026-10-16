use rb_sheet::{Output, Sheet, Warning};

const BALK: &str = include_str!("../../../examples/balk.rkn");
const AANDRIJVING: &str = include_str!("../../../examples/aandrijving.rkn");
const FOUTEN: &str = include_str!("../../../examples/fouten.rkn");
const FASE1: &str = include_str!("../../../examples/fase1.rkn");
const FASE2: &str = include_str!("../../../examples/fase2.rkn");
const FASE3: &str = include_str!("../../../examples/fase3.rkn");

#[test]
fn fase3_grafieken_hebben_de_juiste_waarden() {
    let sheet = Sheet::load(FASE3);
    assert_eq!(sheet.error_count(), 0, "{}", sheet.render());
    let plots: Vec<&rb_sheet::PlotData> = (0..sheet.regions().len())
        .filter_map(|i| match sheet.output(i) {
            Output::Plot(p) => Some(p),
            _ => None,
        })
        .collect();
    assert_eq!(plots.len(), 4);
    let max_y = |p: &rb_sheet::PlotData, s: usize| p.series[s].points.iter().map(|q| q[1]).fold(f64::MIN, f64::max);
    // grootste doorbuiging 5 q L⁴ / (384 E I) = 5.95238 mm, grootste moment q L² / 8 = 6 kN·m
    assert!((max_y(plots[0], 0) - 5.95238).abs() < 1e-3, "{}", max_y(plots[0], 0));
    assert_eq!((plots[0].x_label.as_str(), plots[0].y_label.as_str()), ("x [m]", "[mm]"));
    assert!((max_y(plots[1], 0) - 6.0).abs() < 1e-3);
    // trilling begint op 10 mm; 400 punten
    assert_eq!(plots[2].series[0].points[0], [0.0, 10.0]);
    assert_eq!(plots[2].series[0].points.len(), 400);
    // meetpunten plus de best passende rechte: F(0) = 4.04 − 1.98 · 2 = 0.08 N
    assert_eq!(plots[3].series.len(), 2);
    assert!((plots[3].series[1].points[0][1] - 0.08).abs() < 1e-12);
    assert_eq!(plots[3].x_label, "x.m [mm]");
}

#[test]
fn fase2_voorbeeld_geeft_de_juiste_getallen() {
    let sheet = Sheet::load(FASE2);
    let text = sheet.render();
    assert_eq!(sheet.error_count(), 0, "{text}");
    // onafhankelijk nagerekend (losse rekenregels in PowerShell, zie docs/PLAN.md §2)
    assert!(text.contains("stappen(27) = 111"), "{text}");
    assert!(text.contains("f = ⎡ 2.19947 ⎤ Hz\n    ⎣ 5.75828 ⎦"), "{text}");
    assert!(text.contains("z.1 = 4.49341"), "{text}");
    assert!(text.contains("F.k = 942.234 kN"), "{text}");
    assert!(text.contains("Find(F.1, F.2) = ⎡ 6 ⎤ kN"), "{text}");
    assert!(text.contains("slope(x.m, F.m) = 1.98 N/mm"), "{text}");
    assert!(text.contains("= 0.0227501"), "{text}");
    // u(t) = e^(−ζω₀t)(A cos ω_d t + B sin ω_d t), ω_d = ω₀√(1−ζ²)
    assert!(text.contains("u(1 s) = -3.36852 mm"), "{text}");
}

fn index_of(sheet: &Sheet, source_start: &str) -> usize {
    sheet
        .regions()
        .iter()
        .position(|r| r.source.starts_with(source_start))
        .unwrap_or_else(|| panic!("regel '{source_start}' niet gevonden"))
}

#[test]
fn balk_voorbeeld_geeft_de_juiste_getallen() {
    let sheet = Sheet::load(BALK);
    assert_eq!(sheet.error_count(), 0, "{}", sheet.render());
    let text = sheet.render();
    // handberekening: M = q L²/8 = 3000·4/8 = 1500 N·m; W = b h²/6 = 1.2e-4 m³; σ = M/W = 12.5 MPa
    assert!(text.contains("M.max = 1.5 kN*m"), "{text}");
    assert!(text.contains("W = 120 cm^3"), "{text}");
    assert!(text.contains("sigma = 12.5 MPa"), "{text}");
    // f = 5 q L⁴ / (384 E I), I = 7.2e-6 m⁴ → 0.41336 mm
    assert!(text.contains("f.max = 0.41336 mm"), "{text}");
    // sweep: σ(h) = 6 M / (b h²)
    assert!(text.contains("hh = 80 mm   →  6 * M.max / (b * hh^2) = 28.125 MPa"), "{text}");
    assert!(text.contains("= 7.03125 MPa"), "{text}");
}

#[test]
fn aandrijving_voorbeeld_geeft_de_juiste_getallen() {
    let sheet = Sheet::load(AANDRIJVING);
    assert_eq!(sheet.error_count(), 0, "{}", sheet.render());
    let text = sheet.render();
    // ω = 1450·2π/60 = 151.844 rad/s; M = 15000/ω = 98.7858 N·m
    assert!(text.contains("M = 98.7858 N*m"), "{text}");
    assert!(text.contains("v = 15.1844 m/s"), "{text}");
    assert!(text.contains("F.t = 0.987858 kN"), "{text}");
    assert!(text.contains("koppel(7.5 kW, 960 rpm) = 74.6039 N*m"), "{text}");
}

#[test]
fn fouten_voorbeeld_meldt_de_juiste_fouten() {
    let sheet = Sheet::load(FOUTEN);
    let text = sheet.render();
    assert!(text.contains("eenheden komen niet overeen bij optellen: m en s"), "{text}");
    assert!(text.contains("c = 6 km/hr"), "{text}");
    assert!(text.contains("'x' is hier nog niet gedefinieerd (wel op regel 12"), "{text}");
    assert!(text.contains("deling door nul"), "{text}");
    assert!(text.contains("dit haakje wordt nergens gesloten"), "{text}");
    assert!(text.contains("Gebruik ':='"), "{text}");
    assert!(text.contains("'s' geldt hier als eenheid"), "{text}");
}

#[test]
fn fase1_voorbeeld_geeft_de_juiste_getallen() {
    let sheet = Sheet::load(FASE1);
    let text = sheet.render();
    assert_eq!(sheet.error_count(), 0, "{text}");
    // ligger met puntlast: R.A = F (L - a) / L = 7.5 kN, M(a) = 7.5 kN·m
    assert!(text.contains("M(a) = 7.5 kN*m"), "{text}");
    assert!(text.contains("x = 2 m  →  M(x) = 5 kN*m"), "{text}");
    // oppervlak momentenlijn: driehoek ½ · 4 m · 7.5 kN·m; dwarskracht rechts van a: 7.5 - 10
    assert!(text.contains("integral(M(x), x, 0 m, L) = 15 kN*m^2"), "{text}");
    assert!(text.contains("deriv(M(x), x, 2 m) = -2.5 kN"), "{text}");
    // W = b h²/6 met b = 50 mm, h = 80 ... 200 mm
    assert!(text.contains("⎢     120 ⎥ cm^3"), "{text}");
    assert!(text.contains("max(W) = 333.333 cm^3"), "{text}");
    // veren in serie: u1 = P/k1, u2 = u1 + P/k2
    assert!(text.contains("u = ⎡  7.5 ⎤ mm"), "{text}");
    assert!(text.contains("⎣ 22.5 ⎦"), "{text}");
    assert!(text.contains("det(K) = 20000 (N/mm)^2"), "{text}");
    // ΔL = α L ΔT = 12e-6 · 2 m · 60 K
    assert!(text.contains("ΔL = 1.44 mm"), "{text}");
    assert!(text.contains("T.2 = 80 °C"), "{text}");
    assert!(text.contains("T.2 = 176 °F"), "{text}");
    // RLC: Z = R + j(ωL - 1/(ωC)) = 10 - 16.1230j Ω, |Z| = 18.9724 Ω, arg = -58.1915°
    assert!(text.contains("Z = (10 - 16.123i) ohm"), "{text}");
    assert!(text.contains("abs(Z) = 18.9724 ohm"), "{text}");
    assert!(text.contains("arg(Z) = -58.1915 deg"), "{text}");
    assert!(text.contains("L > 3 m and F < 20 kN = 1"), "{text}");
    assert!(text.contains("0 < ΔL < 1 mm = 0"), "{text}");
    assert!(text.contains("E.tot = 8 kJ"), "{text}");
    assert!(text.contains("n = 1450 rpm"), "{text}");
}

#[test]
fn elementdefinities_hangen_af_van_de_vorige_definitie() {
    let src = "i := 0..2\nv[i] := i^2\nv[1] := 10\nv =\n";
    let mut sheet = Sheet::load(src);
    // regel 3 vult de vector van regel 2 aan: geen 'opnieuw gedefinieerd'-waarschuwing, wel een afhankelijkheid
    assert!(sheet.graph().deps[2].contains(&1));
    assert!(sheet.graph().warnings[2].is_empty(), "{:?}", sheet.graph().warnings[2]);
    assert_eq!(sheet.output(3), &Output::Value("⎡  0 ⎤\n⎢ 10 ⎥\n⎣  4 ⎦".into()));
    // het bereik wijzigen werkt door tot in het resultaat
    let redone = sheet.update_region(0, "i := 0..3");
    assert_eq!(redone, vec![0, 1, 2, 3]);
    assert_eq!(sheet.output(3), &Output::Value("⎡  0 ⎤\n⎢ 10 ⎥\n⎢  4 ⎥\n⎣  9 ⎦".into()));
}

#[test]
fn posities_bepalen_de_volgorde_ook_na_een_wijziging() {
    // c gebruikt b, dat in het bestand eronder staat maar op het blad op rij 1 rechts van a
    let mut sheet = Sheet::load("a := 1\nc := a + b\n@30,1 b := 2\nc =\n");
    assert_eq!(sheet.error_count(), 0, "{}", sheet.render());
    assert_eq!(sheet.output(3), &Output::Value("3".into()));
    // b naar beneden verplaatsen: nu staat b onder c, en c klopt niet meer
    let idx = sheet.regions().iter().position(|r| r.source == "b := 2").unwrap();
    sheet.update_region(idx, "@30,9 b := 2");
    let fresh = Sheet::load("a := 1\nc := a + b\n@30,9 b := 2\nc =\n");
    assert_eq!(sheet.render(), fresh.render());
    assert!(sheet.render().contains("'b' is hier nog niet gedefinieerd"), "{}", sheet.render());
}

#[test]
fn bereik_in_een_resultaat_geeft_een_tabel_met_fouten_per_rij() {
    let sheet = Sheet::load("x := -1..1\nsqrt(x) =\n1 / x =\n");
    match sheet.output(1) {
        Output::Table(rows) => {
            assert!(rows[0].ends_with("sqrt(x) = 1i"), "{rows:?}");
            assert!(rows[2].ends_with("sqrt(x) = 1"), "{rows:?}");
        }
        o => panic!("{o:?}"),
    }
    match sheet.output(2) {
        Output::Table(rows) => assert!(rows[1].contains("✗ deling door nul"), "{rows:?}"),
        o => panic!("{o:?}"),
    }
}

#[test]
fn programmablok_is_een_regio_over_meerdere_regels() {
    let src = "n := 5\n\nsom(k) := {\n    s := 0\n    for i in 1..k   // optellen\n        s := s + i\n    s\n}\nsom(n) =\n";
    let mut sheet = Sheet::load(src);
    assert_eq!(sheet.regions().len(), 3, "{:?}", sheet.regions());
    assert_eq!(sheet.regions()[1].line, 3);
    assert_eq!(sheet.output(2), &Output::Value("15".into()));
    let text = sheet.render();
    assert!(text.contains("som(k) := {\n    s := 0\n"), "{text}");
    assert!(text.contains("}\nsom(n) = 15"), "{text}");
    // n wijzigen herberekent alleen de resultaatregel, niet de functie
    assert_eq!(sheet.update_region(0, "n := 10"), vec![0, 2]);
    assert_eq!(sheet.output(2), &Output::Value("55".into()));
    // een blok zonder afsluiting is een fout zonder caret
    let bad = Sheet::load("f(x) := {\n  x\n");
    assert!(bad.render().contains("niet afgesloten"), "{}", bad.render());
}

fn value_of(sheet: &Sheet, source: &str) -> String {
    let i = index_of(sheet, source);
    match sheet.output(i) {
        Output::Value(v) => v.clone(),
        o => panic!("regel '{source}': {o:?}\n{}", sheet.render()),
    }
}

#[test]
fn given_find_lost_een_stelsel_op() {
    let src = "x := 1\ny := 1\nGiven\nx + y == 10\nx - y == 2\ns := Find(x, y)\ns =\n";
    let mut sheet = Sheet::load(src);
    assert_eq!(sheet.error_count(), 0, "{}", sheet.render());
    assert_eq!(value_of(&sheet, "s ="), "⎡ 6 ⎤\n⎣ 4 ⎦");
    // een beperking wijzigen rekent de oplossing opnieuw uit
    let idx = index_of(&sheet, "x - y == 2");
    sheet.update_region(idx, "x - y == 4");
    assert_eq!(value_of(&sheet, "s ="), "⎡ 7 ⎤\n⎣ 3 ⎦");
    // met eenheden: veer met stijfheid k onder kracht F
    let sheet = Sheet::load("F := 100 N\nk := 2 N/mm\nu := 1 mm\nGiven\nk * u == F\nFind(u) = mm\n");
    assert_eq!(value_of(&sheet, "Find(u)"), "50 mm");
    // zonder oplossing een duidelijke fout
    let sheet = Sheet::load("x := 1\nGiven\nx^2 == -1\nFind(x) =\n");
    match sheet.output(3) {
        Output::Error { message, .. } => assert!(message.contains("geen oplossing"), "{message}"),
        o => panic!("{o:?}"),
    }
    // Minerr geeft de beste benadering (x² + 1 is het kleinst in x = 0)
    let sheet = Sheet::load("x := 1\nGiven\nx^2 == -1\nabs(Minerr(x)) < 1e-6 =\n");
    assert_eq!(value_of(&sheet, "abs(Minerr(x))"), "1");
}

#[test]
fn minimize_met_en_zonder_beperkingen() {
    let sheet = Sheet::load("f(x, y) := (x - 3)^2 + (y + 1)^2 + 2\nx := 0\ny := 0\nMinimize(f, x, y) =\n");
    assert_eq!(value_of(&sheet, "Minimize(f, x, y)"), "⎡  3 ⎤\n⎣ -1 ⎦");
    let sheet = Sheet::load("g(x, y) := x^2 + y^2\nx := 1\ny := 0\nGiven\nx + y == 1\nMinimize(g, x, y) =\n");
    assert_eq!(value_of(&sheet, "Minimize(g, x, y)"), "⎡ 0.5 ⎤\n⎣ 0.5 ⎦");
    // maximale oppervlakte van een rechthoek met omtrek 4 m
    let sheet = Sheet::load("A(a, b) := a * b\na := 0.5 m\nb := 0.5 m\nGiven\n2 a + 2 b == 4 m\nMaximize(A, a, b) = m\n");
    assert_eq!(value_of(&sheet, "Maximize(A, a, b)"), "⎡ 1 ⎤ m\n⎣ 1 ⎦");
}

#[test]
fn odesolve_met_eenheden() {
    let src = "Given\ny''(t) + y(t) == 0\ny(0) == 1\ny'(0) == 0\ny := odesolve(t, 10)\ny(π) =\ny(π/2) < 1e-8 =\n";
    let sheet = Sheet::load(src);
    assert_eq!(sheet.error_count(), 0, "{}", sheet.render());
    assert_eq!(value_of(&sheet, "y(π) ="), "-1");
    assert_eq!(value_of(&sheet, "y(π/2) <"), "1");
    // trilling met eenheden: x'' + ω² x = 0, x(0) = 5 mm → x(π/ω) = −5 mm
    let src = "ω := 2 rad/s\nGiven\nx''(t) + ω^2 x(t) == 0\nx(0 s) == 5 mm\nx'(0 s) == 0 m/s\nx := odesolve(t, 5 s)\nx(π/2 * s) =\n";
    let sheet = Sheet::load(src);
    assert_eq!(sheet.error_count(), 0, "{}", sheet.render());
    assert_eq!(value_of(&sheet, "x(π/2 * s)"), "-5 mm");
    // eerste orde: y' = -2 y → e^-2t
    let sheet = Sheet::load("Given\nv'(t) == -2 v(t)\nv(0) == 1\nv := odesolve(t, 3)\nv(1) =\n");
    assert_eq!(value_of(&sheet, "v(1)"), "0.135335");
}

#[test]
fn odesolve_stelsel() {
    // x' = y, y' = -x met x(0) = 1, y(0) = 0: x = cos t, y = -sin t
    let src = "Given\nx'(t) == y(t)\ny'(t) == -x(t)\nx(0) == 1\ny(0) == 0\n[x; y] := odesolve([x; y], t, 4)\nx(π) =\ny(π/2) =\n";
    let sheet = Sheet::load(src);
    assert_eq!(sheet.error_count(), 0, "{}", sheet.render());
    assert_eq!(value_of(&sheet, "x(π) ="), "-1");
    assert_eq!(value_of(&sheet, "y(π/2) ="), "-1");
    // gemengde orde met eenheden: een massa-veer, aangedreven door een kracht die zelf uitdooft
    let src = "k := 4 N/m\nmassa := 1 kg\nGiven\nmassa * u''(t) == -k * u(t) + F(t)\nF'(t) == -F(t) / (1 s)\nu(0 s) == 0 m\nu'(0 s) == 0 m/s\nF(0 s) == 2 N\n[u; F] := odesolve([u; F], t, 2 s)\nF(1 s) =\n";
    let sheet = Sheet::load(src);
    assert_eq!(sheet.error_count(), 0, "{}", sheet.render());
    assert_eq!(value_of(&sheet, "F(1 s)"), "735.759 mN");
    // een vector links mag alleen bij odesolve
    assert!(Sheet::load("[a; b] := [1; 2]\n").error_count() > 0);
}

#[test]
fn odesolve_randwaardeprobleem() {
    // ligger op twee steunpunten met gelijkmatige belasting: w'''' = q/EI,
    // w = w'' = 0 aan beide einden; doorbuiging in het midden 5 q L^4 / (384 EI)
    let src = "q := 10 kN/m\nEI := 20000 kN*m^2\nL := 6 m\nGiven\nw''''(x) == q / EI\nw(0 m) == 0 m\nw''(0 m) == 0\nw(L) == 0 m\nw''(L) == 0\nw := odesolve(x, L)\nw(L/2) = mm\n";
    let sheet = Sheet::load(src);
    assert_eq!(sheet.error_count(), 0, "{}", sheet.render());
    assert_eq!(value_of(&sheet, "w(L/2)"), "8.4375 mm");
    // y'' = -y met y(0) = 0 en y(π/2) = 1: y = sin x
    let sheet = Sheet::load("Given\ny''(x) + y(x) == 0\ny(0) == 0\ny(π/2) == 1\ny := odesolve(x, 3)\ny(π/6) =\n");
    assert_eq!(sheet.error_count(), 0, "{}", sheet.render());
    assert_eq!(value_of(&sheet, "y(π/6)"), "0.5");
    // een voorwaarde buiten het bereik, en een ontbrekende voorwaarde
    let sheet = Sheet::load("Given\ny''(x) + y(x) == 0\ny(0) == 0\ny(5) == 1\ny := odesolve(x, 3)\n");
    assert!(sheet.render().contains("buiten het bereik"), "{}", sheet.render());
    let sheet = Sheet::load("Given\ny''(x) + y(x) == 0\ny(0) == 0\ny := odesolve(x, 3)\n");
    assert!(sheet.render().contains("y'(0) ontbreekt"), "{}", sheet.render());
}

#[test]
fn diepe_recursie_in_een_blad() {
    let sheet = Sheet::load("som(n) := if(n == 0, 0, n + som(n - 1))\nsom(150) =\n");
    assert_eq!(sheet.output(1), &Output::Value("11325".into()));
}

const KLEIN: &str ="a := 2 m\nb := 3 m\nc := a * 2\nd := b * 2\nc =\nd =\n";

#[test]
fn herberekening_raakt_alleen_afhankelijke_regels() {
    let mut sheet = Sheet::load(KLEIN);
    assert_eq!(sheet.output(4), &Output::Value("4 m".into()));
    assert_eq!(sheet.output(5), &Output::Value("6 m".into()));

    // a wijzigt: a (0), c (2) en `c =` (4) worden herberekend, de rest niet
    let redone = sheet.update_region(0, "a := 5 m");
    assert_eq!(redone, vec![0, 2, 4]);
    assert_eq!(sheet.output(4), &Output::Value("10 m".into()));
    assert_eq!(sheet.output(5), &Output::Value("6 m".into()));

    // b wijzigt: b (1), d (3) en `d =` (5)
    let redone = sheet.update_region(1, "b := 1 km");
    assert_eq!(redone, vec![1, 3, 5]);
    assert_eq!(sheet.output(5), &Output::Value("2 km".into()));
}

#[test]
fn herberekening_geeft_hetzelfde_als_opnieuw_laden() {
    let mut sheet = Sheet::load(KLEIN);
    sheet.update_region(0, "a := 7 m");
    sheet.update_region(3, "d := b * b");
    let fresh = Sheet::load("a := 7 m\nb := 3 m\nc := a * 2\nd := b * b\nc =\nd =\n");
    assert_eq!(sheet.render(), fresh.render());
}

#[test]
fn hernoemen_van_een_definitie_breekt_de_gebruikers() {
    let mut sheet = Sheet::load(KLEIN);
    sheet.update_region(2, "cc := a * 2");
    match sheet.output(4) {
        Output::Error { message, .. } => assert!(message.contains("onbekende naam 'c'"), "{message}"),
        o => panic!("verwacht fout, kreeg {o:?}"),
    }
    // en terugzetten herstelt het
    sheet.update_region(2, "c := a * 2");
    assert_eq!(sheet.output(4), &Output::Value("4 m".into()));
}

#[test]
fn definitie_onder_het_gebruik_geeft_een_fout_die_verdwijnt_na_verplaatsen() {
    let mut sheet = Sheet::load("y := x + 1\nx := 2\ny =\n");
    match sheet.output(0) {
        Output::Error { message, .. } => assert!(message.contains("nog niet gedefinieerd"), "{message}"),
        o => panic!("{o:?}"),
    }
    // regel 1 (x := 2) wordt een andere definitie: de 'later gedefinieerd'-fout verdwijnt
    // maar y blijft stuk (x bestaat nergens meer)
    sheet.update_region(1, "z := 2");
    match sheet.output(0) {
        Output::Error { message, .. } => assert!(message.contains("onbekende naam 'x'"), "{message}"),
        o => panic!("{o:?}"),
    }
}

#[test]
fn fout_in_een_definitie_werkt_door_naar_gebruikers_en_herstelt() {
    let mut sheet = Sheet::load("a := 1 / 0\nb := a + 1\nb =\n");
    assert_eq!(sheet.error_count(), 3);
    sheet.update_region(0, "a := 4");
    assert_eq!(sheet.error_count(), 0);
    assert_eq!(sheet.output(2), &Output::Value("5".into()));
}

#[test]
fn opnieuw_definieren_gebruikt_de_vorige_waarde() {
    let sheet = Sheet::load("a := 1 m\na := a + 1 m\na =\n");
    assert_eq!(sheet.output(2), &Output::Value("2 m".into()));
    assert!(sheet
        .graph()
        .warnings[1]
        .iter()
        .any(|w| matches!(w, Warning::Redefined { name, previous_line: 1 } if name == "a")));
}

#[test]
fn recursieve_functie_krijgt_geen_zelf_afhankelijkheid() {
    let sheet = Sheet::load("f(x) := f(x)\nf(1) =\n");
    assert!(sheet.graph().deps[0].is_empty());
    match sheet.output(1) {
        Output::Error { message, .. } => assert!(message.contains("te diep"), "{message}"),
        o => panic!("{o:?}"),
    }
}

#[test]
fn sweep_toont_fouten_per_rij() {
    let sheet = Sheet::load("sweep x = 0 ; 2 ; 3 : 1 / x =\n");
    match sheet.output(0) {
        Output::Table(rows) => {
            assert_eq!(rows.len(), 3);
            assert!(rows[0].contains("✗ deling door nul"), "{rows:?}");
            assert!(rows[1].ends_with("= 1"), "{rows:?}");
            assert!(rows[2].ends_with("= 0.5"), "{rows:?}");
        }
        o => panic!("{o:?}"),
    }
}

#[test]
fn sweep_variabele_lekt_niet_naar_het_blad() {
    let sheet = Sheet::load("sweep x = 1 ; 3 ; 3 : x^2 =\nx =\n");
    match sheet.output(1) {
        Output::Error { message, .. } => assert!(message.contains("onbekende naam 'x'"), "{message}"),
        o => panic!("{o:?}"),
    }
}

#[test]
fn afhankelijkheden_uitleggen() {
    let sheet = Sheet::load(BALK);
    let text = sheet.explain("sigma").unwrap();
    assert!(text.contains("M.max := q * L^2 / 8"), "{text}");
    assert!(text.contains("W := b * h^2 / 6"), "{text}");
    // indirect: q, L, b, h
    assert!(text.contains("q := 3 kN/m"), "{text}");
    assert!(text.contains("sigma = MPa"), "{text}");
    assert!(sheet.explain("bestaat_niet").is_none());

    // 'E' wordt gebruikt door de doorbuiging, maar niet door sigma
    assert!(!text.contains("E := 210 GPa"), "{text}");
    let idx = index_of(&sheet, "E :=");
    assert!(sheet.graph().dependents_closure(idx).len() >= 2);
}

#[test]
fn overzicht_markeert_ongebruikte_definities() {
    let sheet = Sheet::load("a := 1\nb := 2\nc := a + 1\nc =\n");
    let text = sheet.overview();
    let b_line = text.lines().find(|l| l.contains(" b ")).unwrap();
    assert!(b_line.contains("nergens gebruikt"), "{text}");
    let a_line = text.lines().find(|l| l.contains(" a ")).unwrap();
    assert!(!a_line.contains("nergens gebruikt"), "{text}");
}

#[test]
fn definitie_met_direct_resultaat() {
    // zoals in Mathcad: definiëren en meteen tonen, ook in een gekozen eenheid
    let sheet = Sheet::load("q := 3 kN/m\nL := 4 m\nM := q * L^2 / 8 = kN*m\nV := 5 V / 1024 = mV\nM * 2 =\n");
    let r = sheet.render();
    assert!(r.contains("M := q * L^2 / 8 = 6 kN*m"), "{r}");
    assert!(r.contains("V := 5 V / 1024 = 4.88281 mV"), "{r}");
    assert!(r.contains("M * 2 = 12 kN·m"), "{r}");
    assert_eq!(sheet.error_count(), 0, "{r}");
}

#[test]
fn tabel_definieert_kolommen() {
    let src = "tabel x [mm], F [kN] {\n    0, 0\n    10, 2.5\n    20, 4\n}\n\nF[1] =\n\nsum(F) * x[2] = kN*m\n";
    let sheet = Sheet::load(src);
    assert_eq!(sheet.regions().len(), 3, "{:?}", sheet.regions());
    assert_eq!(sheet.output(1), &Output::Value("2.5 kN".into()));
    assert_eq!(sheet.output(2), &Output::Value("0.13 kN*m".into()));
    // met een positie ervoor, en de tekstweergave laat de tabel zien
    let sheet = Sheet::load(&format!("@0,1 {src}"));
    assert!(sheet.render().contains("tabel x [mm], F [kN] {"), "{}", sheet.render());
    assert_eq!(sheet.output(1), &Output::Value("2.5 kN".into()));
}

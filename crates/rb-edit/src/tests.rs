//! Typen zoals in Mathcad Prime, en de omzetting naar tekst.

use super::*;
use crate::tree::Template;
use rb_core::parse_stmt;

fn typed(steps: &[&str]) -> String {
    let mut ed = Editor::new();
    for s in steps {
        match *s {
            "→" => {
                ed.apply(Cmd::Right);
            }
            "←" => {
                ed.apply(Cmd::Left);
            }
            "↓" => {
                ed.apply(Cmd::Down);
            }
            "↑" => {
                ed.apply(Cmd::Up);
            }
            "⌫" => {
                ed.apply(Cmd::Backspace);
            }
            "⏎" => {
                ed.apply(Cmd::Enter);
            }
            "^G" => {
                ed.apply(Cmd::Greek);
            }
            "^M" => {
                ed.apply(Cmd::Template(Template::Matrix));
            }
            "^Σ" => {
                ed.apply(Cmd::Template(Template::Sum));
            }
            "^∫" => {
                ed.apply(Cmd::Template(Template::Integral));
            }
            "^d" => {
                ed.apply(Cmd::Template(Template::Deriv));
            }
            "^while" => {
                ed.apply(Cmd::Template(Template::While));
            }
            "^for" => {
                ed.apply(Cmd::Template(Template::For));
            }
            "^else" => {
                ed.apply(Cmd::Template(Template::Else));
            }
            "^return" => {
                ed.apply(Cmd::Template(Template::Return));
            }
            // Ctrl+. (symbolisch)
            "^." => {
                ed.apply(Cmd::Char('→'));
            }
            text => {
                type_str(&mut ed, text);
            }
        }
    }
    let t = region_text(&ed.root);
    // wat de editor maakt, moet altijd te lezen zijn
    parse_stmt(&t, false).unwrap_or_else(|e| panic!("'{t}' is niet te lezen: {e}"));
    t
}

#[test]
fn definities_en_eenheden() {
    assert_eq!(typed(&["x:5kN"]), "x := 5 kN");
    assert_eq!(typed(&["L:2.5m"]), "L := 2.5 m");
    assert_eq!(typed(&["z:1i"]), "z := 1i");
    assert_eq!(typed(&["x:1e-3"]), "x := 1e-3");
    assert_eq!(typed(&["E:210GPa"]), "E := 210 GPa");
    // literal subscript met een punt, zoals in Prime
    assert_eq!(typed(&["M.max:12"]), "M.max := 12");
    // Ctrl+G: Griekse letter
    assert_eq!(typed(&["s", "^G", ":3MPa"]), "σ := 3 MPa");
}

#[test]
fn breuken_nemen_het_product_ervoor() {
    // q·L²/8: na de macht eerst → (uit de exponent), dan /
    assert_eq!(typed(&["M:q*L^2", "→", "/8"]), "M := (q * L^2)/8");
    assert_eq!(typed(&["q:3kN/m"]), "q := (3 kN)/m");
    // in de noemer blijft alles staan wat je daarna typt, zoals in Prime: a/b+c is a/(b+c)
    assert_eq!(typed(&["y:a/b+c"]), "y := a/(b + c)");
    // ... tenzij je eerst uit de noemer stapt
    assert_eq!(typed(&["y:a/b", "→", "+c"]), "y := a/b + c");
    // zonder iets ervoor: lege teller
    assert_eq!(typed(&["y:/", "1", "↓", "2"]), "y := 1/2");
}

#[test]
fn spatiebalk_vergroot_de_selectie() {
    // a+b, spatie (b), spatie (a+b), dan / : (a+b)/2
    assert_eq!(typed(&["y:a+b", " ", " ", "/2"]), "y := (a + b)/2");
    // macht op een selectie krijgt haakjes
    assert_eq!(typed(&["y:a+b", " ", " ", "^2"]), "y := (a + b)^2");
}

#[test]
fn wortels_haakjes_en_functies() {
    assert_eq!(typed(&["y:\\2"]), "y := sqrt(2)");
    assert_eq!(typed(&["f(x:x^2"]), "f(x) := x^2");
    assert_eq!(typed(&["y:sin(30deg"]), "y := sin(30 deg)");
    assert_eq!(typed(&["y:2(a+b)", "+1"]), "y := 2(a + b) + 1");
    assert_eq!(typed(&["y:|x-1"]), "y := abs(x - 1)");
    assert_eq!(typed(&["y:5!"]), "y := 5!");
}

#[test]
fn resultaatregels() {
    let mut ed = Editor::new();
    assert_eq!(type_str(&mut ed, "x*2="), Effect::Commit);
    assert_eq!(region_text(&ed.root), "x * 2 =");
    // eenheid in de plaats achter de =
    type_str(&mut ed, "kN");
    assert_eq!(region_text(&ed.root), "x * 2 = kN");
    // definiëren en meteen tonen, zoals in Mathcad: `a := 2 m = cm`
    let mut ed = Editor::new();
    type_str(&mut ed, "a:2m");
    assert_eq!(ed.apply(Cmd::Char('=')), Effect::Commit);
    type_str(&mut ed, "cm");
    assert_eq!(region_text(&ed.root), "a := 2 m = cm");
}

#[test]
fn vergelijkingen() {
    assert_eq!(typed(&["b:x<=2"]), "b := x <= 2");
    assert_eq!(typed(&["b:x<>2"]), "b := x != 2");
    assert_eq!(typed(&["b:x>=2"]), "b := x >= 2");
}

#[test]
fn matrices_en_grote_operatoren() {
    assert_eq!(typed(&["A:", "^M", "1", "→", "2", "→", "3", "→", "4"]), "A := [1, 2; 3, 4]");
    assert_eq!(typed(&["s:", "^Σ", "i", "→", "1", "→", "10", "→", "i^2"]), "s := sum(i^2, i, 1, 10)");
    assert_eq!(typed(&["I:", "^∫", "0", "→", "1", "→", "x^2", "→", "→", "x"]), "I := integral(x^2, x, 0, 1)");
    // d/dx f(x): de afgeleide in de huidige waarde van x, zoals in Mathcad
    assert_eq!(typed(&["v:", "^d", "t", "→", "s(t)"]), "v := deriv(s(t), t, t)");
    assert_eq!(typed(&["y:v[2"]), "y := v[2]");
}

#[test]
fn programmablokken() {
    // ] maakt een programma, { een lokale toekenning, Enter een nieuwe regel
    assert_eq!(
        typed(&["f(n):", "]", "s{0", "⏎", "^for", "k", "→", "1..n", "→", "s{s+k", "↓", "s"]),
        "f(n) := {\n    s := 0\n    for k in 1..n\n        s := s + k\n    s\n}"
    );
    assert_eq!(
        typed(&["g(x):", "]", "}", "x<0", "→", "-x", "↓", "^else", "x"]),
        "g(x) := {\n    if x < 0\n        -x\n    else\n        x\n}"
    );
    assert_eq!(typed(&["h(x):", "]", "^return", "x"]), "h(x) := {\n    return x\n}");
}

#[test]
fn backspace_haalt_structuur_weg() {
    // Backspace in een lege noemer haalt de breuk weg; de teller blijft staan
    let mut ed = Editor::new();
    type_str(&mut ed, "y:a/");
    ed.apply(Cmd::Backspace);
    assert_eq!(region_text(&ed.root), "y := a");
    type_str(&mut ed, "b");
    assert_eq!(region_text(&ed.root), "y := ab");
    // haakjes weghalen laat de inhoud staan
    let mut ed = Editor::new();
    type_str(&mut ed, "y:(a+b");
    for _ in 0..3 {
        ed.apply(Cmd::Left);
    }
    ed.apply(Cmd::Backspace);
    assert_eq!(region_text(&ed.root), "y := a + b");
}

#[test]
fn bestaande_regels_bewerken_en_terug() {
    // tekst → bewerkbare boom → tekst geeft dezelfde regel (zelfde syntaxboom)
    let lines = [
        "x := 5 kN",
        "M.max := q * L^2 / 8",
        "σ := M.max / W",
        "f(x, y) := sqrt(x^2 + y^2) / nthroot(8, 3)",
        "v[i] := -i^2",
        "A := [1, 2; 3, 4]",
        "s := sum(1/k^2, k, 1, 10) + prod(k, k, 1, 5)",
        "I := integral(exp(-x^2), x, 0, ∞)",
        "c := 0 < x ≤ 1 and not y == 2 or abs(z) != 3",
        "r := 0, 0.5 .. 3",
        "n := 3! + 2i + 1.5e-7",
        "z.1 := root(tan(z) - z, z, 4, 4.6)",
        "q * L = kN",
        "T := 20 °C",
        "M := q * L^2 / 8 = kN*m",
        "v := 3 m/s =",
        "f(n) := {\n    s := 0\n    for k in 1..n\n        if mod(k, 2) == 0\n            continue\n        s := s + k\n    return s\n}",
    ];
    for l in lines {
        let a = parse_stmt(l, false).unwrap();
        let back = region_text(&from_stmt(&a));
        let b = parse_stmt(&back, false).unwrap_or_else(|e| panic!("{l} → {back}: {e}"));
        assert_eq!(strip(&a), strip(&b), "{l} → {back}");
    }
}

/// Vergelijk zonder de brontekst van resultaatregels (die verschilt in spaties).
fn strip(s: &rb_core::Stmt) -> rb_core::Stmt {
    match s.clone() {
        rb_core::Stmt::Eval { expr, unit, bare, .. } => rb_core::Stmt::Eval {
            expr,
            lhs_src: String::new(),
            unit: unit.map(|(u, _)| (u, String::new())),
            bare,
        },
        rb_core::Stmt::Define { name, params, index, body, show } => rb_core::Stmt::Define {
            name,
            params,
            index,
            body,
            show: show.map(|s| rb_core::Show { unit: s.unit.map(|(u, _)| (u, String::new())), head_src: String::new() }),
        },
        rb_core::Stmt::Symbolic { expr, op, .. } => rb_core::Stmt::Symbolic { expr, op, lhs_src: String::new() },
    }
}

#[test]
fn opmaak_heeft_een_cursorplek_voor_elke_positie() {
    let mut ed = Editor::new();
    type_str(&mut ed, "y:a/b");
    let l = layout(&ed, &rb_layout::FixedMetrics, 20.0, None);
    let c = l.caret(&ed).expect("cursor zichtbaar");
    // de cursor staat in de noemer: onder de breukstreep
    assert!(c.top > -5.0, "{c:?}");
    // klikken links bovenaan geeft een positie in de wortel of de teller
    let hit = l.hit(0.0, -5.0).unwrap();
    assert!(hit.path.len() <= 1);
    // placeholders hebben ook een cursorplek
    let mut ed = Editor::new();
    ed.apply(Cmd::Template(Template::Matrix));
    let l = layout(&ed, &rb_layout::FixedMetrics, 20.0, None);
    assert!(l.caret(&ed).is_some());
    assert!(l.marks.len() >= 5);
}

#[test]
fn tekst_als_waarde() {
    // " midden in een formule begint een tekst; spaties en tekens komen er letterlijk in
    assert_eq!(typed(&["s:\"Hallo wereld + 1\""]), "s := \"Hallo wereld + 1\"");
    assert_eq!(typed(&["n:strlen(\"ab\""]), "n := strlen(\"ab\")");
    // na de afsluitende " gaat het typen gewoon verder in de formule
    assert_eq!(typed(&["t:concat(\"a\",\"b\""]), "t := concat(\"a\", \"b\")");
    // heen en terug, ook met een aanhalingsteken binnenin
    for l in ["t := concat(\"a b\", \"zeg \"\"hoi\"\"\")", "x := search(\"abc\", \"c\", 0) ="] {
        let a = parse_stmt(l, false).unwrap();
        let back = region_text(&from_stmt(&a));
        assert_eq!(strip(&parse_stmt(&back, false).unwrap()), strip(&a), "{l} → {back}");
    }
}

#[test]
fn symbolisch_typen() {
    assert_eq!(typed(&["2*x+3*x", "^."]), "2 * x + 3 * x →");
    assert_eq!(typed(&["2*x+3*x", "^.", "simplify"]), "2 * x + 3 * x → simplify");
    assert_eq!(typed(&["x*x-4", "^.", "solve,x"]), "x * x - 4 → solve, x");
    // `=` achter de pijl is een vergelijking, geen uitrekenen
    assert_eq!(typed(&["a*x+b", "^.", "substitute,x=2"]), "a * x + b → substitute, x == 2");
    for src in ["2 * x + 3 * x → simplify", "x^2 - 4 → solve, x", "a * x + b → substitute, x == 2", "deriv(x^3, x, x) →"] {
        let back = crate::convert::block_text(&from_text(src).unwrap());
        assert_eq!(strip(&parse_stmt(&back, false).unwrap()), strip(&parse_stmt(src, false).unwrap()), "{src} → {back}");
    }
}

#[test]
fn spatie_selecteert_tot_de_hele_formule() {
    let mut ed = Editor::new();
    for c in "L:2+3*x".chars() {
        ed.apply(Cmd::Char(c));
    }
    let len = ed.root.len();
    let mut steps = Vec::new();
    for _ in 0..6 {
        ed.apply(Cmd::Grow);
        steps.push(ed.selection);
    }
    // eerst de operand, dan de kant rechts van :=, dan de hele formule (met L :=), dan opnieuw
    assert_eq!((steps[0], steps[1]), (Some((len - 1, len)), Some((len - 3, len))), "{steps:?}");
    let rhs = steps.iter().position(|s| *s == Some((2, len))).expect("rechterkant");
    assert_eq!(steps[rhs + 1], Some((0, len)), "hele formule na de rechterkant: {steps:?}");
    assert_eq!(steps[rhs + 2], None, "daarna opnieuw beginnen: {steps:?}");

    // vanuit een macht: eerst het blok, dan het atoom, dan verder naar buiten tot alles
    let mut ed = Editor::new();
    for c in "a+b^2".chars() {
        ed.apply(Cmd::Char(c));
    }
    let mut last = None;
    for _ in 0..8 {
        ed.apply(Cmd::Grow);
        if ed.cursor.path.is_empty() && ed.selection == Some((0, ed.root.len())) {
            last = ed.selection;
            break;
        }
    }
    assert_eq!(last, Some((0, ed.root.len())), "de hele formule wordt bereikt");
}

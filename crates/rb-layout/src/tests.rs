use super::*;
use rb_core::{parse_expr_str, parse_stmt, Env, Outcome};

fn lay(src: &str) -> MBox {
    let stmt = parse_stmt(src, true).unwrap();
    stmt_box(&FixedMetrics, &stmt, None, LayoutOptions { size: 20.0 })
}

fn texts(b: &MBox) -> Vec<String> {
    b.items
        .iter()
        .filter_map(|i| match i {
            Item::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn breuk_is_hoger_dan_een_regel_tekst() {
    let flat = lay("a + b =");
    let stacked = lay("a / b =");
    assert!(stacked.height() > 1.8 * flat.height(), "{} vs {}", stacked.height(), flat.height());
    // de breukstreep is een lijn
    assert!(stacked.items.iter().any(|i| matches!(i, Item::Line { .. })));
}

#[test]
fn machten_en_indices_zijn_kleiner_en_verschoven() {
    let b = lay("x^2 + M.max =");
    let sizes: Vec<(String, f32, f32)> = b
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Text { text, size, y, .. } => Some((text.clone(), *size, *y)),
            _ => None,
        })
        .collect();
    let two = sizes.iter().find(|s| s.0 == "2").unwrap();
    let max = sizes.iter().find(|s| s.0 == "max").unwrap();
    assert!(two.1 < 20.0 && two.2 < 0.0, "exponent kleiner en hoger: {two:?}");
    assert!(max.1 < 20.0 && max.2 > 0.0, "index kleiner en lager: {max:?}");
}

#[test]
fn haakjes_alleen_waar_nodig() {
    // (a + b)·c heeft haakjes (als lijnen), a + b·c niet
    let with = lay("(a + b) * c =");
    let without = lay("a + b * c =");
    let polys = |b: &MBox| b.items.iter().filter(|i| matches!(i, Item::Poly { .. })).count();
    assert_eq!(polys(&with), 2);
    assert_eq!(polys(&without), 0);
    // getal voor een naam: geen vermenigvuldigingsteken
    assert!(!texts(&lay("5 kN =")).contains(&"·".to_string()));
    assert!(texts(&lay("a * b =")).contains(&"·".to_string()));
}

#[test]
fn alle_soorten_uitdrukkingen_hebben_een_vak() {
    for src in [
        "f(x, y) := sqrt(x^2 + y^2) / nthroot(8, 3)",
        "v[i] := -i^2",
        "M := [1, 2; 3, 4]",
        "s := sum(1/k^2, k, 1, 10) + prod(k, k, 1, 5)",
        "I := integral(exp(-x^2), x, 0, ∞)",
        "d := deriv(x^3, x, 2, 2)",
        "z := root(cos(x) - x, x, 0, 1)",
        "c := 0 < x ≤ 1 and not y == 2 or abs(z) != 3",
        "r := 0, 0.5 .. 3",
        "n := 3! + 2i + 1.5e-7",
        "f(n) := {\n  s := 0\n  for k in 1..n\n    if k > 2\n      break\n    else\n      s := s + k\n  while s > 100\n    s := s - 1\n  return s\n}",
    ] {
        let b = lay(src);
        assert!(b.width > 0.0 && b.height() > 0.0, "{src}");
        for i in &b.items {
            if let Item::Text { x, y, .. } = i {
                assert!(x.is_finite() && y.is_finite(), "{src}");
            }
        }
    }
}

#[test]
fn resultaten_met_eenheid_en_matrix() {
    let mut env = Env::new();
    let opts = LayoutOptions { size: 20.0 };
    for (src, want) in [("9.81 m/s^2 =", vec!["9.81", "m", "/", "s", "2"]), ("[1, 2; 3, 4] kN =", vec!["1", "4", "kN"])] {
        let stmt = parse_stmt(src, false).unwrap();
        let (value, display) = match env.eval_stmt(&stmt).unwrap() {
            Outcome::Value { value, display, .. } => (value, display),
            o => panic!("{o:?}"),
        };
        let r = value_box(&FixedMetrics, &value, &display, opts);
        let t = texts(&r);
        for w in want {
            assert!(t.contains(&w.to_string()), "{src}: {t:?}");
        }
        let full = stmt_box(&FixedMetrics, &stmt, Some(r), opts);
        assert!(full.width > 0.0);
    }
    env.run_line("x := 2").unwrap();
    let e = parse_expr_str("x^2 == 4").unwrap();
    assert!(expr::expr_box(&FixedMetrics, &e, opts).width > 0.0);
}

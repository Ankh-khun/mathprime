//! De testbladen in `tests/corpus` (ook gebruikt door `tools/mathcad-vergelijk.ps1`): elk blad rekent
//! zonder fouten, en een regel met `// verwacht: …` moet precies die uitkomst geven. De verwachte
//! waarden zijn met de hand nagerekend (of met Mathcad, waar het vergelijkscript ze ophaalde).

use rb_sheet::{Output, Sheet};

#[test]
fn testbladen_rekenen_zoals_verwacht() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus");
    let mut files: Vec<_> = std::fs::read_dir(&dir).expect("tests/corpus").flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "rkn")).collect();
    files.sort();
    assert!(files.len() >= 12, "te weinig testbladen in {}", dir.display());
    let mut checked = 0;
    for f in &files {
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        let sheet = Sheet::load(&std::fs::read_to_string(f).unwrap());
        assert_eq!(sheet.error_count(), 0, "{name}:\n{}", sheet.render());
        for (i, r) in sheet.regions().iter().enumerate() {
            let Some((_, want)) = r.source.split_once("// verwacht:") else { continue };
            let want = want.trim();
            let got = match sheet.output(i) {
                Output::Value(v) => v.trim().to_string(),
                Output::Symbolic(e) => rb_core::symbolic::print(e),
                other => panic!("{name} regel {}: geen uitkomst ({other:?})", r.line),
            };
            assert_eq!(got, want, "{name} regel {}: {}", r.line, r.source);
            checked += 1;
        }
    }
    assert!(checked >= 15, "maar {checked} verwachte uitkomsten gecontroleerd");
}

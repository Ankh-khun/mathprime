//! Bestandsfuncties van het werkblad: CSV, tekstbestanden (PRN) en Excel (.xlsx) lezen en schrijven,
//! met de namen van Mathcad Prime. Paden zijn relatief aan de map van het blad.
//!
//! ```text
//! data := READCSV("metingen.csv")              matrix met de getallen (kopregels worden overgeslagen)
//! M := READEXCEL("proef.xlsx", "Blad1!A2:C20") een bereik uit een werkblad
//! WRITECSV("uit.csv", M)                         schrijft de matrix (in SI) en geeft hem terug
//! WRITEEXCEL("uit.xlsx", M)
//! READFILE / WRITEFILE                           kiezen het formaat aan de extensie
//! ```

use rb_core::{Error, Matrix, Value};
use rb_units::Quantity;
use rb_zipxml::{xml, zip};
use std::path::{Path, PathBuf};

/// Bestandsfuncties met paden relatief aan `base` (de map van het blad).
pub struct FileIo {
    pub base: PathBuf,
}

fn err(name: &str, msg: impl Into<String>) -> Error {
    Error::Domain(format!("{name}: {}", msg.into()))
}

#[derive(Clone, Copy, PartialEq)]
enum Format {
    Csv,
    Prn,
    Excel,
}

fn format_of(path: &Path) -> Format {
    match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("xlsx" | "xlsm") => Format::Excel,
        Some("csv") => Format::Csv,
        _ => Format::Prn,
    }
}

impl rb_core::FileIo for FileIo {
    fn call(&self, name: &str, args: &[Value]) -> Option<Result<Value, Error>> {
        let (write, fixed) = match name {
            "READCSV" => (false, Some(Format::Csv)),
            "READPRN" => (false, Some(Format::Prn)),
            "READEXCEL" => (false, Some(Format::Excel)),
            "READFILE" => (false, None),
            "WRITECSV" => (true, Some(Format::Csv)),
            "WRITEPRN" => (true, Some(Format::Prn)),
            "WRITEEXCEL" => (true, Some(Format::Excel)),
            "WRITEFILE" => (true, None),
            _ => return None,
        };
        Some(self.run(name, args, write, fixed))
    }
}

impl FileIo {
    fn path(&self, name: &str, v: Option<&Value>) -> Result<PathBuf, Error> {
        match v {
            Some(Value::Str(s)) => {
                let p = Path::new(s);
                Ok(if p.is_absolute() { p.to_path_buf() } else { self.base.join(p) })
            }
            _ => Err(err(name, "het eerste argument is de bestandsnaam, als tekst: \"metingen.csv\"")),
        }
    }

    fn run(&self, name: &str, args: &[Value], write: bool, fixed: Option<Format>) -> Result<Value, Error> {
        let path = self.path(name, args.first())?;
        let format = fixed.unwrap_or_else(|| format_of(&path));
        let extra = args.get(1..).unwrap_or(&[]);
        if write {
            let data = match extra.iter().find(|v| matches!(v, Value::Matrix(_) | Value::Num(_))) {
                Some(Value::Matrix(m)) => m.clone(),
                Some(Value::Num(q)) => Matrix::new(1, 1, vec![*q]),
                _ => return Err(err(name, "geef de gegevens mee: WRITECSV(\"uit.csv\", M)")),
            };
            if data.is_complex() {
                return Err(err(name, "complexe getallen kunnen niet in een bestand"));
            }
            // een optionele beginsel voor Excel (bijv. "Blad1!B2")
            let start = extra.iter().find_map(|v| if let Value::Str(s) = v { Some(s.as_str()) } else { None });
            let bytes = match format {
                Format::Csv => table_text(&data, ',').into_bytes(),
                Format::Prn => table_text(&data, '\t').into_bytes(),
                Format::Excel => write_xlsx(&data, start).map_err(|e| err(name, e))?,
            };
            std::fs::write(&path, bytes).map_err(|e| err(name, format!("kan {} niet schrijven: {e}", path.display())))?;
            return Ok(Value::Matrix(data));
        }
        let rows = match format {
            Format::Csv | Format::Prn => {
                let text = std::fs::read_to_string(&path).map_err(|e| err(name, format!("kan {} niet lezen: {e}", path.display())))?;
                read_table(&text, format == Format::Prn)
            }
            Format::Excel => {
                let bytes = std::fs::read(&path).map_err(|e| err(name, format!("kan {} niet lezen: {e}", path.display())))?;
                let range = extra.iter().find_map(|v| if let Value::Str(s) = v { Some(s.as_str()) } else { None });
                read_xlsx(&bytes, range).map_err(|e| err(name, e))?
            }
        };
        to_matrix(name, rows)
    }
}

/// Een cel: een getal, tekst of leeg.
#[derive(Debug, Clone, PartialEq)]
enum Cell {
    Num(f64),
    Text(String),
    Empty,
}

fn parse_num(s: &str, decimal_comma: bool) -> Option<f64> {
    let s = s.trim().trim_matches('"');
    let s = if decimal_comma { s.replace(',', ".") } else { s.to_string() };
    s.parse().ok()
}

/// Lees een CSV- of tekstbestand. Een `;` als scheidingsteken (zoals Excel met Nederlandse
/// instellingen) betekent ook: een komma als decimaalteken.
fn read_table(text: &str, whitespace: bool) -> Vec<Vec<Cell>> {
    let first = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let sep = if whitespace {
        None
    } else if first.contains(';') {
        Some(';')
    } else if first.contains('\t') {
        Some('\t')
    } else {
        Some(',')
    };
    let decimal_comma = sep == Some(';');
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let parts: Vec<&str> = match sep {
                Some(c) => l.split(c).collect(),
                None => l.split_whitespace().collect(),
            };
            parts
                .into_iter()
                .map(|p| {
                    if p.trim().is_empty() {
                        Cell::Empty
                    } else {
                        parse_num(p, decimal_comma).map_or(Cell::Text(p.trim().trim_matches('"').to_string()), Cell::Num)
                    }
                })
                .collect()
        })
        .collect()
}

/// Van cellen naar een matrix: kopregels (zonder getallen) bovenaan vallen weg; daarna moet alles
/// een getal zijn.
fn to_matrix(name: &str, mut rows: Vec<Vec<Cell>>) -> Result<Value, Error> {
    while rows.first().is_some_and(|r| !r.iter().any(|c| matches!(c, Cell::Num(_)))) {
        rows.remove(0);
    }
    if rows.is_empty() {
        return Err(err(name, "het bestand bevat geen getallen"));
    }
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut data = Vec::with_capacity(rows.len() * cols);
    for (r, row) in rows.iter().enumerate() {
        for c in 0..cols {
            match row.get(c).unwrap_or(&Cell::Empty) {
                Cell::Num(v) => data.push(Quantity::scalar(*v)),
                Cell::Empty => return Err(err(name, format!("lege cel in rij {}, kolom {}", r + 1, c + 1))),
                Cell::Text(t) => return Err(err(name, format!("rij {}, kolom {} is geen getal: '{t}'", r + 1, c + 1))),
            }
        }
    }
    Ok(Value::Matrix(Matrix::new(rows.len(), cols, data)))
}

/// Een matrix als tekst, één rij per regel. Waarden in SI.
fn table_text(m: &Matrix, sep: char) -> String {
    let mut s = String::new();
    for r in 0..m.rows {
        let row: Vec<String> = (0..m.cols).map(|c| format!("{}", m.get(r, c).value)).collect();
        s.push_str(&row.join(&sep.to_string()));
        s.push('\n');
    }
    s
}

/// `B12` → (rij 11, kolom 1), vanaf 0.
fn cell_ref(s: &str) -> Option<(usize, usize)> {
    let s = s.trim().trim_start_matches('$');
    let letters: String = s.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    let digits = s[letters.len()..].trim_start_matches('$');
    if letters.is_empty() || digits.is_empty() {
        return None;
    }
    let col = letters.to_ascii_uppercase().bytes().fold(0usize, |acc, b| acc * 26 + (b - b'A' + 1) as usize) - 1;
    let row: usize = digits.parse().ok()?;
    Some((row.checked_sub(1)?, col))
}

fn col_name(mut c: usize) -> String {
    let mut s = String::new();
    loop {
        s.insert(0, (b'A' + (c % 26) as u8) as char);
        if c < 26 {
            break;
        }
        c = c / 26 - 1;
    }
    s
}

/// Lees een werkblad uit een .xlsx. `range`: `"Blad1!A1:C10"`, `"A1:C10"` of `"Blad1"`.
fn read_xlsx(bytes: &[u8], range: Option<&str>) -> Result<Vec<Vec<Cell>>, String> {
    let files = zip::read(bytes).map_err(|_| "geen geldig Excel-bestand (.xlsx)".to_string())?;
    let text = |name: &str| files.get(name).map(|b| String::from_utf8_lossy(b).into_owned());
    let (sheet_name, cells) = match range {
        Some(r) => match r.split_once('!') {
            Some((s, c)) => (Some(s.trim_matches('\'').to_string()), Some(c.to_string())),
            None if r.contains(':') || cell_ref(r).is_some() => (None, Some(r.to_string())),
            None => (Some(r.to_string()), None),
        },
        None => (None, None),
    };
    // welk werkblad: de naam uit workbook.xml, via de relaties naar het bestand
    let workbook = xml::parse(&text("xl/workbook.xml").ok_or("xl/workbook.xml ontbreekt")?)?;
    let mut sheets = Vec::new();
    workbook.descendants("sheet", &mut sheets);
    let sheet = match &sheet_name {
        Some(n) => sheets.iter().find(|s| s.attr("name") == Some(n)).ok_or(format!("werkblad '{n}' bestaat niet"))?,
        None => sheets.first().ok_or("geen werkbladen")?,
    };
    let rid = sheet.attr("r:id").ok_or("werkblad zonder relatie")?;
    let rels = xml::parse(&text("xl/_rels/workbook.xml.rels").ok_or("relaties ontbreken")?)?;
    let target = rels.elems().find(|r| r.attr("Id") == Some(rid)).and_then(|r| r.attr("Target")).ok_or("werkblad niet gevonden")?;
    let target = target.trim_start_matches('/');
    let path = if target.starts_with("xl/") { target.to_string() } else { format!("xl/{target}") };
    let shared: Vec<String> = match text("xl/sharedStrings.xml") {
        Some(s) => xml::parse(&s)?.elems().map(|si| si.text()).collect(),
        None => Vec::new(),
    };
    let doc = xml::parse(&text(&path).ok_or(format!("{path} ontbreekt"))?)?;
    let mut cs = Vec::new();
    doc.descendants("c", &mut cs);
    let mut grid: Vec<((usize, usize), Cell)> = Vec::new();
    for c in cs {
        let Some(pos) = c.attr("r").and_then(cell_ref) else { continue };
        let v = c.child("v").map(|v| v.text());
        let cell = match (c.attr("t"), v) {
            (Some("s"), Some(i)) => Cell::Text(i.trim().parse::<usize>().ok().and_then(|k| shared.get(k).cloned()).unwrap_or_default()),
            (Some("inlineStr"), _) => Cell::Text(c.text()),
            (Some("str"), Some(t)) => Cell::Text(t),
            (Some("b"), Some(t)) => Cell::Num(if t.trim() == "1" { 1.0 } else { 0.0 }),
            (_, Some(t)) => t.trim().parse().map_or(Cell::Text(t), Cell::Num),
            (_, None) => Cell::Empty,
        };
        grid.push((pos, cell));
    }
    // het bereik: gegeven, of alles wat gevuld is
    let ((r0, c0), (r1, c1)) = match cells.as_deref().map(|c| c.split_once(':').map_or((c, c), |(a, b)| (a, b))) {
        Some((a, b)) => (cell_ref(a).ok_or(format!("ongeldige cel '{a}'"))?, cell_ref(b).ok_or(format!("ongeldige cel '{b}'"))?),
        None if grid.is_empty() => return Ok(Vec::new()),
        None => (
            (grid.iter().map(|g| g.0 .0).min().unwrap_or(0), grid.iter().map(|g| g.0 .1).min().unwrap_or(0)),
            (grid.iter().map(|g| g.0 .0).max().unwrap_or(0), grid.iter().map(|g| g.0 .1).max().unwrap_or(0)),
        ),
    };
    let mut rows = vec![vec![Cell::Empty; c1 - c0 + 1]; r1 - r0 + 1];
    for ((r, c), cell) in grid {
        if (r0..=r1).contains(&r) && (c0..=c1).contains(&c) {
            rows[r - r0][c - c0] = cell;
        }
    }
    Ok(rows)
}

/// Schrijf een matrix als .xlsx (één werkblad, getallen in SI). `start`: `"Blad1!B2"` of `"B2"`.
fn write_xlsx(m: &Matrix, start: Option<&str>) -> Result<Vec<u8>, String> {
    let (sheet, cell) = match start {
        Some(s) => match s.split_once('!') {
            Some((n, c)) => (n.to_string(), c.to_string()),
            None if cell_ref(s).is_some() => ("Blad1".to_string(), s.to_string()),
            None => (s.to_string(), "A1".to_string()),
        },
        None => ("Blad1".to_string(), "A1".to_string()),
    };
    let (r0, c0) = cell_ref(&cell).ok_or(format!("ongeldige cel '{cell}'"))?;
    let mut rows = String::new();
    for r in 0..m.rows {
        let mut cells = String::new();
        for c in 0..m.cols {
            let v = m.get(r, c).value;
            cells.push_str(&format!("<c r=\"{}{}\"><v>{v}</v></c>", col_name(c0 + c), r0 + r + 1));
        }
        rows.push_str(&format!("<row r=\"{}\">{cells}</row>", r0 + r + 1));
    }
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('"', "&quot;");
    let x = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>";
    let files = vec![
        (
            "[Content_Types].xml".to_string(),
            format!("{x}<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/><Override PartName=\"/xl/worksheets/sheet1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/></Types>").into_bytes(),
        ),
        (
            "_rels/.rels".to_string(),
            format!("{x}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>").into_bytes(),
        ),
        (
            "xl/workbook.xml".to_string(),
            format!("{x}<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><sheets><sheet name=\"{}\" sheetId=\"1\" r:id=\"rId1\"/></sheets></workbook>", esc(&sheet)).into_bytes(),
        ),
        (
            "xl/_rels/workbook.xml.rels".to_string(),
            format!("{x}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet1.xml\"/></Relationships>").into_bytes(),
        ),
        (
            "xl/worksheets/sheet1.xml".to_string(),
            format!("{x}<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData>{rows}</sheetData></worksheet>").into_bytes(),
        ),
    ];
    Ok(zip::write(&files))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Sheet;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("mathprime-io-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn csv_lezen_met_kopregel_en_nederlandse_notatie() {
        let d = dir("csv");
        std::fs::write(d.join("a.csv"), "x,F\n1,2.5\n2,5\n").unwrap();
        std::fs::write(d.join("b.csv"), "x;F\n1;2,5\n2;5\n").unwrap();
        let sheet = Sheet::load_in("A := READCSV(\"a.csv\")\nA[1, 1] =\nB := READCSV(\"b.csv\")\nB[0, 1] =\nrows(A) =\n", &d);
        let r = sheet.render();
        assert!(r.contains("A[1, 1] = 5"), "{r}");
        assert!(r.contains("B[0, 1] = 2.5"), "{r}");
        assert!(r.contains("rows(A) = 2"), "{r}");
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn schrijven_en_teruglezen_csv_en_excel() {
        let d = dir("excel");
        let src = "M := [1, 2; 3, 4.5]\nWRITECSV(\"m.csv\", M) =\nWRITEEXCEL(\"m.xlsx\", M) =\nC := READCSV(\"m.csv\")\nX := READEXCEL(\"m.xlsx\")\nY := READFILE(\"m.xlsx\", \"Blad1!A2:B2\")\nC[1, 1] + X[1, 1] =\nY[0, 0] =\n";
        let r = Sheet::load_in(src, &d).render();
        assert!(r.contains("C[1, 1] + X[1, 1] = 9"), "{r}");
        assert!(r.contains("Y[0, 0] = 3"), "{r}");
        // het Excel-bestand is een echt .xlsx (ZIP met werkblad)
        let files = zip::read(&std::fs::read(d.join("m.xlsx")).unwrap()).unwrap();
        assert!(files.contains_key("xl/worksheets/sheet1.xml"));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn duidelijke_fouten() {
        let d = dir("fout");
        std::fs::write(d.join("t.csv"), "1,2\n3,abc\n").unwrap();
        let r = Sheet::load_in("A := READCSV(\"t.csv\")\nB := READCSV(\"bestaat-niet.csv\")\nC := READCSV(12)\n", &d).render();
        assert!(r.contains("is geen getal: 'abc'"), "{r}");
        assert!(r.contains("kan ") && r.contains("niet lezen"), "{r}");
        assert!(r.contains("bestandsnaam, als tekst"), "{r}");
        assert_eq!(cell_ref("AB12"), Some((11, 27)));
        assert_eq!(col_name(27), "AB");
        let _ = std::fs::remove_dir_all(d);
    }
}

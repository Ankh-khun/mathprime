//! Ingebouwde opzoektabellen: `IPE := tabel("IPE")`, dan `IPE("IPE200").Iy` of `IPE("IPE200", "Iy")`.
//!
//! Van de staalprofielen staan alleen de afmetingen (h, b, tw, tf, r) hier; A, I, W en i worden uit de
//! geometrie uitgerekend, met de vier afrondingen. Dat is dezelfde berekening als in de profieltabellen, en de
//! test vergelijkt de uitkomst met bekende tabelwaarden. Materiaalwaarden zijn de gangbare normwaarden voor de
//! kleinste diktes (staal) of typische waarden; controleer ze voor een echt ontwerp.

use crate::error::Error;
use rb_units::{lookup_unit, Quantity};
use std::collections::BTreeMap;
use std::f64::consts::PI;
use std::sync::{Arc, OnceLock};

/// Een tabel: rijen met een sleutel en per kolom een waarde.
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub name: String,
    /// Kolomnaam en de eenheid waarin hij getoond wordt (leeg: dimensieloos).
    pub columns: Vec<(String, String)>,
    pub rows: Vec<(String, Vec<Quantity>)>,
}

/// Sleutels vergelijken zonder spaties en hoofdletters: `IPE 200` is `ipe200`.
fn normal(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).flat_map(char::to_lowercase).collect()
}

impl Table {
    /// De kolom met deze naam (hoofdletters tellen mee: `Iy`, niet `IY`).
    pub fn column(&self, name: &str) -> Result<usize, Error> {
        self.columns.iter().position(|(c, _)| c == name).ok_or_else(|| {
            let all: Vec<&str> = self.columns.iter().map(|(c, _)| c.as_str()).collect();
            Error::Domain(format!("tabel {} heeft geen kolom '{name}'; wel: {}", self.name, all.join(", ")))
        })
    }

    /// De rij met deze sleutel, als tabel met één rij. Een getal mag ook: `IPE(200)` is `IPE200`.
    pub fn row(&self, key: &str) -> Result<Table, Error> {
        let want = normal(key);
        let found = self.rows.iter().find(|(k, _)| normal(k) == want).or_else(|| {
            let digits = want.chars().all(|c| c.is_ascii_digit() || c == '.');
            if digits {
                self.rows.iter().find(|(k, _)| normal(k).trim_start_matches(|c: char| c.is_alphabetic()) == want)
            } else {
                None
            }
        });
        match found {
            Some(row) => Ok(Table { name: self.name.clone(), columns: self.columns.clone(), rows: vec![row.clone()] }),
            None => Err(Error::Domain(format!("tabel {} heeft geen rij '{key}'; bijvoorbeeld {}", self.name, self.rows[0].0))),
        }
    }

    /// Eén waarde uit de eerste rij (voor een tabel met één rij).
    pub fn value(&self, column: &str) -> Result<Quantity, Error> {
        Ok(self.rows[0].1[self.column(column)?])
    }

    /// De eenheid om een kolom in te tonen.
    pub fn unit_of(&self, column: &str) -> Option<&str> {
        self.columns.iter().find(|(c, _)| c == column).map(|(_, u)| u.as_str()).filter(|u| !u.is_empty())
    }
}

fn unit(text: &str) -> f64 {
    if text.is_empty() {
        return 1.0;
    }
    lookup_unit_expr(text)
}

/// Een eenheid zoals `kg/m`, `mm^4` of `MPa` als factor naar SI.
fn lookup_unit_expr(text: &str) -> f64 {
    let (num, den) = text.split_once('/').map_or((text, None), |(a, b)| (a, Some(b)));
    let one = |t: &str| -> f64 {
        let (name, pow) = t.split_once('^').map_or((t, 1), |(n, p)| (n, p.parse::<i32>().expect("macht")));
        lookup_unit(name).expect("eenheid in tabel").0.powi(pow)
    };
    one(num) / den.map_or(1.0, one)
}

fn dims_of(text: &str) -> rb_units::Dim {
    let (num, den) = text.split_once('/').map_or((text, None), |(a, b)| (a, Some(b)));
    let one = |t: &str| -> rb_units::Dim {
        let (name, pow) = t.split_once('^').map_or((t, 1), |(n, p)| (n, p.parse::<i32>().expect("macht")));
        let d = lookup_unit(name).expect("eenheid in tabel").1;
        d.checked_powi(pow).expect("dimensie")
    };
    let a = one(num);
    match den {
        Some(b) => a.checked_div(one(b)).expect("dimensie"),
        None => a,
    }
}

/// Maak een tabel uit getallen in de eenheden van de kolommen.
fn build(name: &str, columns: &[(&str, &str)], rows: Vec<(String, Vec<f64>)>) -> Table {
    let scale: Vec<(f64, rb_units::Dim)> = columns
        .iter()
        .map(|(_, u)| if u.is_empty() { (1.0, rb_units::Dim::NONE) } else { (unit(u), dims_of(u)) })
        .collect();
    Table {
        name: name.to_string(),
        columns: columns.iter().map(|(c, u)| (c.to_string(), u.to_string())).collect(),
        rows: rows
            .into_iter()
            .map(|(k, vals)| (k, vals.iter().zip(&scale).map(|(v, (f, d))| Quantity::new(v * f, *d)).collect()))
            .collect(),
    }
}

/// Doorsnedegrootheden van een gewalst I-profiel uit de afmetingen in mm: (A, G, Iy, Wy, Wply, iy, Iz, Wz, Wplz, iz)
/// in cm², kg/m, cm⁴, cm³, cm³, cm, cm⁴, cm³, cm³, cm.
fn i_section(h: f64, b: f64, tw: f64, tf: f64, r: f64) -> [f64; 10] {
    // de vier afrondingen: oppervlak, zwaartepunt vanaf de hoek, eigen traagheidsmoment
    let fa = (1.0 - PI / 4.0) * r * r;
    let fc = r * (10.0 - 3.0 * PI) / (12.0 - 3.0 * PI);
    // om de rand tegen de flens: vierkant min kwartcirkel (Steiner vanuit het middelpunt van de cirkel), dan
    // naar het eigen zwaartepunt (samen 0,0075 r⁴)
    let yq = 4.0 / (3.0 * PI);
    let edge = 1.0 / 3.0 - (PI / 16.0 - PI / 4.0 * yq * yq + PI / 4.0 * (1.0 - yq) * (1.0 - yq));
    let fi = (edge - (1.0 - PI / 4.0) * (fc / r).powi(2)) * r.powi(4);
    let hw = h - 2.0 * tf;
    let a = 2.0 * b * tf + hw * tw + 4.0 * fa;
    let dy = h / 2.0 - tf - fc;
    let dz = tw / 2.0 + fc;
    let iy = (b * h.powi(3) - (b - tw) * hw.powi(3)) / 12.0 + 4.0 * (fi + fa * dy * dy);
    let iz = (2.0 * tf * b.powi(3) + hw * tw.powi(3)) / 12.0 + 4.0 * (fi + fa * dz * dz);
    let wply = 2.0 * (b * tf * (h - tf) / 2.0 + tw * (hw / 2.0).powi(2) / 2.0 + 2.0 * fa * dy);
    let wplz = 2.0 * (2.0 * tf * (b / 2.0).powi(2) / 2.0 + hw * (tw / 2.0).powi(2) / 2.0 + 2.0 * fa * dz);
    [
        a / 1e2,
        a * 1e-6 * 7850.0,
        iy / 1e4,
        iy / (h / 2.0) / 1e3,
        wply / 1e3,
        (iy / a).sqrt() / 10.0,
        iz / 1e4,
        iz / (b / 2.0) / 1e3,
        wplz / 1e3,
        (iz / a).sqrt() / 10.0,
    ]
}

/// De kolommen van een profieltabel.
const PROFILE: &[(&str, &str)] = &[
    ("h", "mm"),
    ("b", "mm"),
    ("tw", "mm"),
    ("tf", "mm"),
    ("r", "mm"),
    ("A", "cm^2"),
    ("G", "kg/m"),
    ("Iy", "cm^4"),
    ("Wy", "cm^3"),
    ("Wply", "cm^3"),
    ("iy", "cm"),
    ("Iz", "cm^4"),
    ("Wz", "cm^3"),
    ("Wplz", "cm^3"),
    ("iz", "cm"),
];

fn profiles(name: &str, data: &[(u32, f64, f64, f64, f64, f64)]) -> Table {
    let rows = data
        .iter()
        .map(|&(size, h, b, tw, tf, r)| {
            let mut vals = vec![h, b, tw, tf, r];
            vals.extend(i_section(h, b, tw, tf, r));
            (format!("{name}{size}"), vals)
        })
        .collect();
    build(name, PROFILE, rows)
}

fn all() -> &'static BTreeMap<String, Arc<Table>> {
    static TABLES: OnceLock<BTreeMap<String, Arc<Table>>> = OnceLock::new();
    TABLES.get_or_init(|| {
        let mut list = vec![
            // Europese profielen: maat, h, b, tw, tf, r in mm
            profiles("IPE", &[
                (80, 80.0, 46.0, 3.8, 5.2, 5.0), (100, 100.0, 55.0, 4.1, 5.7, 7.0), (120, 120.0, 64.0, 4.4, 6.3, 7.0),
                (140, 140.0, 73.0, 4.7, 6.9, 7.0), (160, 160.0, 82.0, 5.0, 7.4, 9.0), (180, 180.0, 91.0, 5.3, 8.0, 9.0),
                (200, 200.0, 100.0, 5.6, 8.5, 12.0), (220, 220.0, 110.0, 5.9, 9.2, 12.0), (240, 240.0, 120.0, 6.2, 9.8, 15.0),
                (270, 270.0, 135.0, 6.6, 10.2, 15.0), (300, 300.0, 150.0, 7.1, 10.7, 15.0), (330, 330.0, 160.0, 7.5, 11.5, 18.0),
                (360, 360.0, 170.0, 8.0, 12.7, 18.0), (400, 400.0, 180.0, 8.6, 13.5, 21.0), (450, 450.0, 190.0, 9.4, 14.6, 21.0),
                (500, 500.0, 200.0, 10.2, 16.0, 21.0), (550, 550.0, 210.0, 11.1, 17.2, 24.0), (600, 600.0, 220.0, 12.0, 19.0, 24.0),
            ]),
            profiles("HEA", &[
                (100, 96.0, 100.0, 5.0, 8.0, 12.0), (120, 114.0, 120.0, 5.0, 8.0, 12.0), (140, 133.0, 140.0, 5.5, 8.5, 12.0),
                (160, 152.0, 160.0, 6.0, 9.0, 15.0), (180, 171.0, 180.0, 6.0, 9.5, 15.0), (200, 190.0, 200.0, 6.5, 10.0, 18.0),
                (220, 210.0, 220.0, 7.0, 11.0, 18.0), (240, 230.0, 240.0, 7.5, 12.0, 21.0), (260, 250.0, 260.0, 7.5, 12.5, 24.0),
                (280, 270.0, 280.0, 8.0, 13.0, 24.0), (300, 290.0, 300.0, 8.5, 14.0, 27.0), (320, 310.0, 300.0, 9.0, 15.5, 27.0),
                (340, 330.0, 300.0, 9.5, 16.5, 27.0), (360, 350.0, 300.0, 10.0, 17.5, 27.0), (400, 390.0, 300.0, 11.0, 19.0, 27.0),
                (450, 440.0, 300.0, 11.5, 21.0, 27.0), (500, 490.0, 300.0, 12.0, 23.0, 27.0), (550, 540.0, 300.0, 12.5, 24.0, 27.0),
                (600, 590.0, 300.0, 13.0, 25.0, 27.0), (650, 640.0, 300.0, 13.5, 26.0, 27.0), (700, 690.0, 300.0, 14.5, 27.0, 27.0),
                (800, 790.0, 300.0, 15.0, 28.0, 30.0), (900, 890.0, 300.0, 16.0, 30.0, 30.0), (1000, 990.0, 300.0, 16.5, 31.0, 30.0),
            ]),
            profiles("HEB", &[
                (100, 100.0, 100.0, 6.0, 10.0, 12.0), (120, 120.0, 120.0, 6.5, 11.0, 12.0), (140, 140.0, 140.0, 7.0, 12.0, 12.0),
                (160, 160.0, 160.0, 8.0, 13.0, 15.0), (180, 180.0, 180.0, 8.5, 14.0, 15.0), (200, 200.0, 200.0, 9.0, 15.0, 18.0),
                (220, 220.0, 220.0, 9.5, 16.0, 18.0), (240, 240.0, 240.0, 10.0, 17.0, 21.0), (260, 260.0, 260.0, 10.0, 17.5, 24.0),
                (280, 280.0, 280.0, 10.5, 18.0, 24.0), (300, 300.0, 300.0, 11.0, 19.0, 27.0), (320, 320.0, 300.0, 11.5, 20.5, 27.0),
                (340, 340.0, 300.0, 12.0, 21.5, 27.0), (360, 360.0, 300.0, 12.5, 22.5, 27.0), (400, 400.0, 300.0, 13.5, 24.0, 27.0),
                (450, 450.0, 300.0, 14.0, 26.0, 27.0), (500, 500.0, 300.0, 14.5, 28.0, 27.0), (550, 550.0, 300.0, 15.0, 29.0, 27.0),
                (600, 600.0, 300.0, 15.5, 30.0, 27.0), (650, 650.0, 300.0, 16.0, 31.0, 27.0), (700, 700.0, 300.0, 17.0, 32.0, 27.0),
                (800, 800.0, 300.0, 17.5, 33.0, 30.0), (900, 900.0, 300.0, 18.5, 35.0, 30.0), (1000, 1000.0, 300.0, 19.0, 36.0, 30.0),
            ]),
        ];
        // metrische boutdraad (grof): d, spoed, sleutelwijdte (ISO 4017); As uit de flankdiameter en kerndiameter
        let bolts: &[(f64, f64, f64)] = &[
            (3.0, 0.5, 5.5), (4.0, 0.7, 7.0), (5.0, 0.8, 8.0), (6.0, 1.0, 10.0), (8.0, 1.25, 13.0), (10.0, 1.5, 16.0),
            (12.0, 1.75, 18.0), (14.0, 2.0, 21.0), (16.0, 2.0, 24.0), (18.0, 2.5, 27.0), (20.0, 2.5, 30.0), (22.0, 2.5, 34.0),
            (24.0, 3.0, 36.0), (27.0, 3.0, 41.0), (30.0, 3.5, 46.0), (33.0, 3.5, 50.0), (36.0, 4.0, 55.0),
        ];
        list.push(build(
            "bout",
            &[("d", "mm"), ("P", "mm"), ("As", "mm^2"), ("A", "mm^2"), ("s", "mm")],
            bolts
                .iter()
                .map(|&(d, p, s)| {
                    let (d2, d3) = (d - 0.649519 * p, d - 1.226869 * p);
                    (format!("M{d}"), vec![d, p, PI / 4.0 * ((d2 + d3) / 2.0).powi(2), PI / 4.0 * d * d, s])
                })
                .collect(),
        ));
        // sterkteklassen van bouten (EN 1993-1-8)
        list.push(build(
            "boutklasse",
            &[("fub", "MPa"), ("fyb", "MPa")],
            [("4.6", 400.0, 240.0), ("4.8", 400.0, 320.0), ("5.6", 500.0, 300.0), ("5.8", 500.0, 400.0), ("6.8", 600.0, 480.0), ("8.8", 800.0, 640.0), ("10.9", 1000.0, 900.0), ("12.9", 1200.0, 1080.0)]
                .iter()
                .map(|&(k, u, y)| (k.to_string(), vec![u, y]))
                .collect(),
        ));
        // materialen: fy, fu (MPa), E (GPa), ρ (kg/m³); staal: EN 10025 voor t ≤ 16 mm; overige: typische waarden
        list.push(build(
            "materiaal",
            &[("fy", "MPa"), ("fu", "MPa"), ("E", "GPa"), ("rho", "kg/m^3")],
            [
                ("S235", 235.0, 360.0, 210.0, 7850.0), ("S275", 275.0, 430.0, 210.0, 7850.0), ("S355", 355.0, 490.0, 210.0, 7850.0), ("S460", 460.0, 540.0, 210.0, 7850.0),
                ("RVS304", 210.0, 520.0, 200.0, 7900.0), ("RVS316", 220.0, 520.0, 200.0, 8000.0),
                ("Al6060T6", 150.0, 190.0, 70.0, 2700.0), ("Al6061T6", 240.0, 260.0, 70.0, 2700.0), ("Al6082T6", 260.0, 310.0, 70.0, 2700.0),
                ("Ti6Al4V", 880.0, 950.0, 114.0, 4430.0),
            ]
            .iter()
            .map(|&(k, fy, fu, e, rho)| (k.to_string(), vec![fy, fu, e, rho]))
            .collect(),
        ));
        // beton (EN 1992-1-1): fcm, fctm en Ecm volgen uit fck
        list.push(build(
            "beton",
            &[("fck", "MPa"), ("fckcube", "MPa"), ("fcm", "MPa"), ("fctm", "MPa"), ("Ecm", "GPa")],
            [(12.0, 15.0), (16.0, 20.0), (20.0, 25.0), (25.0, 30.0), (30.0, 37.0), (35.0, 45.0), (40.0, 50.0), (45.0, 55.0), (50.0, 60.0)]
                .iter()
                .map(|&(fck, cube): &(f64, f64)| {
                    let fcm = fck + 8.0;
                    (format!("C{fck}/{cube}"), vec![fck, cube, fcm, 0.3 * fck.powf(2.0 / 3.0), 22.0 * (fcm / 10.0).powf(0.3)])
                })
                .collect(),
        ));
        list.into_iter().map(|t| (normal(&t.name), Arc::new(t))).collect()
    })
}

/// De tabel met deze naam.
pub fn load(name: &str) -> Result<Arc<Table>, Error> {
    all().get(&normal(name)).cloned().ok_or_else(|| {
        let names: Vec<&str> = all().values().map(|t| t.name.as_str()).collect();
        Error::Domain(format!("onbekende tabel '{name}'; er zijn: {}", names.join(", ")))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn val(t: &str, row: &str, col: &str, unit: &str) -> f64 {
        load(t).unwrap().row(row).unwrap().value(col).unwrap().value / lookup_unit_expr(unit)
    }

    fn near(a: f64, b: f64, tol: f64) {
        assert!((a - b).abs() <= tol, "{a} ≠ {b}");
    }

    /// Bekende waarden uit de profieltabellen (afgerond zoals daar).
    #[test]
    fn profielen_kloppen_met_de_tabelboeken() {
        for (row, a, g, iy, wy, wply, iz, wz) in [
            ("IPE200", 28.5, 22.4, 1943.0, 194.0, 221.0, 142.0, 28.5),
            ("IPE300", 53.8, 42.2, 8356.0, 557.0, 628.0, 604.0, 80.5),
            ("IPE80", 7.64, 6.0, 80.1, 20.0, 23.2, 8.49, 3.69),
            ("HEA200", 53.8, 42.3, 3692.0, 389.0, 430.0, 1336.0, 134.0),
            ("HEB200", 78.1, 61.3, 5696.0, 570.0, 642.0, 2003.0, 200.0),
            ("HEB300", 149.0, 117.0, 25170.0, 1680.0, 1869.0, 8563.0, 571.0),
        ] {
            let t = if row.starts_with("IPE") { "IPE" } else if row.starts_with("HEA") { "HEA" } else { "HEB" };
            near(val(t, row, "A", "cm^2"), a, 0.006 * a);
            near(val(t, row, "G", "kg/m"), g, 0.006 * g);
            near(val(t, row, "Iy", "cm^4"), iy, 0.0015 * iy);
            near(val(t, row, "Wy", "cm^3"), wy, 0.005 * wy);
            near(val(t, row, "Wply", "cm^3"), wply, 0.005 * wply);
            near(val(t, row, "Iz", "cm^4"), iz, 0.006 * iz);
            near(val(t, row, "Wz", "cm^3"), wz, 0.006 * wz);
        }
    }

    #[test]
    fn bouten_en_materialen() {
        near(val("bout", "M12", "As", "mm^2"), 84.3, 0.1);
        near(val("bout", "M20", "As", "mm^2"), 245.0, 0.5);
        near(val("boutklasse", "8.8", "fub", "MPa"), 800.0, 1e-9);
        near(val("materiaal", "S355", "fy", "MPa"), 355.0, 1e-9);
        // EN 1992-1-1 tabel 3.1
        near(val("beton", "C30/37", "fctm", "MPa"), 2.9, 0.05);
        near(val("beton", "C30/37", "Ecm", "GPa"), 33.0, 0.5);
    }

    #[test]
    fn sleutels_en_fouten() {
        let t = load("ipe").unwrap();
        assert_eq!(t.row("IPE 200").unwrap().rows[0].0, "IPE200");
        assert_eq!(t.row("200").unwrap().rows[0].0, "IPE200");
        assert!(t.row("IPE201").is_err());
        assert!(t.row("IPE200").unwrap().value("Q").is_err());
        assert!(load("bestaatniet").is_err());
    }
}

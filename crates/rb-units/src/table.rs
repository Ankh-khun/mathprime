//! Tabel met eenheden. Alle factoren zijn omrekenfactoren naar SI (coherente eenheden).

use crate::dim::Dim;
use std::f64::consts::PI;

pub const NONE: Dim = Dim::NONE;
pub const LENGTH: Dim = Dim::new(1, 0, 0, 0, 0, 0, 0);
pub const MASS: Dim = Dim::new(0, 1, 0, 0, 0, 0, 0);
pub const TIME: Dim = Dim::new(0, 0, 1, 0, 0, 0, 0);
pub const CURRENT: Dim = Dim::new(0, 0, 0, 1, 0, 0, 0);
pub const TEMPERATURE: Dim = Dim::new(0, 0, 0, 0, 1, 0, 0);
pub const AMOUNT: Dim = Dim::new(0, 0, 0, 0, 0, 1, 0);
pub const LUMINOUS: Dim = Dim::new(0, 0, 0, 0, 0, 0, 1);

pub const FORCE: Dim = Dim::new(1, 1, -2, 0, 0, 0, 0);
pub const PRESSURE: Dim = Dim::new(-1, 1, -2, 0, 0, 0, 0);
pub const ENERGY: Dim = Dim::new(2, 1, -2, 0, 0, 0, 0);
pub const POWER: Dim = Dim::new(2, 1, -3, 0, 0, 0, 0);
pub const FREQUENCY: Dim = Dim::new(0, 0, -1, 0, 0, 0, 0);
pub const ACCELERATION: Dim = Dim::new(1, 0, -2, 0, 0, 0, 0);
pub const VOLUME: Dim = Dim::new(3, 0, 0, 0, 0, 0, 0);
pub const VOLTAGE: Dim = Dim::new(2, 1, -3, -1, 0, 0, 0);
pub const CHARGE: Dim = Dim::new(0, 0, 1, 1, 0, 0, 0);
pub const CAPACITANCE: Dim = Dim::new(-2, -1, 4, 2, 0, 0, 0);
pub const RESISTANCE: Dim = Dim::new(2, 1, -3, -2, 0, 0, 0);
pub const CONDUCTANCE: Dim = Dim::new(-2, -1, 3, 2, 0, 0, 0);
pub const INDUCTANCE: Dim = Dim::new(2, 1, -2, -2, 0, 0, 0);
pub const FLUX: Dim = Dim::new(2, 1, -2, -1, 0, 0, 0);
pub const FLUX_DENSITY: Dim = Dim::new(0, 1, -2, -1, 0, 0, 0);

/// Definitie van een eenheid.
pub struct UnitDef {
    pub name: &'static str,
    /// Factor naar de SI-eenheid van dezelfde dimensie.
    pub factor: f64,
    pub dim: Dim,
    /// Mag er een SI-voorvoegsel (k, m, M, ...) voor staan?
    pub prefixable: bool,
}

const fn u(name: &'static str, factor: f64, dim: Dim, prefixable: bool) -> UnitDef {
    UnitDef {
        name,
        factor,
        dim,
        prefixable,
    }
}

/// Alle bekende eenheden. Let op: `g` is de valversnelling (zoals in Mathcad); gram is `gm`.
pub static UNITS: &[UnitDef] = &[
    // lengte
    u("m", 1.0, LENGTH, true),
    u("in", 0.0254, LENGTH, false),
    u("ft", 0.3048, LENGTH, false),
    u("yd", 0.9144, LENGTH, false),
    u("mi", 1609.344, LENGTH, false),
    // massa
    u("kg", 1.0, MASS, false),
    u("gm", 1e-3, MASS, true),
    u("gram", 1e-3, MASS, true),
    u("mg", 1e-6, MASS, false),
    u("t", 1000.0, MASS, false),
    u("lb", 0.45359237, MASS, false),
    u("oz", 0.028349523125, MASS, false),
    // tijd
    u("s", 1.0, TIME, true),
    u("min", 60.0, TIME, false),
    u("hr", 3600.0, TIME, false),
    u("h", 3600.0, TIME, false),
    u("day", 86400.0, TIME, false),
    // overige SI-basiseenheden
    u("A", 1.0, CURRENT, true),
    u("K", 1.0, TEMPERATURE, true),
    u("mol", 1.0, AMOUNT, true),
    u("cd", 1.0, LUMINOUS, false),
    // kracht
    u("N", 1.0, FORCE, true),
    u("lbf", 4.4482216152605, FORCE, false),
    u("kgf", 9.80665, FORCE, false),
    // druk en spanning
    u("Pa", 1.0, PRESSURE, true),
    u("bar", 1e5, PRESSURE, true),
    u("atm", 101325.0, PRESSURE, false),
    u("psi", 6894.757293168361, PRESSURE, false),
    u("mmHg", 133.322387415, PRESSURE, false),
    // energie
    u("J", 1.0, ENERGY, true),
    u("Wh", 3600.0, ENERGY, true),
    u("cal", 4.184, ENERGY, true),
    u("Btu", 1055.05585262, ENERGY, false),
    // vermogen
    u("W", 1.0, POWER, true),
    u("hp", 745.6998715822702, POWER, false),
    u("PS", 735.49875, POWER, false),
    // frequentie en toerental (hoeken zijn dimensieloos: 1 rpm = 2π/60 rad/s)
    u("Hz", 1.0, FREQUENCY, true),
    u("rpm", 2.0 * PI / 60.0, FREQUENCY, false),
    // valversnelling
    u("g", 9.80665, ACCELERATION, false),
    // elektrisch
    u("V", 1.0, VOLTAGE, true),
    u("ohm", 1.0, RESISTANCE, true),
    u("Ω", 1.0, RESISTANCE, true),
    u("C", 1.0, CHARGE, true),
    u("F", 1.0, CAPACITANCE, true),
    u("H", 1.0, INDUCTANCE, true),
    u("Wb", 1.0, FLUX, true),
    u("T", 1.0, FLUX_DENSITY, true),
    u("S", 1.0, CONDUCTANCE, true),
    // volume
    u("L", 1e-3, VOLUME, true),
    u("gal", 3.785411784e-3, VOLUME, false),
    // hoeken (dimensieloos)
    u("rad", 1.0, NONE, true),
    u("deg", PI / 180.0, NONE, false),
    u("rev", 2.0 * PI, NONE, false),
    // temperatuurverschillen (zonder nulpunt) en de absolute Rankine-schaal
    u("Δ°C", 1.0, TEMPERATURE, false),
    u("ΔdegC", 1.0, TEMPERATURE, false),
    u("Δ°F", 5.0 / 9.0, TEMPERATURE, false),
    u("ΔdegF", 5.0 / 9.0, TEMPERATURE, false),
    u("°R", 5.0 / 9.0, TEMPERATURE, false),
];

/// Een temperatuurschaal met een nulpunt: `kelvin = waarde · factor + offset`.
pub struct OffsetUnit {
    pub name: &'static str,
    pub factor: f64,
    pub offset: f64,
}

/// Temperatuurschalen met een nulpunt. Dit zijn geen gewone eenheden: `20 °C` is 293.15 K, maar
/// `2 · °C` betekent niets. Voor verschillen zijn er `Δ°C` en `Δ°F` (gewone eenheden).
pub static OFFSET_UNITS: &[OffsetUnit] = &[
    OffsetUnit { name: "°C", factor: 1.0, offset: 273.15 },
    OffsetUnit { name: "degC", factor: 1.0, offset: 273.15 },
    OffsetUnit { name: "°F", factor: 5.0 / 9.0, offset: 459.67 * 5.0 / 9.0 },
    OffsetUnit { name: "degF", factor: 5.0 / 9.0, offset: 459.67 * 5.0 / 9.0 },
];

/// Zoek een temperatuurschaal met nulpunt op. Geeft (factor, offset) naar kelvin.
pub fn lookup_offset_unit(name: &str) -> Option<(f64, f64)> {
    OFFSET_UNITS
        .iter()
        .find(|u| u.name == name)
        .map(|u| (u.factor, u.offset))
}

/// De basiseenheid achter een naam, zonder voorvoegsel: `kJ` → `J`, `mm` → `m`, `rpm` → `rpm`.
pub fn unit_base(name: &str) -> Option<&'static UnitDef> {
    if let Some(u) = exact(name) {
        return Some(u);
    }
    for &(prefix, _) in PREFIXES {
        if let Some(u) = name.strip_prefix(prefix).and_then(exact) {
            if u.prefixable {
                return Some(u);
            }
        }
    }
    None
}

/// SI-voorvoegsels. Bewust zonder hecto/deca/deci (conflicteren met `h`, `da` en `d`) en zonder peta.
pub const PREFIXES: &[(&str, f64)] = &[
    ("f", 1e-15),
    ("p", 1e-12),
    ("n", 1e-9),
    ("µ", 1e-6),
    ("μ", 1e-6),
    ("u", 1e-6),
    ("m", 1e-3),
    ("c", 1e-2),
    ("k", 1e3),
    ("M", 1e6),
    ("G", 1e9),
    ("T", 1e12),
];

pub(crate) fn exact(name: &str) -> Option<&'static UnitDef> {
    UNITS.iter().find(|u| u.name == name)
}

/// Zoek een eenheid op naam (met of zonder SI-voorvoegsel). Geeft (factor naar SI, dimensie).
pub fn lookup_unit(name: &str) -> Option<(f64, Dim)> {
    if let Some(u) = exact(name) {
        return Some((u.factor, u.dim));
    }
    for &(prefix, pf) in PREFIXES {
        if let Some(rest) = name.strip_prefix(prefix) {
            if let Some(u) = exact(rest) {
                if u.prefixable {
                    return Some((pf * u.factor, u.dim));
                }
            }
        }
    }
    None
}

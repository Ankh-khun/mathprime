use crate::dim::Dim;
use crate::quantity::{Quantity, UnitError};
use crate::table::*;

/// Het aantal significante cijfers in uitkomsten (instelling; standaard 6).
static DIGITS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(6);

/// Stel het aantal significante cijfers in (2 tot 15) voor [`format_number`].
pub fn set_significant_digits(n: usize) {
    DIGITS.store(n.clamp(2, 15), std::sync::atomic::Ordering::Relaxed);
}

pub fn significant_digits() -> usize {
    DIGITS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Formatteer een getal met ten hoogste het ingestelde aantal significante cijfers (standaard 6),
/// zonder overbodige nullen.
pub fn format_number(v: f64) -> String {
    format_number_digits(v, significant_digits())
}

/// Als [`format_number`], met `digits` significante cijfers.
pub fn format_number_digits(v: f64, digits: usize) -> String {
    let digits = digits.clamp(2, 15);
    if v.is_nan() {
        return "NaN".to_string();
    }
    if v.is_infinite() {
        return if v > 0.0 { "∞" } else { "-∞" }.to_string();
    }
    if v == 0.0 {
        return "0".to_string();
    }
    let a = v.abs();
    if a >= 1e7 || a < 1e-4 {
        let s = format!("{:.*e}", digits - 1, v);
        let (mant, exp) = s.split_once('e').unwrap_or((s.as_str(), "0"));
        return format!("{}e{}", trim_zeros(mant), exp);
    }
    let exp10 = a.log10().floor() as i32;
    let decimals = (digits as i32 - 1 - exp10).max(0) as usize;
    trim_zeros(&format!("{:.*}", decimals, v))
}

fn trim_zeros(s: &str) -> String {
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s.to_string()
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Scale {
    /// Nooit een voorvoegsel gebruiken.
    None,
    /// Voorvoegsels van nano tot giga.
    Full,
    /// Voorvoegsels van nano tot kilo (voor lengtes: geen megameters).
    UpToKilo,
}

/// Benoemde dimensies voor de automatische weergave: (dimensie, eenheid, suffix, schaling).
/// Het voorvoegsel komt voor `eenheid`; `suffix` blijft ongewijzigd (zoals de `·m` in `kN·m`).
///
/// Let op: energie wordt standaard als `N·m` getoond (veel gebruikt voor momenten en koppels).
/// Wil je joule, schrijf dan `= J` achter de uitdrukking.
const AUTO_NAMES: &[(Dim, &str, &str, Scale)] = &[
    (FORCE, "N", "", Scale::Full),
    (PRESSURE, "Pa", "", Scale::Full),
    (ENERGY, "N", "·m", Scale::Full),
    (POWER, "W", "", Scale::Full),
    (VOLTAGE, "V", "", Scale::Full),
    (CHARGE, "C", "", Scale::Full),
    (CAPACITANCE, "F", "", Scale::Full),
    (RESISTANCE, "Ω", "", Scale::Full),
    (INDUCTANCE, "H", "", Scale::Full),
    (FLUX, "Wb", "", Scale::Full),
    (FLUX_DENSITY, "T", "", Scale::Full),
    (CONDUCTANCE, "S", "", Scale::Full),
    (CURRENT, "A", "", Scale::Full),
    (LENGTH, "m", "", Scale::UpToKilo),
    (TIME, "s", "", Scale::None),
    (MASS, "kg", "", Scale::None),
];

const STEPS: &[(&str, f64)] = &[
    ("n", 1e-9),
    ("µ", 1e-6),
    ("m", 1e-3),
    ("", 1.0),
    ("k", 1e3),
    ("M", 1e6),
    ("G", 1e9),
];

fn choose_prefix(v: f64, scale: Scale) -> (&'static str, f64) {
    match scale {
        Scale::None => ("", 1.0),
        Scale::UpToKilo => prefix_up_to(v, 1e3),
        Scale::Full => prefix_up_to(v, 1e9),
    }
}

fn prefix_up_to(v: f64, cap: f64) -> (&'static str, f64) {
    let a = v.abs();
    if a == 0.0 || !a.is_finite() {
        return ("", 1.0);
    }
    let mut best = STEPS[0];
    for &(p, f) in STEPS {
        if f <= cap && a >= f * (1.0 - 1e-12) {
            best = (p, f);
        }
    }
    best
}

/// Kies automatisch een eenheid voor een dimensie, met een voorvoegsel dat past bij `magnitude`
/// (in SI). Geeft (deler, eenheidstekst); de tekst is leeg voor dimensieloze waarden.
/// Zo toont `12.5e6 Pa` als `12.5 MPa`: deler 1e6, tekst `MPa`.
pub fn auto_scale(magnitude: f64, dim: Dim) -> (f64, String) {
    if dim.is_none() {
        return (1.0, String::new());
    }
    for &(d, base, suffix, scale) in AUTO_NAMES {
        if d == dim {
            let (p, f) = choose_prefix(magnitude, scale);
            return (f, format!("{p}{base}{suffix}"));
        }
    }
    if let Some((f, s)) = composite(magnitude, dim) {
        return (f, s);
    }
    (1.0, dim.base_string())
}

/// Een dimensie zonder eigen naam uitdrukken als benoemde eenheid (eventueel in het kwadraat of
/// in de noemer) maal meters, seconden en kelvin, zoals Mathcad: `kg/s^2` → `N/m`,
/// `kg^2·m^3/s^4` → `N^2·m`, `kg/s^3` → `W/m^2`. Alleen als dat korter is dan SI-basiseenheden.
fn composite(magnitude: f64, dim: Dim) -> Option<(f64, String)> {
    // gewicht per exponent: seconden in de rest zijn minder leesbaar dan meters
    let weight = [1.0, 9.0, 1.5, 9.0, 1.0, 9.0, 9.0];
    let cost = |d: &Dim| d.0.iter().zip(weight).map(|(e, w)| (*e as f64).abs() * w).sum::<f64>();
    let mut best: Option<(f64, &str, &str, i32, Dim)> = None;
    for &(d, base, suffix, _) in AUTO_NAMES {
        if matches!(base, "m" | "s" | "kg") {
            continue;
        }
        for p in [1, -1, 2] {
            let Some(rest) = d.checked_powi(p).and_then(|dp| dim.checked_div(dp)) else { continue };
            // de rest alleen in m, s en K
            if [1, 3, 5, 6].iter().any(|&k| rest.0[k] != 0) {
                continue;
            }
            let c = cost(&rest) + if p == 1 { 0.0 } else { 1.0 };
            if best.as_ref().is_none_or(|b| c < b.0) {
                best = Some((c, base, suffix, p, rest));
            }
        }
    }
    let (c, base, suffix, p, rest) = best?;
    if c >= cost(&dim) {
        return None;
    }
    let named = format!("{base}{suffix}");
    let mut num: Vec<String> = Vec::new();
    let mut den: Vec<String> = Vec::new();
    let mut factor = 1.0;
    match p {
        1 => {
            // een voorvoegsel alleen bij de eerste macht (kN/m, niet (kN)^2)
            let (pre, f) = choose_prefix(magnitude, Scale::Full);
            factor = f;
            num.push(format!("{pre}{named}"));
        }
        2 => num.push(if suffix.is_empty() { format!("{base}^2") } else { format!("({named})^2") }),
        _ => den.push(named),
    }
    for (idx, sym) in [(0usize, "m"), (2, "s"), (4, "K")] {
        let e = rest.0[idx];
        let s = if e.abs() == 1 { sym.to_string() } else { format!("{sym}^{}", e.abs()) };
        if e > 0 {
            num.push(s);
        } else if e < 0 {
            den.push(s);
        }
    }
    let n = if num.is_empty() { "1".to_string() } else { num.join("·") };
    let text = match den.len() {
        0 => n,
        1 => format!("{n}/{}", den[0]),
        _ => format!("{n}/({})", den.join("·")),
    };
    Some((factor, text))
}

/// Heeft deze dimensie een eigen naam in de automatische weergave (N, Pa, N·m, W, ...)?
/// Zo niet, dan toont [`auto_scale`] SI-basiseenheden, zoals `kg/s^2` voor N/m.
pub fn has_auto_name(dim: Dim) -> bool {
    AUTO_NAMES.iter().any(|&(d, ..)| d == dim)
}

/// Schaal voor een eenheid met een naam (`J`, `rpm`, `psi`, `m`). Eenheden die een voorvoegsel mogen
/// hebben krijgen er automatisch een (`5000 J` → `5 kJ`). `None` als de naam onbekend is of de
/// dimensie niet past.
pub fn named_scale(name: &str, magnitude: f64, dim: Dim) -> Option<(f64, String)> {
    let def = crate::table::exact(name)?;
    if def.dim != dim {
        return None;
    }
    if !def.prefixable {
        return Some((def.factor, def.name.to_string()));
    }
    // geen megameters, kiloseconden of kiloliters
    let cap = match def.name {
        "m" => 1e3,
        "s" | "gm" | "gram" | "L" => 1.0,
        _ => 1e9,
    };
    let (p, f) = prefix_up_to(magnitude / def.factor, cap);
    Some((f * def.factor, format!("{p}{}", def.name)))
}

/// Getal en eenheid samenvoegen (`12.5` + `MPa` → `12.5 MPa`; zonder eenheid alleen het getal).
pub fn join_unit(number: &str, unit: &str) -> String {
    if unit.is_empty() {
        number.to_string()
    } else {
        format!("{number} {unit}")
    }
}

/// Automatische weergave van een grootheid, zoals `12.5 MPa`, `1.5 kN·m` of `10 m/s`.
pub fn format_quantity(q: Quantity) -> String {
    let (f, unit) = auto_scale(q.value, q.dim);
    join_unit(&format_number(q.value / f), &unit)
}

/// Weergave in een door de gebruiker gekozen eenheid. `unit` is de eenheid als grootheid (waarde in SI),
/// `unit_text` de tekst zoals de gebruiker die typte.
pub fn format_in_unit(q: Quantity, unit: Quantity, unit_text: &str) -> Result<String, UnitError> {
    q.same_dim(unit, "weergeven in deze eenheid")?;
    let v = q.value / unit.value;
    if unit_text.trim().is_empty() {
        Ok(format_number(v))
    } else {
        Ok(format!("{} {}", format_number(v), unit_text.trim()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lookup_unit;

    #[test]
    fn aantal_cijfers() {
        // los van de instelling (die is gedeeld tussen tests die tegelijk lopen)
        assert_eq!(format_number_digits(2.0 / 3.0, 6), "0.666667");
        assert_eq!(format_number_digits(2.0 / 3.0, 3), "0.667");
        assert_eq!(format_number_digits(1234.5678, 3), "1235");
        assert_eq!(format_number_digits(12.5, 3), "12.5");
        assert_eq!(format_number_digits(1.23456e-6, 3), "1.23e-6");
        assert_eq!(format_number_digits(1.0 / 3.0, 10), "0.3333333333");
        assert_eq!(format_number_digits(2.0 / 3.0, 6), format_number(2.0 / 3.0));
    }

    fn q(v: f64, name: &str) -> Quantity {
        let (f, d) = lookup_unit(name).unwrap();
        Quantity::new(v * f, d)
    }

    #[test]
    fn getallen() {
        assert_eq!(format_number(0.1 + 0.2), "0.3");
        assert_eq!(format_number(100.0), "100");
        assert_eq!(format_number(1234567.0), "1234567");
        assert_eq!(format_number(1e7), "1e7");
        assert_eq!(format_number(1.5e-5), "1.5e-5");
        assert_eq!(format_number(-12.5), "-12.5");
        assert_eq!(format_number(0.000413359788), "0.00041336");
        assert_eq!(format_number(0.0), "0");
    }

    #[test]
    fn grootheden() {
        assert_eq!(format_quantity(q(12.5, "MPa")), "12.5 MPa");
        assert_eq!(format_quantity(q(1.5, "kN")), "1.5 kN");
        assert_eq!(format_quantity(q(25.0, "mm")), "25 mm");
        assert_eq!(format_quantity(q(1500.0, "N").mul(q(1.0, "m")).unwrap()), "1.5 kN·m");
        assert_eq!(
            format_quantity(q(10.0, "m").div(q(1.0, "s")).unwrap()),
            "10 m/s"
        );
        assert_eq!(format_quantity(q(500.0, "gm")), "0.5 kg");
        assert_eq!(format_quantity(q(3600.0, "s")), "3600 s");
    }

    #[test]
    fn benoemde_eenheid() {
        let j = q(5000.0, "J");
        assert_eq!(named_scale("J", j.value, j.dim), Some((1000.0, "kJ".to_string())));
        let n = q(1450.0, "rpm");
        let (f, t) = named_scale("rpm", n.value, n.dim).unwrap();
        assert_eq!((format_number(n.value / f), t.as_str()), ("1450".to_string(), "rpm"));
        // lengtes niet boven kilometer, tijden zonder voorvoegsel naar boven
        let l = q(2.0e7, "m");
        assert_eq!(named_scale("m", l.value, l.dim).unwrap().1, "km");
        let t = q(7200.0, "s");
        assert_eq!(named_scale("s", t.value, t.dim).unwrap().1, "s");
        // verkeerde dimensie
        assert!(named_scale("J", 1.0, LENGTH).is_none());
    }

    #[test]
    fn in_eenheid() {
        let kn = q(1.0, "kN");
        let s = format_in_unit(q(2500.0, "N"), kn, "kN").unwrap();
        assert_eq!(s, "2.5 kN");
        assert!(format_in_unit(q(1.0, "m"), kn, "kN").is_err());
    }
}

//! rb-units: dimensies, eenheden en grootheden voor MathPrime.
//!
//! Elke waarde is een [`Quantity`]: een getal in SI-eenheden met een [`Dim`] (exponenten van de
//! zeven SI-basisgrootheden). Optellen van `m` en `s` is daardoor een fout, `N·m / s` is vermogen.

mod dim;
mod format;
mod quantity;
pub mod table;

pub use dim::{Dim, NDIM};
pub use format::{
    auto_scale, format_in_unit, format_number, format_number_digits, format_quantity, has_auto_name, join_unit,
    named_scale, set_significant_digits, significant_digits,
};
pub use quantity::{Quantity, UnitError};
pub use table::{lookup_offset_unit, lookup_unit, unit_base, OffsetUnit, UnitDef};

#[cfg(test)]
mod tests {
    use super::*;

    fn u(name: &str) -> Quantity {
        let (f, d) = lookup_unit(name).unwrap_or_else(|| panic!("onbekende eenheid {name}"));
        Quantity::new(f, d)
    }

    #[test]
    fn voorvoegsels() {
        assert_eq!(u("kN").value, 1000.0);
        assert_eq!(u("kN").dim, table::FORCE);
        assert_eq!(u("MPa").value, 1e6);
        assert!((u("kWh").value - 3.6e6).abs() < 1e-6);
        assert_eq!(u("mm").value, 1e-3);
        assert_eq!(u("cm").value, 1e-2);
        assert!((u("µm").value - 1e-6).abs() < 1e-18);
        assert_eq!(u("kg").value, 1.0);
        assert_eq!(u("mgm").value, 1e-6);
    }

    #[test]
    fn exacte_namen_gaan_voor_voorvoegsel() {
        // `min` is minuut, niet milli-inch; `cd` is candela, niet centi-dag; `mi` is mijl.
        assert_eq!(u("min").value, 60.0);
        assert_eq!(u("cd").dim, table::LUMINOUS);
        assert_eq!(u("mi").value, 1609.344);
    }

    #[test]
    fn onbekend_of_niet_prefixbaar() {
        assert!(lookup_unit("xyz").is_none());
        assert!(lookup_unit("kin").is_none()); // `in` mag geen voorvoegsel krijgen
        assert!(lookup_unit("").is_none());
    }

    #[test]
    fn optellen_met_verschillende_dimensies_faalt() {
        let e = u("m").add(u("s")).unwrap_err();
        assert!(e.to_string().contains("optellen"), "{e}");
        assert!(u("m").add(u("km")).is_ok());
    }

    #[test]
    fn machtsverheffen() {
        let m = u("m");
        let sq = m.pow(Quantity::scalar(2.0)).unwrap();
        assert_eq!(sq.dim, Dim::new(2, 0, 0, 0, 0, 0, 0));
        // m^0.5 kan niet, (m^2)^0.5 wel
        assert!(m.pow(Quantity::scalar(0.5)).is_err());
        let back = sq.pow(Quantity::scalar(0.5)).unwrap();
        assert_eq!(back.dim, m.dim);
        // exponent met eenheid is onzin
        assert!(m.pow(u("s")).is_err());
    }

    #[test]
    fn temperatuurschalen() {
        let (f, o) = lookup_offset_unit("°C").unwrap();
        assert!((20.0 * f + o - 293.15).abs() < 1e-12);
        let (f, o) = lookup_offset_unit("°F").unwrap();
        // 32 °F = 273.15 K, 212 °F = 373.15 K
        assert!((32.0 * f + o - 273.15).abs() < 1e-9);
        assert!((212.0 * f + o - 373.15).abs() < 1e-9);
        // °C is geen gewone eenheid; Δ°C wel
        assert!(lookup_unit("°C").is_none());
        assert_eq!(u("Δ°C").dim, table::TEMPERATURE);
        assert!((u("Δ°F").value - 5.0 / 9.0).abs() < 1e-15);
    }

    #[test]
    fn basiseenheid() {
        assert_eq!(unit_base("kJ").unwrap().name, "J");
        assert_eq!(unit_base("mm").unwrap().name, "m");
        assert_eq!(unit_base("rpm").unwrap().name, "rpm");
        assert!(unit_base("kin").is_none());
    }

    #[test]
    fn rpm_is_rad_per_seconde() {
        let rpm = u("rpm").value;
        assert!((rpm - 0.10471975511965977).abs() < 1e-12);
    }
}

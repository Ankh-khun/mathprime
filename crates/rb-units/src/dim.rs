use std::fmt;

/// Aantal SI-basisdimensies: lengte, massa, tijd, stroom, temperatuur, hoeveelheid stof, lichtsterkte.
pub const NDIM: usize = 7;

/// Volgorde en symbolen waarmee dimensies worden weergegeven (index in de exponentenreeks, symbool).
const DISPLAY: [(usize, &str); NDIM] = [
    (1, "kg"),
    (0, "m"),
    (2, "s"),
    (3, "A"),
    (4, "K"),
    (5, "mol"),
    (6, "cd"),
];

/// Een dimensie als exponenten van de zeven SI-basisgrootheden: [L, M, T, I, Θ, N, J].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct Dim(pub [i8; NDIM]);

impl Dim {
    /// Dimensieloos.
    pub const NONE: Dim = Dim([0; NDIM]);

    pub const fn new(l: i8, m: i8, t: i8, i: i8, k: i8, n: i8, j: i8) -> Dim {
        Dim([l, m, t, i, k, n, j])
    }

    pub fn is_none(&self) -> bool {
        self.0 == [0; NDIM]
    }

    pub fn checked_mul(self, o: Dim) -> Option<Dim> {
        let mut r = [0i8; NDIM];
        for i in 0..NDIM {
            r[i] = self.0[i].checked_add(o.0[i])?;
        }
        Some(Dim(r))
    }

    pub fn checked_div(self, o: Dim) -> Option<Dim> {
        let mut r = [0i8; NDIM];
        for i in 0..NDIM {
            r[i] = self.0[i].checked_sub(o.0[i])?;
        }
        Some(Dim(r))
    }

    pub fn checked_powi(self, n: i32) -> Option<Dim> {
        let mut r = [0i8; NDIM];
        for i in 0..NDIM {
            let v = (self.0[i] as i32).checked_mul(n)?;
            r[i] = i8::try_from(v).ok()?;
        }
        Some(Dim(r))
    }

    /// n-de machtswortel; alleen mogelijk als alle exponenten deelbaar zijn door n.
    pub fn root(self, n: u32) -> Option<Dim> {
        if n == 0 || n > 100 {
            return None;
        }
        let n = n as i32;
        let mut r = [0i8; NDIM];
        for i in 0..NDIM {
            let e = self.0[i] as i32;
            if e % n != 0 {
                return None;
            }
            r[i] = (e / n) as i8;
        }
        Some(Dim(r))
    }

    /// De dimensie uitgedrukt in SI-basiseenheden, bijvoorbeeld `kg/(m·s^2)`.
    pub fn base_string(&self) -> String {
        let mut num: Vec<String> = Vec::new();
        let mut den: Vec<String> = Vec::new();
        for (idx, sym) in DISPLAY {
            let e = self.0[idx];
            if e > 0 {
                num.push(pow_str(sym, e));
            } else if e < 0 {
                den.push(pow_str(sym, -e));
            }
        }
        let n = if num.is_empty() {
            "1".to_string()
        } else {
            num.join("·")
        };
        match den.len() {
            0 => n,
            1 => format!("{}/{}", n, den[0]),
            _ => format!("{}/({})", n, den.join("·")),
        }
    }

    /// Leesbare beschrijving voor foutmeldingen.
    pub fn describe(&self) -> String {
        if self.is_none() {
            "dimensieloos".to_string()
        } else {
            self.base_string()
        }
    }
}

fn pow_str(sym: &str, e: i8) -> String {
    if e == 1 {
        sym.to_string()
    } else {
        format!("{sym}^{e}")
    }
}

impl fmt::Display for Dim {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.base_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_string_voorbeelden() {
        assert_eq!(Dim::new(1, 1, -2, 0, 0, 0, 0).base_string(), "kg·m/s^2");
        assert_eq!(Dim::new(-1, 1, -2, 0, 0, 0, 0).base_string(), "kg/(m·s^2)");
        assert_eq!(Dim::new(0, 0, -1, 0, 0, 0, 0).base_string(), "1/s");
        assert_eq!(Dim::NONE.base_string(), "1");
    }

    #[test]
    fn rekenen_met_dimensies() {
        let l = Dim::new(1, 0, 0, 0, 0, 0, 0);
        let t = Dim::new(0, 0, 1, 0, 0, 0, 0);
        let v = l.checked_div(t).unwrap();
        assert_eq!(v, Dim::new(1, 0, -1, 0, 0, 0, 0));
        assert_eq!(l.checked_powi(2).unwrap().root(2), Some(l));
        assert_eq!(l.root(2), None);
    }
}

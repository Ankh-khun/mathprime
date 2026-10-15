use crate::dim::Dim;
use std::fmt;

/// Fouten bij het rekenen met eenheden.
#[derive(Debug, Clone, PartialEq)]
pub enum UnitError {
    Mismatch {
        op: &'static str,
        left: Dim,
        right: Dim,
    },
    NotDimensionless {
        what: String,
        dim: Dim,
    },
    NonIntegerPower {
        dim: Dim,
    },
    NoRoot {
        dim: Dim,
        n: u32,
    },
    Overflow,
}

impl fmt::Display for UnitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnitError::Mismatch { op, left, right } => write!(
                f,
                "eenheden komen niet overeen bij {op}: {} en {}",
                left.describe(),
                right.describe()
            ),
            UnitError::NotDimensionless { what, dim } => write!(
                f,
                "{what} moet dimensieloos zijn, maar heeft eenheid {}",
                dim.describe()
            ),
            UnitError::NonIntegerPower { dim } => write!(
                f,
                "een grootheid met eenheid {} kan alleen tot een geheel getal (of een wortelmacht 1/n) worden verheven",
                dim.describe()
            ),
            UnitError::NoRoot { dim, n } => write!(
                f,
                "kan geen {n}e-machtswortel nemen van eenheid {}",
                dim.describe()
            ),
            UnitError::Overflow => write!(f, "eenheidsexponenten worden te groot"),
        }
    }
}

impl std::error::Error for UnitError {}

/// Een grootheid: een getal (altijd opgeslagen in SI) met een dimensie.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quantity {
    pub value: f64,
    pub dim: Dim,
}

impl Quantity {
    pub const fn new(value: f64, dim: Dim) -> Self {
        Quantity { value, dim }
    }

    pub const fn scalar(value: f64) -> Self {
        Quantity {
            value,
            dim: Dim::NONE,
        }
    }

    pub fn is_dimensionless(&self) -> bool {
        self.dim.is_none()
    }

    pub fn neg(self) -> Self {
        Quantity::new(-self.value, self.dim)
    }

    pub fn add(self, o: Self) -> Result<Self, UnitError> {
        self.same_dim(o, "optellen")?;
        Ok(Quantity::new(self.value + o.value, self.dim))
    }

    pub fn sub(self, o: Self) -> Result<Self, UnitError> {
        self.same_dim(o, "aftrekken")?;
        Ok(Quantity::new(self.value - o.value, self.dim))
    }

    pub fn mul(self, o: Self) -> Result<Self, UnitError> {
        let dim = self.dim.checked_mul(o.dim).ok_or(UnitError::Overflow)?;
        Ok(Quantity::new(self.value * o.value, dim))
    }

    pub fn div(self, o: Self) -> Result<Self, UnitError> {
        let dim = self.dim.checked_div(o.dim).ok_or(UnitError::Overflow)?;
        Ok(Quantity::new(self.value / o.value, dim))
    }

    /// Machtsverheffen. Een exponent moet dimensieloos zijn. Een grootheid met eenheid kan alleen
    /// tot een geheel getal worden verheven, of tot 1/n als de eenheid een n-de machtswortel toelaat.
    pub fn pow(self, e: Quantity) -> Result<Self, UnitError> {
        if !e.dim.is_none() {
            return Err(UnitError::NotDimensionless {
                what: "de exponent".to_string(),
                dim: e.dim,
            });
        }
        let x = e.value;
        if self.dim.is_none() {
            return Ok(Quantity::scalar(self.value.powf(x)));
        }
        if x.fract() == 0.0 && x.abs() <= 100.0 {
            let n = x as i32;
            let dim = self.dim.checked_powi(n).ok_or(UnitError::Overflow)?;
            return Ok(Quantity::new(self.value.powi(n), dim));
        }
        let inv = 1.0 / x;
        if inv.is_finite() && inv >= 1.0 && inv <= 100.0 && (inv - inv.round()).abs() < 1e-9 {
            return self.root(inv.round() as u32);
        }
        Err(UnitError::NonIntegerPower { dim: self.dim })
    }

    /// n-de machtswortel (ook van grootheden met eenheid, mits de eenheid dat toelaat).
    pub fn root(self, n: u32) -> Result<Self, UnitError> {
        let dim = self
            .dim
            .root(n)
            .ok_or(UnitError::NoRoot { dim: self.dim, n })?;
        let inv = 1.0 / n as f64;
        let v = if self.value < 0.0 {
            if n % 2 == 1 {
                -(-self.value).powf(inv)
            } else {
                f64::NAN
            }
        } else {
            self.value.powf(inv)
        };
        Ok(Quantity::new(v, dim))
    }

    /// Geeft de waarde terug als de grootheid dimensieloos is, anders een fout.
    pub fn dimensionless_value(self, what: &str) -> Result<f64, UnitError> {
        if self.dim.is_none() {
            Ok(self.value)
        } else {
            Err(UnitError::NotDimensionless {
                what: what.to_string(),
                dim: self.dim,
            })
        }
    }

    /// Controleert dat beide grootheden dezelfde dimensie hebben.
    pub fn same_dim(self, o: Self, op: &'static str) -> Result<(), UnitError> {
        if self.dim == o.dim {
            Ok(())
        } else {
            Err(UnitError::Mismatch {
                op,
                left: self.dim,
                right: o.dim,
            })
        }
    }
}

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", crate::format::format_quantity(*self))
    }
}

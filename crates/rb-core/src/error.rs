use rb_units::UnitError;
use std::fmt;

/// Alle fouten die tijdens lezen of rekenen kunnen optreden.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    Lex { pos: usize, msg: String },
    Parse { pos: usize, msg: String },
    Unit(UnitError),
    Unknown(String),
    NotAFunction(String),
    Arity { name: String, expected: String, got: usize },
    Domain(String),
    Depth,
}

impl Error {
    /// Byte-positie in de brontekst, als de fout bij een bepaalde plek hoort.
    pub fn position(&self) -> Option<usize> {
        match self {
            Error::Lex { pos, .. } | Error::Parse { pos, .. } => Some(*pos),
            _ => None,
        }
    }

    pub(crate) fn parse(pos: usize, msg: impl Into<String>) -> Error {
        Error::Parse {
            pos,
            msg: msg.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Lex { msg, .. } | Error::Parse { msg, .. } => write!(f, "{msg}"),
            Error::Unit(e) => write!(f, "{e}"),
            Error::Unknown(n) => write!(f, "onbekende naam '{n}' (geen variabele, functie, constante of eenheid)"),
            Error::NotAFunction(n) => write!(f, "'{n}' is geen functie"),
            Error::Arity { name, expected, got } => {
                write!(f, "{name} verwacht {expected} argument(en), maar kreeg er {got}")
            }
            Error::Domain(m) => write!(f, "{m}"),
            Error::Depth => write!(f, "functieaanroepen zijn te diep geneest (recursie zonder einde?)"),
        }
    }
}

impl std::error::Error for Error {}

impl From<UnitError> for Error {
    fn from(e: UnitError) -> Self {
        Error::Unit(e)
    }
}

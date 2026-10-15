//! Automatisch aanvullen: welke namen passen bij wat er voor de cursor getypt is.
//!
//! Los van de interface, zodat het te testen is: de interface geeft de namen van het blad en
//! toont de lijst; dit bestand kiest en sorteert.

use crate::tree::{Atom, Cursor, Editor};

/// Soort voorstel; bepaalt de volgorde (eerst je eigen namen) en het teken in de lijst.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    /// Een naam die het blad definieert.
    Variable,
    /// Een eigen functie van het blad (`f(x) := ...`).
    UserFunction,
    /// Een ingebouwde functie.
    Function,
    Unit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub name: String,
    pub kind: Kind,
}

impl Suggestion {
    /// Functies krijgen haakjes erbij.
    pub fn is_function(&self) -> bool {
        matches!(self.kind, Kind::Function | Kind::UserFunction)
    }
}

/// Eenheden voor de lijst: de namen uit de tabel en de gangbare voorvoegsels (kN, MPa, mm, ...).
pub fn unit_names() -> Vec<String> {
    let mut out = Vec::new();
    for u in rb_units::table::UNITS {
        out.push(u.name.to_string());
        if u.prefixable {
            for p in ["k", "M", "G", "m"] {
                out.push(format!("{p}{}", u.name));
            }
            if u.name == "m" {
                out.push("cm".into());
            }
        }
    }
    out
}

/// Voorstellen voor `prefix`, hoogstens `max`. `sheet`: de namen van het blad, met of het een
/// functie is. Een naam die al volledig getypt is, komt niet in de lijst.
pub fn suggest(prefix: &str, sheet: &[(String, bool)], units: &[String], max: usize) -> Vec<Suggestion> {
    if prefix.is_empty() {
        return Vec::new();
    }
    let lower = prefix.to_lowercase();
    let mut all: Vec<(u8, Suggestion)> = Vec::new();
    let mut add = |name: &str, kind: Kind| {
        if name == prefix || !name.chars().next().is_some_and(|c| c.is_alphabetic()) {
            return;
        }
        // hoofdlettergevoelig eerst, daarna ook bij andere hoofdletters
        let rank = if name.starts_with(prefix) {
            0
        } else if name.to_lowercase().starts_with(&lower) {
            1
        } else {
            return;
        };
        if !all.iter().any(|(_, s)| s.name == name) {
            all.push((rank, Suggestion { name: name.to_string(), kind }));
        }
    };
    for (n, f) in sheet {
        add(n, if *f { Kind::UserFunction } else { Kind::Variable });
    }
    for n in rb_core::builtins::NAMES {
        add(n, Kind::Function);
    }
    for n in units {
        add(n, Kind::Unit);
    }
    all.sort_by(|(ra, a), (rb, b)| (ra, a.kind, a.name.len(), &a.name).cmp(&(rb, b.kind, b.name.len(), &b.name)));
    all.into_iter().take(max).map(|(_, s)| s).collect()
}

fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '\'' | '.')
}

impl Editor {
    /// De naam die voor de cursor getypt wordt (alleen als de cursor aan het eind ervan staat).
    /// Getallen ervoor tellen niet mee: in `5kN` is het `kN`. Niet in tekst tussen aanhalingstekens.
    pub fn partial_name(&self) -> Option<String> {
        if self.selection.is_some() {
            return None;
        }
        if let Some((&(i, _), outer)) = self.cursor.path.split_last() {
            if matches!(self.block(outer).atoms[i], Atom::Str(_)) {
                return None;
            }
        }
        let b = self.block(&self.cursor.path);
        let char_at = |k: usize| match b.atoms.get(k) {
            Some(Atom::Char(c)) if is_name_char(*c) => Some(*c),
            _ => None,
        };
        if char_at(self.cursor.pos).is_some() {
            return None;
        }
        let mut start = self.cursor.pos;
        while start > 0 && char_at(start - 1).is_some() {
            start -= 1;
        }
        let word: String = (start..self.cursor.pos).filter_map(char_at).collect();
        let name = word.trim_start_matches(|c: char| !c.is_alphabetic());
        (!name.is_empty()).then(|| name.to_string())
    }

    /// Vervang de getypte naam voor de cursor door `s`; een functie krijgt haakjes met de cursor erin.
    pub fn complete_with(&mut self, s: &Suggestion) {
        let Some(partial) = self.partial_name() else { return };
        let n = partial.chars().count();
        let pos = self.cursor.pos;
        let path = self.cursor.path.clone();
        let b = self.block_mut(&path);
        b.atoms.splice(pos - n..pos, s.name.chars().map(Atom::Char));
        self.cursor = Cursor { path, pos: pos - n + s.name.chars().count() };
        if s.is_function() {
            self.apply(crate::tree::Cmd::Char('('));
        }
    }

    /// Staat de cursor links van `:=` (een nieuwe naam typen)? Dan geen voorstellen.
    pub fn naming(&self) -> bool {
        self.cursor.path.is_empty()
            && self.root.atoms[..self.cursor.pos].iter().all(|a| !matches!(a, Atom::Op(crate::tree::Op::Define)))
            && self.root.atoms.iter().any(|a| matches!(a, Atom::Op(crate::tree::Op::Define)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::Cmd;

    fn ed(s: &str) -> Editor {
        let mut e = Editor::new();
        for c in s.chars() {
            e.apply(Cmd::Char(c));
        }
        e
    }

    #[test]
    fn de_naam_voor_de_cursor() {
        assert_eq!(ed("2*sq").partial_name().as_deref(), Some("sq"));
        assert_eq!(ed("5kN").partial_name().as_deref(), Some("kN"));
        assert_eq!(ed("M.ma").partial_name().as_deref(), Some("M.ma"));
        assert_eq!(ed("3+").partial_name(), None);
        assert_eq!(ed("12").partial_name(), None);
        assert_eq!(ed("concat(\"ab").partial_name(), None);
        assert!(ed("La:5").naming() == false && ed("La:5").partial_name().is_none());
    }

    #[test]
    fn volgorde_en_aanvullen() {
        let sheet = vec![("sigma.max".to_string(), false), ("sterkte".to_string(), true)];
        let units = unit_names();
        let s = suggest("s", &sheet, &units, 8);
        // eigen namen eerst (variabelen, dan functies), daarna ingebouwde
        assert_eq!((s[0].name.as_str(), s[1].name.as_str()), ("sigma.max", "sterkte"), "{s:?}");
        assert_eq!(suggest("sq", &sheet, &units, 8)[0].name, "sqrt");
        let s = suggest("MP", &sheet, &units, 8);
        assert_eq!(s[0], Suggestion { name: "MPa".into(), kind: Kind::Unit });
        // al volledig getypt: niets meer voor te stellen
        assert!(suggest("sqrt", &[], &units, 8).iter().all(|x| x.name != "sqrt"));

        let mut e = ed("2*sq");
        e.complete_with(&Suggestion { name: "sqrt".into(), kind: Kind::Function });
        e.apply(Cmd::Char('9'));
        assert_eq!(crate::convert::region_text(&e.root), "2 * sqrt(9)");
        let mut e = ed("F:5kN+3M");
        e.complete_with(&Suggestion { name: "MN".into(), kind: Kind::Unit });
        assert_eq!(crate::convert::region_text(&e.root), "F := 5 kN + 3 MN");
    }
}

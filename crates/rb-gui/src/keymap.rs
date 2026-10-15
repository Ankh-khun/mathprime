//! Sneltoetsen uit een configuratiebestand.
//!
//! Het bestand `keymap.txt` staat in de instellingenmap (Windows: `%APPDATA%\MathPrime`, elders
//! `$XDG_CONFIG_HOME/mathprime` of `~/.config/mathprime`). Elke regel is `actie = toetscombinatie`,
//! bijvoorbeeld `opslaan = Ctrl+S`. Regels met `#` zijn commentaar. Acties die er niet in staan
//! houden hun standaardtoets. Zo zijn verschillen tussen toetsenborden (QWERTY, QWERTZ) en persoonlijke
//! voorkeuren gegevens, geen code (zie docs/PLAN.md §6).

use eframe::egui::{Key, KeyboardShortcut, Modifiers};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Nieuw,
    Openen,
    Opslaan,
    OpslaanAls,
    ExportPdf,
    ExportSvg,
    BronTonen,
    Groter,
    Kleiner,
    Herberekenen,
    Grieks,
    Sneltoetsen,
    Help,
    Welkomstblad,
    Instellingen,
    Zoeken,
    Vervangen,
}

impl Action {
    pub const ALL: [Action; 17] = [
        Action::Nieuw,
        Action::Openen,
        Action::Opslaan,
        Action::OpslaanAls,
        Action::ExportPdf,
        Action::ExportSvg,
        Action::BronTonen,
        Action::Groter,
        Action::Kleiner,
        Action::Herberekenen,
        Action::Grieks,
        Action::Sneltoetsen,
        Action::Help,
        Action::Welkomstblad,
        Action::Instellingen,
        Action::Zoeken,
        Action::Vervangen,
    ];

    /// Naam in het configuratiebestand.
    pub fn key(self) -> &'static str {
        match self {
            Action::Nieuw => "nieuw",
            Action::Openen => "openen",
            Action::Opslaan => "opslaan",
            Action::OpslaanAls => "opslaan_als",
            Action::ExportPdf => "export_pdf",
            Action::ExportSvg => "export_svg",
            Action::BronTonen => "bron_tonen",
            Action::Groter => "groter",
            Action::Kleiner => "kleiner",
            Action::Herberekenen => "herberekenen",
            Action::Grieks => "grieks",
            Action::Sneltoetsen => "sneltoetsen",
            Action::Help => "help",
            Action::Welkomstblad => "welkomstblad",
            Action::Instellingen => "instellingen",
            Action::Zoeken => "zoeken",
            Action::Vervangen => "vervangen",
        }
    }

    /// Omschrijving in menu's en het overzicht.
    pub fn label(self) -> &'static str {
        match self {
            Action::Nieuw => "Nieuw werkblad",
            Action::Openen => "Openen…",
            Action::Opslaan => "Opslaan",
            Action::OpslaanAls => "Opslaan als…",
            Action::ExportPdf => "Exporteren naar PDF…",
            Action::ExportSvg => "Exporteren naar SVG…",
            Action::BronTonen => "Bron tonen of verbergen",
            Action::Groter => "Groter",
            Action::Kleiner => "Kleiner",
            Action::Herberekenen => "Alles herberekenen",
            Action::Grieks => "Letter voor de cursor ↔ Griekse letter",
            Action::Sneltoetsen => "Sneltoetsen…",
            Action::Help => "Syntax en hulp",
            Action::Welkomstblad => "Welkomstblad (voorbeeld)",
            Action::Instellingen => "Instellingen…",
            Action::Zoeken => "Zoeken…",
            Action::Vervangen => "Vervangen of hernoemen…",
        }
    }

    fn default_shortcut(self) -> &'static str {
        match self {
            Action::Nieuw => "Ctrl+N",
            Action::Openen => "Ctrl+O",
            Action::Opslaan => "Ctrl+S",
            Action::OpslaanAls => "Ctrl+Shift+S",
            Action::ExportPdf => "Ctrl+P",
            Action::ExportSvg => "Ctrl+Shift+E",
            Action::BronTonen => "Ctrl+Shift+B",
            Action::Groter => "Ctrl+Shift+Equals",
            Action::Kleiner => "Ctrl+Shift+Minus",
            Action::Herberekenen => "F5",
            Action::Grieks => "Ctrl+G",
            Action::Sneltoetsen => "Ctrl+Shift+K",
            Action::Help => "F1",
            Action::Welkomstblad => "Ctrl+Shift+F1",
            Action::Instellingen => "Ctrl+Comma",
            Action::Zoeken => "Ctrl+F",
            Action::Vervangen => "Ctrl+H",
        }
    }
}

/// Lees een toetscombinatie zoals `Ctrl+Shift+S`, `F5` of `Ctrl+Plus`. `Ctrl` is op een Mac Cmd.
pub fn parse_shortcut(s: &str) -> Option<KeyboardShortcut> {
    let s = s.trim();
    // `Ctrl++` betekent Ctrl en de plus-toets
    let (mods_part, key_part) = match s.strip_suffix("++") {
        Some(rest) => (rest, "+"),
        None => match s.rsplit_once('+') {
            Some((m, k)) => (m, k),
            None => ("", s),
        },
    };
    let mut mods = Modifiers::NONE;
    for m in mods_part.split('+').map(str::trim).filter(|m| !m.is_empty()) {
        match m.to_lowercase().as_str() {
            "ctrl" | "control" | "cmd" | "command" => mods = mods | Modifiers::COMMAND,
            "shift" => mods = mods | Modifiers::SHIFT,
            "alt" | "option" => mods = mods | Modifiers::ALT,
            _ => return None,
        }
    }
    let key = Key::from_name(key_part.trim()).or_else(|| Key::from_name(&key_part.trim().to_uppercase()))?;
    Some(KeyboardShortcut::new(mods, key))
}

pub struct Keymap {
    pub binds: Vec<(Action, KeyboardShortcut)>,
    /// Het configuratiebestand (ook als het nog niet bestaat).
    pub path: Option<PathBuf>,
    /// Regels die niet gelezen konden worden.
    pub problems: Vec<String>,
}

/// De map voor instellingen.
pub fn config_dir() -> Option<PathBuf> {
    let (base, new, old) = if cfg!(windows) {
        (std::env::var_os("APPDATA").map(PathBuf::from), "MathPrime", "Rekenblad")
    } else {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")));
        (base, "mathprime", "rekenblad")
    };
    let base = base?;
    let dir = base.join(new);
    // instellingen van vóór de naamswijziging (Rekenblad) overnemen
    let previous = base.join(old);
    if !dir.exists() && previous.is_dir() {
        if std::fs::create_dir_all(&dir).is_ok() {
            for f in ["keymap.txt", "instellingen.txt"] {
                let _ = std::fs::copy(previous.join(f), dir.join(f));
            }
        }
    }
    Some(dir)
}

impl Keymap {
    pub fn defaults() -> Keymap {
        Keymap {
            binds: Action::ALL
                .iter()
                .map(|a| (*a, parse_shortcut(a.default_shortcut()).expect("geldige standaardtoets")))
                .collect(),
            path: config_dir().map(|d| d.join("keymap.txt")),
            problems: Vec::new(),
        }
    }

    /// Standaardtoetsen, overschreven door het configuratiebestand (als dat bestaat).
    pub fn load() -> Keymap {
        let mut km = Keymap::defaults();
        if let Some(text) = km.path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()) {
            km.apply(&text);
        }
        km
    }

    /// Pas de regels van een configuratiebestand toe.
    pub fn apply(&mut self, text: &str) {
        for (n, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((name, keys)) = line.split_once('=') else {
                self.problems.push(format!("regel {}: verwacht 'actie = toets'", n + 1));
                continue;
            };
            let Some(action) = Action::ALL.iter().find(|a| a.key() == name.trim()) else {
                self.problems.push(format!("regel {}: onbekende actie '{}'", n + 1, name.trim()));
                continue;
            };
            match parse_shortcut(keys) {
                Some(sc) => {
                    if let Some(b) = self.binds.iter_mut().find(|b| b.0 == *action) {
                        b.1 = sc;
                    }
                }
                None => self.problems.push(format!("regel {}: onbekende toets '{}'", n + 1, keys.trim())),
            }
        }
    }

    pub fn shortcut(&self, a: Action) -> Option<&KeyboardShortcut> {
        self.binds.iter().find(|b| b.0 == a).map(|b| &b.1)
    }

    /// De inhoud van een standaardbestand, om aan te passen.
    pub fn default_file() -> String {
        let mut s = String::from(
            "# Sneltoetsen voor MathPrime. Formaat: actie = toetscombinatie\n\
             # Modifiers: Ctrl (op een Mac Cmd), Shift, Alt. Toetsen: A-Z, 0-9, F1-F12, Plus, Minus, Equals, ...\n",
        );
        for a in Action::ALL {
            s.push_str(&format!("{} = {}\n", a.key(), a.default_shortcut()));
        }
        s
    }
}

/// Zet een Latijnse letter om in de Griekse (en terug), zoals Ctrl+G in Mathcad.
pub fn toggle_greek(c: char) -> Option<char> {
    const PAIRS: &[(char, char)] = &[
        ('a', 'α'), ('b', 'β'), ('c', 'χ'), ('d', 'δ'), ('e', 'ε'), ('f', 'φ'), ('g', 'γ'), ('h', 'η'),
        ('i', 'ι'), ('k', 'κ'), ('l', 'λ'), ('m', 'μ'), ('n', 'ν'), ('o', 'ο'), ('p', 'π'), ('q', 'θ'),
        ('r', 'ρ'), ('s', 'σ'), ('t', 'τ'), ('u', 'υ'), ('w', 'ω'), ('x', 'ξ'), ('y', 'ψ'), ('z', 'ζ'),
        ('A', 'Α'), ('B', 'Β'), ('C', 'Χ'), ('D', 'Δ'), ('E', 'Ε'), ('F', 'Φ'), ('G', 'Γ'), ('H', 'Η'),
        ('I', 'Ι'), ('K', 'Κ'), ('L', 'Λ'), ('M', 'Μ'), ('N', 'Ν'), ('O', 'Ο'), ('P', 'Π'), ('Q', 'Θ'),
        ('R', 'Ρ'), ('S', 'Σ'), ('T', 'Τ'), ('U', 'Υ'), ('W', 'Ω'), ('X', 'Ξ'), ('Y', 'Ψ'), ('Z', 'Ζ'),
    ];
    PAIRS
        .iter()
        .find_map(|&(l, g)| if c == l { Some(g) } else if c == g { Some(l) } else { None })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toetsen_lezen() {
        let sc = parse_shortcut("Ctrl+Shift+S").unwrap();
        assert_eq!(sc.logical_key, Key::S);
        assert!(sc.modifiers.shift && sc.modifiers.command);
        assert_eq!(parse_shortcut("F5").unwrap().logical_key, Key::F5);
        assert_eq!(parse_shortcut("Ctrl++").unwrap().logical_key, Key::Plus);
        assert_eq!(parse_shortcut("ctrl+g").unwrap().logical_key, Key::G);
        assert!(parse_shortcut("Hyper+S").is_none());
    }

    #[test]
    fn configuratie_overschrijft_standaard() {
        let mut km = Keymap::defaults();
        km.apply("# commentaar\nopslaan = Ctrl+Shift+W\nbestaat_niet = F2\nhelp = Ctrl+Kapot\n");
        assert_eq!(km.shortcut(Action::Opslaan).unwrap().logical_key, Key::W);
        assert_eq!(km.problems.len(), 2, "{:?}", km.problems);
        // het standaardbestand leest zichzelf foutloos
        let mut km = Keymap::defaults();
        km.apply(&Keymap::default_file());
        assert!(km.problems.is_empty(), "{:?}", km.problems);
    }

    #[test]
    fn griekse_letters() {
        assert_eq!(toggle_greek('s'), Some('σ'));
        assert_eq!(toggle_greek('σ'), Some('s'));
        assert_eq!(toggle_greek('D'), Some('Δ'));
        assert_eq!(toggle_greek('1'), None);
    }
}

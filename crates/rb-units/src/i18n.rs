//! Meldingen in het Engels. De code schrijft meldingen in het Nederlands; `tr` zet de hele tekst om met de
//! patronen uit `messages.txt`. Wat op `{}` valt wordt zelf ook omgezet (`{} moet groter dan 0 zijn` met
//! `de bandbreedte`), behalve tussen aanhalingstekens: dat is een naam uit het blad. Omzetten van een al
//! Engelse tekst verandert niets, dus dubbel `tr` kan geen kwaad.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

static ENGLISH: AtomicBool = AtomicBool::new(false);

pub fn set_english(on: bool) {
    ENGLISH.store(on, Ordering::Relaxed);
}

pub fn english() -> bool {
    ENGLISH.load(Ordering::Relaxed)
}

/// De melding in de gekozen taal.
pub fn tr(msg: &str) -> String {
    if english() {
        translate(msg)
    } else {
        msg.to_string()
    }
}

struct Pattern {
    /// De vaste stukken rond de `{}`.
    nl: Vec<&'static str>,
    en: Vec<&'static str>,
}

struct Table {
    exact: HashMap<&'static str, &'static str>,
    /// Met `{}`; de langste vaste tekst eerst, zodat de meest specifieke wint.
    patterns: Vec<Pattern>,
}

fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut exact = HashMap::new();
        let mut patterns = Vec::new();
        for (nl, en) in entries() {
            if nl.contains("{}") {
                patterns.push(Pattern { nl: nl.split("{}").collect(), en: en.split("{}").collect() });
            } else {
                exact.insert(nl, en);
            }
        }
        patterns.sort_by_key(|p| std::cmp::Reverse(p.nl.iter().map(|s| s.len()).sum::<usize>()));
        Table { exact, patterns }
    })
}

fn entries() -> impl Iterator<Item = (&'static str, &'static str)> {
    include_str!("messages.txt").lines().filter(|l| !l.starts_with('#')).filter_map(|l| l.split_once(" => "))
}

/// Zet `msg` om; zonder passend patroon blijft de tekst zoals hij is.
pub fn translate(msg: &str) -> String {
    let t = table();
    if let Some(en) = t.exact.get(msg) {
        return en.to_string();
    }
    for p in &t.patterns {
        if let Some(caps) = captures(&p.nl, msg) {
            let mut out = String::with_capacity(msg.len());
            for (i, part) in p.en.iter().enumerate() {
                out.push_str(part);
                if let Some(c) = caps.get(i) {
                    let quoted = p.nl[i].ends_with('\'') && p.nl[i + 1].starts_with('\'');
                    if quoted {
                        out.push_str(c);
                    } else {
                        out.push_str(&translate(c));
                    }
                }
            }
            return out;
        }
    }
    msg.to_string()
}

/// Wat er op de `{}` valt, als `msg` helemaal bij het patroon past (elke `{}` zo kort mogelijk).
fn captures<'a>(parts: &[&str], msg: &'a str) -> Option<Vec<&'a str>> {
    let (first, rest) = parts.split_first()?;
    let (last, middle) = rest.split_last()?;
    let body = msg.strip_prefix(first)?.strip_suffix(last)?;
    let mut caps = Vec::with_capacity(rest.len());
    let mut at = 0;
    for part in middle {
        let found = if part.is_empty() { 0 } else { body[at..].find(part)? };
        caps.push(&body[at..at + found]);
        at += found + part.len();
    }
    caps.push(&body[at..]);
    Some(caps)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn samengestelde_meldingen() {
        assert_eq!(translate("deling door nul"), "division by zero");
        assert_eq!(translate("de bandbreedte moet groter dan 0 zijn"), "the bandwidth must be greater than 0");
        assert_eq!(translate("sin: werkt niet met een matrix"), "sin: does not work with a matrix");
        assert_eq!(
            translate("eenheden komen niet overeen bij optellen: m en s"),
            "units do not match in addition: m and s"
        );
        // een naam uit het blad blijft staan, ook als het een woord uit de tabel is
        assert_eq!(translate("onbekende naam 'matrix' (geen variabele, functie, constante of eenheid)"), "unknown name 'matrix' (not a variable, function, constant or unit)");
        assert_eq!(translate("regel 3 van het blok: deling door nul"), "line 3 of the block: division by zero");
        // al Engels of onbekend: ongewijzigd
        assert_eq!(translate("division by zero"), "division by zero");
        assert_eq!(translate("iets onbekends"), "iets onbekends");
    }

    #[test]
    fn tabel_is_in_orde() {
        for (nl, en) in entries() {
            assert_eq!(nl.matches("{}").count(), en.matches("{}").count(), "{nl}");
        }
    }

    /// `{naam:fmt}` wordt `{}`, `{{` wordt `{`.
    fn normalize(s: &str) -> String {
        let mut out = String::new();
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '{' if chars.peek() == Some(&'{') => {
                    chars.next();
                    out.push('{');
                }
                '}' if chars.peek() == Some(&'}') => {
                    chars.next();
                    out.push('}');
                }
                '{' => {
                    let inner: String = chars.clone().take_while(|c| *c != '}').collect();
                    if inner.chars().all(|c| c.is_alphanumeric() || "_:.".contains(c)) && chars.clone().any(|c| c == '}') {
                        for _ in 0..=inner.chars().count() {
                            chars.next();
                        }
                        out.push_str("{}");
                    } else {
                        out.push('{');
                    }
                }
                c => out.push(c),
            }
        }
        out
    }

    /// De stringliterals in `text`, zonder tests en commentaarregels.
    fn literals(text: &str) -> Vec<String> {
        let code: Vec<&str> = text.lines().take_while(|l| !l.contains("#[cfg(test)]")).filter(|l| !l.trim_start().starts_with("//")).collect();
        let code = code.join("\n");
        let mut out = Vec::new();
        let mut chars = code.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\'' && chars.peek() == Some(&'"') {
                chars.next(); // het teken '"'
                continue;
            }
            if c != '"' {
                continue;
            }
            let mut s = String::new();
            while let Some(c) = chars.next() {
                match c {
                    '\\' => match chars.next() {
                        // doorlopende string: de regelovergang en de inspringing horen er niet bij
                        Some('\n') => {
                            while chars.peek().is_some_and(|c| c.is_whitespace()) {
                                chars.next();
                            }
                        }
                        Some('n') => s.push('\n'),
                        Some(n) => s.push(n),
                        None => {}
                    },
                    '"' => break,
                    c => s.push(c),
                }
            }
            out.push(s);
        }
        out
    }

    /// Woorden die alleen in Nederlandse meldingen voorkomen.
    const DUTCH: &[&str] = &[
        "aantal", "alle", "alleen", "bestaat", "bevat", "bij", "buiten", "een", "eenheid", "eerst", "geen", "gebruik",
        "gebruikt", "groter", "hebben", "het", "hoort", "kan", "lege", "met", "moet", "moeten", "mogen", "na", "naam",
        "niet", "nog", "ontbreekt", "ontbreken", "ongeldig", "ongeldige", "onbekend", "onbekende", "regel", "tussen",
        "van", "verwacht", "voor", "vraagt", "wordt", "worden", "zijn",
    ];

    /// Elke Nederlandse melding uit de broncode staat in de tabel. Faalt deze test, voeg de melding dan toe aan
    /// `messages.txt`; is het geen melding, dan aan `GEEN_MELDING`.
    #[test]
    fn elke_melding_is_vertaald() {
        // de uitvoer van `mathprime-cli deps` blijft Nederlands, net als de rest van de command-line
        const GEEN_MELDING: &[&str] = &["bekende functie", "eenheid in tabel", "    (geen)
", "    regel {}: {}
", "{} (regel {}): {}
", "  hangt direct af van:
", "  en indirect van:
", "  wordt direct gebruikt door:
", "regel {:>3}  {:<12} gebruikt {} definitie(s), {} regel(s) gebruiken dit{}
", "(geen definities)
"];
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut missing = Vec::new();
        for dir in ["rb-units/src", "rb-core/src", "rb-core/src/eval", "rb-sheet/src"] {
            for file in std::fs::read_dir(root.join(dir)).expect("map").flatten() {
                if file.path().extension().is_none_or(|e| e != "rs") {
                    continue;
                }
                let text = std::fs::read_to_string(file.path()).expect("bestand");
                for lit in literals(&text) {
                    let msg = normalize(&lit);
                    let dutch = msg.split(|c: char| !c.is_alphabetic()).any(|w| DUTCH.contains(&w));
                    if dutch && !GEEN_MELDING.contains(&msg.as_str()) && translate(&msg) == msg {
                        missing.push(format!("{}: {msg}", file.file_name().to_string_lossy()));
                    }
                }
            }
        }
        assert!(missing.is_empty(), "niet in messages.txt:\n{}", missing.join("\n"));
    }
}

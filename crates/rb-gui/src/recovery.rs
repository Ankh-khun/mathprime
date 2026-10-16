//! Automatisch bewaren en herstellen na een crash.
//!
//! Elke minuut krijgt elk tabblad met niet-opgeslagen wijzigingen een kopie in
//! `herstel\<nummer>.herstel` in de instellingenmap: de eerste regel is het oorspronkelijke pad
//! (leeg voor een nieuw blad), de rest de tekst van het blad. Tabbladen zonder wijzigingen
//! verliezen hun kopie; netjes afsluiten leegt de map. Blijven er kopieën staan (een crash), dan
//! vraagt MathPrime bij de volgende start of ze terug moeten.

use crate::app::App;
use eframe::egui::{self, RichText};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use crate::taal::t;

/// Hoe vaak er bewaard wordt.
const INTERVAL: Duration = Duration::from_secs(60);

/// Een kopie die bij de start werd gevonden.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub file: PathBuf,
    /// Het oorspronkelijke bestand (geen: een nieuw, nog niet opgeslagen blad).
    pub path: Option<PathBuf>,
    pub source: String,
}

impl Found {
    pub fn name(&self) -> String {
        self.path.as_ref().and_then(|p| p.file_name()).map_or("naamloos.rkn".into(), |n| n.to_string_lossy().into_owned())
    }
}

/// De inhoud van een kopie: pad op de eerste regel, dan de bron.
fn encode(path: Option<&Path>, source: &str) -> String {
    format!("{}\n{source}", path.map_or(String::new(), |p| p.display().to_string()))
}

fn decode(file: PathBuf, text: &str) -> Found {
    let (first, source) = text.split_once('\n').unwrap_or((text, ""));
    let first = first.trim();
    Found { file, path: (!first.is_empty()).then(|| PathBuf::from(first)), source: source.to_string() }
}

/// Alle kopieën in een map, oudste eerst.
pub fn scan(dir: &Path) -> Vec<Found> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "herstel"))
        .map(|p| (std::fs::metadata(&p).and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH), p))
        .collect();
    files.sort();
    files.into_iter().filter_map(|(_, p)| std::fs::read_to_string(&p).ok().map(|t| decode(p, &t))).collect()
}

impl App {
    fn recovery_dir(&self) -> Option<PathBuf> {
        Some(self.keymap.path.as_ref()?.parent()?.join("herstel"))
    }

    /// Bij het starten: kopieën van een vorige keer gevonden? Dan vraagt `recovery_window` wat ermee moet.
    pub(crate) fn check_recovery(&mut self) {
        if let Some(dir) = self.recovery_dir() {
            let found = scan(&dir);
            if !found.is_empty() {
                self.recovery = Some(found);
            }
        }
        self.last_autosave = Instant::now();
    }

    /// Elke minuut: kopieën bijwerken. Elke frame aangeroepen; doet meestal niets.
    pub(crate) fn autosave(&mut self, ctx: &egui::Context) {
        ctx.request_repaint_after(INTERVAL);
        if self.last_autosave.elapsed() < INTERVAL || self.recovery.is_some() {
            return;
        }
        self.last_autosave = Instant::now();
        self.write_recovery();
    }

    /// Schrijf de kopieën nu (ook gebruikt door de test).
    pub(crate) fn write_recovery(&mut self) {
        let Some(dir) = self.recovery_dir() else { return };
        // (nummer, pad, bron, gewijzigd) van alle tabbladen; het actieve staat in de velden van App
        let mut docs = vec![(self.doc_id, self.path.clone(), self.source.clone(), self.dirty)];
        for (i, d) in self.tabs.iter().enumerate() {
            if i != self.active {
                docs.push((d.id, d.path.clone(), d.source.clone(), d.dirty));
            }
        }
        for (id, path, source, dirty) in docs {
            let file = dir.join(format!("{id}.herstel"));
            if dirty {
                let _ = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&file, encode(path.as_deref(), &source)));
            } else {
                let _ = std::fs::remove_file(&file);
            }
        }
    }

    /// Netjes afsluiten: geen kopieën meer nodig. Behalve als de vraag na een crash nog open staat:
    /// dan blijven ze, zodat ze de volgende keer weer aangeboden worden.
    pub(crate) fn clear_recovery(&mut self) {
        if self.recovery.is_some() {
            return;
        }
        if let Some(dir) = self.recovery_dir() {
            for f in scan(&dir) {
                let _ = std::fs::remove_file(f.file);
            }
        }
    }

    /// De vraag na een crash: herstellen of weggooien.
    pub(crate) fn recovery_window(&mut self, ctx: &egui::Context) {
        let Some(found) = self.recovery.clone() else { return };
        let mut choice = None;
        egui::Modal::new(egui::Id::new("herstellen")).show(ctx, |ui| {
            ui.set_max_width(420.0);
            ui.heading(format!("{}  Niet-opgeslagen werk gevonden", crate::icons::REFRESH));
            ui.label(t("MathPrime is de vorige keer niet netjes afgesloten. Deze bladen hadden wijzigingen die nog niet waren opgeslagen:"));
            ui.add_space(6.0);
            for f in &found {
                let wanneer = f.path.as_ref().map_or("nieuw blad".to_string(), |p| p.display().to_string());
                ui.label(format!("{}  {}", crate::icons::FILE, f.name()));
                ui.label(RichText::new(wanneer).small().weak());
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(RichText::new(t("Herstellen")).strong()).clicked() {
                    choice = Some(true);
                }
                if ui.button(t("Weggooien")).clicked() {
                    choice = Some(false);
                }
            });
        });
        let Some(restore) = choice else { return };
        self.recovery = None;
        for f in &found {
            if restore {
                // elk in een eigen tabblad, als niet-opgeslagen (met de oranje stip)
                self.text_in_tab(&f.source);
                self.path = f.path.clone();
                if let Some(d) = f.path.as_ref().and_then(|p| p.parent()) {
                    self.base_dir = d.to_path_buf();
                }
                let name = self.file_name();
                self.sheet.set_name(&name);
                self.dirty = true;
            }
            let _ = std::fs::remove_file(&f.file);
        }
        if restore {
            self.status = format!("{} blad(en) hersteld; sla ze op om ze te bewaren", found.len());
            self.write_recovery();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kopie_heen_en_terug() {
        let t = encode(Some(Path::new(r"C:\bladen\balk.rkn")), "L := 2 m\nL =\n");
        let f = decode(PathBuf::from("1.herstel"), &t);
        assert_eq!(f.path.as_deref(), Some(Path::new(r"C:\bladen\balk.rkn")));
        assert_eq!(f.source, "L := 2 m\nL =\n");
        assert_eq!(f.name(), "balk.rkn");
        let nieuw = decode(PathBuf::from("2.herstel"), &encode(None, "a := 1"));
        assert_eq!((nieuw.path.clone(), nieuw.name(), nieuw.source.as_str()), (None, "naamloos.rkn".to_string(), "a := 1"));
    }

    #[test]
    fn map_lezen() {
        let dir = std::env::temp_dir().join(format!("mathprime-herstel-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("5.herstel"), encode(None, "x := 3")).unwrap();
        std::fs::write(dir.join("iets-anders.txt"), "niet meenemen").unwrap();
        let found = scan(&dir);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].source, "x := 3");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

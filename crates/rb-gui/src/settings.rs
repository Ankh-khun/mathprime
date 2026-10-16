//! Instellingen (Bestand → Instellingen…, Ctrl+,): bewaard in `instellingen.txt` in de
//! configuratiemap, als regels `naam = waarde`. Oudere bestanden (alleen `thema` en `achtergrond`)
//! blijven werken; onbekende regels worden overgeslagen.

use crate::app::App;
use crate::canvas::Background;
use eframe::egui::{self, RichText};
use crate::taal::t;

/// Licht, donker of zoals Windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Thema {
    #[default]
    Licht,
    Donker,
    Systeem,
}

impl Thema {
    const ALL: [Thema; 3] = [Thema::Licht, Thema::Donker, Thema::Systeem];

    fn key(self) -> &'static str {
        match self {
            Thema::Licht => "licht",
            Thema::Donker => "donker",
            Thema::Systeem => "systeem",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Thema::Licht => "☀ Licht",
            Thema::Donker => "🌙 Donker",
            Thema::Systeem => "Zoals Windows",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub thema: Thema,
    pub background: Background,
    /// Beginnen met het welkomstblad in plaats van een leeg blad.
    pub start_welcome: bool,
    /// Automatisch aanvullen tijdens het typen (Ctrl+Spatie werkt altijd).
    pub autocomplete: bool,
    /// Significante cijfers in uitkomsten.
    pub digits: usize,
    /// Zoom bij het starten (1.5 = 100%).
    pub start_zoom: f32,
    /// Eens per dag kijken of er een nieuwere versie is (update.rs).
    pub updates: bool,
    /// De interface in het Engels (taal.rs).
    pub english: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            thema: Thema::Licht,
            background: Background::default(),
            start_welcome: false,
            autocomplete: true,
            digits: 6,
            start_zoom: 1.5,
            updates: true,
            english: false,
        }
    }
}

impl Settings {
    /// Lees `instellingen.txt`; wat ontbreekt of niet klopt, houdt de standaardwaarde.
    pub fn parse(text: &str) -> Settings {
        let mut s = Settings::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let v = v.trim();
            match k.trim() {
                "thema" => s.thema = Thema::ALL.into_iter().find(|t| t.key() == v).unwrap_or_default(),
                "achtergrond" => {
                    if let Some(b) = Background::ALL.iter().find(|b| b.key() == v) {
                        s.background = *b;
                    }
                }
                "start" => s.start_welcome = v == "welkom",
                "aanvullen" => s.autocomplete = v != "uit",
                "cijfers" => s.digits = v.parse::<usize>().map_or(6, |n| n.clamp(2, 15)),
                "updates" => s.updates = v != "uit",
                "taal" => s.english = v == "en",
                "zoom" => s.start_zoom = v.parse::<f32>().map_or(1.5, |z| z.clamp(0.6, 5.0)),
                _ => {}
            }
        }
        s
    }

    pub fn to_text(&self) -> String {
        format!(
            "# Instellingen van MathPrime\nthema = {}\nachtergrond = {}\nstart = {}\naanvullen = {}\ncijfers = {}\nzoom = {}\nupdates = {}\ntaal = {}\n",
            self.thema.key(),
            self.background.key(),
            if self.start_welcome { "welkom" } else { "leeg" },
            if self.autocomplete { "aan" } else { "uit" },
            self.digits,
            self.start_zoom,
            if self.updates { "aan" } else { "uit" },
            if self.english { "en" } else { "nl" }
        )
    }
}

impl App {
    fn recent_path(&self) -> Option<std::path::PathBuf> {
        Some(self.keymap.path.as_ref()?.parent()?.join("recent.txt"))
    }

    /// Lees de lijst met recent geopende bestanden (`recent.txt`, één pad per regel).
    pub(crate) fn load_recent(&mut self) {
        let Some(text) = self.recent_path().and_then(|p| std::fs::read_to_string(p).ok()) else { return };
        self.recent = text.lines().map(str::trim).filter(|l| !l.is_empty()).map(std::path::PathBuf::from).take(8).collect();
    }

    /// Zet een bestand vooraan in de lijst (hooguit 8) en bewaar die.
    pub(crate) fn remember_recent(&mut self, p: &std::path::Path) {
        let p = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        self.recent.retain(|q| q != &p);
        self.recent.insert(0, p);
        self.recent.truncate(8);
        if let Some(path) = self.recent_path() {
            let text: String = self.recent.iter().map(|q| format!("{}\n", q.display())).collect();
            let _ = path.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|_| std::fs::write(&path, text));
        }
    }

    /// Een leeg blad: een rustige aanwijzing en de recente bestanden, midden op de pagina.
    pub(crate) fn empty_state(&mut self, ui: &mut egui::Ui, center: egui::Pos2) {
        let recent: Vec<std::path::PathBuf> = self.recent.iter().filter(|p| p.exists()).take(6).cloned().collect();
        let mut open = None;
        let rect = egui::Rect::from_center_size(center, egui::vec2(380.0, 300.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.vertical_centered(|ui| {
                let weak = ui.visuals().weak_text_color();
                ui.label(RichText::new(crate::icons::FUNCTION.to_string()).size(34.0).color(weak));
                ui.add_space(6.0);
                ui.label(RichText::new(t("Klik ergens en begin te typen")).size(17.0));
                ui.label(RichText::new(t("Of open een bestand met Ctrl+O. Een voorbeeld staat onder Help → Welkomstblad.")).size(12.0).color(weak));
                if !recent.is_empty() {
                    ui.add_space(18.0);
                    ui.label(RichText::new(t("Recent geopend")).size(12.0).strong().color(weak));
                    ui.add_space(4.0);
                    for p in &recent {
                        let name = p.file_name().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().into_owned());
                        let dir = p.parent().map(|d| d.display().to_string()).unwrap_or_default();
                        let text = format!("{}  {name}", crate::icons::FILE);
                        let r = ui.add(egui::Button::new(RichText::new(text).size(13.0)).frame_when_inactive(false));
                        if r.on_hover_text(dir).clicked() {
                            open = Some(p.clone());
                        }
                    }
                }
            });
        });
        if let Some(p) = open {
            self.open_in_tab(p);
        }
    }
    /// Het thema toepassen; bij "zoals Windows" elke frame (Windows kan intussen wisselen).
    pub(crate) fn apply_theme(&mut self, ctx: &egui::Context) {
        let dark = match self.cv.set.thema {
            Thema::Licht => false,
            Thema::Donker => true,
            Thema::Systeem => ctx.system_theme().map_or(self.cv.dark, |t| t == egui::Theme::Dark),
        };
        if dark != self.cv.dark || ctx.theme() != if dark { egui::Theme::Dark } else { egui::Theme::Light } {
            self.set_dark(ctx, dark);
        }
    }

    /// Het venster Instellingen. Een wijziging werkt meteen en wordt bewaard.
    pub(crate) fn settings_window(&mut self, ctx: &egui::Context) {
        let mut open = self.cv.show_settings;
        let before = self.cv.set.clone();
        let mut s = before.clone();
        egui::Window::new(t("Instellingen")).open(&mut open).resizable(false).collapsible(false).fade_in(false).anchor(egui::Align2::CENTER_TOP, [0.0, 140.0]).show(ctx, |ui| {
            ui.set_min_width(440.0);
            // een kop per onderdeel, de instellingen eronder in twee kolommen
            let section = |ui: &mut egui::Ui, icon: char, title: &str| {
                ui.add_space(6.0);
                ui.label(RichText::new(format!("{icon}  {}", t(title))).size(15.0).strong());
                ui.separator();
            };
            let row_label = |ui: &mut egui::Ui, text: &str| {
                ui.allocate_ui_with_layout(egui::vec2(150.0, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.label(RichText::new(t(text)).color(ui.visuals().weak_text_color()))
                });
            };

            section(ui, crate::icons::SUN, "Uiterlijk");
            egui::Grid::new("inst-uiterlijk").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| {
                row_label(ui, "Taal");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut s.english, false, "Nederlands");
                    ui.selectable_value(&mut s.english, true, "English");
                });
                ui.end_row();
                row_label(ui, "Thema");
                ui.horizontal(|ui| {
                    for th in Thema::ALL {
                        ui.selectable_value(&mut s.thema, th, t(th.label()));
                    }
                });
                ui.end_row();
                row_label(ui, "Achtergrond");
                ui.horizontal(|ui| {
                    for b in Background::ALL {
                        ui.selectable_value(&mut s.background, b, t(b.label()));
                    }
                });
                ui.end_row();
                row_label(ui, "Zoom bij het starten");
                let mut pct = (s.start_zoom / 1.5 * 100.0).round();
                if ui.add(egui::Slider::new(&mut pct, 40.0..=300.0).step_by(10.0).suffix(" %")).changed() {
                    s.start_zoom = pct / 100.0 * 1.5;
                }
                ui.end_row();
            });

            section(ui, crate::icons::FILE, "Starten");
            egui::Grid::new("inst-starten").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| {
                row_label(ui, "Bij het starten");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut s.start_welcome, false, t("Leeg blad"));
                    ui.selectable_value(&mut s.start_welcome, true, t("Welkomstblad"));
                });
                ui.end_row();
            });

            section(ui, crate::icons::FUNCTION, "Rekenen en typen");
            egui::Grid::new("inst-rekenen").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| {
                row_label(ui, "Cijfers in uitkomsten");
                ui.add(egui::Slider::new(&mut s.digits, 2..=15).text("significant"));
                ui.end_row();
                row_label(ui, "Automatisch aanvullen");
                ui.checkbox(&mut s.autocomplete, t("lijst met namen tijdens het typen"));
                ui.end_row();
            });

            section(ui, crate::icons::REFRESH, "Updates");
            egui::Grid::new("inst-updates").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| {
                row_label(ui, "Nieuwe versies");
                ui.checkbox(&mut s.updates, t("eens per dag kijken op GitHub"))
                    .on_hover_text(t("Vraagt alleen het nummer van de nieuwste versie op; er gaat niets van jou mee"));
                ui.end_row();
            });
            ui.add_space(10.0);
            ui.label(RichText::new(t("Wijzigingen werken meteen. Sneltoetsen staan in keymap.txt (Help → Sneltoetsen).")).small().weak());
        });
        self.cv.show_settings = open;
        if s != before {
            let digits_changed = s.digits != before.digits;
            crate::taal::set_english(s.english);
            self.cv.set = s;
            self.apply_theme(ctx);
            if digits_changed {
                rb_units::set_significant_digits(self.cv.set.digits);
                self.reload();
            }
            self.save_settings();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lezen_en_schrijven() {
        let s = Settings { thema: Thema::Systeem, background: Background::Lijnen, start_welcome: true, autocomplete: false, digits: 4, start_zoom: 2.25, updates: false, english: true };
        assert_eq!(Settings::parse(&s.to_text()), s);
        // een oud bestand: alleen thema en achtergrond, de rest standaard
        let old = Settings::parse("# Instellingen van MathPrime\nachtergrond = ruitjes\nthema = donker\n");
        assert_eq!(old, Settings { thema: Thema::Donker, background: Background::Ruitjes, ..Settings::default() });
        // onzin wordt de standaard
        assert_eq!(Settings::parse("cijfers = veel\nzoom = x\nthema = paars"), Settings::default());
    }
}

//! Instellingen (Bestand → Instellingen…, Ctrl+,): bewaard in `instellingen.txt` in de
//! configuratiemap, als regels `naam = waarde`. Oudere bestanden (alleen `thema` en `achtergrond`)
//! blijven werken; onbekende regels worden overgeslagen.

use crate::app::App;
use crate::canvas::Background;
use eframe::egui::{self, RichText};
use crate::taal::t;
use crate::update::Phase;

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
    /// Wachten op het scherm bij het tekenen (vsync). Uit: het venster volgt de muis direct bij
    /// verplaatsen (met vsync bleef het op sommige systemen achter en gleed het na). Werkt na herstarten.
    pub vsync: bool,
    /// De naam van het kleurthema (theme.rs); leeg: het gewone licht of donker.
    pub kleuren: String,
    /// De eigen titelbalk in plaats van die van Windows (titlebar.rs). Werkt na herstarten.
    pub own_titlebar: bool,
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
            vsync: false,
            kleuren: String::new(),
            own_titlebar: true,
        }
    }
}

impl Settings {
    /// Lees `instellingen.txt`; wat ontbreekt of niet klopt, houdt de standaardwaarde.
    pub fn parse(text: &str) -> Settings {
        let mut s = Settings::default();
        // een BOM vooraan (Kladblok, PowerShell) hoort niet bij de eerste sleutel
        for line in text.trim_start_matches('\u{feff}').lines() {
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
                "vsync" => s.vsync = v == "aan",
                "kleuren" => s.kleuren = v.to_string(),
                "titelbalk" => s.own_titlebar = v != "windows",
                "zoom" => s.start_zoom = v.parse::<f32>().map_or(1.5, |z| z.clamp(0.6, 5.0)),
                _ => {}
            }
        }
        s
    }

    pub fn to_text(&self) -> String {
        format!(
            "# Instellingen van MathPrime\nthema = {}\nachtergrond = {}\nstart = {}\naanvullen = {}\ncijfers = {}\nzoom = {}\nupdates = {}\ntaal = {}\nvsync = {}\nkleuren = {}\ntitelbalk = {}\n",
            self.thema.key(),
            self.background.key(),
            if self.start_welcome { "welkom" } else { "leeg" },
            if self.autocomplete { "aan" } else { "uit" },
            self.digits,
            self.start_zoom,
            if self.updates { "aan" } else { "uit" },
            if self.english { "en" } else { "nl" },
            if self.vsync { "aan" } else { "uit" },
            self.kleuren,
            if self.own_titlebar { "eigen" } else { "windows" }
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
        // tijdens het maken van een eigen thema geldt de opzet
        if self.theme_draft.is_some() {
            return;
        }
        let palette = crate::theme::find(&self.own_themes, &self.cv.set.kleuren);
        let changed = palette != crate::theme::active();
        crate::theme::set_active(palette);
        let dark = match self.cv.set.thema {
            _ if palette.is_some() => palette.is_some_and(|p| p.is_dark()),
            Thema::Licht => false,
            Thema::Donker => true,
            Thema::Systeem => ctx.system_theme().map_or(self.cv.dark, |t| t == egui::Theme::Dark),
        };
        if changed || dark != self.cv.dark || ctx.theme() != if dark { egui::Theme::Dark } else { egui::Theme::Light } {
            self.set_dark(ctx, dark);
        }
    }

    /// Het venster om een eigen thema te maken: elke kleur werkt meteen, Opslaan bewaart het in de instellingenmap.
    pub(crate) fn theme_window(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.theme_draft.take() else { return };
        let mut open = true;
        let mut close = false;
        let dir = self.keymap.path.as_ref().and_then(|p| p.parent()).map(|d| d.to_path_buf());
        egui::Window::new(t("Eigen thema")).open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
            egui::Grid::new("thema-kleuren").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label(t("Naam"));
                ui.text_edit_singleline(&mut draft.name);
                ui.end_row();
                let p = &mut draft.colors;
                let rows: [(&str, &mut egui::Color32); 5] = [
                    ("Achtergrond", &mut p.bg),
                    ("Tekst", &mut p.fg),
                    ("Paneel", &mut p.panel),
                    ("Rand", &mut p.border),
                    ("Accent", &mut p.accent),
                ];
                for (label, color) in rows {
                    ui.label(t(label));
                    ui.color_edit_button_srgba(color);
                    ui.end_row();
                }
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(t("Opslaan")).clicked() {
                    if let Some(d) = &dir {
                        let name = draft.name.trim().to_string();
                        draft.name = if name.is_empty() { t("Mijn thema").to_string() } else { name };
                        match crate::theme::save_own(d, &draft) {
                            Ok(()) => {
                                self.own_themes = crate::theme::load_own(d);
                                self.cv.set.kleuren = draft.name.clone();
                                self.save_settings();
                                close = true;
                            }
                            Err(e) => self.status = format!("Kan {} niet opslaan: {e}", d.display()),
                        }
                    }
                }
                if ui.button(t("Exporteren…")).clicked() {
                    if let Some(p) = rfd::FileDialog::new().set_file_name(format!("{}.txt", draft.name)).add_filter("MathPrime theme", &["txt"]).save_file() {
                        self.status = match std::fs::write(&p, draft.to_text()) {
                            Ok(()) => format!("Opgeslagen: {}", p.display()),
                            Err(e) => format!("Kan {} niet opslaan: {e}", p.display()),
                        };
                    }
                }
                if ui.button(t("Importeren…")).clicked() {
                    if let Some(p) = rfd::FileDialog::new().add_filter("MathPrime theme", &["txt"]).pick_file() {
                        match std::fs::read_to_string(&p).ok().and_then(|text| crate::theme::Theme::parse(&text)) {
                            Some(th) => draft = th,
                            None => self.status = format!("{} is geen thema", p.display()),
                        }
                    }
                }
                let is_own = self.own_themes.iter().any(|o| o.name == draft.name);
                if is_own && ui.button(t("Verwijderen")).clicked() {
                    if let Some(d) = &dir {
                        let _ = crate::theme::delete_own(d, &draft.name);
                        self.own_themes = crate::theme::load_own(d);
                        if self.cv.set.kleuren == draft.name {
                            self.cv.set.kleuren.clear();
                            self.save_settings();
                        }
                        close = true;
                    }
                }
            });
        });
        if open && !close {
            // meteen zien wat het wordt
            if crate::theme::active() != Some(draft.colors) {
                crate::theme::set_active(Some(draft.colors));
                let dark = draft.colors.is_dark();
                self.set_dark(ctx, dark);
            }
            self.theme_draft = Some(draft);
        } else {
            // terug naar wat er gekozen is
            self.apply_theme(ctx);
        }
    }

    /// Het venster Instellingen. Een wijziging werkt meteen en wordt bewaard.
    pub(crate) fn settings_window(&mut self, ctx: &egui::Context) {
        let mut open = self.cv.show_settings;
        let before = self.cv.set.clone();
        let mut s = before.clone();
        egui::Window::new(t("Instellingen")).open(&mut open).resizable(false).collapsible(false).fade_in(false).anchor(egui::Align2::CENTER_TOP, [0.0, 90.0]).show(ctx, |ui| {
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
                row_label(ui, "Kleuren");
                ui.horizontal(|ui| {
                    let shown = if s.kleuren.is_empty() { t("Standaard").to_string() } else { s.kleuren.clone() };
                    egui::ComboBox::from_id_salt("kleuren").selected_text(shown).width(170.0).show_ui(ui, |ui| {
                        ui.selectable_value(&mut s.kleuren, String::new(), t("Standaard"));
                        for th in crate::theme::builtin() {
                            ui.selectable_value(&mut s.kleuren, th.name.clone(), th.name);
                        }
                        if !self.own_themes.is_empty() {
                            ui.separator();
                        }
                        for th in &self.own_themes {
                            ui.selectable_value(&mut s.kleuren, th.name.clone(), th.name.as_str());
                        }
                    });
                    if ui.button(t("Eigen thema…")).clicked() {
                        let base = crate::theme::active().unwrap_or(crate::theme::builtin()[0].colors);
                        let name = if s.kleuren.is_empty() { t("Mijn thema").to_string() } else { s.kleuren.clone() };
                        self.theme_draft = Some(crate::theme::Theme { name, colors: base });
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
                row_label(ui, "Titelbalk");
                ui.checkbox(&mut s.own_titlebar, t("in de kleuren van het thema (na herstarten)"));
                ui.end_row();
                row_label(ui, "Vsync");
                ui.checkbox(&mut s.vsync, t("wachten op het scherm (na herstarten)"))
                    .on_hover_text(t("Aan: geen scheuren in beeld. Uit: het venster volgt de muis direct bij verplaatsen."));
                ui.end_row();
            });

            section(ui, crate::icons::REFRESH, "Updates");
            egui::Grid::new("inst-updates").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| {
                row_label(ui, "Nieuwe versies");
                ui.checkbox(&mut s.updates, t("eens per dag kijken op GitHub"))
                    .on_hover_text(t("Vraagt alleen het nummer van de nieuwste versie op; er gaat niets van jou mee"));
                ui.end_row();
                row_label(ui, "Versie");
                ui.horizontal(|ui| {
                    ui.label(RichText::new(env!("CARGO_PKG_VERSION")).strong());
                    let busy = matches!(self.upd.phase, Phase::Checking | Phase::Downloading(_) | Phase::Installing);
                    if ui.add_enabled(!busy, egui::Button::new(t("Zoeken naar updates"))).clicked() {
                        self.upd.check(ctx);
                    }
                });
                ui.end_row();
                let status = match &self.upd.phase {
                    Phase::Idle => None,
                    Phase::Checking => Some(t("Zoeken…").to_string()),
                    Phase::Latest => Some(t("Je hebt de nieuwste versie.").to_string()),
                    Phase::Failed(e) => Some(format!("{}: {e}", t("Niet gelukt"))),
                    Phase::Newer(u) => Some(format!("{} {} {}", crate::taal::nl_en("Versie", "Version"), u.version, crate::taal::nl_en("is uit.", "is out."))),
                    Phase::Downloading(u) => Some(format!("{} {}…", t("Downloaden van versie"), u.version)),
                    Phase::Installing => Some(t("Installeren; MathPrime sluit en start opnieuw.").to_string()),
                };
                if let Some(text) = status {
                    ui.label("");
                    ui.vertical(|ui| {
                        ui.label(RichText::new(text).color(crate::icons::accent()));
                        if let Phase::Newer(u) = self.upd.phase.clone() {
                            ui.horizontal(|ui| {
                                if u.setup.is_some() && ui.button(t("Downloaden en installeren")).on_hover_text(t("Open werk wordt eerst gevraagd op te slaan")).clicked() {
                                    self.upd.download(ctx);
                                }
                                if ui.button(t("Wat is nieuw")).clicked() {
                                    ctx.open_url(egui::OpenUrl::new_tab(&u.url));
                                }
                            });
                        }
                    });
                    ui.end_row();
                }
                row_label(ui, "Over");
                ui.horizontal(|ui| {
                    ui.label(t("MIT-licentie ·"));
                    ui.hyperlink_to("GitHub", "https://github.com/Ankh-khun/mathprime");
                });
                ui.end_row();
            });
            ui.add_space(10.0);
            ui.label(RichText::new(t("Wijzigingen werken meteen. Sneltoetsen staan in keymap.txt (Help → Sneltoetsen).")).small().weak());
        });
        self.cv.show_settings = open;
        if s != before {
            let digits_changed = s.digits != before.digits;
            let language_changed = s.english != before.english;
            crate::taal::set_english(s.english);
            rb_units::i18n::set_english(s.english);
            self.cv.set = s;
            if language_changed {
                self.load_library();
                self.reload();
            }
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
        let s = Settings { thema: Thema::Systeem, background: Background::Lijnen, start_welcome: true, autocomplete: false, digits: 4, start_zoom: 2.25, updates: false, english: true, vsync: true, kleuren: "The Moon".into(), own_titlebar: false };
        assert_eq!(Settings::parse(&s.to_text()), s);
        // een oud bestand: alleen thema en achtergrond, de rest standaard
        let old = Settings::parse("# Instellingen van MathPrime\nachtergrond = ruitjes\nthema = donker\n");
        assert_eq!(old, Settings { thema: Thema::Donker, background: Background::Ruitjes, ..Settings::default() });
        // onzin wordt de standaard
        assert_eq!(Settings::parse("cijfers = veel\nzoom = x\nthema = paars"), Settings::default());
        // een BOM vooraan (Kladblok, PowerShell)
        assert!(Settings::parse("\u{feff}taal = en\n").english);
    }
}

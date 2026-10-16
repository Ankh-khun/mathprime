//! Installatieprogramma van MathPrime (`MathPrime-<versie>-setup.exe`).
//!
//! - zonder argumenten: een venster met welkom, installatiemap, voortgang en "MathPrime starten";
//! - `/Q` (of `--stil`): zonder venster installeren, voor scripts;
//! - `--wacht <pid>`: eerst wachten tot dat proces (MathPrime) gesloten is, dan meteen installeren; met
//!   `--starten` start MathPrime daarna weer (zo werkt "Downloaden en installeren" in MathPrime);
//! - `--doel <map>`: een andere installatiemap; `--alleen-bestanden`: alleen kopiëren (voor tests);
//! - `--pak <map> <uit.exe>`: maak een installatiepakket van dit programma en de bestanden in de map
//!   (zie `tools/maak-installer.ps1`).
//!
//! Installeren gebeurt voor de huidige gebruiker, zonder beheerdersrechten (zie `install.rs`).

// geen consolevenster op Windows in een release-build
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod install;
mod payload;

use eframe::egui::{self, Color32, RichText};
use install::{Job, Msg};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};

const ACCENT: Color32 = Color32::from_rgb(37, 99, 235);

struct Opts {
    quiet: bool,
    wait_for: Option<u32>,
    start_after: bool,
    target: Option<PathBuf>,
    /// Alleen de bestanden kopiëren (geen snelkoppelingen, register of PATH); voor tests.
    files_only: bool,
}

fn parse_args(args: &[String]) -> Opts {
    let mut o = Opts { quiet: false, wait_for: None, start_after: false, target: None, files_only: false };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.to_ascii_lowercase().as_str() {
            "/q" | "-q" | "--stil" | "--quiet" => o.quiet = true,
            "--wacht" | "--wait" => o.wait_for = it.next().and_then(|p| p.parse().ok()),
            "--starten" | "--start" => o.start_after = true,
            "--doel" | "--target" => o.target = it.next().map(PathBuf::from),
            "--alleen-bestanden" => o.files_only = true,
            _ => {}
        }
    }
    o
}

/// Engels, tenzij MathPrime op Nederlands staat of Windows Nederlands is.
fn english() -> bool {
    let settings = std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("MathPrime").join("instellingen.txt"));
    if let Some(text) = settings.and_then(|p| std::fs::read_to_string(p).ok()) {
        if let Some(line) = text.lines().find(|l| l.trim_start().starts_with("taal")) {
            return line.contains("en");
        }
    }
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        extern "system" {
            fn GetUserDefaultUILanguage() -> u16;
        }
        // primaire taal 0x13: Nederlands
        // SAFETY: een Win32-functie zonder argumenten die alleen een getal teruggeeft
        let lang = unsafe { GetUserDefaultUILanguage() };
        lang & 0x3ff != 0x13
    }
    #[cfg(not(windows))]
    true
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--pak") {
        let code = match (args.get(1), args.get(2)) {
            (Some(dir), Some(out)) => match std::env::current_exe()
                .and_then(std::fs::read)
                .map_err(|e| e.to_string())
                .and_then(|stub| payload::pack(&stub, std::path::Path::new(dir)))
                .and_then(|exe| std::fs::write(out, exe).map_err(|e| e.to_string()))
            {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("mathprime-setup --pak: {e}");
                    1
                }
            },
            _ => {
                eprintln!("gebruik: mathprime-setup --pak <map> <uit.exe>");
                2
            }
        };
        std::process::exit(code);
    }
    let opts = parse_args(&args);
    let en = english();
    let files = match std::env::current_exe().and_then(std::fs::read).map(|b| payload::files(&b)) {
        Ok(Some(Ok(files))) => Ok(files),
        Ok(Some(Err(e))) => Err(e),
        Ok(None) => Err(if en { "This program contains no MathPrime package." } else { "Dit programma bevat geen MathPrime-pakket." }.to_string()),
        Err(e) => Err(e.to_string()),
    };
    if opts.quiet {
        let code = match files {
            Ok(files) => {
                let target = opts.target.clone().unwrap_or_else(install::default_target);
                let job = Job { files, target: target.clone(), desktop: true, wait_for: opts.wait_for, english: en, files_only: opts.files_only };
                let failed = std::cell::Cell::new(false);
                install::run(job, &|m| match m {
                    Msg::Failed(e) => {
                        eprintln!("{e}");
                        failed.set(true);
                    }
                    Msg::Done(t) if opts.start_after => install::launch(&t),
                    _ => {}
                });
                i32::from(failed.get())
            }
            Err(e) => {
                eprintln!("{e}");
                1
            }
        };
        std::process::exit(code);
    }
    let version = files
        .as_ref()
        .ok()
        .and_then(|f| f.iter().find(|(n, _)| n == "versie.txt"))
        .map(|(_, d)| String::from_utf8_lossy(d).trim().to_string())
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([560.0, 400.0])
            .with_resizable(false)
            .with_maximize_button(false)
            .with_title(if en { format!("Install MathPrime {version}") } else { format!("MathPrime {version} installeren") })
            .with_icon(std::sync::Arc::new(egui::IconData {
                rgba: include_bytes!("../../rb-gui/assets/mathprime-128.rgba").to_vec(),
                width: 128,
                height: 128,
            })),
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        "MathPrime-setup",
        options,
        Box::new(move |cc| Ok(Box::new(Setup::new(cc, files, version, opts, en)))),
    )
}

enum Page {
    Welcome,
    Busy,
    Done,
    Failed(String),
}

struct Setup {
    files: Result<Vec<(String, Vec<u8>)>, String>,
    version: String,
    target: String,
    desktop: bool,
    start_now: bool,
    page: Page,
    step: String,
    progress: f32,
    rx: Option<Receiver<Msg>>,
    installed: Option<PathBuf>,
    auto: bool,
    start_after: bool,
    wait_for: Option<u32>,
    en: bool,
    logo: Option<egui::TextureHandle>,
}

impl Setup {
    fn new(cc: &eframe::CreationContext<'_>, files: Result<Vec<(String, Vec<u8>)>, String>, version: String, opts: Opts, en: bool) -> Setup {
        cc.egui_ctx.set_theme(egui::Theme::Light);
        cc.egui_ctx.style_mut_of(egui::Theme::Light, |s| {
            s.interaction.selectable_labels = false;
            s.visuals.selection.bg_fill = ACCENT.gamma_multiply(0.25);
            s.visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT);
            s.spacing.button_padding = egui::vec2(14.0, 6.0);
            // tekstvakken lichtgrijs, zodat ze op de witte achtergrond te zien zijn
            s.visuals.extreme_bg_color = Color32::from_gray(243);
        });
        let rgba = include_bytes!("../../rb-gui/assets/mathprime-128.rgba");
        let logo = cc.egui_ctx.load_texture("logo", egui::ColorImage::from_rgba_unmultiplied([128, 128], rgba), egui::TextureOptions::LINEAR);
        let page = match &files {
            Err(e) => Page::Failed(e.clone()),
            Ok(_) => Page::Welcome,
        };
        let mut s = Setup {
            files,
            version,
            target: opts.target.clone().unwrap_or_else(install::default_target).to_string_lossy().into_owned(),
            desktop: true,
            start_now: true,
            page,
            step: String::new(),
            progress: 0.0,
            rx: None,
            installed: None,
            auto: opts.wait_for.is_some(),
            start_after: opts.start_after,
            wait_for: opts.wait_for,
            en,
            logo: Some(logo),
        };
        // bijwerken vanuit MathPrime: meteen beginnen
        if s.auto && matches!(s.page, Page::Welcome) {
            s.begin(&cc.egui_ctx);
        }
        s
    }

    fn tr<'a>(&self, nl: &'a str, en: &'a str) -> &'a str {
        if self.en {
            en
        } else {
            nl
        }
    }

    fn begin(&mut self, ctx: &egui::Context) {
        let Ok(files) = &self.files else { return };
        let job = Job {
            files: files.clone(),
            target: PathBuf::from(self.target.trim()),
            desktop: self.desktop,
            wait_for: self.wait_for.take(),
            english: self.en,
            files_only: false,
        };
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            install::run(job, &|m| {
                let _ = tx.send(m);
                ctx.request_repaint();
            })
        });
        self.rx = Some(rx);
        self.page = Page::Busy;
        self.progress = 0.0;
        self.step = self.tr("Beginnen…", "Starting…").to_string();
    }

    fn poll(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.rx else { return };
        while let Ok(m) = rx.try_recv() {
            match m {
                Msg::Step(t, p) => {
                    self.step = t;
                    self.progress = p;
                }
                Msg::Done(t) => {
                    self.installed = Some(t.clone());
                    self.page = Page::Done;
                    if self.auto {
                        if self.start_after {
                            install::launch(&t);
                        }
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
                Msg::Failed(e) => self.page = Page::Failed(e),
            }
        }
    }

    fn header(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if let Some(logo) = &self.logo {
                ui.add(egui::Image::new(logo).fit_to_exact_size(egui::vec2(56.0, 56.0)));
            }
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.add_space(6.0);
                ui.label(RichText::new(format!("MathPrime {}", self.version)).size(24.0).strong());
                ui.label(RichText::new(self.tr("Rekenblad met eenheden", "Worksheet with units")).size(14.0).weak());
            });
        });
        ui.add_space(14.0);
        ui.separator();
        ui.add_space(10.0);
    }
}

impl eframe::App for Setup {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = &ui.ctx().clone();
        self.poll(ctx);
        egui::CentralPanel::default().frame(egui::Frame::new().fill(Color32::WHITE).inner_margin(24.0)).show(ui, |ui| {
            self.header(ui);
            let bottom = 40.0;
            let body = ui.available_height() - bottom;
            ui.allocate_ui(egui::vec2(ui.available_width(), body), |ui| match &self.page {
                Page::Welcome => {
                    ui.label(RichText::new(self.tr(
                        "MathPrime wordt geïnstalleerd voor jou alleen; er zijn geen beheerdersrechten nodig. Je werkbladen en instellingen blijven staan.",
                        "MathPrime will be installed for you only; no administrator rights are needed. Your worksheets and settings are kept.",
                    )).size(14.0));
                    ui.add_space(14.0);
                    ui.label(RichText::new(self.tr("Installatiemap", "Install folder")).strong());
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.target).desired_width(390.0));
                        if ui.button(self.tr("Bladeren…", "Browse…")).clicked() {
                            if let Some(p) = rfd::FileDialog::new().set_directory(&self.target).pick_folder() {
                                self.target = p.join("MathPrime").to_string_lossy().into_owned();
                            }
                        }
                    });
                    ui.add_space(10.0);
                    let label = self.tr("Snelkoppeling op het bureaublad", "Desktop shortcut").to_string();
                    ui.checkbox(&mut self.desktop, label);
                    ui.add_space(4.0);
                    ui.label(RichText::new(self.tr(
                        "Ook: Startmenu, .rkn-bestanden openen met MathPrime, en verwijderen via Instellingen > Apps.",
                        "Also: Start menu, open .rkn files with MathPrime, and uninstall via Settings > Apps.",
                    )).small().weak());
                }
                Page::Busy => {
                    ui.add_space(20.0);
                    ui.label(RichText::new(&self.step).size(14.0));
                    ui.add_space(10.0);
                    ui.add(egui::ProgressBar::new(self.progress).fill(ACCENT).animate(true));
                }
                Page::Done => {
                    ui.add_space(10.0);
                    ui.label(RichText::new(self.tr("MathPrime is geïnstalleerd.", "MathPrime is installed.")).size(18.0).color(Color32::from_rgb(0, 120, 60)));
                    ui.add_space(8.0);
                    ui.label(RichText::new(self.tr(
                        "Je vindt het in het Startmenu; .rkn-bestanden openen met een dubbelklik.",
                        "You will find it in the Start menu; .rkn files open with a double-click.",
                    )).size(14.0));
                    ui.add_space(14.0);
                    let label = self.tr("MathPrime nu starten", "Start MathPrime now").to_string();
                    ui.checkbox(&mut self.start_now, label);
                }
                Page::Failed(e) => {
                    ui.add_space(10.0);
                    ui.label(RichText::new(self.tr("De installatie is niet gelukt.", "The installation did not succeed.")).size(18.0).color(Color32::from_rgb(190, 30, 30)));
                    ui.add_space(8.0);
                    egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| ui.label(RichText::new(e.as_str()).size(13.0)));
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Max), |ui| {
                let primary = |text: &str| egui::Button::new(RichText::new(text).color(Color32::WHITE).strong()).fill(ACCENT);
                match &self.page {
                    Page::Welcome => {
                        if ui.add(primary(self.tr("Installeren", "Install"))).clicked() {
                            self.begin(ctx);
                        }
                        if ui.button(self.tr("Annuleren", "Cancel")).clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                    Page::Busy => {
                        ui.add_enabled(false, egui::Button::new(self.tr("Bezig…", "Working…")));
                    }
                    Page::Done => {
                        if ui.add(primary(self.tr("Voltooien", "Finish"))).clicked() {
                            if self.start_now {
                                if let Some(t) = &self.installed {
                                    install::launch(t);
                                }
                            }
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                    Page::Failed(_) => {
                        if ui.button(self.tr("Sluiten", "Close")).clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if self.files.is_ok() && ui.add(primary(self.tr("Opnieuw proberen", "Try again"))).clicked() {
                            self.begin(ctx);
                        }
                    }
                }
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argumenten() {
        let a: Vec<String> = ["/Q", "--wacht", "1234", "--starten", "--doel", r"D:\Apps\MathPrime"].iter().map(|s| s.to_string()).collect();
        let o = parse_args(&a);
        assert!(o.quiet && o.start_after);
        assert_eq!(o.wait_for, Some(1234));
        assert_eq!(o.target, Some(PathBuf::from(r"D:\Apps\MathPrime")));
        let o = parse_args(&[]);
        assert!(!o.quiet && o.wait_for.is_none());
    }
}

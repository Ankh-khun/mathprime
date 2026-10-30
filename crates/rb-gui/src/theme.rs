//! Kleurthema's: een palet van vijf kleuren (achtergrond, tekst, paneel, rand, accent); de rest wordt daaruit
//! afgeleid. De ingebouwde thema's zijn naar de grote arcana genoemd; eigen thema's staan als tekstbestand in de map
//! `themas` in de instellingenmap, en zijn zo ook te delen. Zonder thema blijft het gewone licht of donker. De PDF
//! blijft altijd zwart op wit.

use eframe::egui::{self, Color32, Stroke};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub bg: Color32,
    pub fg: Color32,
    pub panel: Color32,
    pub border: Color32,
    pub accent: Color32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub name: String,
    pub colors: Palette,
}

const fn c(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// De ingebouwde thema's: naam, achtergrond, tekst, paneel, rand, accent.
const BUILTIN: &[(&str, Palette)] = &[
    ("The Magician", Palette { bg: c(27, 20, 32), fg: c(242, 232, 213), panel: c(36, 26, 43), border: c(74, 58, 85), accent: c(224, 169, 59) }),
    ("The High Priestess", Palette { bg: c(14, 23, 38), fg: c(219, 228, 242), panel: c(20, 32, 51), border: c(47, 69, 99), accent: c(122, 168, 232) }),
    ("The Empress", Palette { bg: c(241, 240, 228), fg: c(43, 58, 42), panel: c(251, 250, 242), border: c(207, 210, 184), accent: c(78, 138, 60) }),
    ("The Emperor", Palette { bg: c(31, 20, 18), fg: c(240, 220, 210), panel: c(42, 27, 24), border: c(87, 53, 45), accent: c(217, 83, 59) }),
    ("The Hermit", Palette { bg: c(21, 23, 26), fg: c(217, 214, 204), panel: c(28, 31, 35), border: c(58, 63, 69), accent: c(201, 162, 39) }),
    ("Justice", Palette { bg: c(236, 238, 242), fg: c(30, 36, 48), panel: c(250, 251, 252), border: c(197, 203, 214), accent: c(122, 79, 191) }),
    ("The Star", Palette { bg: c(11, 16, 48), fg: c(230, 233, 255), panel: c(19, 26, 69), border: c(47, 58, 122), accent: c(143, 183, 255) }),
    ("The Moon", Palette { bg: c(18, 16, 31), fg: c(216, 211, 238), panel: c(26, 23, 48), border: c(59, 53, 96), accent: c(168, 139, 239) }),
    ("The Sun", Palette { bg: c(253, 244, 220), fg: c(58, 42, 16), panel: c(255, 250, 240), border: c(232, 212, 160), accent: c(224, 138, 18) }),
];

static ACTIVE: Mutex<Option<Palette>> = Mutex::new(None);

/// Het actieve palet; `None`: het gewone licht of donker.
pub fn active() -> Option<Palette> {
    *ACTIVE.lock().expect("thema")
}

pub fn set_active(p: Option<Palette>) {
    *ACTIVE.lock().expect("thema") = p;
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

fn luminance(c: Color32) -> f32 {
    (0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32) / 255.0
}

impl Palette {
    pub fn is_dark(&self) -> bool {
        luminance(self.bg) < 0.45
    }

    /// De kleur van de pagina: iets lichter (of bij een licht thema: bijna wit) dan de achtergrond.
    pub fn paper(&self) -> Color32 {
        if self.is_dark() {
            mix(self.bg, self.fg, 0.05)
        } else {
            mix(self.panel, Color32::WHITE, 0.6)
        }
    }

    /// De stijl van de vensters en knoppen.
    pub fn apply(&self, v: &mut egui::Visuals) {
        let hover = |t: f32| mix(self.panel, self.fg, t);
        v.dark_mode = self.is_dark();
        v.override_text_color = Some(self.fg);
        v.weak_text_color = Some(mix(self.fg, self.panel, 0.4));
        v.panel_fill = self.panel;
        v.window_fill = self.panel;
        v.window_stroke = Stroke::new(1.0, self.border);
        v.extreme_bg_color = mix(self.bg, self.panel, 0.3);
        v.faint_bg_color = hover(0.04);
        v.code_bg_color = hover(0.06);
        v.hyperlink_color = self.accent;
        v.selection.bg_fill = mix(self.panel, self.accent, 0.45);
        v.selection.stroke = Stroke::new(1.0, self.accent);
        let w = &mut v.widgets;
        w.noninteractive.bg_fill = self.panel;
        w.noninteractive.weak_bg_fill = self.panel;
        w.noninteractive.bg_stroke = Stroke::new(1.0, self.border);
        w.noninteractive.fg_stroke = Stroke::new(1.0, self.fg);
        for (s, t) in [(&mut w.inactive, 0.07), (&mut w.hovered, 0.14), (&mut w.active, 0.2), (&mut w.open, 0.14)] {
            s.bg_fill = hover(t);
            s.weak_bg_fill = hover(t);
            s.fg_stroke = Stroke::new(1.0, self.fg);
        }
        w.inactive.bg_stroke = Stroke::NONE;
        w.hovered.bg_stroke = Stroke::new(1.0, self.border);
        w.active.bg_stroke = Stroke::new(1.0, self.accent);
    }
}

pub fn builtin() -> Vec<Theme> {
    BUILTIN.iter().map(|(n, p)| Theme { name: n.to_string(), colors: *p }).collect()
}

fn hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

fn parse_hex(s: &str) -> Option<Color32> {
    let s = s.trim().trim_start_matches('#');
    let d = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok();
    match s.len() {
        6 => Some(c(d(0)?, d(2)?, d(4)?)),
        _ => None,
    }
}

impl Theme {
    /// Het bestandsformaat: `naam = waarde`, per regel.
    pub fn to_text(&self) -> String {
        let p = &self.colors;
        format!(
            "# MathPrime theme\nname = {}\nbg = {}\nfg = {}\npanel = {}\nborder = {}\naccent = {}\n",
            self.name,
            hex(p.bg),
            hex(p.fg),
            hex(p.panel),
            hex(p.border),
            hex(p.accent)
        )
    }

    /// Lees een thema; ontbreekt er iets of klopt een kleur niet, dan `None`.
    pub fn parse(text: &str) -> Option<Theme> {
        let get = |key: &str| {
            text.trim_start_matches('\u{feff}').lines().find_map(|l| {
                let (k, v) = l.split_once('=')?;
                (k.trim() == key).then(|| v.trim().to_string())
            })
        };
        let col = |key: &str| get(key).and_then(|v| parse_hex(&v));
        let name = get("name").filter(|n| !n.is_empty())?;
        Some(Theme {
            name,
            colors: Palette { bg: col("bg")?, fg: col("fg")?, panel: col("panel")?, border: col("border")?, accent: col("accent")? },
        })
    }
}

fn dir_in(config: &Path) -> PathBuf {
    config.join("themas")
}

/// Een veilige bestandsnaam voor een thema.
fn file_name(name: &str) -> String {
    let safe: String = name.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' }).collect();
    format!("{}.txt", safe.trim())
}

/// De eigen thema's uit de instellingenmap, op naam gesorteerd.
pub fn load_own(config: &Path) -> Vec<Theme> {
    let mut out: Vec<Theme> = std::fs::read_dir(dir_in(config))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| Theme::parse(&std::fs::read_to_string(e.path()).ok()?))
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

pub fn save_own(config: &Path, t: &Theme) -> std::io::Result<()> {
    std::fs::create_dir_all(dir_in(config))?;
    std::fs::write(dir_in(config).join(file_name(&t.name)), t.to_text())
}

pub fn delete_own(config: &Path, name: &str) -> std::io::Result<()> {
    std::fs::remove_file(dir_in(config).join(file_name(name)))
}

/// Zoek een thema op naam: eerst de eigen, dan de ingebouwde.
pub fn find(own: &[Theme], name: &str) -> Option<Palette> {
    own.iter().find(|t| t.name == name).map(|t| t.colors).or_else(|| BUILTIN.iter().find(|(n, _)| *n == name).map(|(_, p)| *p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heen_en_terug() {
        for t in builtin() {
            assert_eq!(Theme::parse(&t.to_text()).as_ref(), Some(&t), "{}", t.name);
        }
        assert!(Theme::parse("name = x\nbg = #12\n").is_none());
        assert!(Theme::parse("bg = #000000").is_none());
        // een BOM vooraan (Kladblok)
        let t = builtin().remove(0);
        assert_eq!(Theme::parse(&format!("\u{feff}{}", t.to_text())), Some(t));
    }

    #[test]
    fn leesbaar() {
        // de tekst moet op elk ingebouwd thema goed af te zien zijn (WCAG: contrast minstens 4,5)
        let lin = |v: u8| {
            let s = v as f32 / 255.0;
            if s <= 0.03928 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
        };
        let l = |c: Color32| 0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b());
        let contrast = |a: Color32, b: Color32| {
            let (x, y) = (l(a).max(l(b)), l(a).min(l(b)));
            (x + 0.05) / (y + 0.05)
        };
        for t in builtin() {
            let p = t.colors;
            for (what, back) in [("achtergrond", p.bg), ("paneel", p.panel), ("pagina", p.paper())] {
                assert!(contrast(p.fg, back) >= 4.5, "{} op {what}: {}", t.name, contrast(p.fg, back));
            }
            assert!(contrast(p.accent, p.panel) >= 2.0, "accent {}", t.name);
        }
    }

    #[test]
    fn bewaren_en_lezen() {
        let dir = std::env::temp_dir().join(format!("mathprime-thema-test-{}", std::process::id()));
        let t = Theme { name: "Mijn/thema".into(), colors: builtin()[0].colors };
        save_own(&dir, &t).unwrap();
        let own = load_own(&dir);
        assert_eq!(own, vec![t.clone()]);
        assert_eq!(find(&own, "Mijn/thema"), Some(t.colors));
        assert!(find(&own, "The Sun").is_some() && find(&own, "bestaat niet").is_none());
        delete_own(&dir, &t.name).unwrap();
        assert!(load_own(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

//! Schermafdruk voor tests: met `MATHPRIME_SCREENSHOT=pad.png` maakt de app na het opstarten een
//! schermafdruk van het venster, schrijft die als PNG en sluit af.

use eframe::egui::{self, ColorImage, ViewportCommand};

fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (n, t) in table.iter_mut().enumerate() {
        let mut c = n as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *t = c;
    }
    let mut c = 0xFFFF_FFFFu32;
    for b in data {
        c = table[((c ^ *b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

/// Een minimale PNG-schrijver (RGBA, 8 bit, zonder filters).
pub fn png(img: &ColorImage) -> Vec<u8> {
    let [w, h] = img.size;
    let mut raw = Vec::with_capacity(h * (1 + 4 * w));
    for y in 0..h {
        raw.push(0);
        for x in 0..w {
            raw.extend_from_slice(&img.pixels[y * w + x].to_array());
        }
    }
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut chunk = |kind: &[u8], data: &[u8]| {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(w as u32).to_be_bytes());
    ihdr.extend_from_slice(&(h as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(b"IHDR", &ihdr);
    chunk(b"IDAT", &miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6));
    chunk(b"IEND", &[]);
    out
}

/// Zet een toetsenscript (zie [`Shot::inject`]) om in invoer voor egui.
pub fn script_events(script: &str) -> Vec<egui::Event> {
    use egui::{Event, Key, Modifiers};
    let mut out = Vec::new();
    let mut mods = Modifiers::NONE;
    let key = |key: Key, modifiers: Modifiers| Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers };
    let button = |pos: egui::Pos2, pressed: bool| Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    let mut chars = script.chars().peekable();
    // `x,y;` na ⊙ (klik) of ⇲ (slepen, twee punten)
    let point = |chars: &mut std::iter::Peekable<std::str::Chars>| {
        let s: String = chars.by_ref().take_while(|c| *c != ';').collect();
        let (x, y) = s.split_once(',').unwrap_or(("0", "0"));
        egui::pos2(x.trim().parse().unwrap_or(0.0), y.trim().parse().unwrap_or(0.0))
    };
    while let Some(c) = chars.next() {
        if c == '⊙' {
            let p = point(&mut chars);
            out.extend([Event::PointerMoved(p), button(p, true), button(p, false)]);
            continue;
        }
        // één frame wachten (bijvoorbeeld tot een menu open is)
        if c == '⏸' {
            out.push(Event::Text(String::new()));
            continue;
        }
        if c == '⇲' {
            let (a, b) = (point(&mut chars), point(&mut chars));
            out.extend([Event::PointerMoved(a), button(a, true)]);
            out.extend((1..=6).map(|k| Event::PointerMoved(a + (b - a) * (k as f32 / 6.0))));
            out.push(button(b, false));
            continue;
        }
        let special = match c {
            '⌃' => {
                mods.ctrl = true;
                mods.command = true;
                continue;
            }
            '⇧' => {
                mods.shift = true;
                continue;
            }
            '⌥' => {
                mods.alt = true;
                continue;
            }
            '⏎' => Some(Key::Enter),
            '⎋' => Some(Key::Escape),
            '⌫' => Some(Key::Backspace),
            '→' => Some(Key::ArrowRight),
            '←' => Some(Key::ArrowLeft),
            '↑' => Some(Key::ArrowUp),
            '↓' => Some(Key::ArrowDown),
            '⇤' => Some(Key::Home),
            '⇥' => Some(Key::End),
            '↹' => Some(Key::Tab),
            _ => None,
        };
        if let Some(k) = special {
            out.push(key(k, mods));
        } else if mods.ctrl {
            let named = match c {
                '\'' => Some(Key::Quote),
                ']' => Some(Key::CloseBracket),
                '[' => Some(Key::OpenBracket),
                ';' => Some(Key::Semicolon),
                '\\' => Some(Key::Backslash),
                '=' => Some(Key::Equals),
                '-' => Some(Key::Minus),
                '/' => Some(Key::Slash),
                '.' => Some(Key::Period),
                ',' => Some(Key::Comma),
                '`' => Some(Key::Backtick),
                _ => Key::from_name(&c.to_ascii_uppercase().to_string()),
            };
            if let Some(k) = named {
                out.push(key(k, mods));
            }
        } else {
            out.push(Event::Text(c.to_string()));
        }
        mods = Modifiers::NONE;
    }
    out
}

/// Houdt bij hoeveel frames er zijn getekend en vraagt daarna om een schermafdruk.
pub struct Shot {
    pub path: std::path::PathBuf,
    frames: u32,
    requested: bool,
    /// Toetsen die nog afgespeeld moeten worden (één per frame).
    keys: std::collections::VecDeque<egui::Event>,
}

impl Shot {
    pub fn from_env() -> Option<Shot> {
        std::env::var_os("MATHPRIME_SCREENSHOT").map(|p| Shot {
            path: p.into(),
            frames: 0,
            requested: false,
            keys: std::env::var("MATHPRIME_KEYS").map(|s| script_events(&s).into()).unwrap_or_default(),
        })
    }

    /// Aanroepen aan het begin van elk frame: speelt vanaf het tweede frame de toetsen uit
    /// `MATHPRIME_KEYS` af, één per frame, als getypte tekst en toetsen. `⏎` Enter, `⎋` Esc,
    /// `⌫` Backspace, `→←↑↓` pijltjes, `⌃` Ctrl, `⇧` Shift en `⌥` Alt voor de volgende toets (`⌃⇧D`).
    pub fn inject(&mut self, raw: &mut egui::RawInput) {
        if self.frames < 2 {
            return;
        }
        if let Some(ev) = self.keys.pop_front() {
            raw.events.push(ev);
        }
    }

    /// Aanroepen aan het eind van elk frame.
    pub fn tick(&mut self, ctx: &egui::Context) {
        self.frames += 1;
        let image = ctx.input(|i| {
            i.raw.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(img) = image {
            let _ = std::fs::write(&self.path, png(&img));
            ctx.send_viewport_cmd(ViewportCommand::Close);
            return;
        }
        // een paar frames wachten, zodat de layout en de grafieken klaar zijn
        if self.frames >= 8 && self.keys.is_empty() && !self.requested {
            self.requested = true;
            ctx.send_viewport_cmd(ViewportCommand::Screenshot(Default::default()));
        }
        ctx.request_repaint();
    }
}

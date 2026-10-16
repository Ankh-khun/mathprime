//! PDF-uitvoer, zelf geschreven (PDF 1.7, ISO 32000-1).
//!
//! Beide lettertypen worden volledig ingebed als TrueType (Type0/CIDFontType2 met Identity-H), met
//! een ToUnicode-tabel zodat tekst te zoeken en te kopiëren is. Streams worden met deflate
//! gecomprimeerd.

use crate::doc::{Color, Op, Page};
use crate::font::{fonts, FontId};
use std::collections::BTreeMap;
use std::fmt::Write;

fn deflate(data: &[u8]) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec_zlib(data, 6)
}

fn n(v: f32) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".to_string() } else { s.to_string() }
}

fn pdf_string(s: &str) -> String {
    // PDF-tekststring in UTF-16BE met BOM, als hex
    let mut out = String::from("<FEFF");
    for u in s.encode_utf16() {
        let _ = write!(out, "{u:04X}");
    }
    out.push('>');
    out
}

struct Writer {
    buf: Vec<u8>,
    offsets: Vec<usize>,
}

impl Writer {
    /// Reserveer een objectnummer (vanaf 1).
    fn reserve(&mut self) -> usize {
        self.offsets.push(0);
        self.offsets.len()
    }

    fn obj(&mut self, id: usize, body: &str) {
        self.offsets[id - 1] = self.buf.len();
        self.buf.extend_from_slice(format!("{id} 0 obj\n{body}\nendobj\n").as_bytes());
    }

    fn stream(&mut self, id: usize, dict: &str, data: &[u8]) {
        let packed = deflate(data);
        self.offsets[id - 1] = self.buf.len();
        self.buf.extend_from_slice(format!("{id} 0 obj\n<< {dict} /Length {} /Filter /FlateDecode >>\nstream\n", packed.len()).as_bytes());
        self.buf.extend_from_slice(&packed);
        self.buf.extend_from_slice(b"\nendstream\nendobj\n");
    }
}

/// Gebruikte glyphs per lettertype: glyph-id → teken.
type Used = [BTreeMap<u16, char>; 2];

fn font_index(id: FontId) -> usize {
    match id {
        FontId::Main => 0,
        FontId::Mono => 1,
    }
}

fn rg(c: Color) -> String {
    format!("{} {} {}", n(c.0 as f32 / 255.0), n(c.1 as f32 / 255.0), n(c.2 as f32 / 255.0))
}

/// Afbeeldingen in het document: (kenmerk, PNG-inhoud), in volgorde van gebruik (`/Im0`, ...).
type Images = Vec<(u64, std::sync::Arc<Vec<u8>>)>;

/// De inhoud van één pagina als PDF-operatoren.
fn content(page: &Page, used: &mut Used, images: &mut Images) -> String {
    let h = page.height;
    let fs = fonts();
    let mut s = String::from("1 J 1 j\n");
    for o in &page.ops {
        match o {
            Op::Text { x, y, size, text, font, color } => {
                let mut cx = *x;
                for (id, run) in fs.runs(text, *font) {
                    let f = fs.get(id);
                    let mut hex = String::new();
                    for c in run.chars() {
                        let gid = f.glyph(c).unwrap_or(0);
                        used[font_index(id)].insert(gid, c);
                        let _ = write!(hex, "{gid:04X}");
                    }
                    let _ = writeln!(
                        s,
                        "BT {} rg /F{} {} Tf 1 0 0 1 {} {} Tm <{hex}> Tj ET",
                        rg(*color),
                        font_index(id) + 1,
                        n(*size),
                        n(cx),
                        n(h - y)
                    );
                    cx += fs.width(&run, *size, id);
                }
            }
            Op::Line { x1, y1, x2, y2, width, color } => {
                let _ = writeln!(s, "{} RG {} w {} {} m {} {} l S", rg(*color), n(*width), n(*x1), n(h - y1), n(*x2), n(h - y2));
            }
            Op::Poly { points, width, color } => {
                if points.len() < 2 {
                    continue;
                }
                let _ = write!(s, "{} RG {} w {} {} m", rg(*color), n(*width), n(points[0].0), n(h - points[0].1));
                for (x, y) in &points[1..] {
                    let _ = write!(s, " {} {} l", n(*x), n(h - y));
                }
                s.push_str(" S\n");
            }
            Op::Polygon { points, fill, width, color } => {
                if points.len() < 3 {
                    continue;
                }
                let _ = write!(s, "{} rg {} RG {} w {} {} m", rg(*fill), rg(*color), n(*width), n(points[0].0), n(h - points[0].1));
                for (x, y) in &points[1..] {
                    let _ = write!(s, " {} {} l", n(*x), n(h - y));
                }
                // vullen, en de rand alleen als die er is
                s.push_str(if *width > 0.0 { " h B\n" } else { " h f\n" });
            }
            Op::Fill { x, y, w, h: rh, color } => {
                let _ = writeln!(s, "{} rg {} {} {} {} re f", rg(*color), n(*x), n(h - y - rh), n(*w), n(*rh));
            }
            Op::Rect { x, y, w, h: rh, width, color } => {
                let _ = writeln!(s, "{} RG {} w {} {} {} {} re S", rg(*color), n(*width), n(*x), n(h - y - rh), n(*w), n(*rh));
            }
            Op::Image { x, y, w, h: ih, png, hash } => {
                let k = match images.iter().position(|(hh, _)| hh == hash) {
                    Some(k) => k,
                    None => {
                        images.push((*hash, png.clone()));
                        images.len() - 1
                    }
                };
                let _ = writeln!(s, "q {} 0 0 {} {} {} cm /Im{k} Do Q", n(*w), n(*ih), n(*x), n(h - y - ih));
            }
        }
    }
    s
}

fn to_unicode(used: &BTreeMap<u16, char>) -> String {
    let mut s = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n\
         /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
         /CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n\
         1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    let entries: Vec<(&u16, &char)> = used.iter().collect();
    for chunk in entries.chunks(100) {
        let _ = writeln!(s, "{} beginbfchar", chunk.len());
        for (gid, c) in chunk {
            let mut u = String::new();
            let mut buf = [0u16; 2];
            for unit in c.encode_utf16(&mut buf) {
                let _ = write!(u, "{unit:04X}");
            }
            let _ = writeln!(s, "<{gid:04X}> <{u}>");
        }
        s.push_str("endbfchar\n");
    }
    s.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    s
}

/// Schrijf de lettertypeobjecten; geeft het objectnummer van het Type0-lettertype.
fn write_font(w: &mut Writer, id: FontId, used: &BTreeMap<u16, char>) -> usize {
    let f = fonts().get(id);
    let (t0, cid, desc, file, tu) = (w.reserve(), w.reserve(), w.reserve(), w.reserve(), w.reserve());
    let scale = 1000.0 / f.units();
    let bb = f.face.global_bounding_box();
    let mut widths = String::new();
    for gid in used.keys() {
        let _ = write!(widths, "{gid} [{}] ", n(f.advance(*gid) * 1000.0));
    }
    w.obj(
        t0,
        &format!(
            "<< /Type /Font /Subtype /Type0 /BaseFont /{} /Encoding /Identity-H /DescendantFonts [{cid} 0 R] /ToUnicode {tu} 0 R >>",
            f.name
        ),
    );
    w.obj(
        cid,
        &format!(
            "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /{} /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> \
             /FontDescriptor {desc} 0 R /DW 500 /W [{widths}] /CIDToGIDMap /Identity >>",
            f.name
        ),
    );
    w.obj(
        desc,
        &format!(
            "<< /Type /FontDescriptor /FontName /{} /Flags {} /FontBBox [{} {} {} {}] /ItalicAngle 0 /Ascent {} /Descent {} /CapHeight {} /StemV 80 /FontFile2 {file} 0 R >>",
            f.name,
            if id == FontId::Mono { 33 } else { 32 },
            n(bb.x_min as f32 * scale),
            n(bb.y_min as f32 * scale),
            n(bb.x_max as f32 * scale),
            n(bb.y_max as f32 * scale),
            n(f.face.ascender() as f32 * scale),
            n(f.face.descender() as f32 * scale),
            n(f.face.capital_height().unwrap_or(700) as f32 * scale),
        ),
    );
    w.stream(file, &format!("/Length1 {}", f.data.len()), f.data);
    w.stream(tu, "", to_unicode(used).as_bytes());
    t0
}

/// Zet pagina's om in een PDF-bestand.
pub fn to_pdf(pages: &[Page], title: &str) -> Vec<u8> {
    let mut w = Writer {
        buf: b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec(),
        offsets: Vec::new(),
    };
    let (catalog, pages_id, info) = (w.reserve(), w.reserve(), w.reserve());
    let mut used: Used = [BTreeMap::new(), BTreeMap::new()];
    let mut images: Images = Vec::new();
    let contents: Vec<String> = pages.iter().map(|p| content(p, &mut used, &mut images)).collect();
    // altijd glyph 0 en de spatie, zodat beide lettertypen geldig zijn
    for (k, u) in used.iter_mut().enumerate() {
        let f = fonts().get(if k == 0 { FontId::Main } else { FontId::Mono });
        u.entry(f.glyph(' ').unwrap_or(0)).or_insert(' ');
    }
    let f1 = write_font(&mut w, FontId::Main, &used[0]);
    let f2 = write_font(&mut w, FontId::Mono, &used[1]);
    // afbeeldingen: RGB met (als er doorzichtige pixels zijn) een zachte masker voor alfa
    let mut xobjects = String::new();
    for (k, (_, png)) in images.iter().enumerate() {
        let id = w.reserve();
        let img = crate::png::decode(png).unwrap_or(crate::png::Rgba { width: 1, height: 1, pixels: vec![255, 255, 255, 0] });
        let rgb: Vec<u8> = img.pixels.chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
        let alpha: Vec<u8> = img.pixels.chunks(4).map(|p| p[3]).collect();
        let mut dict = format!("/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8", img.width, img.height);
        if alpha.iter().any(|a| *a < 255) {
            let mask = w.reserve();
            w.stream(mask, &format!("/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceGray /BitsPerComponent 8", img.width, img.height), &alpha);
            let _ = write!(dict, " /SMask {mask} 0 R");
        }
        w.stream(id, &dict, &rgb);
        let _ = write!(xobjects, "/Im{k} {id} 0 R ");
    }
    let xobjects = if xobjects.is_empty() { String::new() } else { format!(" /XObject << {xobjects}>>") };
    let mut kids = Vec::new();
    for (p, c) in pages.iter().zip(&contents) {
        let (page_id, content_id) = (w.reserve(), w.reserve());
        w.stream(content_id, "", c.as_bytes());
        w.obj(
            page_id,
            &format!(
                "<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 {} {}] /Resources << /Font << /F1 {f1} 0 R /F2 {f2} 0 R >>{xobjects} >> /Contents {content_id} 0 R >>",
                n(p.width),
                n(p.height)
            ),
        );
        kids.push(format!("{page_id} 0 R"));
    }
    w.obj(pages_id, &format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), kids.len()));
    w.obj(catalog, &format!("<< /Type /Catalog /Pages {pages_id} 0 R >>"));
    w.obj(info, &format!("<< /Title {} /Producer {} >>", pdf_string(title), pdf_string("MathPrime")));
    let xref = w.buf.len();
    let mut tail = format!("xref\n0 {}\n0000000000 65535 f \n", w.offsets.len() + 1);
    for off in &w.offsets {
        let _ = writeln!(tail, "{off:010} 00000 n ");
    }
    let _ = write!(
        tail,
        "trailer\n<< /Size {} /Root {catalog} 0 R /Info {info} 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        w.offsets.len() + 1
    );
    w.buf.extend_from_slice(tail.as_bytes());
    w.buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{layout_document, Setup};
    use rb_sheet::Sheet;

    #[test]
    fn pdf_structuur_klopt() {
        let sheet = Sheet::load("# Titel\n\" Een regel tekst\nσ := 12.5 MPa\nσ / 2 =\nplot x = 0 ; 1 : x^2\n");
        let pages = layout_document(&sheet, &Setup::a4());
        let pdf = to_pdf(&pages, "Test ∑");
        assert!(pdf.starts_with(b"%PDF-1.7"));
        assert!(pdf.ends_with(b"%%EOF\n"));
        // elke xref-verwijzing wijst naar het begin van het juiste object (op bytes, want de
        // ingebedde lettertypen zijn geen tekst)
        let find = |needle: &[u8]| pdf.windows(needle.len()).rposition(|w| w == needle).unwrap();
        let sx = find(b"startxref\n") + 10;
        let end = sx + pdf[sx..].iter().position(|b| *b == b'\n').unwrap();
        let xref_at: usize = std::str::from_utf8(&pdf[sx..end]).unwrap().parse().unwrap();
        let table = std::str::from_utf8(&pdf[xref_at..]).unwrap();
        let mut count = 0;
        for (k, line) in table.lines().skip(3).take_while(|l| l.ends_with(" n ")).enumerate() {
            let off: usize = line[..10].parse().unwrap();
            let want = format!("{} 0 obj", k + 1);
            assert_eq!(&pdf[off..off + want.len()], want.as_bytes(), "object {}", k + 1);
            count += 1;
        }
        assert!(count > 10);
        let has = |s: &str| pdf.windows(s.len()).any(|w| w == s.as_bytes());
        assert!(has("/FontFile2") && has("/Count 1") && has("/ToUnicode"));
    }
}

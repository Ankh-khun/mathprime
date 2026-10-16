//! SVG-uitvoer. De lettertypen worden ingebed, zodat elke browser de formules precies zo toont als de
//! interface en de PDF.

use crate::doc::{Color, Op, Page};
use crate::font::{fonts, FontId};
use std::fmt::Write;

pub(crate) fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        for k in 0..4 {
            if k <= chunk.len() {
                out.push(T[(n >> (18 - 6 * k) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn rgb(c: Color) -> String {
    format!("rgb({},{},{})", c.0, c.1, c.2)
}

fn f(v: f32) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Zet pagina's om in één SVG-document (pagina's onder elkaar).
pub fn to_svg(pages: &[Page]) -> String {
    let gap = 20.0;
    let width = pages.iter().map(|p| p.width).fold(0.0, f32::max);
    let height: f32 = pages.iter().map(|p| p.height).sum::<f32>() + gap * pages.len().saturating_sub(1) as f32;
    let fs = fonts();
    let mut s = String::new();
    let _ = writeln!(
        s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}">"#,
        f(width),
        f(height),
        f(width),
        f(height)
    );
    let _ = writeln!(
        s,
        "<style>@font-face{{font-family:RbMain;src:url(data:font/ttf;base64,{})}}@font-face{{font-family:RbMono;src:url(data:font/ttf;base64,{})}}\
         .m{{font-family:RbMain,RbMono,sans-serif;white-space:pre}}.c{{font-family:RbMono,RbMain,monospace;white-space:pre}}</style>",
        base64(fs.main.data),
        base64(fs.mono.data)
    );
    let mut top = 0.0;
    for p in pages {
        let _ = writeln!(s, r#"<g transform="translate(0 {})">"#, f(top));
        let _ = writeln!(s, r#"<rect x="0" y="0" width="{}" height="{}" fill="white" stroke="rgb(200,200,200)"/>"#, f(p.width), f(p.height));
        for o in &p.ops {
            match o {
                Op::Image { x, y, w, h, png, .. } => {
                    let _ = writeln!(
                        s,
                        r#"<image x="{}" y="{}" width="{}" height="{}" preserveAspectRatio="none" href="data:image/png;base64,{}"/>"#,
                        f(*x),
                        f(*y),
                        f(*w),
                        f(*h),
                        base64(png)
                    );
                }
                Op::Text { x, y, size, text, font, color } => {
                    let class = if *font == FontId::Mono { "c" } else { "m" };
                    let _ = writeln!(
                        s,
                        r#"<text class="{class}" x="{}" y="{}" font-size="{}" fill="{}">{}</text>"#,
                        f(*x),
                        f(*y),
                        f(*size),
                        rgb(*color),
                        esc(text)
                    );
                }
                Op::Line { x1, y1, x2, y2, width, color } => {
                    let _ = writeln!(
                        s,
                        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}" stroke-linecap="round"/>"#,
                        f(*x1),
                        f(*y1),
                        f(*x2),
                        f(*y2),
                        rgb(*color),
                        f(*width)
                    );
                }
                Op::Poly { points, width, color } => {
                    let pts: Vec<String> = points.iter().map(|(x, y)| format!("{},{}", f(*x), f(*y))).collect();
                    let _ = writeln!(
                        s,
                        r#"<polyline points="{}" fill="none" stroke="{}" stroke-width="{}" stroke-linejoin="round" stroke-linecap="round"/>"#,
                        pts.join(" "),
                        rgb(*color),
                        f(*width)
                    );
                }
                Op::Fill { x, y, w, h, color } => {
                    let _ = writeln!(s, r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>"#, f(*x), f(*y), f(*w), f(*h), rgb(*color));
                }
                Op::Polygon { points, fill, width, color } => {
                    let pts: Vec<String> = points.iter().map(|(x, y)| format!("{},{}", f(*x), f(*y))).collect();
                    let stroke = if *width > 0.0 { format!(r#" stroke="{}" stroke-width="{}" stroke-linejoin="round""#, rgb(*color), f(*width)) } else { String::new() };
                    let _ = writeln!(s, r#"<polygon points="{}" fill="{}"{stroke}/>"#, pts.join(" "), rgb(*fill));
                }
                Op::Rect { x, y, w, h, width, color } => {
                    let _ = writeln!(
                        s,
                        r#"<rect x="{}" y="{}" width="{}" height="{}" fill="none" stroke="{}" stroke-width="{}"/>"#,
                        f(*x),
                        f(*y),
                        f(*w),
                        f(*h),
                        rgb(*color),
                        f(*width)
                    );
                }
            }
        }
        s.push_str("</g>\n");
        top += p.height + gap;
    }
    s.push_str("</svg>\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_klopt() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }
}

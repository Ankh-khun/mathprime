//! Tekenopdrachten uit de paginalayout ([`rb_export::doc::Op`]) tekenen met egui, en grafieken
//! interactief tonen met egui_plot.

use eframe::egui::{self, Color32, FontFamily, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2};
use rb_export::doc::{Color, Op, SERIES};
use rb_export::font::FontId as RbFont;
use rb_sheet::PlotData;

fn color(c: Color) -> Color32 {
    Color32::from_rgb(c.0, c.1, c.2)
}

/// Dezelfde kleur voor een donkere pagina: de lichtheid omgekeerd, de tint gelijk (zwarte tekst wordt
/// lichtgrijs, de groene uitkomsten lichtgroen).
pub fn darken(c: Color32) -> Color32 {
    let [r, g, b, a] = c.to_array().map(|v| v as f32 / 255.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let l = (max + min) / 2.0;
    let d = max - min;
    let (h, s) = if d == 0.0 {
        (0.0, 0.0)
    } else {
        let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
        let h = if max == r {
            (g - b) / d + if g < b { 6.0 } else { 0.0 }
        } else if max == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        (h / 6.0, s)
    };
    // omkeren, maar tekst niet te donker en vlakken niet te licht
    let l2 = if l < 0.5 { (1.0 - l).clamp(0.62, 0.9) } else { (1.0 - l).max(0.1) };
    let q = if l2 < 0.5 { l2 * (1.0 + s) } else { l2 + s - l2 * s };
    let p = 2.0 * l2 - q;
    let hue = |mut t: f32| {
        t = t.rem_euclid(1.0);
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    let to = |v: f32| (v * 255.0).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_unmultiplied(to(hue(h + 1.0 / 3.0)), to(hue(h)), to(hue(h - 1.0 / 3.0)), to(a))
}

/// Teken `ops` met de oorsprong van de pagina op `origin` en vergroting `zoom`; `dark`: voor een
/// donkere pagina.
pub fn paint_ops(painter: &Painter, ops: &[Op], origin: Pos2, zoom: f32, dark: bool) {
    let color = |c: Color| if dark { darken(color(c)) } else { color(c) };
    let p = |x: f32, y: f32| origin + Vec2::new(x * zoom, y * zoom);
    for o in ops {
        match o {
            Op::Text { x, y, size, text, font, color: c } => {
                let family = if *font == RbFont::Mono { FontFamily::Monospace } else { FontFamily::Proportional };
                let galley = painter.layout_no_wrap(text.clone(), FontId::new(size * zoom, family), color(*c));
                // de basislijn van de eerste regel ligt op `y`
                let baseline = galley
                    .rows
                    .first()
                    .and_then(|r| r.row.glyphs.first().map(|g| r.pos.y + g.pos.y))
                    .unwrap_or(0.8 * size * zoom);
                painter.galley(p(*x, *y) - Vec2::new(0.0, baseline), galley, color(*c));
            }
            Op::Line { x1, y1, x2, y2, width, color: c } => {
                painter.line_segment([p(*x1, *y1), p(*x2, *y2)], Stroke::new((width * zoom).max(0.6), color(*c)));
            }
            Op::Poly { points, width, color: c } => {
                let pts: Vec<Pos2> = points.iter().map(|(x, y)| p(*x, *y)).collect();
                painter.add(Shape::line(pts, Stroke::new((width * zoom).max(0.6), color(*c))));
            }
            Op::Rect { x, y, w, h, width, color: c } => {
                painter.rect_stroke(
                    Rect::from_min_size(p(*x, *y), Vec2::new(w * zoom, h * zoom)),
                    0.0,
                    Stroke::new((width * zoom).max(0.6), color(*c)),
                    StrokeKind::Middle,
                );
            }
            Op::Image { x, y, w, h, png, hash } => {
                let rect = Rect::from_min_size(p(*x, *y), Vec2::new(w * zoom, h * zoom));
                match texture(painter.ctx(), png, *hash) {
                    Some(tex) => {
                        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
                        painter.image(tex.id(), rect, uv, Color32::WHITE);
                    }
                    None => {
                        painter.rect_stroke(rect, 0.0, Stroke::new(1.0, Color32::GRAY), StrokeKind::Middle);
                        painter.line_segment([rect.left_top(), rect.right_bottom()], Stroke::new(1.0, Color32::GRAY));
                    }
                }
            }
            Op::Fill { x, y, w, h, color: c } => {
                painter.rect_filled(Rect::from_min_size(p(*x, *y), Vec2::new(w * zoom, h * zoom)), 0.0, color(*c));
            }
        }
    }
}

/// De textuur bij een PNG-afbeelding (één keer uitgepakt, daarna uit het geheugen van egui).
fn texture(ctx: &egui::Context, png: &[u8], hash: u64) -> Option<egui::TextureHandle> {
    let id = egui::Id::new(("afbeelding", hash));
    if let Some(t) = ctx.data(|d| d.get_temp::<Option<egui::TextureHandle>>(id)) {
        return t;
    }
    let tex = rb_export::png::decode(png).ok().map(|img| {
        let image = egui::ColorImage::from_rgba_unmultiplied([img.width, img.height], &img.pixels);
        ctx.load_texture(format!("afbeelding-{hash:x}"), image, egui::TextureOptions::LINEAR)
    });
    ctx.data_mut(|d| d.insert_temp(id, tex.clone()));
    tex
}

/// Een interactieve grafiek (zoomen, verschuiven, waarden aflezen) in `rect`.
pub fn plot(ui: &mut egui::Ui, id: usize, rect: Rect, data: &PlotData) {
    use rb_export::doc::{axis_value, log_ticks};
    let (lx, ly) = (data.log_x, data.log_y);
    // een andere as-instelling is een nieuwe grafiek, zodat egui geen oude grenzen onthoudt
    let bits = |r: Option<(f64, f64)>| r.map(|(a, b)| (a.to_bits(), b.to_bits()));
    let key = ("grafiek", id, lx, ly, bits(data.x_range), bits(data.y_range));
    let mut plot = egui_plot::Plot::new(key)
        .width(rect.width())
        .height(rect.height())
        .x_axis_label(data.x_label.clone())
        .y_axis_label(data.y_label.clone())
        .legend(egui_plot::Legend::default())
        .allow_scroll(false);
    // logaritmische as: we tekenen log10 van de waarden, met strepen op 1, 2 .. 9 × 10^k
    // en labels op de machten van 10
    let spacer = |input: egui_plot::GridInput| {
        log_ticks(input.bounds.0, input.bounds.1)
            .into_iter()
            .map(|(value, major)| egui_plot::GridMark { value, step_size: if major { 1.0 } else { 0.1 } })
            .collect()
    };
    let decade = |m: egui_plot::GridMark, _: &std::ops::RangeInclusive<f64>| {
        if (m.value - m.value.round()).abs() < 1e-9 {
            rb_units::format_number(10f64.powf(m.value.round()))
        } else {
            String::new()
        }
    };
    if lx {
        plot = plot.x_grid_spacer(spacer).x_axis_formatter(decade);
    }
    if ly {
        plot = plot.y_grid_spacer(spacer).y_axis_formatter(decade);
    }
    if lx || ly {
        let back = move |v: f64, log: bool| rb_units::format_number(if log { 10f64.powf(v) } else { v });
        plot = plot.label_formatter(move |pos| {
            Some(match pos {
                egui_plot::HoverPosition::NearDataPoint { plot_name, position, .. } => {
                    format!("{plot_name}\nx = {}\ny = {}", back(position.x, lx), back(position.y, ly))
                }
                egui_plot::HoverPosition::Elsewhere { position } => format!("x = {}\ny = {}", back(position.x, lx), back(position.y, ly)),
            })
        });
    }
    // vaste grenzen: begin- en resetgrenzen (in- en uitzoomen blijft kan, dubbelklik zet terug)
    if let Some((a, b)) = data.x_range {
        let (a, b) = (axis_value(a, lx), axis_value(b, lx));
        if a < b {
            plot = plot.default_x_bounds(a, b);
        }
    }
    if let Some((a, b)) = data.y_range {
        let (a, b) = (axis_value(a, ly), axis_value(b, ly));
        if a < b {
            plot = plot.default_y_bounds(a, b);
        }
    }
    ui.put(rect, |ui: &mut egui::Ui| {
        plot.show(ui, |pui| {
            for (k, s) in data.series.iter().enumerate() {
                let c = color(SERIES[k % SERIES.len()]);
                let pts: Vec<[f64; 2]> = s.points.iter().map(|q| [axis_value(q[0], lx), axis_value(q[1], ly)]).collect();
                // een NaN (niet uit te rekenen punt, of 0 op een logaritmische as) onderbreekt de lijn
                for seg in pts.split(|q| !q[0].is_finite() || !q[1].is_finite()) {
                    if seg.is_empty() {
                        continue;
                    }
                    let pts: egui_plot::PlotPoints = seg.to_vec().into();
                    if seg.len() == 1 {
                        pui.points(egui_plot::Points::new(s.label.clone(), pts).color(c).radius(2.5));
                    } else {
                        pui.line(egui_plot::Line::new(s.label.clone(), pts).color(c).width(1.6));
                    }
                }
            }
        })
        .response
    });
}

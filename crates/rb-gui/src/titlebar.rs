//! De eigen titelbalk: in de kleuren van het thema, met de menu's erin. Slepen verplaatst het venster, dubbelklik
//! maximaliseert, en aan de randen kun je vergroten. Windows regelt het verplaatsen en vergroten zelf
//! (`StartDrag`, `BeginResize`), dus vastklikken aan de schermrand blijft werken.

use eframe::egui::{self, viewport::ResizeDirection, Color32, CursorIcon, Sense, Stroke, ViewportCommand};

const HEIGHT: f32 = 32.0;
const EDGE: f32 = 5.0;

/// Waar de menu's de rechterkant van hun laatste knop bewaren.
pub fn menu_end_id() -> egui::Id {
    egui::Id::new("titelbalk-menu-einde")
}

/// De randen van het venster: de juiste cursor, en bij een klik vergroten.
pub fn resize_edges(ctx: &egui::Context) {
    if ctx.input(|i| i.viewport().maximized.unwrap_or(false) || i.viewport().fullscreen.unwrap_or(false)) {
        return;
    }
    let screen = ctx.content_rect();
    let Some(p) = ctx.input(|i| i.pointer.hover_pos()) else { return };
    let (l, r, t, b) = (p.x - screen.left() < EDGE, screen.right() - p.x < EDGE, p.y - screen.top() < EDGE, screen.bottom() - p.y < EDGE);
    use ResizeDirection as D;
    let (dir, cursor) = match (l, r, t, b) {
        (true, _, true, _) => (D::NorthWest, CursorIcon::ResizeNorthWest),
        (_, true, true, _) => (D::NorthEast, CursorIcon::ResizeNorthEast),
        (true, _, _, true) => (D::SouthWest, CursorIcon::ResizeSouthWest),
        (_, true, _, true) => (D::SouthEast, CursorIcon::ResizeSouthEast),
        (true, ..) => (D::West, CursorIcon::ResizeWest),
        (_, true, ..) => (D::East, CursorIcon::ResizeEast),
        (_, _, true, _) => (D::North, CursorIcon::ResizeNorth),
        (_, _, _, true) => (D::South, CursorIcon::ResizeSouth),
        _ => return,
    };
    ctx.set_cursor_icon(cursor);
    if ctx.input(|i| i.pointer.primary_pressed()) {
        ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
    }
}

/// Een knop van de balk: het pictogram wordt in `draw` getekend.
fn control(ui: &mut egui::Ui, danger: bool, tip: &str, draw: impl FnOnce(&egui::Painter, egui::Rect, Stroke)) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(44.0, HEIGHT), Sense::click());
    let hovered = resp.hovered();
    if hovered {
        let fill = if danger { Color32::from_rgb(196, 43, 28) } else { ui.visuals().widgets.hovered.weak_bg_fill };
        ui.painter().rect_filled(rect, 0.0, fill);
    }
    let color = if hovered && danger { Color32::WHITE } else { ui.visuals().widgets.noninteractive.fg_stroke.color };
    draw(ui.painter(), rect.shrink2(egui::vec2(17.0, 11.0)), Stroke::new(1.1, color));
    resp.on_hover_text(tip).clicked()
}

/// De balk: pictogram, `menus`, de titel in het midden, en de vensterknoppen.
pub fn show(ctx: &egui::Context, ui: &mut egui::Ui, title: &str, menus: impl FnOnce(&mut egui::Ui)) {
    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    let full = ui.max_rect();
    ui.horizontal(|ui| {
        ui.set_height(HEIGHT);
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_space(8.0);
        let tex = logo(ctx);
        ui.add(egui::Image::new(&tex).fit_to_exact_size(egui::vec2(18.0, 18.0)));
        ui.add_space(6.0);
        // een menubalk neemt alle ruimte die hij krijgt: alleen de breedte van vorige keer geven
        let start = ui.cursor().left();
        let width = ctx.data(|d| d.get_temp::<f32>(menu_end_id())).map_or(220.0, |end| end - start + 4.0);
        ui.allocate_ui(egui::vec2(width, HEIGHT), |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            menus(ui);
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            use crate::taal::t;
            if control(ui, true, t("Sluiten"), |p, r, s| {
                p.line_segment([r.left_top(), r.right_bottom()], s);
                p.line_segment([r.right_top(), r.left_bottom()], s);
            }) {
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            if control(ui, false, t(if maximized { "Verkleinen" } else { "Maximaliseren" }), |p, r, s| {
                if maximized {
                    p.rect_stroke(r.shrink(1.5).translate(egui::vec2(-1.5, 1.5)), 0.0, s, egui::StrokeKind::Middle);
                    p.line_segment([r.left_top() + egui::vec2(1.5, 0.0), r.right_top()], s);
                    p.line_segment([r.right_top(), r.right_bottom() - egui::vec2(0.0, 1.5)], s);
                } else {
                    p.rect_stroke(r, 0.0, s, egui::StrokeKind::Middle);
                }
            }) {
                ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
            }
            if control(ui, false, t("Minimaliseren"), |p, r, s| {
                p.line_segment([r.left_center(), r.right_center()], s);
            }) {
                ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
            }
            // wat er tussen menu's en knoppen overblijft is de sleepbalk, met de titel in het midden
            let left = ui.min_rect().left();
            let rest = egui::Rect::from_min_max(egui::pos2(ui.max_rect().left(), full.top()), egui::pos2(left, full.top() + HEIGHT));
            let drag = ui.interact(rest, ui.id().with("sleepbalk"), Sense::click_and_drag());
            if drag.double_clicked() {
                ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
            } else if drag.is_pointer_button_down_on() && ctx.input(|i| i.pointer.is_decidedly_dragging()) {
                ctx.send_viewport_cmd(ViewportCommand::StartDrag);
            }
            let font = egui::TextStyle::Body.resolve(ui.style());
            let color = ui.visuals().weak_text_color();
            let galley = ui.painter().layout_no_wrap(title.to_string(), font, color);
            if galley.size().x + 12.0 < rest.width() {
                // midden van het hele venster, of anders van wat er over is
                let x = full.center().x.clamp(rest.left() + galley.size().x / 2.0 + 6.0, rest.right() - galley.size().x / 2.0 - 6.0);
                ui.painter().galley(egui::pos2(x - galley.size().x / 2.0, full.top() + (HEIGHT - galley.size().y) / 2.0), galley, color);
            }
        });
    });
}

fn logo(ctx: &egui::Context) -> egui::TextureHandle {
    let id = egui::Id::new("titelbalk-logo");
    if let Some(t) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return t;
    }
    let rgba = include_bytes!("../assets/mathprime-128.rgba");
    let img = egui::ColorImage::from_rgba_unmultiplied([128, 128], rgba);
    let tex = ctx.load_texture("titelbalk-logo", img, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, tex.clone()));
    tex
}

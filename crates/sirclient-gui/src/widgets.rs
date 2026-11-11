//! Reusable widgets.

use egui::{
    Align2, Color32, CornerRadius, FontId, Pos2, Rect, Response, RichText, Sense, Stroke, Ui, Vec2,
    pos2, vec2,
};
use sirclient_core::protocol::g7pro::curve;
use sirclient_core::protocol::g7pro::keys::{self, Group, KEY_CODES};
use sirclient_core::protocol::g7pro::profile::{Curve, Deadzone};

use crate::i18n::t;
use crate::theme::*;

/// Segmented-control style button.
pub fn pill(ui: &mut Ui, selected: bool, text: impl Into<String>) -> Response {
    let text = RichText::new(text.into()).color(if selected { ON_ACCENT } else { TEXT_2 });
    let b = egui::Button::new(text)
        .fill(if selected {
            ACCENT
        } else {
            Color32::TRANSPARENT
        })
        .stroke(if selected {
            Stroke::NONE
        } else {
            Stroke::new(1.0, BORDER_STRONG)
        })
        .corner_radius(CornerRadius::same(7));
    ui.add(b)
}

pub fn primary_button(ui: &mut Ui, enabled: bool, text: impl Into<String>) -> Response {
    let b = egui::Button::new(RichText::new(text.into()).color(ON_ACCENT).strong())
        .fill(ACCENT)
        .corner_radius(CornerRadius::same(8));
    ui.add_enabled(enabled, b)
}

pub fn heading(ui: &mut Ui, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).size(15.0).strong());
}

pub fn caption(ui: &mut Ui, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).size(12.0).color(FAINT));
}

/// Label on the left, value on the right, slider below.
pub fn slider_u16(
    ui: &mut Ui,
    label: &str,
    v: &mut u16,
    range: std::ops::RangeInclusive<u16>,
    fmt: impl Fn(u16) -> String,
) -> bool {
    let shown = fmt(*v);
    ui.horizontal(|ui| {
        ui.label(label);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(shown).color(MUTED));
        });
    });
    let w = ui.available_width();
    ui.spacing_mut().slider_width = w;
    ui.add(egui::Slider::new(v, range).show_value(false))
        .changed()
}

pub fn slider_u8(
    ui: &mut Ui,
    label: &str,
    v: &mut u8,
    range: std::ops::RangeInclusive<u8>,
    fmt: impl Fn(u8) -> String,
) -> bool {
    let mut x = *v as u16;
    let c = slider_u16(
        ui,
        label,
        &mut x,
        *range.start() as u16..=*range.end() as u16,
        |v| fmt(v as u8),
    );
    *v = x as u8;
    c
}

pub fn percent(raw_thousandths: u16) -> String {
    format!("{:.1}%", raw_thousandths as f32 / 10.0)
}

fn key_group_title(g: Group) -> String {
    match g {
        Group::Gamepad => t("keys-gamepad"),
        Group::Keyboard => t("keys-keyboard"),
        Group::Mouse => t("keys-mouse"),
        Group::Special => t("keys-special"),
    }
}

/// Pick a key code. `default_label` adds an entry for "no remap" (code 0).
pub fn key_combo(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    code: &mut u8,
    default_label: Option<&str>,
) -> bool {
    let current = if *code == 0 {
        default_label
            .map(str::to_string)
            .unwrap_or_else(|| "—".into())
    } else {
        keys::display(*code)
    };
    let mut changed = false;
    egui::ComboBox::from_id_salt(id)
        .selected_text(current)
        .height(360.0)
        .show_ui(ui, |ui| {
            if let Some(d) = default_label {
                changed |= ui.selectable_value(code, 0, d).changed();
            }
            for g in [
                Group::Gamepad,
                Group::Special,
                Group::Keyboard,
                Group::Mouse,
            ] {
                ui.separator();
                ui.label(RichText::new(key_group_title(g)).size(11.0).color(FAINT));
                for (name, c) in KEY_CODES.iter().filter(|(_, c)| keys::group(*c) == g) {
                    changed |= ui
                        .selectable_value(code, *c, keys::display_name(name))
                        .changed();
                }
            }
        });
    changed
}

/// Circle with a dot for a stick position (-1..1, y down).
pub fn stick_view(ui: &mut Ui, pos: (f32, f32), size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let p = ui.painter_at(rect);
    let c = rect.center();
    let r = size / 2.0 - 3.0;
    p.circle(c, r, BG, Stroke::new(2.0, BORDER_STRONG));
    p.line_segment(
        [pos2(c.x, c.y - r), pos2(c.x, c.y + r)],
        Stroke::new(1.0, BORDER),
    );
    p.line_segment(
        [pos2(c.x - r, c.y), pos2(c.x + r, c.y)],
        Stroke::new(1.0, BORDER),
    );
    let dot = pos2(
        c.x + pos.0.clamp(-1.0, 1.0) * (r - 8.0),
        c.y + pos.1.clamp(-1.0, 1.0) * (r - 8.0),
    );
    p.circle_filled(dot, 8.0, ACCENT);
}

/// Vertical bar for an analog trigger (0..=255).
pub fn trigger_bar(ui: &mut Ui, v: u8, height: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(22.0, height), Sense::hover());
    let p = ui.painter_at(rect);
    p.rect(
        rect,
        CornerRadius::same(6),
        BG,
        Stroke::new(1.0, BORDER_STRONG),
        egui::StrokeKind::Inside,
    );
    let h = rect.height() * v as f32 / 255.0;
    let fill = Rect::from_min_max(pos2(rect.min.x, rect.max.y - h), rect.max);
    p.rect_filled(fill, CornerRadius::same(5), ACCENT);
}

pub fn chip(ui: &mut Ui, on: bool, text: &str) {
    // A Button (atomic widget) so horizontal_wrapped can move it to the
    // next line as a whole.
    let rt = RichText::new(text)
        .size(12.0)
        .color(if on { ON_ACCENT } else { MUTED });
    let b = egui::Button::new(rt)
        .fill(if on { ACCENT } else { BG })
        .stroke(Stroke::new(1.0, if on { ACCENT } else { BORDER }))
        .corner_radius(CornerRadius::same(6))
        .sense(Sense::hover());
    ui.add(b);
}

/// App mark: a ring with a dot, painted so no font glyph is needed.
pub fn logo(ui: &mut Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let p = ui.painter_at(rect);
    p.circle_stroke(rect.center(), size * 0.38, Stroke::new(2.2, ACCENT));
    p.circle_filled(rect.center(), size * 0.14, ACCENT);
}

/// Marker appended to labels of the item active on the controller.
pub const ACTIVE_MARK: &str = " •";

/// Response-curve editor. Points 1..=3 can be dragged, which turns the
/// curve into a custom one. Returns true when the curve changed.
// Neighbouring points are read by index while dragging.
#[allow(clippy::needless_range_loop)]
pub fn curve_editor(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    c: &mut Curve,
    dz: &Deadzone,
    size: f32,
) -> bool {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let p = ui.painter_at(rect);
    p.rect(
        rect,
        CornerRadius::same(10),
        BG,
        Stroke::new(1.0, BORDER),
        egui::StrokeKind::Inside,
    );
    for i in 1..4 {
        let f = i as f32 / 4.0;
        p.line_segment(
            [
                pos2(rect.min.x + f * size, rect.min.y),
                pos2(rect.min.x + f * size, rect.max.y),
            ],
            Stroke::new(1.0, BORDER.linear_multiply(0.6)),
        );
        p.line_segment(
            [
                pos2(rect.min.x, rect.min.y + f * size),
                pos2(rect.max.x, rect.min.y + f * size),
            ],
            Stroke::new(1.0, BORDER.linear_multiply(0.6)),
        );
    }
    // Dead zone shading.
    let dz_w = dz.inner as f32 / 1000.0 * size;
    p.rect_filled(
        Rect::from_min_size(rect.min, vec2(dz_w, size)),
        CornerRadius::same(10),
        ACCENT.linear_multiply(0.08),
    );

    let to_screen = |x: f32, y: f32| -> Pos2 {
        pos2(rect.min.x + x / 100.0 * size, rect.max.y - y / 100.0 * size)
    };
    let pct = |pt: (u8, u8)| -> (f32, f32) {
        (
            curve::from_byte(pt.0, dz.inner, dz.outer),
            curve::from_byte(pt.1, dz.anti_inner, dz.anti_outer),
        )
    };
    let pts: Vec<Pos2> = c
        .points
        .iter()
        .map(|&pt| {
            let (x, y) = pct(pt);
            to_screen(x, y)
        })
        .collect();
    let mut line = vec![to_screen(0.0, dz.anti_inner as f32 / 10.0)];
    line.extend(pts.iter().copied());
    line.push(to_screen(100.0, dz.anti_outer as f32 / 10.0));
    p.add(egui::Shape::line(line, Stroke::new(2.5, ACCENT)));

    let mut changed = false;
    let base = ui.scope_id().with(id);
    for i in 1..4 {
        let hit = Rect::from_center_size(pts[i], Vec2::splat(18.0));
        let r = ui.interact(hit, base.with(i), Sense::drag());
        if r.dragged()
            && let Some(m) = r.interact_pointer_pos()
        {
            let x = ((m.x - rect.min.x) / size * 100.0).clamp(0.0, 100.0);
            let y = ((rect.max.y - m.y) / size * 100.0).clamp(0.0, 100.0);
            let span_x = (dz.outer as f32 - dz.inner as f32) / 10.0;
            let span_y = (dz.anti_outer as f32 - dz.anti_inner as f32) / 10.0;
            if span_x > 0.0 && span_y > 0.0 {
                let bx = ((x - dz.inner as f32 / 10.0) / span_x * 255.0).clamp(0.0, 255.0) as u8;
                let by =
                    ((y - dz.anti_inner as f32 / 10.0) / span_y * 255.0).clamp(0.0, 255.0) as u8;
                // Keep points ordered left to right.
                let lo = c.points[i - 1].0.saturating_add(1);
                let hi = c.points[i + 1].0.saturating_sub(1).max(lo);
                c.points[i] = (bx.clamp(lo, hi), by);
                c.kind = curve::CUSTOM;
                c.enabled = 1;
                changed = true;
            }
        }
        let hovered = r.hovered() || r.dragged();
        p.circle(
            pts[i],
            if hovered { 7.0 } else { 5.5 },
            BG,
            Stroke::new(2.0, ACCENT),
        );
    }
    p.text(
        rect.right_bottom() + vec2(-6.0, -6.0),
        Align2::RIGHT_BOTTOM,
        t("curve-input"),
        FontId::proportional(10.0),
        FAINT,
    );
    p.text(
        rect.left_top() + vec2(6.0, 6.0),
        Align2::LEFT_TOP,
        t("curve-output"),
        FontId::proportional(10.0),
        FAINT,
    );
    changed
}

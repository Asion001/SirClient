//! Colours and styling that match the design mockup.

use egui::{Color32, CornerRadius, Frame, Margin, Stroke, Visuals};

pub const BG: Color32 = Color32::from_rgb(0x10, 0x12, 0x15);
pub const SURFACE: Color32 = Color32::from_rgb(0x16, 0x19, 0x1D);
pub const SUNKEN: Color32 = Color32::from_rgb(0x0B, 0x0D, 0x10);
pub const BORDER: Color32 = Color32::from_rgb(0x26, 0x2A, 0x30);
pub const BORDER_STRONG: Color32 = Color32::from_rgb(0x34, 0x3A, 0x42);
pub const SELECTED: Color32 = Color32::from_rgb(0x23, 0x2A, 0x35);
pub const TEXT: Color32 = Color32::from_rgb(0xE6, 0xE8, 0xEB);
pub const TEXT_2: Color32 = Color32::from_rgb(0xC9, 0xCF, 0xD6);
pub const MUTED: Color32 = Color32::from_rgb(0xAE, 0xB6, 0xC0);
pub const FAINT: Color32 = Color32::from_rgb(0x8B, 0x94, 0x9E);
pub const ACCENT: Color32 = Color32::from_rgb(0x8A, 0xB4, 0xFF);
pub const ON_ACCENT: Color32 = Color32::from_rgb(0x0B, 0x12, 0x20);
pub const OK: Color32 = Color32::from_rgb(0x4A, 0xDE, 0x80);
pub const WARN: Color32 = Color32::from_rgb(0xFB, 0xBF, 0x24);
pub const BT: Color32 = Color32::from_rgb(0xD2, 0xB8, 0xFF);

pub fn apply(ctx: &egui::Context) {
    let mut v = Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = SURFACE;
    v.extreme_bg_color = BG;
    v.faint_bg_color = SURFACE;
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill = ACCENT.linear_multiply(0.45);
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.hyperlink_color = ACCENT;
    let r = CornerRadius::same(7);
    for w in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = r;
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    v.widgets.inactive.bg_fill = BG;
    v.widgets.inactive.weak_bg_fill = BG;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER_STRONG);
    v.widgets.hovered.weak_bg_fill = SELECTED;
    v.widgets.hovered.bg_fill = SELECTED;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT.linear_multiply(0.6));
    v.widgets.active.bg_fill = SELECTED;
    v.widgets.active.weak_bg_fill = SELECTED;
    v.slider_trailing_fill = true;
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 8.0);
        s.spacing.button_padding = egui::vec2(10.0, 5.0);
        s.spacing.slider_width = 220.0;
        s.spacing.combo_width = 180.0;
    });
}

pub fn card() -> Frame {
    Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::same(16))
}

pub fn sunken() -> Frame {
    Frame::new()
        .fill(BG)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(10, 8))
}

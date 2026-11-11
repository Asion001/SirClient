pub mod buttons;
pub mod devices;
pub mod lighting;
pub mod overview;
pub mod sticks;
pub mod xbox_lighting;

use egui::{RichText, Ui};

use crate::i18n::t;
use crate::theme::*;

pub fn empty_state(ui: &mut Ui) {
    card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(t("empty-title")).size(16.0).strong());
        ui.label(RichText::new(t("empty-body")).color(MUTED));
    });
}

/// Shown instead of the editors when the G7 Pro XBox is on Bluetooth.
pub fn bt_notice(ui: &mut Ui) {
    card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(t("bt-notice-title")).size(16.0).strong());
        ui.label(RichText::new(t("bt-notice-body")).color(MUTED));
    });
}

/// Lay out cards in a wrapping row with roughly `min` px per card.
pub fn columns(ui: &mut Ui, n: usize, min: f32, mut f: impl FnMut(usize, &mut Ui)) {
    let w = ui.available_width();
    let per_row = ((w / min).floor() as usize).clamp(1, n);
    let gap = 14.0;
    let cw = (w - gap * (per_row as f32 - 1.0)) / per_row as f32;
    let mut i = 0;
    while i < n {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for _ in 0..per_row {
                if i >= n {
                    break;
                }
                ui.allocate_ui_with_layout(
                    egui::vec2(cw, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(cw);
                        ui.set_max_width(cw);
                        f(i, ui)
                    },
                );
                i += 1;
            }
        });
        ui.add_space(14.0);
    }
}

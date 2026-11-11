use egui::{Color32, RichText, Ui};
use fluent_bundle::FluentValue;
use sirclient_core::protocol::g7pro::light::{Hsv, MODES, PRESETS, ZONES};

use crate::app::{Connected, UiState};
use crate::i18n::{t, ta};
use crate::theme::*;
use crate::widgets::*;

fn zone_name(i: usize) -> String {
    t([
        "zone-lower-left",
        "zone-upper-left",
        "zone-upper-right",
        "zone-lower-right",
        "zone-home",
    ][i])
}

fn mode_name(m: u8) -> String {
    t(match m {
        0 => "light-off",
        1 => "light-static",
        2 => "light-breathing",
        3 => "light-rainbow",
        4 => "light-phantom",
        _ => "light-radar",
    })
}

pub fn show(ui: &mut Ui, c: &mut Connected, st: &mut UiState) {
    let active = c.state.as_ref().map(|s| s.light_preset as usize);
    let Some(l) = c.light.as_mut() else { return };
    ui.horizontal_wrapped(|ui| {
        for i in 0..PRESETS {
            let mut label = ta("preset-n", &[("n", FluentValue::from(i + 1))]);
            if active == Some(i) {
                label.push_str(ACTIVE_MARK);
            }
            if pill(ui, st.light_preset == i, label).clicked() {
                st.light_preset = i;
            }
        }
    });
    ui.add_space(10.0);
    let p = &mut l.presets[st.light_preset];
    let mut flags = l.flags;
    let mut standby = l.standby_time;
    super::columns(ui, 2, 380.0, |col, ui| {
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            if col == 0 {
                heading(ui, t("zones"));
                for (i, _) in ZONES.iter().enumerate() {
                    sunken().show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            let mut rgb = p.colors[i].to_rgb();
                            if ui.color_edit_button_srgb(&mut rgb).changed() {
                                p.colors[i] = Hsv::from_rgb(rgb);
                            }
                            ui.label(zone_name(i));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let [r, g, b] = rgb;
                                    ui.label(
                                        RichText::new(format!("#{r:02X}{g:02X}{b:02X}"))
                                            .monospace()
                                            .color(MUTED),
                                    );
                                },
                            );
                        });
                    });
                }
                if ui.button(t("zones-same")).clicked() {
                    let c0 = p.colors[0];
                    p.colors = [c0; 5];
                }
            } else {
                heading(ui, t("effect"));
                ui.horizontal_wrapped(|ui| {
                    for (m, _) in MODES {
                        if pill(ui, p.mode == m, mode_name(m)).clicked() {
                            p.mode = m;
                        }
                    }
                });
                if p.mode != 0 {
                    slider_u8(ui, &t("brightness"), p.brightness_mut(), 0..=100, |v| {
                        format!("{v}%")
                    });
                    if let Some(s) = p.speed_mut() {
                        slider_u8(ui, &t("speed"), s, 0..=100, |v| v.to_string());
                    }
                }
                ui.separator();
                ui.label(RichText::new(t("all-presets")).strong().color(TEXT_2));
                let mut wake = flags & 1 != 0;
                if ui.checkbox(&mut wake, t("wake-on-lift")).changed() {
                    flags = if wake { flags | 1 } else { flags & !1 };
                }
                ui.horizontal(|ui| {
                    ui.label(t("standby-time"));
                    ui.add(egui::DragValue::new(&mut standby).range(0..=255));
                });
                // Live swatch of what the controller reports right now.
                if let Some(s) = &c.state {
                    ui.horizontal(|ui| {
                        caption(ui, t("live-colors"));
                        for [r, g, b] in s.zone_rgb {
                            let (rect, _) = ui
                                .allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                            ui.painter()
                                .rect_filled(rect, 4.0, Color32::from_rgb(r, g, b));
                        }
                    });
                }
            }
        });
    });
    l.flags = flags;
    l.standby_time = standby;
}

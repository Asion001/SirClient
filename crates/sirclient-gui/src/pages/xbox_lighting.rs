//! Lighting page for the G7 Pro XBox: four keyframe animations over five
//! LEDs, plus a few global settings.

use egui::{Color32, RichText, Ui};
use fluent_bundle::FluentValue;
use sirclient_core::protocol::g7pro::xbox::{ANIMATIONS, MAX_KEYFRAMES};

use crate::app::{Connected, UiState};
use crate::i18n::{t, ta};
use crate::theme::*;
use crate::widgets::*;

/// LED names in the XBox keyframe order (see `xbox::ZONES`).
fn zone_name(i: usize) -> String {
    t([
        "zone-lower-left",
        "zone-lower-right",
        "zone-upper-left",
        "zone-home",
        "zone-upper-right",
    ][i])
}

/// Standby values Nexus offers (raw value, minutes; 0 = never).
const STANDBY: [(u8, u8); 5] = [(0, 0), (1, 1), (2, 5), (3, 10), (4, 20)];

fn color_row(ui: &mut Ui, label: String, rgb: &mut [u8; 3]) -> bool {
    let mut changed = false;
    sunken().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            changed = ui.color_edit_button_srgb(rgb).changed();
            ui.label(label);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let [r, g, b] = *rgb;
                ui.label(
                    RichText::new(format!("#{r:02X}{g:02X}{b:02X}"))
                        .monospace()
                        .color(MUTED),
                );
            });
        });
    });
    changed
}

pub fn show(ui: &mut Ui, c: &mut Connected, st: &mut UiState) {
    let active = c.state.as_ref().map(|s| s.light_preset as usize);
    let live = c.state.as_ref().map(|s| s.zone_rgb);
    let Some(l) = c.xlight.as_mut() else { return };
    st.light_preset = st.light_preset.min(ANIMATIONS - 1);
    ui.horizontal_wrapped(|ui| {
        for i in 0..ANIMATIONS {
            let mut label = ta("light-effect-n", &[("n", FluentValue::from(i + 1))]);
            if active == Some(i) {
                label.push_str(ACTIVE_MARK);
            }
            if pill(ui, st.light_preset == i, label).clicked() {
                st.light_preset = i;
                st.light_frame = 0;
            }
        }
    });
    ui.label(RichText::new(t("xbox-light-note")).color(MUTED));
    ui.add_space(10.0);
    let a = &mut l.animations[st.light_preset];
    a.keyframes = a.keyframes.clamp(1, MAX_KEYFRAMES as u8);
    a.frames.resize(MAX_KEYFRAMES, Default::default());
    st.light_frame = st.light_frame.min(a.keyframes as usize - 1);

    let mut led = l.profile_led;
    let mut sleep = l.sleep_time;
    let mut wake = l.pedestal_wake != 0;
    let mut audio = l.audio_reactive != 0;
    super::columns(ui, 2, 380.0, |col, ui| {
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            if col == 0 {
                heading(ui, t("light-frames"));
                ui.horizontal_wrapped(|ui| {
                    for f in 0..a.keyframes as usize {
                        let label = ta("light-frame-n", &[("n", FluentValue::from(f + 1))]);
                        if pill(ui, st.light_frame == f, label).clicked() {
                            st.light_frame = f;
                        }
                    }
                    if ui
                        .add_enabled(
                            (a.keyframes as usize) < MAX_KEYFRAMES,
                            egui::Button::new("+"),
                        )
                        .on_hover_text(t("light-frame-add"))
                        .clicked()
                    {
                        // New frame starts as a copy of the current one.
                        let n = a.keyframes as usize;
                        a.frames[n] = a.frames[st.light_frame];
                        a.keyframes += 1;
                        st.light_frame = n;
                    }
                    if ui
                        .add_enabled(a.keyframes > 1, egui::Button::new("−"))
                        .on_hover_text(t("light-frame-remove"))
                        .clicked()
                    {
                        let n = a.keyframes as usize;
                        a.frames.remove(st.light_frame);
                        a.frames.push(Default::default());
                        a.keyframes -= 1;
                        st.light_frame = st.light_frame.min(n - 2);
                    }
                });
                ui.add_space(6.0);
                heading(ui, t("zones"));
                let cur = st.light_frame;
                for (z, rgb) in a.frames[cur].iter_mut().enumerate() {
                    color_row(ui, zone_name(z), rgb);
                }
                ui.horizontal(|ui| {
                    if ui.button(t("zones-same")).clicked() {
                        let c0 = a.frames[cur][0];
                        a.frames[cur] = [c0; 5];
                    }
                    if ui.button(t("light-copy-frame")).clicked() {
                        let fr = a.frames[cur];
                        for f in a.frames.iter_mut() {
                            *f = fr;
                        }
                    }
                });
            } else {
                heading(ui, t("effect"));
                slider_u8(ui, &t("brightness"), &mut a.brightness, 0..=100, |v| {
                    format!("{v}%")
                });
                slider_u8(ui, &t("speed"), &mut a.speed, 0..=20, |v| v.to_string());
                ui.separator();
                ui.label(RichText::new(t("all-presets")).strong().color(TEXT_2));
                color_row(ui, t("profile-led"), &mut led);
                ui.checkbox(&mut audio, t("light-audio-reactive"));
                ui.checkbox(&mut wake, t("wake-on-dock"));
                ui.horizontal_wrapped(|ui| {
                    ui.label(t("standby-time"));
                    for (v, min) in STANDBY {
                        let label = if min == 0 {
                            t("standby-off")
                        } else {
                            ta("standby-minutes", &[("n", FluentValue::from(min))])
                        };
                        if pill(ui, sleep == v || (v == 4 && sleep == 255), label).clicked() {
                            sleep = v;
                        }
                    }
                });
                if let Some(zones) = live {
                    ui.horizontal(|ui| {
                        caption(ui, t("live-colors"));
                        for [r, g, b] in zones {
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
    l.profile_led = led;
    l.sleep_time = sleep;
    l.pedestal_wake = wake as u8;
    l.audio_reactive = audio as u8;
}

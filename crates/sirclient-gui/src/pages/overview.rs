use std::time::Duration;

use egui::{RichText, Ui};
use fluent_bundle::FluentValue;
use sirclient_core::protocol::g7pro::Slot;
use sirclient_core::protocol::g7pro::commands::CalibrationPart;
use sirclient_core::protocol::g7pro::profile::{REPORT_RATES, motor_level, motor_raw};
use sirclient_core::protocol::g7pro::state::Inputs;
use sirclient_core::session::Request;

use crate::app::{Connected, UiState, conn_label};
use crate::i18n::{t, ta};
use crate::theme::*;
use crate::widgets::*;

fn stat(ui: &mut Ui, label: String, value: String, sub: String) {
    card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        caption(ui, label);
        ui.label(RichText::new(value).size(18.0).strong());
        ui.label(RichText::new(sub).size(12.0).color(MUTED));
    });
}

pub fn show(ui: &mut Ui, c: &mut Connected, slot: Slot, st: &mut UiState) {
    let battery = c.state.as_ref().map(|s| (s.battery, s.charge_state));
    super::columns(ui, 4, 210.0, |i, ui| match i {
        0 => {
            let conn = c
                .connection
                .map(conn_label)
                .unwrap_or_else(|| t("device-demo"));
            let sub = if c.pending > 0 {
                t("stat-syncing")
            } else {
                t("stat-connection-sub")
            };
            stat(ui, t("stat-connection"), conn, sub);
        }
        1 => {
            let (fw, dongle) = c.firmware.clone().unwrap_or_default();
            stat(
                ui,
                t("stat-firmware"),
                if fw.is_empty() { "—".into() } else { fw },
                ta(
                    "stat-dongle",
                    &[(
                        "version",
                        FluentValue::from(if dongle.is_empty() {
                            "—".to_string()
                        } else {
                            dongle
                        }),
                    )],
                ),
            );
        }
        2 => {
            card().show(ui, |ui| {
                ui.set_width(ui.available_width());
                caption(ui, t("stat-battery"));
                let (lvl, charging) = battery.unwrap_or((0, 0));
                let mut v = match battery {
                    Some(_) => format!("{lvl}%"),
                    None => "—".into(),
                };
                if charging != 0 {
                    v.push_str(" · ");
                    v.push_str(&t("stat-charging"));
                }
                ui.label(RichText::new(v).size(18.0).strong());
                ui.add(
                    egui::ProgressBar::new(lvl.min(100) as f32 / 100.0)
                        .desired_height(6.0)
                        .fill(OK),
                );
            });
        }
        _ => {
            card().show(ui, |ui| {
                ui.set_width(ui.available_width());
                caption(ui, t("stat-polling"));
                if let Some(p) = c.profiles.get_mut(&slot) {
                    ui.horizontal_wrapped(|ui| {
                        for (v, name) in REPORT_RATES {
                            if pill(ui, p.function.extend.report_rate == v, name).clicked() {
                                p.function.extend.report_rate = v;
                            }
                        }
                    });
                }
            });
        }
    });

    // The XBox edition has no live state report and its rumble and
    // calibration commands are not known yet: don't send 8K commands to it.
    if c.xbox {
        super::xbox_notice(ui);
        return;
    }

    super::columns(ui, 2, 420.0, |i, ui| {
        if i == 0 {
            live_input(ui, c, st);
        } else {
            vibration(ui, c, slot, st);
        }
    });

    shortcuts(ui);

    if st.calib_open {
        calibration_window(ui, c, st);
    }
}

/// A key cap; `None` draws a dashed "unknown" cap.
fn keycap(ui: &mut Ui, label: Option<&str>) {
    let (text, fill, stroke, color) = match label {
        Some(l) => (
            l.to_string(),
            SELECTED,
            egui::Stroke::new(1.0, BORDER_STRONG),
            TEXT,
        ),
        None => (
            t("shortcut-unknown"),
            egui::Color32::TRANSPARENT,
            egui::Stroke::new(1.0, FAINT),
            MUTED,
        ),
    };
    let b = egui::Button::new(RichText::new(text).size(12.0).strong().color(color))
        .fill(fill)
        .stroke(stroke)
        .corner_radius(egui::CornerRadius::same(6))
        .sense(egui::Sense::hover());
    ui.add(b);
}

/// Button combos that work on the controller itself.
fn shortcuts(ui: &mut Ui) {
    // (key caps, what it does, note). `None` = combo not known yet.
    let items: [(Option<&[&str]>, &str, &str); 5] = [
        (None, "shortcut-profile", "shortcut-not-documented"),
        (None, "shortcut-remap", "shortcut-not-documented"),
        (Some(&["Shift"]), "shortcut-shift", "shortcut-shift-note"),
        (
            Some(&["M", "+", "Home", "5 s"]),
            "shortcut-1000hz",
            "shortcut-1000hz-note",
        ),
        (
            Some(&["M", "+", "LS left / right"]),
            "shortcut-light",
            "shortcut-light-note",
        ),
    ];
    card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            heading(ui, t("shortcuts-title"));
            caption(ui, t("shortcuts-sub"));
        });
        super::columns(ui, items.len(), 300.0, |i, ui| {
            let (keys, what, note) = items[i];
            sunken().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_wrapped(|ui| match keys {
                    Some(ks) => {
                        for k in ks {
                            if *k == "+" {
                                ui.label(RichText::new("+").color(FAINT));
                            } else {
                                keycap(ui, Some(k));
                            }
                        }
                    }
                    None => keycap(ui, None),
                });
                ui.label(RichText::new(t(what)).strong());
                ui.label(RichText::new(t(note)).size(12.0).color(FAINT));
            });
        });
    });
}

fn live_input(ui: &mut Ui, c: &Connected, st: &mut UiState) {
    card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            heading(ui, t("live-input"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if pill(ui, st.show_physical, t("live-physical")).clicked() {
                    st.show_physical = true;
                }
                if pill(ui, !st.show_physical, t("live-mapped")).clicked() {
                    st.show_physical = false;
                }
            });
        });
        let Some(s) = &c.state else {
            caption(ui, t("live-waiting"));
            return;
        };
        let inp: &Inputs = if st.show_physical {
            &s.physical
        } else {
            &s.mapped
        };
        ui.horizontal(|ui| {
            for (label, pos) in [(t("left-stick"), inp.left), (t("right-stick"), inp.right)] {
                ui.vertical(|ui| {
                    stick_view(ui, pos, 128.0);
                    caption(ui, format!("{label} · {:.2}, {:.2}", pos.0, pos.1));
                });
            }
            ui.add_space(12.0);
            for (label, v) in [("LT", inp.lt), ("RT", inp.rt)] {
                ui.vertical(|ui| {
                    trigger_bar(ui, v, 110.0);
                    caption(ui, format!("{label} {v}"));
                });
            }
        });
        let b = &inp.buttons;
        ui.horizontal_wrapped(|ui| {
            for (on, name) in [
                (b.a, "A"),
                (b.b, "B"),
                (b.x, "X"),
                (b.y, "Y"),
                (b.lb, "LB"),
                (b.rb, "RB"),
                (b.view, "View"),
                (b.menu, "Menu"),
                (b.home, "Home"),
                (b.screenshot, "Share"),
                (b.l4, "L4"),
                (b.r4, "R4"),
                (b.l5, "L5"),
                (b.r5, "R5"),
                (b.ls, "LS"),
                (b.rs, "RS"),
                (b.m, "M"),
                (b.dpad_up, "Up"),
                (b.dpad_down, "Down"),
                (b.dpad_left, "Left"),
                (b.dpad_right, "Right"),
            ] {
                chip(ui, on, name);
            }
        });
    });
}

fn vibration(ui: &mut Ui, c: &mut Connected, slot: Slot, st: &mut UiState) {
    card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        heading(ui, t("vibration"));
        let names = [
            t("level-off"),
            "25%".into(),
            "50%".into(),
            "75%".into(),
            "100%".into(),
        ];
        if let Some(p) = c.profiles.get_mut(&slot) {
            let m = &mut p.function.motors;
            for (label, v) in [
                (t("motor-left"), &mut m.left),
                (t("motor-right"), &mut m.right),
                (t("motor-lt"), &mut m.lt),
                (t("motor-rt"), &mut m.rt),
            ] {
                let mut lvl = motor_level(*v);
                if slider_u8(ui, &label, &mut lvl, 0..=4, |v| names[v as usize].clone()) {
                    *v = motor_raw(lvl);
                }
            }
        }
        ui.horizontal(|ui| {
            if ui.button(t("test-rumble")).clicked() {
                c.session.send(Request::Vibrate {
                    left: 180,
                    right: 180,
                    duration: Duration::from_millis(600),
                });
            }
            if ui.button(t("calibrate")).clicked() {
                st.calib_open = true;
            }
        });
    });
}

fn calibration_window(ui: &mut Ui, c: &mut Connected, st: &mut UiState) {
    let mut open = true;
    egui::Window::new(t("calibrate"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ui.ctx(), |ui| {
            ui.label(t("calibrate-help"));
            if let Some(s) = &c.state {
                let in_mode = s.calibration_mode != 1;
                let bits = s.calibration_status;
                ui.label(
                    RichText::new(if in_mode {
                        t("calibrate-active")
                    } else {
                        t("calibrate-idle")
                    })
                    .color(if in_mode { WARN } else { MUTED }),
                );
                ui.horizontal(|ui| {
                    for (i, n) in ["LS", "RS", "LT", "RT", "Gyro"].iter().enumerate() {
                        chip(ui, bits & (1 << i) != 0, n);
                    }
                });
            }
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                for (part, key) in [
                    (CalibrationPart::Sticks, "calibrate-sticks"),
                    (CalibrationPart::Triggers, "calibrate-triggers"),
                    (CalibrationPart::Motion, "calibrate-motion"),
                ] {
                    if ui.button(t(key)).clicked() {
                        c.session.send(Request::Calibrate { kind: 0, part });
                    }
                }
            });
            ui.horizontal(|ui| {
                if primary_button(ui, true, t("calibrate-finish")).clicked() {
                    c.session.send(Request::Calibrate {
                        kind: 1,
                        part: CalibrationPart::SticksAndTriggers,
                    });
                }
                if ui.button(t("calibrate-exit")).clicked() {
                    c.session.send(Request::Calibrate {
                        kind: 2,
                        part: CalibrationPart::SticksAndTriggers,
                    });
                }
            });
        });
    st.calib_open = open;
}

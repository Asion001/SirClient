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

/// Overview for a live-only connection (the G7 Pro XBox over Bluetooth):
/// battery, live input and a rumble test per motor.
pub fn show_live(ui: &mut Ui, c: &mut Connected, st: &mut UiState) {
    super::bt_notice(ui);
    ui.add_space(14.0);
    let battery = c.state.as_ref().map(|s| s.battery);
    super::columns(ui, 2, 420.0, |i, ui| {
        if i == 0 {
            live_input(ui, c, st);
        } else {
            card().show(ui, |ui| {
                ui.set_width(ui.available_width());
                caption(ui, t("stat-battery"));
                ui.label(
                    RichText::new(battery.map_or("—".into(), |b| format!("{b}%")))
                        .size(18.0)
                        .strong(),
                );
                ui.add_space(8.0);
                heading(ui, t("vibration"));
                let mut test = None;
                ui.horizontal_wrapped(|ui| {
                    for (i, key) in ["motor-left", "motor-right", "motor-lt", "motor-rt"]
                        .into_iter()
                        .enumerate()
                    {
                        if ui
                            .button(ta("test-motor", &[("motor", FluentValue::from(t(key)))]))
                            .clicked()
                        {
                            let mut v = [0u8; 4];
                            v[i] = 255;
                            test = Some(v);
                        }
                    }
                    if ui.button(t("test-rumble")).clicked() {
                        test = Some([180; 4]);
                    }
                });
                if let Some([left, right, lt, rt]) = test {
                    c.session.send(Request::Vibrate {
                        left,
                        right,
                        lt,
                        rt,
                        duration: Duration::from_millis(700),
                    });
                }
            });
        }
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
                let rates: &[(u8, &str)] = if c.xbox {
                    &sirclient_core::protocol::g7pro::xbox::REPORT_RATES
                } else {
                    &REPORT_RATES
                };
                if let Some(p) = c.profiles.get_mut(&slot) {
                    ui.horizontal_wrapped(|ui| {
                        for &(v, name) in rates {
                            if pill(ui, p.function.extend.report_rate == v, name).clicked() {
                                p.function.extend.report_rate = v;
                            }
                        }
                    });
                }
            });
        }
    });

    super::columns(ui, 2, 420.0, |i, ui| {
        if i == 0 {
            live_input(ui, c, st);
        } else {
            vibration(ui, c, slot, st);
        }
    });

    // The combos listed are the 8K's.
    if !c.xbox {
        shortcuts(ui);
    }

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
                    // Fixed width + sign-padded numbers: nothing shifts as
                    // the values change.
                    ui.set_width(136.0);
                    stick_view(ui, pos, 128.0);
                    caption(ui, label);
                    ui.label(
                        RichText::new(format!("{:+.2} {:+.2}", pos.0, pos.1))
                            .monospace()
                            .size(11.0)
                            .color(MUTED),
                    );
                });
            }
            ui.add_space(12.0);
            for (label, v) in [("LT", inp.lt), ("RT", inp.rt)] {
                ui.vertical(|ui| {
                    ui.set_width(44.0);
                    trigger_bar(ui, v, 110.0);
                    ui.label(
                        RichText::new(format!("{label} {v:>3}"))
                            .monospace()
                            .size(11.0)
                            .color(MUTED),
                    );
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
        if let Some(m) = s.motion {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    // Tilt from the accelerometer (about 8192 per g).
                    let g = |v: i16| (v as f32 / 8192.0).clamp(-1.0, 1.0);
                    stick_view(ui, (g(m.accel[0]), g(m.accel[1])), 96.0);
                    caption(ui, t("motion-tilt-view"));
                });
                ui.add_space(12.0);
                ui.vertical(|ui| {
                    caption(ui, t("motion-gyro"));
                    for (axis, v) in ["X", "Y", "Z"].iter().zip(m.gyro) {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(*axis).monospace());
                            let f = (v as f32 / 8000.0).clamp(-1.0, 1.0);
                            ui.add(
                                egui::ProgressBar::new(f.abs())
                                    .desired_width(140.0)
                                    .desired_height(6.0)
                                    .fill(if f < 0.0 { WARN } else { OK }),
                            );
                            ui.label(RichText::new(format!("{v:>6}")).monospace().color(MUTED));
                        });
                    }
                    ui.label(
                        RichText::new(format!(
                            "{} {:>6} {:>6} {:>6}",
                            t("motion-accel"),
                            m.accel[0],
                            m.accel[1],
                            m.accel[2]
                        ))
                        .monospace()
                        .size(11.0)
                        .color(MUTED),
                    );
                });
            });
        }
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
        // The 8K can only test the grip motors; the XBox drives all four.
        let mut test: Option<[u8; 4]> = None;
        if let Some(p) = c.profiles.get_mut(&slot) {
            let m = &mut p.function.motors;
            for (i, (label, v)) in [
                (t("motor-left"), &mut m.left),
                (t("motor-right"), &mut m.right),
                (t("motor-lt"), &mut m.lt),
                (t("motor-rt"), &mut m.rt),
            ]
            .into_iter()
            .enumerate()
            {
                let mut lvl = motor_level(*v);
                if slider_u8(ui, &label, &mut lvl, 0..=4, |v| names[v as usize].clone()) {
                    *v = motor_raw(lvl);
                }
                if c.xbox || i < 2 {
                    let strength = if lvl == 0 {
                        255
                    } else {
                        (lvl as u16 * 255 / 4) as u8
                    };
                    if ui
                        .small_button(ta("test-motor", &[("motor", FluentValue::from(label))]))
                        .clicked()
                    {
                        let mut v = [0u8; 4];
                        v[i] = strength;
                        test = Some(v);
                    }
                }
            }
        }
        ui.horizontal(|ui| {
            if ui.button(t("test-rumble")).clicked() {
                test = Some(if c.xbox { [180; 4] } else { [180, 180, 0, 0] });
            }
            // XBox calibration progress is not in its state stream yet.
            if !c.xbox && ui.button(t("calibrate")).clicked() {
                st.calib_open = true;
            }
        });
        if let Some([left, right, lt, rt]) = test {
            c.session.send(Request::Vibrate {
                left,
                right,
                lt,
                rt,
                duration: Duration::from_millis(700),
            });
        }
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

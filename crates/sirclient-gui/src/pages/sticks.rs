use egui::{RichText, Ui};
use sirclient_core::protocol::g7pro::Slot;
use sirclient_core::protocol::g7pro::curve::{self, AGGRESSIVE, CUSTOM, LINEAR, RELAXED};
use sirclient_core::protocol::g7pro::keys;
use sirclient_core::protocol::g7pro::profile::{AxisMap, Curve, Deadzone, Motion, Stick, Trigger};

use crate::app::{Connected, UiState};
use crate::i18n::t;
use crate::theme::*;
use crate::widgets::*;

pub fn show(ui: &mut Ui, c: &mut Connected, slot: Slot, st: &mut UiState) {
    let Some(p) = c.profiles.get_mut(&slot) else {
        return;
    };
    ui.horizontal(|ui| {
        for (i, key) in ["tab-sticks", "tab-triggers", "tab-motion"]
            .iter()
            .enumerate()
        {
            if pill(ui, st.sticks_tab == i, t(key)).clicked() {
                st.sticks_tab = i;
            }
        }
    });
    ui.add_space(10.0);
    match st.sticks_tab {
        0 => {
            let ext = &mut p.function.extend;
            let mut ls_j = ext.ls_anti_jitter;
            let mut rs_j = ext.rs_anti_jitter;
            let (ls, rs) = (&mut p.left_stick, &mut p.right_stick);
            super::columns(ui, 2, 440.0, |i, ui| {
                if i == 0 {
                    stick_card(ui, "ls", &t("left-stick"), ls, &mut ls_j);
                } else {
                    stick_card(ui, "rs", &t("right-stick"), rs, &mut rs_j);
                }
            });
            p.function.extend.ls_anti_jitter = ls_j;
            p.function.extend.rs_anti_jitter = rs_j;
        }
        1 => {
            let (lt, rt) = (&mut p.lt, &mut p.rt);
            super::columns(ui, 2, 400.0, |i, ui| {
                if i == 0 {
                    trigger_card(ui, "lt", "LT", lt);
                } else {
                    trigger_card(ui, "rt", "RT", rt);
                }
            });
        }
        _ => {
            ui.horizontal(|ui| {
                for (i, key) in ["motion-aim", "motion-tilt"].iter().enumerate() {
                    if pill(ui, st.motion_tab == i, t(key)).clicked() {
                        st.motion_tab = i;
                    }
                }
            });
            ui.add_space(8.0);
            let (aim, tilt) = (&mut p.motion_aim, &mut p.motion_tilt);
            let (this, other) = if st.motion_tab == 0 {
                (aim, tilt)
            } else {
                (tilt, aim)
            };
            motion_card(
                ui,
                if st.motion_tab == 0 { "aim" } else { "tilt" },
                this,
                other,
            );
        }
    }
}

fn curve_controls(
    ui: &mut Ui,
    id: &str,
    curve: &mut Curve,
    dz: &mut Deadzone,
    refresh: fn(&mut Curve, &Deadzone),
) {
    ui.horizontal(|ui| {
        let size = 190.0;
        if curve_editor(ui, (id, "curve"), curve, dz, size) {
            // Dragging makes it custom; nothing to recompute.
        }
        ui.vertical(|ui| {
            ui.horizontal_wrapped(|ui| {
                for (k, key) in [
                    (LINEAR, "curve-linear"),
                    (RELAXED, "curve-relaxed"),
                    (AGGRESSIVE, "curve-aggressive"),
                    (CUSTOM, "curve-custom"),
                ] {
                    if pill(ui, curve.kind == k, t(key)).clicked() {
                        curve.kind = k;
                        refresh(curve, dz);
                    }
                }
            });
            if (curve.kind == RELAXED || curve.kind == AGGRESSIVE)
                && slider_u8(
                    ui,
                    &t("curve-strength"),
                    &mut curve.curvature,
                    0..=100,
                    |v| v.to_string(),
                )
            {
                refresh(curve, dz);
            }
            let mut changed = false;
            changed |= slider_u16(ui, &t("dz-inner"), &mut dz.inner, 0..=500, percent);
            changed |= slider_u16(ui, &t("dz-outer"), &mut dz.outer, 500..=1000, percent);
            changed |= slider_u16(ui, &t("dz-anti"), &mut dz.anti_inner, 0..=500, percent);
            if changed {
                dz.enabled = 1;
                refresh(curve, dz);
            }
        });
    });
}

fn output_combo(ui: &mut Ui, id: &str, map: &mut AxisMap) {
    let opts = [
        (1u8, t("out-left")),
        (2, t("out-right")),
        (3, t("out-keys")),
        (4, t("out-mouse")),
    ];
    let cur = opts
        .iter()
        .find(|(v, _)| *v == map.output)
        .map(|(_, n)| n.clone())
        .unwrap_or_else(|| format!("{}", map.output));
    ui.horizontal(|ui| {
        ui.label(t("acts-as"));
        egui::ComboBox::from_id_salt((id, "out"))
            .selected_text(cur)
            .show_ui(ui, |ui| {
                for (v, n) in &opts {
                    if ui.selectable_label(map.output == *v, n).clicked() {
                        map.output = *v;
                        map.enabled = 1;
                    }
                }
            });
    });
    ui.horizontal(|ui| {
        let mut x = map.flip_x != 0;
        if ui.checkbox(&mut x, t("invert-x")).changed() {
            map.flip_x = x as u8;
        }
        let mut y = map.flip_y != 0;
        if ui.checkbox(&mut y, t("invert-y")).changed() {
            map.flip_y = y as u8;
        }
    });
    if map.output == 4 {
        slider_u8(ui, &t("mouse-speed"), &mut map.mouse_dpi, 1..=100, |v| {
            v.to_string()
        });
    }
    if map.output == 3 {
        ui.horizontal_wrapped(|ui| {
            for (label, k) in [
                ("Up", &mut map.up),
                ("Down", &mut map.down),
                ("Left", &mut map.left),
                ("Right", &mut map.right),
            ] {
                ui.label(label);
                key_combo(ui, (id, label), k, Some("—"));
            }
        });
    }
}

fn stick_card(ui: &mut Ui, id: &str, title: &str, s: &mut Stick, jitter: &mut u8) {
    card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            heading(ui, title);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut sq = s.square != 0;
                if ui.checkbox(&mut sq, t("square-output")).changed() {
                    s.square = sq as u8;
                    s.enabled = 1;
                }
            });
        });
        curve_controls(ui, id, &mut s.curve, &mut s.deadzone, curve::refresh_stick);
        // Raw 1..=21 shown as -10..=10; 0 means "unset" (same as 11).
        let mut shown: i32 = if *jitter == 0 { 0 } else { *jitter as i32 - 11 };
        ui.horizontal(|ui| {
            ui.label(t("anti-jitter"));
            ui.spacing_mut().slider_width = 160.0;
            if ui.add(egui::Slider::new(&mut shown, -10..=10)).changed() {
                *jitter = (shown + 11) as u8;
            }
        });
        ui.separator();
        output_combo(ui, id, &mut s.map);
    });
}

fn trigger_card(ui: &mut Ui, id: &str, title: &str, tr: &mut Trigger) {
    card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        heading(ui, title);
        curve_controls(
            ui,
            id,
            &mut tr.curve,
            &mut tr.deadzone,
            curve::refresh_trigger,
        );
        ui.separator();
        let mut on = tr.hair_trigger & 0x80 != 0;
        if ui.checkbox(&mut on, t("hair-trigger")).changed() {
            tr.hair_trigger = if on { 0x81 } else { 0 };
        }
        if on {
            ui.horizontal(|ui| {
                if ui
                    .radio(tr.hair_trigger & 0x01 != 0, t("hair-mode-1"))
                    .clicked()
                {
                    tr.hair_trigger = 0x81;
                }
                if ui
                    .radio(tr.hair_trigger & 0x02 != 0, t("hair-mode-2"))
                    .clicked()
                {
                    tr.hair_trigger = 0x82;
                }
            });
            slider_u8(ui, &t("hair-start"), &mut tr.hair_start, 1..=100, |v| {
                format!("{v}%")
            });
            slider_u8(ui, &t("hair-end"), &mut tr.hair_end, 1..=100, |v| {
                format!("{v}%")
            });
        }
    });
}

fn motion_card(ui: &mut Ui, id: &str, m: &mut Motion, other: &mut Motion) {
    card().show(ui, |ui| {
        ui.set_width(ui.available_width().min(760.0));
        let acts = [
            (0u8, t("motion-off")),
            (1, t("motion-hold")),
            (2, t("motion-toggle")),
            (3, t("motion-always")),
        ];
        ui.horizontal(|ui| {
            ui.label(t("motion-activation"));
            let cur = acts
                .iter()
                .find(|(v, _)| *v == m.activation)
                .map(|(_, n)| n.clone())
                .unwrap_or_default();
            egui::ComboBox::from_id_salt((id, "act"))
                .selected_text(cur)
                .show_ui(ui, |ui| {
                    for (v, n) in &acts {
                        if ui.selectable_label(m.activation == *v, n).clicked() {
                            m.activation = *v;
                            // Only one gyro mode can be active (same as the official app).
                            if *v > 0 {
                                other.activation = 0;
                            }
                        }
                    }
                });
        });
        if m.activation == 1 || m.activation == 2 {
            ui.horizontal(|ui| {
                ui.label(t("motion-button"));
                let mut k = if m.trigger_keys[0] == keys::NC {
                    0
                } else {
                    m.trigger_keys[0]
                };
                if key_combo(ui, (id, "trig"), &mut k, Some("—")) {
                    m.trigger_keys[0] = if k == 0 { keys::NC } else { k };
                }
            });
        }
        let axes = [
            (0u8, t("axis-yaw")),
            (1, t("axis-roll")),
            (2, t("axis-both")),
        ];
        let cur_axis = match m.active_axis & 3 {
            3 => 2,
            1 => 1,
            _ => 0,
        };
        ui.horizontal(|ui| {
            ui.label(t("motion-axis"));
            for (v, n) in &axes {
                if pill(ui, cur_axis == *v, n.clone()).clicked() {
                    m.active_axis = match v {
                        2 => 3,
                        1 => 1,
                        _ => 2,
                    };
                }
            }
        });
        slider_u8(
            ui,
            &t("motion-sensitivity"),
            &mut m.map.stick_sensitivity,
            1..=100,
            |v| v.to_string(),
        );
        ui.separator();
        output_combo(ui, id, &mut m.map);
        ui.add_space(4.0);
        ui.label(RichText::new(t("motion-note")).size(12.0).color(FAINT));
    });
}

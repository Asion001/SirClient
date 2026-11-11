use egui::{RichText, Ui};
use fluent_bundle::FluentValue;
use sirclient_core::protocol::g7pro::Slot;
use sirclient_core::protocol::g7pro::keys;
use sirclient_core::protocol::g7pro::profile::{
    BACK_BUTTONS, BASIC_BUTTONS, KeyMap, MACRO_STEPS, Macro, MacroStep,
};

use crate::app::{Connected, UiState};
use crate::i18n::{t, ta};
use crate::theme::*;
use crate::widgets::*;

fn map_cell(ui: &mut Ui, id: &str, map: &mut KeyMap) {
    let default = t("map-default");
    if map.enabled == 0 {
        let mut code = 0u8;
        if key_combo(ui, (id, 0), &mut code, Some(&default)) && code != 0 {
            *map = KeyMap {
                enabled: 1,
                keys: [code, 0, 0],
            };
        }
        return;
    }
    ui.horizontal(|ui| {
        let mut first = map.keys[0];
        if key_combo(ui, (id, 0), &mut first, Some(&default)) {
            if first == 0 {
                map.enabled = 0;
                map.keys = [0; 3];
                return;
            }
            map.keys[0] = first;
        }
        // Extra simultaneous keys ("combo").
        for i in 1..3 {
            if map.keys[i] != 0 {
                let mut k = map.keys[i];
                if key_combo(ui, (id, i), &mut k, Some("—")) {
                    map.keys[i] = k;
                }
            }
        }
        if map.keys.iter().filter(|k| **k != 0).count() < 3
            && ui
                .small_button("+")
                .on_hover_text(t("map-add-key"))
                .clicked()
            && let Some(slot) = map.keys.iter_mut().find(|k| **k == 0)
        {
            *slot = keys::code("CRO").unwrap_or(9);
        }
    });
}

fn turbo_cell(ui: &mut Ui, enabled: &mut u8, speed: &mut u8) {
    let mut v = if *enabled != 0 { *speed } else { 0 };
    let text = if v == 0 {
        t("turbo-off")
    } else {
        format!("{v} Hz")
    };
    ui.spacing_mut().slider_width = 80.0;
    if ui
        .add(egui::Slider::new(&mut v, 0..=30).show_value(false))
        .changed()
    {
        *enabled = (v > 0) as u8;
        *speed = v;
    }
    ui.label(RichText::new(text).color(MUTED));
}

pub fn show(ui: &mut Ui, c: &mut Connected, slot: Slot, st: &mut UiState) {
    let Some(p) = c.profiles.get_mut(&slot) else {
        return;
    };
    ui.spacing_mut().combo_width = 150.0;
    super::columns(ui, 2, 460.0, |col, ui| {
        if col == 0 {
            card().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    heading(ui, t("remap"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut swap = p.function.extend.xinput_abxy_swap != 0;
                        if ui.checkbox(&mut swap, t("swap-abxy")).changed() {
                            p.function.extend.xinput_abxy_swap = swap as u8;
                        }
                    });
                });
                egui::Grid::new("remap-grid")
                    .num_columns(4)
                    .striped(false)
                    .spacing([14.0, 6.0])
                    .show(ui, |ui| {
                        for h in [
                            t("col-button"),
                            t("col-sends"),
                            t("col-turbo"),
                            t("col-toggle"),
                        ] {
                            ui.label(RichText::new(h).size(12.0).color(FAINT));
                        }
                        ui.end_row();
                        for (i, name) in BASIC_BUTTONS.iter().enumerate() {
                            let b = &mut p.buttons[i];
                            ui.label(RichText::new(keys::display_name(name)).strong());
                            map_cell(ui, name, &mut b.map);
                            ui.horizontal(|ui| {
                                turbo_cell(ui, &mut b.turbo_enabled, &mut b.turbo_speed)
                            });
                            let mut tog = b.toggle != 0;
                            if ui.checkbox(&mut tog, "").changed() {
                                b.toggle = tog as u8;
                            }
                            ui.end_row();
                        }
                        for (name, trig) in [("L2", &mut p.lt), ("R2", &mut p.rt)] {
                            ui.label(RichText::new(keys::display_name(name)).strong());
                            map_cell(ui, name, &mut trig.map);
                            ui.horizontal(|ui| {
                                turbo_cell(ui, &mut trig.turbo_enabled, &mut trig.turbo_speed)
                            });
                            let mut tog = trig.toggle != 0;
                            if ui.checkbox(&mut tog, "").changed() {
                                trig.toggle = tog as u8;
                            }
                            ui.end_row();
                        }
                    });
            });
        } else {
            back_buttons(ui, p, st);
        }
    });
}

fn back_buttons(ui: &mut Ui, p: &mut sirclient_core::protocol::g7pro::Profile, st: &mut UiState) {
    card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        heading(ui, t("back-buttons"));
        ui.horizontal(|ui| {
            for (i, name) in BACK_BUTTONS.iter().enumerate() {
                if pill(ui, st.back_tab == i, keys::display_name(name)).clicked() {
                    st.back_tab = i;
                }
            }
        });
        let bb = &mut p.back_buttons[st.back_tab];
        let id = BACK_BUTTONS[st.back_tab];
        let mut is_macro = bb.macro_.flags & Macro::ENABLED != 0;
        ui.horizontal(|ui| {
            if ui.radio(!is_macro, t("action-single")).clicked() {
                is_macro = false;
                bb.macro_.flags &= !Macro::ENABLED;
            }
            if ui.radio(is_macro, t("action-macro")).clicked() {
                is_macro = true;
                bb.button.map.enabled = 0;
                if bb.macro_.step_count == 0 {
                    bb.macro_.step_count = 1;
                    bb.macro_.steps.resize(MACRO_STEPS, MacroStep::default());
                    bb.macro_.steps[0] = MacroStep {
                        key: keys::code("CRO").unwrap_or(9),
                        hold: 40,
                        delay: 20,
                    };
                }
                bb.macro_.flags |= Macro::ENABLED;
            }
        });
        if !is_macro {
            ui.horizontal(|ui| {
                ui.label(t("col-sends"));
                map_cell(ui, id, &mut bb.button.map);
            });
            ui.horizontal(|ui| {
                ui.label(t("col-turbo"));
                turbo_cell(ui, &mut bb.button.turbo_enabled, &mut bb.button.turbo_speed);
            });
            return;
        }

        let m = &mut bb.macro_;
        m.steps.resize(MACRO_STEPS, MacroStep::default());
        let count = (m.step_count as usize).min(MACRO_STEPS);
        let mut remove = None;
        egui::Grid::new(("macro", id))
            .num_columns(5)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                for h in ["#", &t("macro-key"), &t("macro-hold"), &t("macro-wait"), ""] {
                    ui.label(RichText::new(h).size(12.0).color(FAINT));
                }
                ui.end_row();
                for i in 0..count {
                    let s = &mut m.steps[i];
                    ui.label(RichText::new(format!("{}", i + 1)).color(FAINT));
                    key_combo(ui, (id, "step", i), &mut s.key, None);
                    ui.add(
                        egui::DragValue::new(&mut s.hold)
                            .range(0..=60000)
                            .suffix(" ms"),
                    );
                    if i + 1 < MACRO_STEPS {
                        ui.add(
                            egui::DragValue::new(&mut s.delay)
                                .range(0..=60000)
                                .suffix(" ms"),
                        );
                    } else {
                        ui.label("—");
                    }
                    if ui
                        .small_button("✕")
                        .on_hover_text(t("macro-remove"))
                        .clicked()
                    {
                        remove = Some(i);
                    }
                    ui.end_row();
                }
            });
        let mut count = count;
        if let Some(i) = remove {
            m.steps.remove(i);
            m.steps.push(MacroStep::default());
            count -= 1;
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(count < MACRO_STEPS, egui::Button::new(t("macro-add")))
                .clicked()
            {
                let last_key = if count > 0 {
                    m.steps[count - 1].key
                } else {
                    keys::code("CRO").unwrap_or(9)
                };
                m.steps[count] = MacroStep {
                    key: last_key,
                    hold: 40,
                    delay: 20,
                };
                count += 1;
            }
        });
        m.step_count = count as u8;
        if count == 0 {
            m.flags &= !Macro::ENABLED;
        }
        ui.separator();
        let mut repeat = m.flags & Macro::REPEAT != 0;
        if ui.checkbox(&mut repeat, t("macro-repeat")).changed() {
            m.flags = if repeat {
                m.flags | Macro::REPEAT
            } else {
                m.flags & !Macro::REPEAT
            };
        }
        if repeat {
            ui.horizontal(|ui| {
                ui.label(t("macro-cycle"));
                ui.add(
                    egui::DragValue::new(&mut m.cycle_time)
                        .range(0..=60000)
                        .suffix(" ms"),
                );
            });
        }
        let mut press = m.flags & Macro::PRESS_TO_TRIGGER != 0;
        if ui.checkbox(&mut press, t("macro-on-press")).changed() {
            m.flags = if press {
                m.flags | Macro::PRESS_TO_TRIGGER
            } else {
                m.flags & !Macro::PRESS_TO_TRIGGER
            };
        }
        caption(
            ui,
            ta(
                "macro-used",
                &[
                    ("used", FluentValue::from(count)),
                    ("max", FluentValue::from(MACRO_STEPS)),
                ],
            ),
        );
    });
}

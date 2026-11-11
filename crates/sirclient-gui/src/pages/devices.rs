//! Every HID collection on the system, plus a probe to see what a
//! controller exposes over a connection (e.g. Bluetooth).

use std::time::Duration;

use crossbeam_channel::{Receiver, unbounded};
use egui::{RichText, Ui};
use fluent_bundle::FluentValue;
use sirclient_core::probe::{self, ProbeResult};
use sirclient_core::registry::GAMESIR_VID;
use sirclient_core::transport::{Bus, Hid, HidInfo};
// Probes open their own hidapi context on a worker thread (never the UI thread).

use crate::app::App;
use crate::i18n::{t, ta};
use crate::theme::*;
use crate::widgets::*;

#[derive(Default)]
pub struct ProbeUi {
    pub target: Option<HidInfo>,
    pub sent: bool,
    pub running: bool,
    pub result: Option<Result<ProbeResult, String>>,
    rx: Option<Receiver<Result<ProbeResult, String>>>,
}

impl ProbeUi {
    pub fn poll(&mut self) {
        if let Some(rx) = &self.rx
            && let Ok(r) = rx.try_recv()
        {
            self.result = Some(r);
            self.running = false;
            self.rx = None;
        }
    }

    fn start(&mut self, ctx: &egui::Context, info: HidInfo, send: bool) {
        let (tx, rx) = unbounded();
        self.target = Some(info.clone());
        self.sent = send;
        self.running = true;
        self.result = None;
        self.rx = Some(rx);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let r = (|| {
                let hid = Hid::new().map_err(|e| e.to_string())?;
                let mut dev = hid.open(&info.path).map_err(|e| e.to_string())?;
                let d = Duration::from_millis(1500);
                if send {
                    probe::handshake(&mut dev, d)
                } else {
                    probe::listen(&mut dev, d)
                }
                .map_err(|e| e.to_string())
            })();
            let _ = tx.send(r);
            ctx.request_repaint();
        });
    }
}

fn is_controllerish(d: &HidInfo) -> bool {
    d.vid == GAMESIR_VID
        || (d.usage_page == 1 && matches!(d.usage, 4 | 5))
        || (d.is_vendor_page() && d.bus == Bus::Bluetooth)
}

/// Writing the 0x0F report is only offered where it can't hurt: GameSir
/// devices and vendor-defined collections.
fn can_handshake(d: &HidInfo) -> bool {
    d.vid == GAMESIR_VID || d.is_vendor_page()
}

fn role(d: &HidInfo) -> String {
    if d.is_config_interface() {
        let implemented = d.model().is_some_and(|(m, _)| m.protocol.is_implemented());
        return if implemented {
            t("role-config")
        } else {
            t("role-config-unsupported")
        };
    }
    match (d.usage_page, d.usage) {
        (1, 4) => t("role-joystick"),
        (1, 5) => t("role-gamepad"),
        (1, 6) => t("role-keyboard"),
        (1, 2) => t("role-mouse"),
        (0x0C, _) => t("role-media"),
        (p, _) if p >= 0xFF00 => t("role-vendor"),
        _ => "—".into(),
    }
}

pub fn show(ui: &mut Ui, app: &mut App) {
    if let Some(e) = &app.hid_error {
        ui.colored_label(WARN, e);
    }
    ui.horizontal(|ui| {
        ui.checkbox(&mut app.ui.controllers_only, t("devices-controllers-only"));
        ui.checkbox(&mut app.ui.show_tech, t("devices-technical"));
        if ui.button(t("devices-rescan")).clicked() {
            app.rescan();
        }
    });
    ui.add_space(6.0);
    let list: Vec<HidInfo> = app
        .all_hid
        .iter()
        .filter(|d| !app.ui.controllers_only || is_controllerish(d))
        .cloned()
        .collect();
    let mut start: Option<(HidInfo, bool)> = None;
    let show_tech = app.ui.show_tech;
    card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        if list.is_empty() {
            caption(ui, t("devices-none"));
            return;
        }
        egui::ScrollArea::horizontal().show(ui, |ui| {
            egui::Grid::new("hid-grid")
                .num_columns(4)
                .striped(true)
                .spacing([18.0, 8.0])
                .show(ui, |ui| {
                    for h in ["col-device", "col-bus", "col-role", ""] {
                        ui.label(
                            RichText::new(if h.is_empty() { String::new() } else { t(h) })
                                .size(12.0)
                                .color(FAINT),
                        );
                    }
                    ui.end_row();
                    for d in &list {
                        ui.vertical(|ui| {
                            ui.label(RichText::new(d.label()).strong());
                            if show_tech {
                                ui.label(
                                    RichText::new(format!(
                                        "{:04X}:{:04X} · page {:04X}/{:04X}",
                                        d.vid, d.pid, d.usage_page, d.usage
                                    ))
                                    .monospace()
                                    .size(11.0)
                                    .color(FAINT),
                                );
                            }
                        });
                        let (txt, col) = match d.bus {
                            Bus::Bluetooth => ("Bluetooth", BT),
                            Bus::Usb => ("USB", OK),
                            Bus::Other => ("?", MUTED),
                        };
                        ui.label(RichText::new(txt).color(col));
                        ui.label(RichText::new(role(d)).color(MUTED));
                        ui.horizontal(|ui| {
                            let busy = app.probe.running;
                            if ui
                                .add_enabled(!busy, egui::Button::new(t("probe-listen")))
                                .clicked()
                            {
                                start = Some((d.clone(), false));
                            }
                            if can_handshake(d)
                                && primary_button(ui, !busy, t("probe-handshake")).clicked()
                            {
                                start = Some((d.clone(), true));
                            }
                        });
                        ui.end_row();
                    }
                });
        });
    });
    if let Some((d, send)) = start {
        app.probe.start(ui.ctx(), d, send);
    }

    ui.add_space(14.0);
    let p = &app.probe;
    let Some(target) = &p.target else {
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            heading(ui, t("probe-title"));
            ui.label(RichText::new(t("probe-help")).color(MUTED));
        });
        return;
    };
    card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        heading(
            ui,
            ta(
                "probe-of",
                &[(
                    "device",
                    FluentValue::from(if show_tech {
                        format!("{} · {:04X}", target.label(), target.usage_page)
                    } else {
                        format!("{} · {}", target.label(), role(target))
                    }),
                )],
            ),
        );
        if p.running {
            ui.horizontal(|ui| {
                spinner(ui);
                ui.label(t("probe-running"));
            });
            return;
        }
        match &p.result {
            Some(Ok(r)) => {
                let verdict = if !p.sent {
                    ta(
                        "probe-listened",
                        &[("count", FluentValue::from(r.log.len()))],
                    )
                } else if let Some(fw) = &r.firmware {
                    ta("probe-ok", &[("version", FluentValue::from(fw.clone()))])
                } else {
                    t("probe-no-answer")
                };
                let ok = r.answered();
                egui::Frame::new()
                    .fill(if ok {
                        egui::Color32::from_rgb(0x16, 0x2A, 0x1E)
                    } else {
                        egui::Color32::from_rgb(0x1A, 0x23, 0x32)
                    })
                    .stroke(egui::Stroke::new(
                        1.0,
                        if ok {
                            OK
                        } else {
                            egui::Color32::from_rgb(0x2C, 0x4A, 0x7A)
                        },
                    ))
                    .corner_radius(egui::CornerRadius::same(10))
                    .inner_margin(egui::Margin::same(12))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(verdict);
                    });
                let mut text = String::new();
                for c in &r.log {
                    text.push_str(&format!(
                        "{:>5} ms {} {}\n",
                        c.at_ms,
                        if c.outgoing { "TX" } else { "RX" },
                        probe::hex(&c.bytes, 24)
                    ));
                }
                ui.add_space(6.0);
                egui::CollapsingHeader::new(ta(
                    "probe-raw",
                    &[("count", FluentValue::from(r.log.len()))],
                ))
                .id_salt("probe-raw")
                .default_open(show_tech)
                .show(ui, |ui| {
                    egui::Frame::new()
                        .fill(SUNKEN)
                        .stroke(egui::Stroke::new(1.0, BORDER))
                        .corner_radius(egui::CornerRadius::same(10))
                        .inner_margin(egui::Margin::same(12))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(
                                RichText::new(if text.is_empty() {
                                    t("probe-empty")
                                } else {
                                    text
                                })
                                .monospace()
                                .size(12.0)
                                .color(TEXT_2),
                            );
                        });
                });
            }
            Some(Err(e)) => {
                ui.colored_label(
                    WARN,
                    ta("probe-error", &[("error", FluentValue::from(e.clone()))]),
                );
            }
            None => {}
        }
    });
}

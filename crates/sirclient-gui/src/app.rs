//! Application state, layout shell (sidebar + header) and page routing.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{RichText, Ui};
use fluent_bundle::FluentValue;
use sirclient_core::mock::{MOCK_PATH, MockG7};
use sirclient_core::protocol::g7pro::xbox::XboxLight;
use sirclient_core::protocol::g7pro::{LightProfile, Profile, Slot, State};
use sirclient_core::registry::{Connection, Protocol};
use sirclient_core::session::{Event, Request, Session};
use sirclient_core::transport::{HidInfo, HidService};

use crate::i18n::{self, t, ta};
use crate::pages::{self, devices::ProbeUi};
use crate::theme::*;
use crate::widgets::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Overview,
    Buttons,
    Sticks,
    Lighting,
    Devices,
}

/// Everything known about the connected controller.
pub struct Connected {
    pub id: String,
    pub title: String,
    pub connection: Option<Connection>,
    pub session: Session,
    pub firmware: Option<(String, String)>,
    /// Bytes as last read from / confirmed written to the controller.
    pub loaded: HashMap<Slot, Vec<u8>>,
    /// Bytes sent to the controller and not acknowledged yet.
    pub writing: HashMap<Slot, Vec<u8>>,
    /// Working copies being edited.
    pub profiles: HashMap<Slot, Profile>,
    pub light: Option<LightProfile>,
    /// Lighting slot of the G7 Pro XBox (a different model from the 8K's).
    pub xlight: Option<XboxLight>,
    pub current_profile: u8,
    pub state: Option<State>,
    pub pending: usize,
    pub connected_at: Instant,
    /// G7 Pro XBox over GIP: no macros, its own lighting model and report
    /// rates.
    pub xbox: bool,
    /// G7 Pro XBox over Bluetooth: live input, battery and rumble only.
    pub bt: bool,
}

impl Connected {
    pub fn is_mock(&self) -> bool {
        self.id == MOCK_PATH
    }

    pub fn loaded_all(&self) -> bool {
        Slot::ALL.iter().all(|s| self.loaded.contains_key(s))
    }

    pub fn dirty_slots(&self) -> Vec<Slot> {
        let mut out = vec![];
        for (slot, p) in &self.profiles {
            if self.loaded.get(slot).is_some_and(|b| *b != p.to_bytes()) {
                out.push(*slot);
            }
        }
        if let (Some(l), Some(b)) = (&self.light, self.loaded.get(&Slot::Light))
            && *b != l.to_bytes()
        {
            out.push(Slot::Light);
        }
        if let (Some(l), Some(b)) = (&self.xlight, self.loaded.get(&Slot::Light))
            && *b != l.to_bytes()
        {
            out.push(Slot::Light);
        }
        out.sort_by_key(|s| s.number());
        out
    }

    pub fn apply(&mut self) {
        for slot in self.dirty_slots() {
            let bytes = match slot {
                Slot::Light if self.xbox => self.xlight.as_ref().map(|l| l.to_bytes()),
                Slot::Light => self.light.as_ref().map(|l| l.to_bytes()),
                s => self.profiles.get(&s).map(|p| p.to_bytes()),
            };
            if let Some(b) = bytes
                && self.writing.get(&slot) != Some(&b)
            {
                self.session.send(Request::WriteSlot(slot, b.clone()));
                // Counts as saved once the controller acknowledges it.
                self.writing.insert(slot, b);
            }
        }
    }

    pub fn revert(&mut self) {
        for (slot, bytes) in &self.loaded {
            match slot {
                Slot::Light if self.xbox => self.xlight = XboxLight::from_bytes(bytes).ok(),
                Slot::Light => self.light = LightProfile::from_bytes(bytes).ok(),
                s => {
                    if let Ok(p) = Profile::from_bytes(bytes) {
                        self.profiles.insert(*s, p);
                    }
                }
            }
        }
    }

    fn take_slot(&mut self, slot: Slot, bytes: Vec<u8>, keep_edits: bool) {
        let dirty = self.dirty_slots().contains(&slot);
        self.loaded.insert(slot, bytes.clone());
        if keep_edits && dirty {
            return;
        }
        match slot {
            Slot::Light if self.xbox => self.xlight = XboxLight::from_bytes(&bytes).ok(),
            Slot::Light => self.light = LightProfile::from_bytes(&bytes).ok(),
            s => {
                if let Ok(p) = Profile::from_bytes(&bytes) {
                    self.profiles.insert(s, p);
                }
            }
        }
    }
}

pub struct Toast {
    pub text: String,
    pub warn: bool,
    pub at: Instant,
}

pub struct App {
    ctx: egui::Context,
    hid: HidService,
    pub hid_error: Option<String>,
    pub all_hid: Vec<HidInfo>,
    scanned: bool,
    pub conn: Option<Connected>,
    /// User disconnected on purpose; don't auto-connect again.
    manual_disconnect: bool,
    pub page: Page,
    pub edit_slot: Slot,
    pub ui: UiState,
    pub probe: ProbeUi,
    toasts: Vec<Toast>,
    /// G7 Pro XBox connection being opened off the UI thread (the USB reset
    /// out of its HID fallback takes several seconds).
    gip_opening: Option<(
        crossbeam_channel::Receiver<Result<sirclient_core::gip::GipLink, String>>,
        HidInfo,
    )>,
    shots: Option<Shots>,
    /// Widths of the header's profile pills and Revert / Apply block,
    /// measured on the previous frame.
    header_tools_w: (f32, f32),
}

/// `--screenshot <dir>`: render every page against the demo device, save
/// PNGs and quit. Used for docs and visual checks.
struct Shots {
    dir: std::path::PathBuf,
    queue: Vec<(Page, &'static str)>,
    frames: u32,
    requested: bool,
    done: Arc<std::sync::atomic::AtomicBool>,
}

/// Per-page view state that is not part of the device.
pub struct UiState {
    pub show_physical: bool,
    pub back_tab: usize,
    pub sticks_tab: usize,
    pub motion_tab: usize,
    pub light_preset: usize,
    /// Keyframe being edited on the G7 Pro XBox lighting page.
    pub light_frame: usize,
    pub calib_open: bool,
    pub controllers_only: bool,
    /// Show VID/PID, usage pages and raw reports on the devices page.
    pub show_tech: bool,
}

impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        mock: bool,
        shots: Option<std::path::PathBuf>,
    ) -> Self {
        apply(&cc.egui_ctx);
        let lang = cc.storage.and_then(|s| s.get_string("language"));
        i18n::init(lang.as_deref());
        let ctx = cc.egui_ctx.clone();
        let hid = HidService::spawn(
            Duration::from_secs(2),
            Arc::new(move || ctx.request_repaint()),
        );
        let mut app = App {
            ctx: cc.egui_ctx.clone(),
            hid,
            hid_error: None,
            all_hid: vec![],
            scanned: false,
            conn: None,
            manual_disconnect: false,
            page: Page::Overview,
            edit_slot: Slot::Profile(1),
            ui: UiState {
                show_physical: false,
                back_tab: 0,
                sticks_tab: 0,
                motion_tab: 0,
                light_preset: 0,
                light_frame: 0,
                calib_open: false,
                controllers_only: true,
                show_tech: false,
            },
            probe: ProbeUi::default(),
            toasts: vec![],
            gip_opening: None,
            header_tools_w: (360.0, 260.0),
            shots: shots.map(|dir| Shots {
                dir,
                queue: vec![
                    (Page::Overview, "overview"),
                    (Page::Buttons, "buttons"),
                    (Page::Sticks, "sticks"),
                    (Page::Lighting, "lighting"),
                    (Page::Devices, "devices"),
                ],
                frames: 0,
                requested: false,
                done: Default::default(),
            }),
        };
        if mock || app.shots.is_some() {
            app.connect_mock();
        }
        app
    }

    pub fn toast(&mut self, text: String, warn: bool) {
        self.toasts.push(Toast {
            text,
            warn,
            at: Instant::now(),
        });
    }

    fn notifier(&self) -> Arc<dyn Fn() + Send + Sync> {
        let ctx = self.ctx.clone();
        Arc::new(move || ctx.request_repaint())
    }

    fn new_connected(
        &self,
        id: String,
        title: String,
        info: Option<&HidInfo>,
        session: Session,
    ) -> Connected {
        Connected {
            id,
            title,
            connection: info.and_then(|i| i.model()).map(|(_, m)| m.connection),
            session,
            firmware: None,
            loaded: HashMap::new(),
            writing: HashMap::new(),
            profiles: HashMap::new(),
            light: None,
            xlight: None,
            current_profile: 1,
            state: None,
            pending: 0,
            connected_at: Instant::now(),
            xbox: info.and_then(|i| i.model()).is_some_and(|(m, _)| {
                matches!(m.protocol, Protocol::G7ProXbox | Protocol::XboxBluetooth)
            }),
            bt: info
                .and_then(|i| i.model())
                .is_some_and(|(m, _)| m.protocol == Protocol::XboxBluetooth),
        }
    }

    pub fn connect_mock(&mut self) {
        let s = Session::spawn(Box::new(MockG7::new()), self.notifier());
        self.conn = Some(self.new_connected(MOCK_PATH.into(), t("device-demo"), None, s));
        self.manual_disconnect = false;
    }

    pub fn connect(&mut self, info: &HidInfo) {
        if info
            .model()
            .is_some_and(|(m, _)| m.protocol == Protocol::G7ProXbox)
        {
            self.connect_gip(info);
            return;
        }
        let bt = info
            .model()
            .is_some_and(|(m, _)| m.protocol == Protocol::XboxBluetooth);
        match self.hid.open(&info.path) {
            Ok(dev) => {
                let link: Box<dyn sirclient_core::session::Link> = if bt {
                    Box::new(sirclient_core::bt::BtLink::new(dev))
                } else {
                    Box::new(dev)
                };
                let s = Session::spawn(link, self.notifier());
                let title = info
                    .model()
                    .map(|(m, _)| m.name.to_string())
                    .unwrap_or_else(|| info.label());
                self.conn = Some(self.new_connected(info.path.clone(), title, Some(info), s));
                self.manual_disconnect = false;
            }
            Err(e) => {
                let mut msg = ta(
                    "toast-open-failed",
                    &[("error", FluentValue::from(e.to_string()))],
                );
                if cfg!(target_os = "linux") {
                    msg.push('\n');
                    msg.push_str(&t("hint-udev"));
                }
                self.toast(msg, true)
            }
        }
    }

    /// G7 Pro XBox: talk GIP over raw USB (resetting it out of the HID
    /// fallback first when needed).
    fn connect_gip(&mut self, info: &HidInfo) {
        if self.gip_opening.is_some() {
            return;
        }
        let (tx, rx) = crossbeam_channel::bounded(1);
        let notify = self.notifier();
        std::thread::spawn(move || {
            let r = sirclient_core::gip::GipLink::open().map_err(|e| e.to_string());
            let _ = tx.send(r);
            notify();
        });
        self.gip_opening = Some((rx, info.clone()));
        self.toast(t("toast-connecting"), false);
    }

    /// Finish a connection started by [`Self::connect_gip`].
    fn poll_gip_opening(&mut self) {
        let Some((rx, _)) = &self.gip_opening else {
            return;
        };
        let Ok(result) = rx.try_recv() else { return };
        let (_, info) = self.gip_opening.take().unwrap();
        match result {
            Ok(link) => {
                let id = link.path().to_string();
                let s = Session::spawn(Box::new(link), self.notifier());
                let title = info
                    .model()
                    .map(|(m, _)| m.name.to_string())
                    .unwrap_or_else(|| info.label());
                self.conn = Some(self.new_connected(id, title, Some(&info), s));
                self.manual_disconnect = false;
                self.hid.rescan();
            }
            Err(e) => {
                let mut msg = ta("toast-open-failed", &[("error", FluentValue::from(e))]);
                if cfg!(target_os = "linux") {
                    msg.push('\n');
                    msg.push_str(&t("hint-udev"));
                }
                self.toast(msg, true)
            }
        }
    }

    pub fn disconnect(&mut self) {
        self.conn = None;
        self.manual_disconnect = true;
    }

    pub fn rescan(&mut self) {
        self.hid.rescan();
    }

    pub fn config_devices(&self) -> Vec<HidInfo> {
        self.all_hid
            .iter()
            .filter(|d| d.is_config_interface())
            .cloned()
            .collect()
    }

    fn poll(&mut self) {
        self.poll_gip_opening();
        if let Some(scan) = self.hid.latest() {
            match scan {
                Ok(list) => {
                    self.all_hid = list;
                    self.hid_error = None;
                }
                Err(e) => self.hid_error = Some(e),
            }
            self.scanned = true;
            // Drop a connection whose device went away. A scan that started
            // before the connection was opened can miss it (the XBox edition
            // re-enumerates under a new path when it is reset out of its HID
            // fallback), so give a new connection a few seconds; a device
            // that really disappears also ends the session with a read error.
            if let Some(c) = &self.conn
                && !c.is_mock()
                && c.connected_at.elapsed() > Duration::from_secs(4)
                && !self.all_hid.iter().any(|d| d.path == c.id)
            {
                self.conn = None;
                self.toast(t("toast-disconnected"), true);
            }
            if self.conn.is_none() && !self.manual_disconnect {
                let supported: Vec<HidInfo> = self
                    .config_devices()
                    .into_iter()
                    .filter(|d| d.model().is_some_and(|(m, _)| m.protocol.is_implemented()))
                    .collect();
                if let Some(d) = supported.first() {
                    let d = d.clone();
                    self.connect(&d);
                }
            }
        }

        let mut toasts = vec![];
        let mut lost = false;
        if let Some(c) = &mut self.conn {
            let events: Vec<Event> = c.session.events().collect();
            for e in events {
                match e {
                    Event::Firmware { controller, dongle } => {
                        c.firmware = Some((controller, dongle))
                    }
                    Event::SlotLoaded(s, b) => c.take_slot(s, b, false),
                    Event::SlotChanged(s, b) => {
                        c.take_slot(s, b, true);
                        toasts.push((
                            ta(
                                "toast-changed-on-device",
                                &[("slot", FluentValue::from(slot_label(s)))],
                            ),
                            false,
                        ));
                    }
                    Event::SlotWritten(s) => {
                        if let Some(b) = c.writing.remove(&s) {
                            c.loaded.insert(s, b);
                        }
                        toasts.push((
                            ta("toast-saved", &[("slot", FluentValue::from(slot_label(s)))]),
                            false,
                        ))
                    }
                    Event::CurrentProfile(n) => c.current_profile = n,
                    Event::State(s) => c.state = Some(*s),
                    Event::Pending(n) => c.pending = n,
                    Event::StickCenter { ok } => {
                        if !ok {
                            toasts.push((t("toast-stick-center-failed"), true));
                        }
                    }
                    Event::UsbModeChanged => {}
                    Event::Warning(w) => {
                        // A write that wasn't answered can be applied again.
                        c.writing.clear();
                        toasts.push((w, true))
                    }
                    Event::Disconnected(why) => {
                        toasts.push((ta("toast-lost", &[("error", FluentValue::from(why))]), true));
                        lost = true;
                    }
                }
            }
        }
        if lost {
            self.conn = None;
        }
        for (t, w) in toasts {
            self.toast(t, w);
        }
        self.toasts
            .retain(|t| t.at.elapsed() < Duration::from_secs(5));
        self.probe.poll();
    }

    fn sidebar(&mut self, ui: &mut Ui) {
        // Room kept for the language picker and version line at the bottom,
        // so a short window scrolls the list instead of overlapping them.
        const FOOTER_H: f32 = 100.0;
        egui::ScrollArea::vertical()
            .id_salt("sidebar-scroll")
            .max_height((ui.available_height() - FOOTER_H).max(80.0))
            .auto_shrink([false, true])
            .show(ui, |ui| self.sidebar_list(ui));
        self.sidebar_footer(ui);
    }

    fn sidebar_list(&mut self, ui: &mut Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            logo(ui, 24.0);
            ui.label(RichText::new("SirClient").size(19.0).strong());
        });
        ui.add_space(14.0);
        caption(ui, t("sidebar-devices").to_uppercase());

        let devices = self.config_devices();
        let mut connect_to = None;
        for d in &devices {
            let (model, mode) = d.model().unwrap();
            let selected = self.conn.as_ref().is_some_and(|c| c.id == d.path);
            let implemented = model.protocol.is_implemented();
            let sub = if !implemented {
                t("device-unsupported")
            } else if let (true, Some(c)) = (selected, &self.conn) {
                let bat = c
                    .state
                    .as_ref()
                    .filter(|s| s.battery > 0)
                    .map(|s| format!(" · {}%", s.battery))
                    .unwrap_or_default();
                format!("{}{}", conn_label(mode.connection), bat)
            } else {
                conn_label(mode.connection)
            };
            if device_button(ui, selected, model.name, &sub, implemented).clicked()
                && implemented
                && !selected
            {
                connect_to = Some(d.clone());
            }
        }
        // GameSir devices with an unknown product ID: offer to try the G7 Pro
        // protocol (it only reads until the user applies changes).
        let unknown: Vec<HidInfo> = self
            .all_hid
            .iter()
            .filter(|d| d.is_unknown_gamesir())
            .cloned()
            .collect();
        for d in &unknown {
            let selected = self.conn.as_ref().is_some_and(|c| c.id == d.path);
            let title = if d.product.is_empty() {
                format!("GameSir {:04X}", d.pid)
            } else {
                d.product.clone()
            };
            let sub = ta(
                "device-unknown-sub",
                &[(
                    "id",
                    FluentValue::from(format!(
                        "{:04X}:{:04X} · {:04X}",
                        d.vid, d.pid, d.usage_page
                    )),
                )],
            );
            if device_button(ui, selected, &title, &sub, true).clicked() && !selected {
                connect_to = Some(d.clone());
            }
        }
        if let Some(d) = connect_to {
            self.connect(&d);
        }
        let mock_selected = self.conn.as_ref().is_some_and(|c| c.is_mock());
        if device_button(
            ui,
            mock_selected,
            &t("device-demo"),
            &t("device-demo-sub"),
            true,
        )
        .clicked()
            && !mock_selected
        {
            self.connect_mock();
        }
        if let Some(e) = &self.hid_error {
            ui.colored_label(WARN, e);
        }
        if devices.is_empty() && unknown.is_empty() && !mock_selected {
            caption(ui, t("device-none"));
        }
        if self.conn.is_some() {
            ui.add_space(2.0);
            let b = egui::Button::new(RichText::new(t("device-disconnect")).color(TEXT_2))
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::new(1.0, BORDER_STRONG))
                .corner_radius(egui::CornerRadius::same(8))
                .min_size(egui::vec2(ui.available_width(), 30.0));
            if ui.add(b).clicked() {
                self.disconnect();
            }
        }

        ui.add_space(14.0);
        caption(ui, t("sidebar-configure").to_uppercase());
        for (page, key) in [
            (Page::Overview, "nav-overview"),
            (Page::Buttons, "nav-buttons"),
            (Page::Sticks, "nav-sticks"),
            (Page::Lighting, "nav-lighting"),
        ] {
            self.nav_item(ui, page, key);
        }
        // Diagnostics live apart from the everyday pages.
        ui.add_space(14.0);
        caption(ui, t("sidebar-advanced").to_uppercase());
        self.nav_item(ui, Page::Devices, "nav-devices");
    }

    fn sidebar_footer(&mut self, ui: &mut Ui) {
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.add_space(6.0);
            caption(
                ui,
                format!("v{} · {}", env!("CARGO_PKG_VERSION"), t("app-unofficial")),
            );
            let cur = i18n::current();
            let langs = i18n::available();
            let cur_name = langs
                .iter()
                .find(|(id, _)| *id == cur)
                .map(|(_, n)| n.clone())
                .unwrap_or(cur.clone());
            let mut chosen = None;
            egui::ComboBox::from_id_salt("lang")
                .selected_text(cur_name)
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    for (id, name) in &langs {
                        if ui.selectable_label(*id == cur, name).clicked() {
                            chosen = Some(id.clone());
                        }
                    }
                });
            caption(ui, t("sidebar-language"));
            if let Some(id) = chosen {
                i18n::set(&id);
            }
        });
    }

    fn nav_item(&mut self, ui: &mut Ui, page: Page, key: &str) {
        let sel = self.page == page;
        let text = RichText::new(t(key)).color(if sel { TEXT } else { TEXT_2 });
        let b = egui::Button::new(text)
            .fill(if sel {
                SELECTED
            } else {
                egui::Color32::TRANSPARENT
            })
            .stroke(egui::Stroke::NONE)
            .min_size(egui::vec2(ui.available_width(), 34.0));
        if ui.add(b).clicked() {
            self.page = page;
        }
    }

    fn header(&mut self, ui: &mut Ui) {
        let (title, subtitle) = match self.page {
            Page::Overview => (t("nav-overview"), None),
            Page::Buttons => (t("nav-buttons"), None),
            Page::Sticks => (t("nav-sticks"), None),
            Page::Lighting => (t("nav-lighting"), Some(t("lighting-subtitle"))),
            Page::Devices => (t("nav-devices"), Some(t("devices-subtitle"))),
        };
        let sub = subtitle.unwrap_or_else(|| match &self.conn {
            Some(c) => {
                let mut s = c.title.clone();
                let dirty = c.dirty_slots().len();
                if dirty > 0 {
                    s.push_str(" · ");
                    s.push_str(&ta(
                        "unsaved-changes",
                        &[("count", FluentValue::from(dirty))],
                    ));
                }
                s
            }
            None => t("no-device"),
        });
        let pills = self.page != Page::Lighting;
        let gap = 12.0;
        let Self {
            conn,
            edit_slot,
            header_tools_w: tools_w,
            ..
        } = self;
        // Nothing to switch or apply over Bluetooth (live input only).
        let conn = if self.page == Page::Devices {
            None
        } else {
            conn.as_mut().filter(|c| !c.bt)
        };
        // Revert / Apply and the profile pills sit right of the title when
        // they fit. Otherwise they move to their own row, and to two rows
        // when even that is too narrow, so nothing is ever drawn over the
        // title in small or tiled windows. Widths come from the last frame.
        let need = if pills {
            tools_w.0 + gap + ui.spacing().item_spacing.x * 2.0 + tools_w.1
        } else {
            tools_w.1
        };
        let mut conn = conn;
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new(title).size(26.0).strong());
                ui.label(RichText::new(sub).color(MUTED));
            });
            let Some(c) = conn.as_deref_mut() else { return };
            if ui.available_width() < need + 16.0 {
                return;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                tools_w.1 = header_actions(ui, c, true);
                if pills {
                    ui.add_space(gap);
                    tools_w.0 = profile_pills(ui, c, edit_slot, true);
                }
            });
            conn = None;
        });
        let Some(c) = conn else { return };
        ui.add_space(4.0);
        // A right-to-left layout straight inside this vertical one would
        // centre itself in the whole page height; give it a row.
        let actions_row = |ui: &mut Ui, c: &mut Connected, w: &mut f32| {
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    *w = header_actions(ui, c, true);
                });
            });
        };
        if !pills {
            actions_row(ui, c, &mut tools_w.1);
        } else if ui.available_width() >= need {
            ui.horizontal(|ui| {
                tools_w.0 = profile_pills(ui, c, edit_slot, false);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    tools_w.1 = header_actions(ui, c, true);
                });
            });
        } else {
            ui.horizontal(|ui| tools_w.0 = profile_pills(ui, c, edit_slot, false));
            actions_row(ui, c, &mut tools_w.1);
        }
    }

    fn toasts_ui(&self, ui: &mut Ui) {
        if self.toasts.is_empty() {
            return;
        }
        egui::Area::new(egui::Id::unique("sirclient-toasts"))
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -16.0))
            .show(ui.ctx(), |ui| {
                for t in &self.toasts {
                    egui::Frame::new()
                        .fill(SURFACE)
                        .stroke(egui::Stroke::new(
                            1.0,
                            if t.warn { WARN } else { BORDER_STRONG },
                        ))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .show(ui, |ui| ui.label(&t.text));
                }
            });
    }
}

impl App {
    fn screenshots(&mut self, ctx: &egui::Context) {
        let Some(sh) = &mut self.shots else { return };
        let Some(&(page, name)) = sh.queue.first() else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        };
        self.page = page;
        ctx.request_repaint();
        if sh.done.swap(false, std::sync::atomic::Ordering::SeqCst) {
            sh.queue.remove(0);
            sh.frames = 0;
            sh.requested = false;
            return;
        }
        sh.frames += 1;
        // Give the session time to load and the live view to settle.
        if sh.frames > 45 && !sh.requested {
            sh.requested = true;
            let path = sh.dir.join(format!("{name}.png"));
            let done = sh.done.clone();
            let ctx2 = ctx.clone();
            ctx.request_screenshot(move |img| {
                let [w, h] = img.size;
                let rgba: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
                match image::save_buffer(
                    &path,
                    &rgba,
                    w as u32,
                    h as u32,
                    image::ExtendedColorType::Rgba8,
                ) {
                    Ok(()) => log::info!("saved {}", path.display()),
                    Err(e) => log::error!("saving {}: {e}", path.display()),
                }
                done.store(true, std::sync::atomic::Ordering::SeqCst);
                ctx2.request_repaint();
            });
        }
    }
}

/// Busy spinner, Revert and Apply as one block; returns its width.
fn header_actions(ui: &mut Ui, c: &mut Connected, rtl: bool) -> f32 {
    let dirty = !c.dirty_slots().is_empty();
    let busy = c.pending > 0;
    let r = ui.horizontal(|ui| {
        let mut apply = false;
        let mut revert = false;
        // Left to right: spinner, Revert, Apply. Built from the right edge
        // when `rtl`, so the calls run in reverse.
        for i in 0..3 {
            match if rtl { 2 - i } else { i } {
                0 => {
                    // Keep the spinner's room so the buttons do not jump.
                    if busy {
                        spinner(ui);
                    } else {
                        ui.add_space(spinner_size(ui));
                    }
                }
                1 => {
                    revert = ui
                        .add_enabled(dirty, egui::Button::new(t("revert")))
                        .clicked();
                }
                _ => apply = primary_button(ui, dirty, t("apply")).clicked(),
            }
        }
        (apply, revert)
    });
    let (apply, revert) = r.inner;
    if apply {
        c.apply();
    }
    if revert {
        c.revert();
    }
    r.response.rect.width()
}

/// Profile 1-4 and Shift pills; returns the block's width.
fn profile_pills(ui: &mut Ui, c: &mut Connected, edit_slot: &mut Slot, rtl: bool) -> f32 {
    let mut slots = [
        Slot::Profile(1),
        Slot::Profile(2),
        Slot::Profile(3),
        Slot::Profile(4),
        Slot::Shift,
    ];
    if rtl {
        slots.reverse();
    }
    ui.horizontal(|ui| {
        for slot in slots {
            let active_on_device = matches!(slot, Slot::Profile(n) if n == c.current_profile);
            let mut label = slot_label(slot);
            if active_on_device {
                label.push_str(ACTIVE_MARK);
            }
            if pill(ui, *edit_slot == slot, label).clicked() {
                *edit_slot = slot;
                if let Slot::Profile(_) = slot {
                    c.session.send(Request::SwitchProfile(slot));
                }
            }
        }
    })
    .response
    .rect
    .width()
}

pub fn slot_label(s: Slot) -> String {
    match s {
        Slot::Profile(n) => ta("profile-n", &[("n", FluentValue::from(n))]),
        Slot::Shift => t("profile-shift"),
        Slot::Light => t("nav-lighting"),
    }
}

pub fn conn_label(c: Connection) -> String {
    match c {
        Connection::Wired => t("conn-wired"),
        Connection::Wireless => t("conn-wireless"),
        Connection::Bluetooth => t("conn-bluetooth"),
    }
}

fn device_button(
    ui: &mut Ui,
    selected: bool,
    title: &str,
    sub: &str,
    enabled: bool,
) -> egui::Response {
    let frame = egui::Frame::new()
        .fill(if selected {
            egui::Color32::from_rgb(0x1F, 0x29, 0x37)
        } else {
            egui::Color32::TRANSPARENT
        })
        .stroke(egui::Stroke::new(
            1.0,
            if selected {
                egui::Color32::from_rgb(0x3B, 0x5B, 0x92)
            } else {
                BORDER_STRONG
            },
        ))
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin::same(10));
    let r = frame
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(title)
                    .strong()
                    .color(if enabled { TEXT } else { FAINT }),
            );
            ui.label(RichText::new(sub).size(12.0).color(MUTED));
        })
        .response;
    ui.interact(
        r.rect,
        ui.scope_id().with(("devbtn", title, sub)),
        egui::Sense::click(),
    )
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.poll();
        // Live input animates; keep repainting while connected.
        if self.conn.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(33));
        } else {
            ui.ctx().request_repaint_after(Duration::from_secs(1));
        }

        egui::Panel::left("sidebar")
            .exact_size(250.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(SURFACE)
                    .inner_margin(egui::Margin::symmetric(14, 16))
                    .stroke(egui::Stroke::new(1.0, BORDER)),
            )
            .show(ui, |ui| self.sidebar(ui));

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin {
                left: 28,
                right: 28,
                top: 22,
                bottom: 22,
            }))
            .show(ui, |ui| {
                self.header(ui);
                ui.add_space(14.0);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| match self.page {
                        Page::Devices => pages::devices::show(ui, self),
                        page => {
                            let Some(c) = self.conn.as_mut() else {
                                pages::empty_state(ui);
                                return;
                            };
                            if c.bt {
                                if page == Page::Overview {
                                    pages::overview::show_live(ui, c, &mut self.ui);
                                } else {
                                    pages::bt_notice(ui);
                                }
                                return;
                            }
                            if !c.loaded_all() {
                                ui.horizontal(|ui| {
                                    spinner(ui);
                                    ui.label(t("loading"));
                                });
                                if c.loaded.is_empty()
                                    && c.connected_at.elapsed() > Duration::from_secs(6)
                                {
                                    ui.label(RichText::new(t("loading-no-answer")).color(WARN));
                                }
                                return;
                            }
                            match page {
                                Page::Overview => {
                                    pages::overview::show(ui, c, self.edit_slot, &mut self.ui)
                                }
                                Page::Buttons => {
                                    pages::buttons::show(ui, c, self.edit_slot, &mut self.ui)
                                }
                                Page::Sticks => {
                                    pages::sticks::show(ui, c, self.edit_slot, &mut self.ui)
                                }
                                Page::Lighting if c.xbox => {
                                    pages::xbox_lighting::show(ui, c, &mut self.ui)
                                }
                                Page::Lighting => pages::lighting::show(ui, c, &mut self.ui),
                                Page::Devices => unreachable!(),
                            }
                        }
                    });
            });
        self.toasts_ui(ui);
        self.screenshots(ui.ctx());
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("language", i18n::current());
    }
}

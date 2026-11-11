//! Background worker that talks to one G7 Pro.
//!
//! The firmware handles one command at a time: every command that expects a
//! reply is resent every 200 ms (up to 10 times) until the matching reply
//! arrives, mirroring GameSir Connect. A heartbeat goes out once per second
//! to keep the controller in configuration mode.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, TryRecvError, unbounded};

use crate::Error;
use crate::protocol::g7pro::commands::{
    self, CURRENT_PROFILE_MARKER, CalibrationPart, Geometry, Nudge, Reply, Slot, StickSide, cmd,
};
use crate::protocol::g7pro::state::State;
use crate::transport::{REPORT_LEN, pad_report};

const RESEND_AFTER: Duration = Duration::from_millis(200);
const MAX_SENDS: u32 = 10;
const HEARTBEAT_EVERY: Duration = Duration::from_secs(1);
const STATE_EVERY: Duration = Duration::from_millis(16);

/// Byte pipe to a device. Implemented for real HID devices and for
/// [`crate::mock::MockG7`].
pub trait Link: Send {
    fn write(&mut self, report: &[u8]) -> Result<(), Error>;
    /// Read one input report; returns 0 on timeout.
    fn read(&mut self, buf: &mut [u8], timeout_ms: i32) -> Result<usize, Error>;
    /// Input report IDs that belong to the configuration collection, when
    /// the link shares a stream with other collections. `None` = accept all.
    fn config_report_ids(&self) -> Option<Vec<u8>> {
        None
    }
    /// Profile sizes and chunking used by the device behind this link.
    fn geometry(&self) -> Geometry {
        Geometry::HID
    }
}

impl Link for hidapi::HidDevice {
    fn write(&mut self, report: &[u8]) -> Result<(), Error> {
        hidapi::HidDevice::write(self, report)?;
        Ok(())
    }
    fn read(&mut self, buf: &mut [u8], timeout_ms: i32) -> Result<usize, Error> {
        Ok(self.read_timeout(buf, timeout_ms)?)
    }
    fn config_report_ids(&self) -> Option<Vec<u8>> {
        let ids = crate::transport::device_report_ids(self, CONFIG_USAGE_PAGE);
        if ids.is_empty() { None } else { Some(ids) }
    }
}

/// HID usage page of the G7 Pro configuration collection.
const CONFIG_USAGE_PAGE: u16 = 0xFFF0;

/// Requests from the UI.
#[derive(Clone, Debug)]
pub enum Request {
    /// Read firmware version, all slots and the active profile.
    Reload,
    /// Write a full slot image; only the changed byte range is sent.
    WriteSlot(Slot, Vec<u8>),
    SwitchProfile(Slot),
    Vibrate {
        left: u8,
        right: u8,
        duration: Duration,
    },
    Calibrate {
        kind: u8,
        part: CalibrationPart,
    },
    NudgeStickCenter {
        side: StickSide,
        dir: Nudge,
    },
    /// Preview a lighting frame (`playing = false` stops the preview).
    LightPreview {
        playing: bool,
        frame: u8,
    },
    Close,
}

/// Notifications for the UI.
#[derive(Clone, Debug)]
pub enum Event {
    Firmware {
        controller: String,
        dongle: String,
    },
    /// A slot was read from the device.
    SlotLoaded(Slot, Vec<u8>),
    /// A slot was changed on the controller itself and re-read.
    SlotChanged(Slot, Vec<u8>),
    /// Write of a slot finished and was acknowledged.
    SlotWritten(Slot),
    CurrentProfile(u8),
    State(Box<State>),
    /// Number of commands still waiting for a reply.
    Pending(usize),
    StickCenter {
        ok: bool,
    },
    UsbModeChanged,
    Warning(String),
    Disconnected(String),
}

struct Outgoing {
    bytes: Vec<u8>,
    expect: Option<u8>,
    sends: u32,
    last_sent: Option<Instant>,
    done: Option<Event>,
}

impl Outgoing {
    fn new(bytes: Vec<u8>) -> Self {
        let expect = Reply::expected_for(bytes[1]);
        Outgoing {
            bytes,
            expect,
            sends: 0,
            last_sent: None,
            done: None,
        }
    }

    fn matches(&self, r: &Reply) -> bool {
        if self.expect != Some(r.code()) {
            return false;
        }
        match r {
            Reply::ProfileData { slot, offset, .. } | Reply::RefreshData { slot, offset, .. } => {
                let off = ((self.bytes[3] as usize) << 8) | self.bytes[4] as usize;
                *slot == self.bytes[2] && *offset == off
            }
            _ => true,
        }
    }
}

/// Handle to a running session.
pub struct Session {
    tx: Sender<Request>,
    rx: Receiver<Event>,
    thread: Option<JoinHandle<()>>,
}

impl Session {
    /// Start talking to a device. `notify` is called whenever new events are
    /// queued (e.g. to wake up the UI).
    pub fn spawn(link: Box<dyn Link>, notify: Arc<dyn Fn() + Send + Sync>) -> Session {
        let (tx, req_rx) = unbounded();
        let (ev_tx, rx) = unbounded();
        let thread = std::thread::Builder::new()
            .name("sirclient-session".into())
            .spawn(move || {
                let mut w = Worker::new(link, req_rx, ev_tx, notify);
                w.send(Request::Reload);
                w.run();
            })
            .expect("spawn session thread");
        Session {
            tx,
            rx,
            thread: Some(thread),
        }
    }

    pub fn send(&self, r: Request) {
        let _ = self.tx.send(r);
    }

    pub fn events(&self) -> impl Iterator<Item = Event> + '_ {
        self.rx.try_iter()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.tx.send(Request::Close);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

struct Worker {
    link: Box<dyn Link>,
    req: Receiver<Request>,
    ev: Sender<Event>,
    notify: Arc<dyn Fn() + Send + Sync>,
    queue: VecDeque<Outgoing>,
    cache: HashMap<u8, Vec<u8>>,
    loading: HashMap<u8, Vec<u8>>,
    /// End offset of a pending device-side change refresh, per slot.
    refresh_end: HashMap<u8, usize>,
    timers: Vec<(Instant, Vec<u8>)>,
    last_heartbeat: Instant,
    last_state: Instant,
    last_pending: usize,
    /// Report IDs of the configuration collection; others are ignored.
    accept: Option<Vec<u8>>,
    geo: Geometry,
}

impl Worker {
    fn new(
        link: Box<dyn Link>,
        req: Receiver<Request>,
        ev: Sender<Event>,
        notify: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        let now = Instant::now();
        let accept = link.config_report_ids();
        let geo = link.geometry();
        log::info!("configuration report ids: {accept:02x?}, geometry {geo:?}");
        Worker {
            accept,
            geo,
            link,
            req,
            ev,
            notify,
            queue: VecDeque::new(),
            cache: HashMap::new(),
            loading: HashMap::new(),
            refresh_end: HashMap::new(),
            timers: Vec::new(),
            last_heartbeat: now - HEARTBEAT_EVERY,
            last_state: now,
            last_pending: 0,
        }
    }

    fn emit(&self, e: Event) {
        let _ = self.ev.send(e);
        (self.notify)();
    }

    fn enqueue(&mut self, bytes: Vec<u8>) {
        self.queue.push_back(Outgoing::new(bytes));
    }

    fn send(&mut self, r: Request) -> bool {
        match r {
            Request::Reload => {
                self.enqueue(commands::read_firmware());
                for slot in Slot::ALL {
                    let len = self.geo.slot_len(slot);
                    self.loading.insert(slot.number(), vec![0; len]);
                    for c in commands::read_slot_in(self.geo, slot) {
                        self.enqueue(c);
                    }
                }
                self.enqueue(commands::read_current_profile());
            }
            Request::WriteSlot(slot, new) => {
                let slot_len = self.geo.slot_len(slot);
                if new.len() != slot_len {
                    self.emit(Event::Warning(format!(
                        "{} has wrong size {}",
                        slot.label(),
                        new.len()
                    )));
                    return true;
                }
                // Only the changed range is sent. Without a cached copy of
                // the slot there is nothing to diff against, so all of it is.
                let (first, last) = match self.cache.get(&slot.number()) {
                    Some(old) => (
                        (0..new.len()).find(|&i| new[i] != old[i]),
                        (0..new.len()).rev().find(|&i| new[i] != old[i]),
                    ),
                    None => (Some(0), Some(new.len() - 1)),
                };
                if let (Some(a), Some(b)) = (first, last) {
                    log::debug!("write {} [{a:#x}..={b:#x}]", slot.label());
                    let cmds = commands::write_range_in(self.geo, slot, a, &new[a..=b]);
                    let n = cmds.len();
                    for (i, c) in cmds.into_iter().enumerate() {
                        let mut o = Outgoing::new(c);
                        if i + 1 == n {
                            o.done = Some(Event::SlotWritten(slot));
                        }
                        self.queue.push_back(o);
                    }
                    self.cache.insert(slot.number(), new);
                } else {
                    self.emit(Event::SlotWritten(slot));
                }
            }
            Request::SwitchProfile(slot) => {
                self.enqueue(commands::switch_profile(slot));
                self.enqueue(commands::read_current_profile());
            }
            Request::Vibrate {
                left,
                right,
                duration,
            } => {
                self.write_now(&commands::vibrate(left, right));
                self.timers
                    .push((Instant::now() + duration, commands::vibrate(0, 0)));
            }
            Request::Calibrate { kind, part } => self.write_now(&commands::calibrate(kind, part)),
            Request::NudgeStickCenter { side, dir } => {
                self.enqueue(commands::set_stick_center(side, dir, 32))
            }
            Request::LightPreview { playing, frame } => {
                self.write_now(&commands::light_preview(playing, frame))
            }
            Request::Close => return false,
        }
        true
    }

    fn write_now(&mut self, bytes: &[u8]) {
        if let Err(e) = self.link.write(&pad_report(bytes)) {
            log::warn!("write failed: {e}");
        }
    }

    fn run(&mut self) {
        let mut buf = [0u8; REPORT_LEN];
        loop {
            loop {
                match self.req.try_recv() {
                    Ok(r) => {
                        if !self.send(r) {
                            return;
                        }
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => return,
                }
            }

            let now = Instant::now();
            if now.duration_since(self.last_heartbeat) >= HEARTBEAT_EVERY {
                self.last_heartbeat = now;
                self.write_now(&commands::heartbeat(false));
            }
            let due: Vec<Vec<u8>> = {
                let (due, rest): (Vec<_>, Vec<_>) =
                    self.timers.drain(..).partition(|(t, _)| *t <= now);
                self.timers = rest;
                due.into_iter().map(|(_, b)| b).collect()
            };
            for b in due {
                self.write_now(&b);
            }

            if let Err(e) = self.pump_queue() {
                self.emit(Event::Disconnected(e.to_string()));
                return;
            }

            match self.link.read(&mut buf, 4) {
                Ok(0) => {}
                Ok(n) => self.handle(&buf[..n]),
                Err(e) => {
                    self.emit(Event::Disconnected(e.to_string()));
                    return;
                }
            }

            if self.queue.len() != self.last_pending {
                self.last_pending = self.queue.len();
                self.emit(Event::Pending(self.last_pending));
            }
        }
    }

    fn pump_queue(&mut self) -> Result<(), Error> {
        let Some(head) = self.queue.front_mut() else {
            return Ok(());
        };
        let now = Instant::now();
        let due = match head.last_sent {
            None => true,
            Some(t) => head.expect.is_some() && now.duration_since(t) >= RESEND_AFTER,
        };
        if head.last_sent.is_some() && head.expect.is_none() {
            let done = self.queue.pop_front().and_then(|o| o.done);
            if let Some(e) = done {
                self.emit(e);
            }
            return Ok(());
        }
        if !due {
            return Ok(());
        }
        if head.sends >= MAX_SENDS {
            let o = self.queue.pop_front().unwrap();
            log::warn!("no reply to {:02x?}", &o.bytes[..o.bytes.len().min(6)]);
            self.emit(Event::Warning(format!(
                "Controller did not answer command 0x{:02x}",
                o.bytes[1]
            )));
            return Ok(());
        }
        let report = pad_report(&head.bytes);
        head.sends += 1;
        head.last_sent = Some(now);
        if head.sends > 1 {
            log::debug!("resend {:02x?}", &head.bytes[..head.bytes.len().min(6)]);
        }
        self.link.write(&report)
    }

    fn handle(&mut self, r: &[u8]) {
        if let Some(ids) = &self.accept
            && !ids.contains(&r[0])
        {
            return; // another collection on the same stream (keyboard, mouse…)
        }
        if r[0] == commands::STATE_REPORT_ID {
            if self.last_state.elapsed() >= STATE_EVERY
                && let Some(s) = State::parse(r)
            {
                self.last_state = Instant::now();
                self.emit(Event::State(Box::new(s)));
            }
            return;
        }
        let Some(reply) = commands::parse_reply(r) else {
            return;
        };
        if reply == Reply::Busy {
            return; // resent after the timeout
        }
        if self.queue.front().is_some_and(|h| h.matches(&reply)) {
            let done = self.queue.pop_front().and_then(|o| o.done);
            if let Some(e) = done {
                self.emit(e);
            }
        }
        match reply {
            Reply::ProfileData { slot, offset, data } => self.on_data(slot, offset, &data, false),
            Reply::RefreshData { slot, offset, data } => self.on_data(slot, offset, &data, true),
            Reply::Firmware { controller, dongle } => {
                self.emit(Event::Firmware { controller, dongle })
            }
            Reply::CurrentProfile(n) => self.emit(Event::CurrentProfile(n)),
            Reply::ProfileChanged { slot, offset, len } => {
                if slot == CURRENT_PROFILE_MARKER {
                    self.enqueue(commands::read_current_profile());
                } else if let Some(s) = Slot::from_number(slot) {
                    let end = (offset + len).min(self.geo.slot_len(s));
                    if end > offset {
                        self.refresh_end.insert(slot, end);
                        for c in commands::read_range_in(
                            self.geo,
                            cmd::REFRESH_PROFILE,
                            s,
                            offset,
                            end - offset,
                        ) {
                            self.enqueue(c);
                        }
                    }
                }
            }
            Reply::StickCenter { ok } => self.emit(Event::StickCenter { ok }),
            Reply::UsbModeChanged => self.emit(Event::UsbModeChanged),
            _ => {}
        }
    }

    fn on_data(&mut self, slot_no: u8, offset: usize, data: &[u8], refresh: bool) {
        let Some(slot) = Slot::from_number(slot_no) else {
            return;
        };
        let slot_len = self.geo.slot_len(slot);
        let end = offset + data.len();
        if end > slot_len {
            return;
        }
        if refresh {
            let cached = self
                .cache
                .entry(slot_no)
                .or_insert_with(|| vec![0; slot_len]);
            cached[offset..end].copy_from_slice(data);
            if self.refresh_end.get(&slot_no) == Some(&end) {
                self.refresh_end.remove(&slot_no);
                let bytes = cached.clone();
                self.emit(Event::SlotChanged(slot, bytes));
            }
        } else if let Some(buf) = self.loading.get_mut(&slot_no) {
            buf[offset..end].copy_from_slice(data);
            if end == slot_len {
                let bytes = self.loading.remove(&slot_no).unwrap();
                self.cache.insert(slot_no, bytes.clone());
                self.emit(Event::SlotLoaded(slot, bytes));
            }
        }
    }
}

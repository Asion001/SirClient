//! Simulated G7 Pro 8K for development and demos without hardware.
//!
//! It answers the same wire protocol as the real controller, so everything
//! above [`crate::session::Link`] runs unchanged.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use crate::Error;
use crate::protocol::g7pro::commands::{
    CURRENT_PROFILE_MARKER, REPORT_ID, STATE_REPORT_ID, Slot, cmd, reply,
};
use crate::protocol::g7pro::keys;
use crate::protocol::g7pro::light::{Hsv, LightProfile, Preset};
use crate::protocol::g7pro::profile::*;
use crate::transport::REPORT_LEN;

pub const MOCK_PATH: &str = "mock://g7-pro";

pub struct MockG7 {
    slots: HashMap<u8, Vec<u8>>,
    current: u8,
    out: VecDeque<[u8; REPORT_LEN]>,
    start: Instant,
    last_state: Instant,
    rumble: (u8, u8),
}

impl Default for MockG7 {
    fn default() -> Self {
        Self::new()
    }
}

fn default_deadzone(inner: u16) -> Deadzone {
    Deadzone {
        enabled: 1,
        inner,
        outer: 1000,
        anti_inner: 0,
        anti_outer: 1000,
    }
}

fn linear_curve() -> Curve {
    Curve {
        enabled: 1,
        kind: 0,
        curvature: 100,
        points: [(0, 0), (64, 64), (128, 128), (191, 191), (255, 255)],
    }
}

/// A plausible factory-default profile.
pub fn default_profile(name: &str) -> Profile {
    let mut p =
        Profile::from_bytes(&vec![0u8; crate::protocol::g7pro::commands::PROFILE_LEN]).unwrap();
    p.name = name.to_string();
    p.function.motors = Motors {
        left: 75,
        right: 75,
        lt: 50,
        rt: 50,
    };
    p.function.extend.report_rate = 5;
    p.function.extend.ls_anti_jitter = 11;
    p.function.extend.rs_anti_jitter = 11;
    p.function.reserved = vec![0; 13];
    for (b, name) in p.buttons.iter_mut().zip(BASIC_BUTTONS) {
        b.map = KeyMap {
            enabled: 0,
            keys: [keys::code(name).unwrap_or(0), 0, 0],
        };
    }
    for bb in &mut p.back_buttons {
        bb.macro_.steps = vec![MacroStep::default(); MACRO_STEPS];
    }
    for t in [&mut p.lt, &mut p.rt] {
        t.deadzone = default_deadzone(50);
        t.deadzone.outer = 950;
        t.curve = linear_curve();
        t.hair_start = 1;
        t.hair_end = 100;
    }
    for (s, out) in [(&mut p.left_stick, 1), (&mut p.right_stick, 2)] {
        s.enabled = 1;
        s.deadzone = default_deadzone(50);
        s.curve = linear_curve();
        s.map = AxisMap {
            enabled: 1,
            output: out,
            axis_ratio: 50,
            mouse_dpi: 50,
            ..Default::default()
        };
    }
    for m in [&mut p.motion_aim, &mut p.motion_tilt] {
        m.trigger_keys = [keys::NC; 3];
        m.deadzone = default_deadzone(0);
        m.curve = linear_curve();
        m.map = AxisMap {
            enabled: 1,
            output: 2,
            axis_ratio: 50,
            mouse_dpi: 50,
            stick_sensitivity: 50,
            ..Default::default()
        };
    }
    p
}

pub fn default_light() -> LightProfile {
    let palette = [(220, 80), (280, 80), (0, 0), (140, 90), (30, 100)];
    LightProfile {
        presets: palette
            .iter()
            .enumerate()
            .map(|(i, &(h, s))| Preset {
                mode: if i == 0 { 1 } else { (i as u8 % 5) + 1 },
                static_brightness: 80,
                breathing_brightness: 80,
                phantom_brightness: 80,
                rainbow_brightness: 80,
                radar_brightness: 80,
                breathing_speed: 50,
                phantom_speed: 50,
                rainbow_speed: 50,
                radar_speed: 50,
                phantom_hue: 0,
                rainbow_hue: 0,
                colors: [Hsv { h, s, v: 100 }; 5],
            })
            .collect(),
        flags: 1,
        standby_time: 5,
        dock_led_mode: 0,
        dock_led_brightness: 50,
        report_rate: 5,
    }
}

impl MockG7 {
    pub fn new() -> Self {
        let mut slots = HashMap::new();
        for n in 1..=4u8 {
            slots.insert(n, default_profile(&format!("Profile {n}")).to_bytes());
        }
        slots.insert(Slot::Shift.number(), default_profile("Shift").to_bytes());
        slots.insert(Slot::Light.number(), default_light().to_bytes());
        let now = Instant::now();
        MockG7 {
            slots,
            current: 1,
            out: VecDeque::new(),
            start: now,
            last_state: now,
            rumble: (0, 0),
        }
    }

    fn reply(&mut self, bytes: &[u8]) {
        let mut r = [0u8; REPORT_LEN];
        r[0] = REPORT_ID;
        r[1..1 + bytes.len()].copy_from_slice(bytes);
        self.out.push_back(r);
    }

    fn state_report(&self) -> [u8; REPORT_LEN] {
        let t = self.start.elapsed().as_secs_f32();
        let mut r = [0u8; REPORT_LEN];
        r[0] = STATE_REPORT_ID;
        let axis = |v: f32| -> u16 { ((v.clamp(-1.0, 1.0) * 32767.0) as i32 + 32768) as u16 };
        let lx = axis((t * 1.3).cos() * 0.8);
        let ly = axis((t * 1.3).sin() * 0.8);
        let rx = axis((t * 0.7).sin() * 0.4);
        let ry = axis(0.0);
        for (i, v) in [lx, ly, rx, ry].into_iter().enumerate() {
            r[1 + i] = (v >> 8) as u8;
            r[26 + i] = v as u8;
        }
        let step = (t * 2.0) as u32;
        let face = 1u8 << (4 + step % 4);
        r[5] = 0x0f | if step.is_multiple_of(3) { face } else { 0 };
        r[6] = if step % 5 == 1 { 0b01 } else { 0 };
        r[7] = if step % 7 == 2 { 1 << 3 } else { 0 };
        r[8] = (((t * 1.1).sin() * 0.5 + 0.5) * 255.0) as u8;
        r[9] = (((t * 0.6).cos() * 0.5 + 0.5) * 255.0) as u8;
        r[31] = 1;
        r[35] = 1;
        r[36] = 80;
        r[37] = 1;
        if let Some(light) = self
            .slots
            .get(&Slot::Light.number())
            .and_then(|b| LightProfile::from_bytes(b).ok())
            && let Some(p) = light.presets.get(1)
        {
            for (i, c) in p.colors.iter().enumerate() {
                let rgb = if p.mode == 0 { [0, 0, 0] } else { c.to_rgb() };
                r[38 + i * 3..41 + i * 3].copy_from_slice(&rgb);
            }
        }
        // Physical block mirrors the mapped block.
        let (head, tail) = r.split_at_mut(53);
        tail[1..11].copy_from_slice(&head[1..11]);
        tail[0] = 0;
        r
    }

    /// Last rumble values received (for tests and the demo UI).
    pub fn rumble(&self) -> (u8, u8) {
        self.rumble
    }
}

impl crate::session::Link for MockG7 {
    fn write(&mut self, r: &[u8]) -> Result<(), Error> {
        if r.len() < 2 || r[0] != REPORT_ID {
            return Ok(());
        }
        match r[1] {
            cmd::READ_PROFILE | cmd::REFRESH_PROFILE => {
                let slot = r[2];
                let off = ((r[3] as usize) << 8) | r[4] as usize;
                let len = r[5] as usize;
                let code = if r[1] == cmd::READ_PROFILE {
                    reply::READ_PROFILE
                } else {
                    reply::REFRESH_PROFILE
                };
                if let Some(data) = self.slots.get(&slot) {
                    let end = (off + len).min(data.len());
                    let mut p = vec![code, slot, r[3], r[4], (end - off) as u8];
                    p.extend_from_slice(&data[off..end]);
                    self.reply(&p);
                }
            }
            cmd::WRITE_PROFILE => {
                let slot = r[2];
                let off = ((r[3] as usize) << 8) | r[4] as usize;
                let len = r[5] as usize;
                if let Some(data) = self.slots.get_mut(&slot) {
                    let end = (off + len).min(data.len()).min(off + r.len() - 6);
                    data[off..end].copy_from_slice(&r[6..6 + (end - off)]);
                }
                self.reply(&[reply::ACK, 0]);
            }
            cmd::SWITCH_PROFILE => {
                if (1..=5).contains(&r[2]) {
                    self.current = r[2];
                }
                self.reply(&[reply::ACK, 0]);
                self.reply(&[reply::PROFILE_CHANGED, CURRENT_PROFILE_MARKER, 0, 0, 0, 0]);
            }
            cmd::READ_FIRMWARE => {
                let mut p = vec![reply::FIRMWARE, 0, 0];
                p.extend_from_slice(&[b'2', b'1', b'9', 0, 0, 0, 0, 0, b'1', b'3', b'4']);
                self.reply(&p);
            }
            cmd::READ_CURRENT_PROFILE => {
                let c = self.current;
                self.reply(&[reply::CURRENT_PROFILE, c]);
            }
            cmd::VIBRATE => self.rumble = (r[4], r[5]),
            cmd::SET_STICK_CENTER => self.reply(&[reply::STICK_CENTER, 0]),
            _ => {}
        }
        Ok(())
    }

    fn read(&mut self, buf: &mut [u8], timeout_ms: i32) -> Result<usize, Error> {
        if let Some(r) = self.out.pop_front() {
            buf[..REPORT_LEN].copy_from_slice(&r);
            return Ok(REPORT_LEN);
        }
        if self.last_state.elapsed() >= Duration::from_millis(8) {
            self.last_state = Instant::now();
            buf[..REPORT_LEN].copy_from_slice(&self.state_report());
            return Ok(REPORT_LEN);
        }
        std::thread::sleep(Duration::from_millis(timeout_ms.clamp(0, 2) as u64));
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{Event, Request, Session};
    use std::sync::Arc;

    fn wait_for<F: FnMut(&Event) -> bool>(s: &Session, mut f: F) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            for e in s.events() {
                if f(&e) {
                    return true;
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        false
    }

    #[test]
    fn session_loads_all_slots_from_mock() {
        let s = Session::spawn(Box::new(MockG7::new()), Arc::new(|| {}));
        let mut loaded = 0;
        let mut fw = false;
        assert!(wait_for(&s, |e| {
            match e {
                Event::SlotLoaded(..) => loaded += 1,
                Event::Firmware { controller, .. } => fw = controller == "2.1.9",
                _ => {}
            }
            loaded == Slot::ALL.len() && fw
        }));
    }

    #[test]
    fn write_then_reload_round_trips() {
        let s = Session::spawn(Box::new(MockG7::new()), Arc::new(|| {}));
        let mut p1 = None;
        assert!(wait_for(&s, |e| {
            if let Event::SlotLoaded(Slot::Profile(1), b) = e {
                p1 = Some(b.clone());
            }
            p1.is_some()
        }));
        let mut prof = Profile::from_bytes(&p1.unwrap()).unwrap();
        prof.name = "Edited".into();
        prof.right_stick.deadzone.inner = 123;
        s.send(Request::WriteSlot(Slot::Profile(1), prof.to_bytes()));
        assert!(wait_for(&s, |e| matches!(
            e,
            Event::SlotWritten(Slot::Profile(1))
        )));
        s.send(Request::Reload);
        let mut again = None;
        assert!(wait_for(&s, |e| {
            if let Event::SlotLoaded(Slot::Profile(1), b) = e {
                again = Some(b.clone());
            }
            again.is_some()
        }));
        let p = Profile::from_bytes(&again.unwrap()).unwrap();
        assert_eq!(p.name, "Edited");
        assert_eq!(p.right_stick.deadzone.inner, 123);
    }
}

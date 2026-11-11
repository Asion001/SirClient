//! G7 Pro 8K profile layout (1070 bytes per profile slot).
//!
//! | offset | size        | section                                   |
//! |-------:|------------:|-------------------------------------------|
//! |      0 | 32          | profile name, UTF-8, zero padded          |
//! |     32 | 32          | function data (motors, audio, extend)     |
//! |     64 | 16 × 7      | basic buttons (see [`BASIC_BUTTONS`])     |
//! |    176 | 4 × 169     | back buttons with macros (L4 R4 L5 R5)    |
//! |    852 | 2 × 32      | triggers LT, RT                           |
//! |    916 | 2 × 36      | sticks left, right                        |
//! |    988 | 2 × 41      | motion aim, tilt                          |
//!
//! Multi-byte numbers are big-endian. The structs here mirror the bytes one
//! to one so a read-modify-write never touches fields the UI doesn't know.

use serde::{Deserialize, Serialize};

use super::commands::PROFILE_LEN;
use crate::Error;

pub const NAME_LEN: usize = 32;
pub const MACRO_STEPS: usize = 32;

/// Order of the 7-byte button blocks.
pub const BASIC_BUTTONS: [&str; 16] = [
    "Up", "Under", "Left", "Right", "L1", "R1", "L3", "R3", "CRO", "CIR", "SQU", "TRI", "PS",
    "Sel", "Sta", "Camera",
];

/// Order of the 169-byte back-button blocks.
pub const BACK_BUTTONS: [&str; 4] = ["FL1", "FR1", "FL2", "FR2"];

struct Reader<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Reader<'a> {
    fn u8(&mut self) -> u8 {
        let v = self.b[self.i];
        self.i += 1;
        v
    }
    fn u16(&mut self) -> u16 {
        u16::from_be_bytes([self.u8(), self.u8()])
    }
    fn bytes(&mut self, n: usize) -> Vec<u8> {
        let v = self.b[self.i..self.i + n].to_vec();
        self.i += n;
        v
    }
}

#[derive(Default)]
struct Writer {
    b: Vec<u8>,
}

impl Writer {
    fn u8(&mut self, v: u8) {
        self.b.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.b.extend_from_slice(&v.to_be_bytes());
    }
    fn bytes(&mut self, v: &[u8]) {
        self.b.extend_from_slice(v);
    }
}

/// Rumble strength per motor. Raw 0/25/50/75/100.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Motors {
    pub left: u8,
    pub right: u8,
    pub lt: u8,
    pub rt: u8,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Audio {
    pub enabled: u8,
    pub volume: u8,
    pub mixer: u8,
    pub mic_mute: u8,
    pub mic_sensitivity: u8,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Extend {
    pub shift_enabled: u8,
    pub shift_value: u8,
    pub dpad_diagonal_lock: u8,
    pub xinput_abxy_swap: u8,
    pub switch_abxy_swap: u8,
    /// 0 = 250 Hz, 1 = 500 Hz, 2 = 1000 Hz, 4 = 4000 Hz, 5 = 8000 Hz.
    pub report_rate: u8,
    pub motion_precision: u8,
    /// Raw value; the app shows `4 - raw`.
    pub stick_resolution: u8,
    /// 1..=21, the app shows `raw - 11` (-10..=10).
    pub ls_anti_jitter: u8,
    pub rs_anti_jitter: u8,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionData {
    pub motors: Motors,
    pub audio: Audio,
    pub extend: Extend,
    /// 13 reserved bytes, kept untouched.
    pub reserved: Vec<u8>,
}

/// Remap to up to three key codes (see [`super::keys`]).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyMap {
    pub enabled: u8,
    pub keys: [u8; 3],
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Button {
    pub turbo_enabled: u8,
    pub turbo_speed: u8,
    pub map: KeyMap,
    pub toggle: u8,
}

impl Button {
    fn read(r: &mut Reader) -> Self {
        Button {
            turbo_enabled: r.u8(),
            turbo_speed: r.u8(),
            map: KeyMap {
                enabled: r.u8(),
                keys: [r.u8(), r.u8(), r.u8()],
            },
            toggle: r.u8(),
        }
    }
    fn write(&self, w: &mut Writer) {
        w.u8(self.turbo_enabled);
        w.u8(self.turbo_speed);
        w.u8(self.map.enabled);
        w.bytes(&self.map.keys);
        w.u8(self.toggle);
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroStep {
    pub key: u8,
    /// Milliseconds.
    pub hold: u16,
    /// Milliseconds. The last step has no delay field on the wire.
    pub delay: u16,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Macro {
    /// bit0 enabled, bit1 trigger-on-press, bit2 repeat.
    pub flags: u8,
    pub cycle_time: u16,
    pub step_count: u8,
    pub steps: Vec<MacroStep>,
}

impl Macro {
    pub const ENABLED: u8 = 1;
    pub const PRESS_TO_TRIGGER: u8 = 2;
    pub const REPEAT: u8 = 4;
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackButton {
    pub button: Button,
    pub macro_: Macro,
}

impl BackButton {
    fn read(r: &mut Reader) -> Self {
        let button = Button::read(r);
        let flags = r.u8();
        let cycle_time = r.u16();
        let step_count = r.u8();
        let steps = (0..MACRO_STEPS)
            .map(|i| MacroStep {
                key: r.u8(),
                hold: r.u16(),
                delay: if i < MACRO_STEPS - 1 { r.u16() } else { 0 },
            })
            .collect();
        BackButton {
            button,
            macro_: Macro {
                flags,
                cycle_time,
                step_count,
                steps,
            },
        }
    }
    fn write(&self, w: &mut Writer) {
        self.button.write(w);
        let m = &self.macro_;
        w.u8(m.flags);
        w.u16(m.cycle_time);
        w.u8(m.step_count);
        for i in 0..MACRO_STEPS {
            let s = m.steps.get(i).cloned().unwrap_or_default();
            w.u8(s.key);
            w.u16(s.hold);
            if i < MACRO_STEPS - 1 {
                w.u16(s.delay);
            }
        }
    }
}

/// Dead zones are 0..=1000 (tenths of a percent).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deadzone {
    pub enabled: u8,
    pub inner: u16,
    pub outer: u16,
    pub anti_inner: u16,
    pub anti_outer: u16,
}

impl Deadzone {
    fn read(r: &mut Reader) -> Self {
        Deadzone {
            enabled: r.u8(),
            inner: r.u16(),
            outer: r.u16(),
            anti_inner: r.u16(),
            anti_outer: r.u16(),
        }
    }
    fn write(&self, w: &mut Writer) {
        w.u8(self.enabled);
        w.u16(self.inner);
        w.u16(self.outer);
        w.u16(self.anti_inner);
        w.u16(self.anti_outer);
    }
}

/// Response curve. Points are (input, output) pairs in 0..=255 scaled
/// between the dead zone and anti dead zone bounds.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Curve {
    pub enabled: u8,
    /// 0 = custom points, other values are presets.
    pub kind: u8,
    pub curvature: u8,
    pub points: [(u8, u8); 5],
}

impl Curve {
    fn read(r: &mut Reader) -> Self {
        let enabled = r.u8();
        let kind = r.u8();
        let curvature = r.u8();
        let mut points = [(0, 0); 5];
        for p in &mut points {
            *p = (r.u8(), r.u8());
        }
        Curve {
            enabled,
            kind,
            curvature,
            points,
        }
    }
    fn write(&self, w: &mut Writer) {
        w.u8(self.enabled);
        w.u8(self.kind);
        w.u8(self.curvature);
        for (x, y) in self.points {
            w.u8(x);
            w.u8(y);
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trigger {
    pub turbo_enabled: u8,
    pub turbo_speed: u8,
    pub deadzone: Deadzone,
    pub map: KeyMap,
    pub toggle: u8,
    /// bit7 enabled, bit0 mode 1, bit1 mode 2.
    pub hair_trigger: u8,
    pub hair_start: u8,
    pub hair_end: u8,
    pub curve: Curve,
}

impl Trigger {
    fn read(r: &mut Reader) -> Self {
        Trigger {
            turbo_enabled: r.u8(),
            turbo_speed: r.u8(),
            deadzone: Deadzone::read(r),
            map: KeyMap {
                enabled: r.u8(),
                keys: [r.u8(), r.u8(), r.u8()],
            },
            toggle: r.u8(),
            hair_trigger: r.u8(),
            hair_start: r.u8(),
            hair_end: r.u8(),
            curve: Curve::read(r),
        }
    }
    fn write(&self, w: &mut Writer) {
        w.u8(self.turbo_enabled);
        w.u8(self.turbo_speed);
        self.deadzone.write(w);
        w.u8(self.map.enabled);
        w.bytes(&self.map.keys);
        w.u8(self.toggle);
        w.u8(self.hair_trigger);
        w.u8(self.hair_start);
        w.u8(self.hair_end);
        self.curve.write(w);
    }
}

/// What a stick or the gyro drives.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AxisMap {
    pub enabled: u8,
    pub flip_x: u8,
    pub flip_y: u8,
    /// Yaw flip, motion only.
    pub flip_z: u8,
    pub axis_ratio: u8,
    pub mouse_dpi: u8,
    /// 1 left stick, 2 right stick, 3 wheel (4 keys), 4 mouse.
    pub output: u8,
    pub cross: u8,
    pub up: u8,
    pub down: u8,
    pub left: u8,
    pub right: u8,
    pub center: u8,
    /// Motion only.
    pub stick_sensitivity: u8,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stick {
    pub enabled: u8,
    /// Non-zero: raw (square) output.
    pub square: u8,
    pub deadzone: Deadzone,
    pub curve: Curve,
    pub map: AxisMap,
}

impl Stick {
    fn read(r: &mut Reader) -> Self {
        let enabled = r.u8();
        let square = r.u8();
        let deadzone = Deadzone::read(r);
        let curve = Curve::read(r);
        let map = AxisMap {
            enabled: r.u8(),
            flip_x: r.u8(),
            flip_y: r.u8(),
            flip_z: 0,
            axis_ratio: r.u8(),
            mouse_dpi: r.u8(),
            output: r.u8(),
            cross: r.u8(),
            up: r.u8(),
            down: r.u8(),
            left: r.u8(),
            right: r.u8(),
            center: r.u8(),
            stick_sensitivity: 0,
        };
        Stick {
            enabled,
            square,
            deadzone,
            curve,
            map,
        }
    }
    fn write(&self, w: &mut Writer) {
        w.u8(self.enabled);
        w.u8(self.square);
        self.deadzone.write(w);
        self.curve.write(w);
        let m = &self.map;
        for v in [
            m.enabled,
            m.flip_x,
            m.flip_y,
            m.axis_ratio,
            m.mouse_dpi,
            m.output,
            m.cross,
            m.up,
            m.down,
            m.left,
            m.right,
            m.center,
        ] {
            w.u8(v);
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Motion {
    /// 0 off, other values select how it activates (always / hold / toggle).
    pub activation: u8,
    pub trigger_keys: [u8; 3],
    /// bit0 / bit1 select the active axes.
    pub active_axis: u8,
    pub deadzone: Deadzone,
    pub curve: Curve,
    pub map: AxisMap,
}

impl Motion {
    fn read(r: &mut Reader) -> Self {
        let activation = r.u8();
        let trigger_keys = [r.u8(), r.u8(), r.u8()];
        let active_axis = r.u8();
        let deadzone = Deadzone::read(r);
        let curve = Curve::read(r);
        let map = AxisMap {
            enabled: r.u8(),
            flip_x: r.u8(),
            flip_y: r.u8(),
            flip_z: r.u8(),
            axis_ratio: r.u8(),
            mouse_dpi: r.u8(),
            output: r.u8(),
            cross: r.u8(),
            up: r.u8(),
            down: r.u8(),
            left: r.u8(),
            right: r.u8(),
            center: r.u8(),
            stick_sensitivity: r.u8(),
        };
        Motion {
            activation,
            trigger_keys,
            active_axis,
            deadzone,
            curve,
            map,
        }
    }
    fn write(&self, w: &mut Writer) {
        w.u8(self.activation);
        w.bytes(&self.trigger_keys);
        w.u8(self.active_axis);
        self.deadzone.write(w);
        self.curve.write(w);
        let m = &self.map;
        for v in [
            m.enabled,
            m.flip_x,
            m.flip_y,
            m.flip_z,
            m.axis_ratio,
            m.mouse_dpi,
            m.output,
            m.cross,
            m.up,
            m.down,
            m.left,
            m.right,
            m.center,
            m.stick_sensitivity,
        ] {
            w.u8(v);
        }
    }
}

/// One full profile slot.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    /// Raw name bytes, kept so unchanged names round-trip exactly.
    #[serde(skip)]
    name_raw: Vec<u8>,
    pub function: FunctionData,
    /// In [`BASIC_BUTTONS`] order.
    pub buttons: Vec<Button>,
    /// In [`BACK_BUTTONS`] order.
    pub back_buttons: Vec<BackButton>,
    pub lt: Trigger,
    pub rt: Trigger,
    pub left_stick: Stick,
    pub right_stick: Stick,
    pub motion_aim: Motion,
    pub motion_tilt: Motion,
}

impl Profile {
    pub fn from_bytes(b: &[u8]) -> Result<Self, Error> {
        if b.len() != PROFILE_LEN {
            return Err(Error::BadLength {
                expected: PROFILE_LEN,
                got: b.len(),
            });
        }
        let mut r = Reader { b, i: 0 };
        let name_raw = r.bytes(NAME_LEN);
        let end = name_raw.iter().position(|&c| c == 0).unwrap_or(NAME_LEN);
        let name = String::from_utf8_lossy(&name_raw[..end]).trim().to_string();

        let fd = r.bytes(32);
        let function = FunctionData {
            motors: Motors {
                left: fd[0],
                right: fd[1],
                lt: fd[2],
                rt: fd[3],
            },
            audio: Audio {
                enabled: fd[4],
                volume: fd[5],
                mixer: fd[6],
                mic_mute: fd[7],
                mic_sensitivity: fd[8],
            },
            extend: Extend {
                shift_enabled: fd[9],
                shift_value: fd[10],
                dpad_diagonal_lock: fd[11],
                xinput_abxy_swap: fd[12],
                switch_abxy_swap: fd[13],
                report_rate: fd[14],
                motion_precision: fd[15],
                stick_resolution: fd[16],
                ls_anti_jitter: fd[17],
                rs_anti_jitter: fd[18],
            },
            reserved: fd[19..].to_vec(),
        };
        let buttons = (0..BASIC_BUTTONS.len())
            .map(|_| Button::read(&mut r))
            .collect();
        let back_buttons = (0..BACK_BUTTONS.len())
            .map(|_| BackButton::read(&mut r))
            .collect();
        let p = Profile {
            name,
            name_raw,
            function,
            buttons,
            back_buttons,
            lt: Trigger::read(&mut r),
            rt: Trigger::read(&mut r),
            left_stick: Stick::read(&mut r),
            right_stick: Stick::read(&mut r),
            motion_aim: Motion::read(&mut r),
            motion_tilt: Motion::read(&mut r),
        };
        debug_assert_eq!(r.i, PROFILE_LEN);
        Ok(p)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer::default();
        let mut name = [0u8; NAME_LEN];
        let unchanged = {
            let end = self
                .name_raw
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(self.name_raw.len());
            self.name_raw.len() == NAME_LEN
                && String::from_utf8_lossy(&self.name_raw[..end]).trim() == self.name
        };
        if unchanged {
            name.copy_from_slice(&self.name_raw);
        } else {
            // Truncate on a char boundary, keep a trailing NUL.
            let mut n = 0;
            for (i, ch) in self.name.char_indices() {
                if i + ch.len_utf8() > NAME_LEN - 1 {
                    break;
                }
                n = i + ch.len_utf8();
            }
            name[..n].copy_from_slice(&self.name.as_bytes()[..n]);
        }
        w.bytes(&name);

        let f = &self.function;
        let e = &f.extend;
        w.bytes(&[f.motors.left, f.motors.right, f.motors.lt, f.motors.rt]);
        w.bytes(&[
            f.audio.enabled,
            f.audio.volume,
            f.audio.mixer,
            f.audio.mic_mute,
            f.audio.mic_sensitivity,
        ]);
        w.bytes(&[
            e.shift_enabled,
            e.shift_value,
            e.dpad_diagonal_lock,
            e.xinput_abxy_swap,
            e.switch_abxy_swap,
            e.report_rate,
            e.motion_precision,
            e.stick_resolution,
            e.ls_anti_jitter,
            e.rs_anti_jitter,
        ]);
        let mut reserved = f.reserved.clone();
        reserved.resize(13, 0);
        w.bytes(&reserved);

        for b in &self.buttons {
            b.write(&mut w);
        }
        for b in &self.back_buttons {
            b.write(&mut w);
        }
        self.lt.write(&mut w);
        self.rt.write(&mut w);
        self.left_stick.write(&mut w);
        self.right_stick.write(&mut w);
        self.motion_aim.write(&mut w);
        self.motion_tilt.write(&mut w);
        debug_assert_eq!(w.b.len(), PROFILE_LEN);
        w.b
    }

    /// Index into `buttons` for a button name from [`BASIC_BUTTONS`].
    pub fn button_index(name: &str) -> Option<usize> {
        BASIC_BUTTONS.iter().position(|b| *b == name)
    }
}

/// Motor raw value (0/25/50/75/100) to the app's 0..=4 level.
pub fn motor_level(raw: u8) -> u8 {
    match raw {
        100.. => 4,
        75.. => 3,
        50.. => 2,
        25.. => 1,
        _ => 0,
    }
}

pub fn motor_raw(level: u8) -> u8 {
    level.min(4) * 25
}

pub const REPORT_RATES: [(u8, &str); 4] = [
    (0, "250 Hz"),
    (2, "1000 Hz"),
    (4, "4000 Hz"),
    (5, "8000 Hz"),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn pseudo_random(len: usize, seed: u32) -> Vec<u8> {
        let mut s = seed;
        (0..len)
            .map(|_| {
                s = s.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                (s >> 16) as u8
            })
            .collect()
    }

    #[test]
    fn round_trip_random_bytes() {
        for seed in 0..32 {
            let bytes = pseudo_random(PROFILE_LEN, seed);
            let p = Profile::from_bytes(&bytes).unwrap();
            assert_eq!(p.to_bytes(), bytes, "seed {seed}");
        }
    }

    #[test]
    fn edit_changes_only_its_bytes() {
        let bytes = pseudo_random(PROFILE_LEN, 99);
        let mut p = Profile::from_bytes(&bytes).unwrap();
        p.right_stick.deadzone.inner = 0x0123;
        let out = p.to_bytes();
        let diff: Vec<usize> = (0..PROFILE_LEN).filter(|&i| out[i] != bytes[i]).collect();
        // Right stick starts at 916 + 36; dead zone inner is bytes 3..5.
        assert!(diff.iter().all(|&i| i == 955 || i == 956), "{diff:?}");
        assert_eq!(&out[955..957], &[0x01, 0x23]);
    }

    #[test]
    fn rename() {
        let mut p = Profile::from_bytes(&vec![0u8; PROFILE_LEN]).unwrap();
        p.name = "FPS".into();
        let b = p.to_bytes();
        assert_eq!(&b[..4], b"FPS\0");
    }

    #[test]
    fn wrong_length() {
        assert!(Profile::from_bytes(&[0; 10]).is_err());
    }
}

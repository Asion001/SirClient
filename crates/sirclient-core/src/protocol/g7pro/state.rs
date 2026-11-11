//! Live state report (input report 0x12, 64 bytes).
//!
//! | byte   | meaning                                               |
//! |-------:|-------------------------------------------------------|
//! | 1..=4  | LX LY RX RY, high byte (low bytes at 26..=29)         |
//! | 5      | low nibble hat (0 up, clockwise, 8+ centred); bits 4..7 X A B Y |
//! | 6      | LB RB LT RT View Menu LS RS                           |
//! | 7      | bit0 Home, bit1 Screenshot, bit3 L4, bit4 R4, bit5 M  |
//! | 8, 9   | LT, RT analog                                         |
//! | 10     | bit0 L5, bit1 R5, bit2 L6, bit3 R6                    |
//! | 31, 32 | calibration mode, calibration status bits             |
//! | 35, 36 | charge state, battery level                           |
//! | 37     | active lighting preset                                |
//! | 38..53 | current RGB of 5 zones                                |
//! | 53     | macro recording key (0 = not recording)               |
//! | 53..64 | same layout as 0..11 but before remapping (physical)  |

use serde::Serialize;

use super::commands::STATE_REPORT_ID;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Buttons {
    pub a: bool,
    pub b: bool,
    pub x: bool,
    pub y: bool,
    pub lb: bool,
    pub rb: bool,
    pub lt_digital: bool,
    pub rt_digital: bool,
    pub view: bool,
    pub menu: bool,
    pub ls: bool,
    pub rs: bool,
    pub home: bool,
    pub screenshot: bool,
    pub l4: bool,
    pub r4: bool,
    pub l5: bool,
    pub r5: bool,
    pub m: bool,
    pub dpad_up: bool,
    pub dpad_down: bool,
    pub dpad_left: bool,
    pub dpad_right: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Inputs {
    /// -1.0..=1.0, y grows downward.
    pub left: (f32, f32),
    pub right: (f32, f32),
    /// 0..=255
    pub lt: u8,
    pub rt: u8,
    pub buttons: Buttons,
}

/// Raw IMU readings (signed 16-bit, sensor units). Accelerometer: about
/// 8192 per g. Only the XBox edition reports these.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Motion {
    pub gyro: [i16; 3],
    pub accel: [i16; 3],
}

/// First byte of the XBox edition's live-state payload (reply type `E0`,
/// delivered by [`crate::gip`] as is).
pub const XBOX_STATE_ID: u8 = 0xE0;

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct State {
    /// After remapping (what the game sees).
    pub mapped: Inputs,
    /// Physical buttons before remapping.
    pub physical: Inputs,
    pub charge_state: u8,
    pub battery: u8,
    pub calibration_mode: u8,
    pub calibration_status: u8,
    pub light_preset: u8,
    pub zone_rgb: [[u8; 3]; 5],
    pub recording_key: u8,
    pub motion: Option<Motion>,
}

fn axis(hi: u8, lo: Option<u8>) -> f32 {
    match lo {
        Some(lo) => {
            let v = (((hi as i32) << 8) | lo as i32) - 32768;
            if v >= 0 {
                v as f32 / 32767.0
            } else {
                v as f32 / 32768.0
            }
        }
        None => {
            let v = hi as i32 - 128;
            if v >= 0 {
                v as f32 / 127.0
            } else {
                v as f32 / 128.0
            }
        }
    }
}

fn inputs(e: &[u8], lo: Option<&[u8]>) -> Inputs {
    let bit = |i: usize, b: u8| e[i] & (1 << b) != 0;
    let l = |i: usize| lo.map(|x| x[i]);
    let hat = e[5] & 0x0f;
    Inputs {
        left: (axis(e[1], l(0)), axis(e[2], l(1))),
        right: (axis(e[3], l(2)), axis(e[4], l(3))),
        lt: e[8],
        rt: e[9],
        buttons: Buttons {
            x: bit(5, 4),
            a: bit(5, 5),
            b: bit(5, 6),
            y: bit(5, 7),
            lb: bit(6, 0),
            rb: bit(6, 1),
            lt_digital: bit(6, 2),
            rt_digital: bit(6, 3),
            view: bit(6, 4),
            menu: bit(6, 5),
            ls: bit(6, 6),
            rs: bit(6, 7),
            home: bit(7, 0),
            screenshot: bit(7, 1),
            l4: bit(7, 3),
            r4: bit(7, 4),
            m: bit(7, 5),
            l5: bit(10, 0),
            r5: bit(10, 1),
            dpad_up: matches!(hat, 0 | 1 | 7),
            dpad_right: matches!(hat, 1..=3),
            dpad_down: matches!(hat, 3..=5),
            dpad_left: matches!(hat, 5..=7),
        },
    }
}

impl State {
    pub fn parse(r: &[u8]) -> Option<State> {
        if r.len() < 64 || r[0] != STATE_REPORT_ID {
            return None;
        }
        let lo = &r[26..30];
        let mut zone_rgb = [[0u8; 3]; 5];
        for (i, z) in zone_rgb.iter_mut().enumerate() {
            z.copy_from_slice(&r[38 + i * 3..41 + i * 3]);
        }
        Some(State {
            mapped: inputs(r, Some(lo)),
            physical: inputs(&r[53..64], Some(lo)),
            charge_state: r[35],
            battery: r[36],
            calibration_mode: r[31],
            calibration_status: r[32],
            light_preset: r[37],
            zone_rgb,
            recording_key: r[53],
            motion: None,
        })
    }

    /// XBox edition state (`E0` payload, 60 bytes): the 8K inputs in bytes
    /// 1..=10, gyro and accelerometer (i16 LE) at 13 and 19, charge state
    /// and battery at 28/29, active light animation at 30, zone RGB at 31,
    /// stick low bytes at 46 and the physical inputs at 49.
    pub fn parse_xbox(e: &[u8]) -> Option<State> {
        if e.len() < 60 || e[0] != XBOX_STATE_ID {
            return None;
        }
        let i16_at = |i: usize| i16::from_le_bytes([e[i], e[i + 1]]);
        let lo = &e[46..50];
        let mut zone_rgb = [[0u8; 3]; 5];
        for (i, z) in zone_rgb.iter_mut().enumerate() {
            z.copy_from_slice(&e[31 + i * 3..34 + i * 3]);
        }
        Some(State {
            mapped: inputs(e, Some(lo)),
            physical: inputs(&e[49..60], None),
            charge_state: e[28],
            battery: e[29],
            calibration_mode: 0,
            calibration_status: 0,
            light_preset: e[30],
            zone_rgb,
            recording_key: 0,
            motion: Some(Motion {
                gyro: [i16_at(13), i16_at(15), i16_at(17)],
                accel: [i16_at(19), i16_at(21), i16_at(23)],
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_centered() {
        let mut r = [0u8; 64];
        r[0] = 0x12;
        r[1..5].copy_from_slice(&[0x80; 4]);
        r[5] = 0x0f | (1 << 5);
        r[36] = 80;
        let s = State::parse(&r).unwrap();
        assert!(s.mapped.buttons.a);
        assert!(!s.mapped.buttons.dpad_up);
        assert_eq!(s.mapped.left, (0.0, 0.0));
        assert_eq!(s.battery, 80);
    }

    #[test]
    fn parse_xbox_real() {
        // E0 payload captured from a resting G7 Pro XBox.
        let hex = concat!(
            "e080807d800f000000000000000000000003004700cb1f35000000000063",
            "0000ffff00ffff00ffff00ffff00ffffbbd159e27f7f7a800f0000000000"
        );
        let hex: String = hex.chars().filter(|c| c.is_ascii_hexdigit()).collect();
        let e: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        let s = State::parse_xbox(&e).unwrap();
        assert_eq!(s.battery, 0x63);
        assert_eq!(s.zone_rgb[0], [0, 255, 255]);
        let m = s.motion.unwrap();
        assert_eq!(m.accel, [0x47, 0x1fcb, 0x35]);
        assert_eq!(m.gyro, [0, 0, 3]);
        assert!(s.mapped.left.0.abs() < 0.01);
    }
}

//! G7 Pro XBox slot layouts and their translation to the G7 Pro 8K ones.
//!
//! The XBox edition (see [`crate::gip`]) runs the same firmware family as the
//! 8K, but its profile slot is 480 bytes instead of 1070: no macros, dead
//! zones in whole percent instead of tenths, one motion trigger key, and the
//! back buttons in FL1, FL2, FR1, FR2 order. Mapped from GameSir Nexus logs,
//! which print each raw slot next to the settings it decoded.
//!
//! | offset | size   | section                                              |
//! |-------:|-------:|------------------------------------------------------|
//! |      0 | 32     | name                                                 |
//! |     32 | 32     | function data: 8K layout with 2 extra bytes after the motors |
//! |     64 | 16 × 7 | basic buttons (8K order)                             |
//! |    176 | 4 × 7  | back buttons FL1 FL2 FR1 FR2, no macros              |
//! |    204 | 2 × 28 | triggers LT, RT (8K trigger, dead zone in u8 %)      |
//! |    260 | 56     | unknown, kept as is                                  |
//! |    316 | 2 × 32 | sticks left, right (8K stick, dead zone in u8 %)     |
//! |    380 | 32     | unknown, kept as is                                  |
//! |    412 | 2 × 34 | motion aim, tilt (one trigger key, no sensitivity)   |
//!
//! The UI edits the 8K [`super::Profile`]; [`profile_to_8k`] and
//! [`profile_from_8k`] convert, keeping every XBox-only byte from the slot
//! as it was read.
//!
//! The lighting slot (511 bytes) is a different model altogether: four
//! keyframe animations, see [`XboxLight`].

use serde::{Deserialize, Serialize};

use super::commands::PROFILE_LEN as K_LEN;
use crate::Error;

pub const PROFILE_LEN: usize = 480;
pub const LIGHT_LEN: usize = 511;

// 8K offsets.
const K_FN: usize = 32;
const K_BUTTONS: usize = 64;
const K_BACK: usize = 176;
const K_BACK_LEN: usize = 169;
const K_LT: usize = 852;
const K_TRIGGER_LEN: usize = 32;
const K_LS: usize = 916;
const K_STICK_LEN: usize = 36;
const K_AIM: usize = 988;
const K_MOTION_LEN: usize = 41;

// XBox offsets.
const X_FN: usize = 32;
const X_BUTTONS: usize = 64;
const X_BACK: usize = 176;
const X_LT: usize = 204;
const X_TRIGGER_LEN: usize = 28;
const X_LS: usize = 316;
const X_STICK_LEN: usize = 32;
const X_AIM: usize = 412;
const X_MOTION_LEN: usize = 34;

/// XBox back-button position -> 8K back-button index (FL1 FR1 FL2 FR2).
const BACK_TO_8K: [usize; 4] = [0, 2, 1, 3];

/// Dead zone: XBox `[en, a, b, c, d]` in % <-> 8K `[en, a16, b16, c16, d16]`
/// in tenths of a percent.
fn dz_to_8k(x: &[u8], k: &mut [u8]) {
    k[0] = x[0];
    for i in 0..4 {
        let v = (x[1 + i] as u16) * 10;
        k[1 + 2 * i..3 + 2 * i].copy_from_slice(&v.to_be_bytes());
    }
}

fn dz_from_8k(k: &[u8], x: &mut [u8]) {
    x[0] = k[0];
    for i in 0..4 {
        let v = u16::from_be_bytes([k[1 + 2 * i], k[2 + 2 * i]]);
        x[1 + i] = (v.saturating_add(5) / 10).min(100) as u8;
    }
}

fn check(b: &[u8], expected: usize) -> Result<(), Error> {
    if b.len() == expected {
        Ok(())
    } else {
        Err(Error::BadLength {
            expected,
            got: b.len(),
        })
    }
}

/// Expand a 480-byte XBox profile slot into the 1070-byte 8K layout.
pub fn profile_to_8k(x: &[u8]) -> Result<Vec<u8>, Error> {
    check(x, PROFILE_LEN)?;
    let mut k = vec![0u8; K_LEN];
    k[..32].copy_from_slice(&x[..32]);
    // Motors, then (skipping the 2 XBox-only motor bytes) audio + extend.
    k[K_FN..K_FN + 4].copy_from_slice(&x[X_FN..X_FN + 4]);
    k[K_FN + 4..K_FN + 19].copy_from_slice(&x[X_FN + 6..X_FN + 21]);
    k[K_BUTTONS..K_BUTTONS + 112].copy_from_slice(&x[X_BUTTONS..X_BUTTONS + 112]);
    for (i, &ki) in BACK_TO_8K.iter().enumerate() {
        let xo = X_BACK + 7 * i;
        let ko = K_BACK + K_BACK_LEN * ki;
        k[ko..ko + 7].copy_from_slice(&x[xo..xo + 7]);
    }
    for t in 0..2 {
        let xo = X_LT + X_TRIGGER_LEN * t;
        let ko = K_LT + K_TRIGGER_LEN * t;
        k[ko..ko + 2].copy_from_slice(&x[xo..xo + 2]);
        dz_to_8k(&x[xo + 2..xo + 7], &mut k[ko + 2..ko + 11]);
        k[ko + 11..ko + 32].copy_from_slice(&x[xo + 7..xo + 28]);
    }
    for s in 0..2 {
        let xo = X_LS + X_STICK_LEN * s;
        let ko = K_LS + K_STICK_LEN * s;
        k[ko..ko + 2].copy_from_slice(&x[xo..xo + 2]);
        dz_to_8k(&x[xo + 2..xo + 7], &mut k[ko + 2..ko + 11]);
        k[ko + 11..ko + 36].copy_from_slice(&x[xo + 7..xo + 32]);
    }
    for m in 0..2 {
        let xo = X_AIM + X_MOTION_LEN * m;
        let ko = K_AIM + K_MOTION_LEN * m;
        k[ko] = x[xo]; // activation
        k[ko + 1] = x[xo + 1]; // trigger key (the 8K has three)
        k[ko + 4] = x[xo + 2]; // active axis
        dz_to_8k(&x[xo + 3..xo + 8], &mut k[ko + 5..ko + 14]);
        k[ko + 14..ko + 27].copy_from_slice(&x[xo + 8..xo + 21]); // curve
        k[ko + 27..ko + 40].copy_from_slice(&x[xo + 21..xo + 34]); // map
    }
    Ok(k)
}

/// Fold an edited 8K-layout profile back into the XBox slot `base` (the
/// bytes last read from the controller). Fields the XBox does not have
/// (macros, extra motion keys, motion stick sensitivity) are dropped.
pub fn profile_from_8k(k: &[u8], base: &[u8]) -> Result<Vec<u8>, Error> {
    check(k, K_LEN)?;
    check(base, PROFILE_LEN)?;
    let mut x = base.to_vec();
    x[..32].copy_from_slice(&k[..32]);
    x[X_FN..X_FN + 4].copy_from_slice(&k[K_FN..K_FN + 4]);
    x[X_FN + 6..X_FN + 21].copy_from_slice(&k[K_FN + 4..K_FN + 19]);
    x[X_BUTTONS..X_BUTTONS + 112].copy_from_slice(&k[K_BUTTONS..K_BUTTONS + 112]);
    for (i, &ki) in BACK_TO_8K.iter().enumerate() {
        let xo = X_BACK + 7 * i;
        let ko = K_BACK + K_BACK_LEN * ki;
        x[xo..xo + 7].copy_from_slice(&k[ko..ko + 7]);
    }
    for t in 0..2 {
        let xo = X_LT + X_TRIGGER_LEN * t;
        let ko = K_LT + K_TRIGGER_LEN * t;
        x[xo..xo + 2].copy_from_slice(&k[ko..ko + 2]);
        dz_from_8k(&k[ko + 2..ko + 11], &mut x[xo + 2..xo + 7]);
        x[xo + 7..xo + 28].copy_from_slice(&k[ko + 11..ko + 32]);
    }
    for s in 0..2 {
        let xo = X_LS + X_STICK_LEN * s;
        let ko = K_LS + K_STICK_LEN * s;
        x[xo..xo + 2].copy_from_slice(&k[ko..ko + 2]);
        dz_from_8k(&k[ko + 2..ko + 11], &mut x[xo + 2..xo + 7]);
        x[xo + 7..xo + 32].copy_from_slice(&k[ko + 11..ko + 36]);
    }
    for m in 0..2 {
        let xo = X_AIM + X_MOTION_LEN * m;
        let ko = K_AIM + K_MOTION_LEN * m;
        x[xo] = k[ko];
        x[xo + 1] = k[ko + 1];
        x[xo + 2] = k[ko + 4];
        dz_from_8k(&k[ko + 5..ko + 14], &mut x[xo + 3..xo + 8]);
        x[xo + 8..xo + 21].copy_from_slice(&k[ko + 14..ko + 27]);
        x[xo + 21..xo + 34].copy_from_slice(&k[ko + 27..ko + 40]);
    }
    Ok(x)
}

/// Report rates the XBox edition offers (raw value, label).
pub const REPORT_RATES: [(u8, &str); 3] = [(0, "250 Hz"), (1, "500 Hz"), (2, "1000 Hz")];

/// LED order inside a keyframe.
pub const ZONES: [&str; 5] = [
    "Lower left",
    "Lower right",
    "Upper left",
    "Home",
    "Upper right",
];
pub const ANIMATIONS: usize = 4;
pub const MAX_KEYFRAMES: usize = 8;
const ANIMATION_LEN: usize = 4 + MAX_KEYFRAMES * ZONES.len() * 3;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Animation {
    /// Frames in use, 1..=8.
    pub keyframes: u8,
    /// LEDs per frame; always 5 in Nexus.
    pub effect_count: u8,
    pub speed: u8,
    /// 0..=100.
    pub brightness: u8,
    /// `MAX_KEYFRAMES` frames of RGB per zone in [`ZONES`] order.
    pub frames: Vec<[[u8; 3]; 5]>,
}

/// The 511-byte lighting slot (slot `0x20`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct XboxLight {
    pub audio_reactive: u8,
    pub animations: Vec<Animation>,
    /// Byte between the animations and the global settings; kept as is.
    pub reserved_a: u8,
    pub user_effect: u8,
    /// Colour of the profile indicator LED.
    pub profile_led: [u8; 3],
    pub pedestal_wake: u8,
    /// Wireless standby time in minutes; 255 = never.
    pub sleep_time: u8,
    pub pedestal_brightness_enabled: u8,
    pub pedestal_brightness: u8,
    pub reserved: [u8; 5],
}

impl XboxLight {
    pub fn from_bytes(b: &[u8]) -> Result<Self, Error> {
        check(b, LIGHT_LEN)?;
        let animations = (0..ANIMATIONS)
            .map(|a| {
                let o = 1 + a * ANIMATION_LEN;
                let frames = (0..MAX_KEYFRAMES)
                    .map(|f| {
                        let mut fr = [[0u8; 3]; 5];
                        for (z, c) in fr.iter_mut().enumerate() {
                            let p = o + 4 + (f * 5 + z) * 3;
                            c.copy_from_slice(&b[p..p + 3]);
                        }
                        fr
                    })
                    .collect();
                Animation {
                    keyframes: b[o],
                    effect_count: b[o + 1],
                    speed: b[o + 2],
                    brightness: b[o + 3],
                    frames,
                }
            })
            .collect();
        let g = 1 + ANIMATIONS * ANIMATION_LEN;
        let mut reserved = [0u8; 5];
        reserved.copy_from_slice(&b[g + 9..g + 14]);
        Ok(XboxLight {
            audio_reactive: b[0],
            animations,
            reserved_a: b[g],
            user_effect: b[g + 1],
            profile_led: [b[g + 2], b[g + 3], b[g + 4]],
            pedestal_wake: b[g + 5],
            sleep_time: b[g + 6],
            pedestal_brightness_enabled: b[g + 7],
            pedestal_brightness: b[g + 8],
            reserved,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(LIGHT_LEN);
        out.push(self.audio_reactive);
        for a in 0..ANIMATIONS {
            let an = self.animations.get(a).cloned().unwrap_or_default();
            out.extend_from_slice(&[an.keyframes, an.effect_count, an.speed, an.brightness]);
            for f in 0..MAX_KEYFRAMES {
                let fr = an.frames.get(f).copied().unwrap_or_default();
                for c in fr {
                    out.extend_from_slice(&c);
                }
            }
        }
        out.push(self.reserved_a);
        out.push(self.user_effect);
        out.extend_from_slice(&self.profile_led);
        out.extend_from_slice(&[
            self.pedestal_wake,
            self.sleep_time,
            self.pedestal_brightness_enabled,
            self.pedestal_brightness,
        ]);
        out.extend_from_slice(&self.reserved);
        debug_assert_eq!(out.len(), LIGHT_LEN);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::g7pro::Profile;

    /// Profile 1 as read from a real G7 Pro XBox (fw 2.4.9) and logged by
    /// GameSir Nexus.
    const REAL: &str = concat!(
        "00000000000000000000000000000000000000000000000000000000000000004b4b4b4b000001323200020000000000",
        "020000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
        "000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000000001070000000000011f0000000000",
        "010800000000000120000000000001005f00640000000000000a5a010064000028298180d7d6ffff000001055f006400",
        "00000000000a5a010064000028298080d7d6ffff00000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000001000105640064010064000028298180d7d6ffff",
        "01000032320100ffffffffff01000106640064010064000029298080d6d6ffff01000032320200ffffffffff01000000",
        "00000000000000000000000000000000000000000000000000000000021f030102640064010064000027298080d8d6ff",
        "ff010000003c320200ffffffffff0012020102640064010064000027298080d8d6ffff010000003c320200ffffffffff",
    );

    fn real() -> Vec<u8> {
        let s = REAL;
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn real_profile_decodes() {
        let x = real();
        assert_eq!(x.len(), PROFILE_LEN);
        let p = Profile::from_bytes(&profile_to_8k(&x).unwrap()).unwrap();
        assert_eq!(p.function.motors.left, 75);
        assert_eq!(p.function.audio.volume, 50);
        assert_eq!(p.function.extend.report_rate, 2);
        // FL1 -> L3 (7), FR1 -> R3 (8), FL2 -> FL2 (31), FR2 -> FR2 (32).
        let keys: Vec<u8> = p
            .back_buttons
            .iter()
            .map(|b| b.button.map.keys[0])
            .collect();
        assert_eq!(keys, vec![7, 8, 31, 32]);
        assert_eq!(p.lt.deadzone.outer, 950);
        assert_eq!(p.rt.deadzone.inner, 50);
        assert_eq!((p.lt.hair_start, p.lt.hair_end), (10, 90));
        assert_eq!(p.left_stick.deadzone.inner, 50);
        assert_eq!(p.right_stick.deadzone.inner, 60);
        assert_eq!(p.left_stick.map.output, 1);
        assert_eq!(p.right_stick.map.output, 2);
        assert_eq!(p.motion_aim.activation, 2);
        assert_eq!(p.motion_aim.trigger_keys[0], 31);
        assert_eq!(p.motion_aim.map.axis_ratio, 60);
        assert_eq!(p.motion_tilt.trigger_keys[0], 18);
    }

    #[test]
    fn profile_round_trips() {
        let x = real();
        let k = profile_to_8k(&x).unwrap();
        let p = Profile::from_bytes(&k).unwrap();
        assert_eq!(profile_from_8k(&p.to_bytes(), &x).unwrap(), x);
    }

    #[test]
    fn edits_land_in_xbox_bytes() {
        let x = real();
        let mut p = Profile::from_bytes(&profile_to_8k(&x).unwrap()).unwrap();
        p.back_buttons[1].button.map.keys[0] = 9; // FR1 -> A
        p.left_stick.deadzone.inner = 120;
        p.function.extend.report_rate = 1;
        let y = profile_from_8k(&p.to_bytes(), &x).unwrap();
        assert_eq!(y[X_BACK + 7 * 2 + 3], 9); // FR1 is third on the XBox
        assert_eq!(y[X_LS + 3], 12);
        assert_eq!(y[X_FN + 16], 1);
        let diff = (0..PROFILE_LEN).filter(|&i| x[i] != y[i]).count();
        assert_eq!(diff, 3);
    }

    #[test]
    fn light_round_trips() {
        let mut b = vec![0u8; LIGHT_LEN];
        for a in 0..ANIMATIONS {
            let o = 1 + a * ANIMATION_LEN;
            b[o..o + 4].copy_from_slice(&[1, 5, 10, 100]);
            for z in 0..5 {
                let p = o + 4 + z * 3;
                b[p..p + 3].copy_from_slice(&[0, 255, 255]);
            }
        }
        let g = 1 + ANIMATIONS * ANIMATION_LEN;
        b[g + 1..g + 9].copy_from_slice(&[1, 255, 255, 255, 1, 10, 1, 100]);
        let l = XboxLight::from_bytes(&b).unwrap();
        assert_eq!(l.animations[3].speed, 10);
        assert_eq!(l.animations[0].frames[0][4], [0, 255, 255]);
        assert_eq!(l.sleep_time, 10);
        assert_eq!(l.profile_led, [255, 255, 255]);
        assert_eq!(l.to_bytes(), b);
    }
}

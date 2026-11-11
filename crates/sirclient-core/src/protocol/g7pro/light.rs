//! G7 Pro 8K lighting slot (slot 32, 165 bytes).
//!
//! Five 32-byte animation presets followed by five global bytes.

use serde::{Deserialize, Serialize};

use super::commands::LIGHT_PROFILE_LEN;
use crate::Error;

pub const PRESETS: usize = 5;
const PRESET_LEN: usize = 32;

/// LED zones in wire order.
pub const ZONES: [&str; 5] = [
    "Lower left",
    "Upper left",
    "Upper right",
    "Lower right",
    "Home",
];

pub const MODES: [(u8, &str); 6] = [
    (0, "Off"),
    (1, "Static"),
    (2, "Breathing"),
    (3, "Rainbow"),
    (4, "Phantom"),
    (5, "Radar"),
];

pub fn mode_name(mode: u8) -> &'static str {
    MODES
        .iter()
        .find(|(m, _)| *m == mode)
        .map(|(_, n)| *n)
        .unwrap_or("Unknown")
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hsv {
    /// 0..=360
    pub h: u16,
    /// 0..=100
    pub s: u8,
    /// 0..=100
    pub v: u8,
}

impl Hsv {
    /// Convert to 8-bit RGB for display.
    pub fn to_rgb(self) -> [u8; 3] {
        let h = (self.h % 360) as f32 / 60.0;
        let s = self.s.min(100) as f32 / 100.0;
        let v = self.v.min(100) as f32 / 100.0;
        let c = v * s;
        let x = c * (1.0 - (h % 2.0 - 1.0).abs());
        let (r, g, b) = match h as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let m = v - c;
        [
            ((r + m) * 255.0) as u8,
            ((g + m) * 255.0) as u8,
            ((b + m) * 255.0) as u8,
        ]
    }

    pub fn from_rgb([r, g, b]: [u8; 3]) -> Self {
        let (r, g, b) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let d = max - min;
        let h = if d == 0.0 {
            0.0
        } else if max == r {
            60.0 * (((g - b) / d).rem_euclid(6.0))
        } else if max == g {
            60.0 * ((b - r) / d + 2.0)
        } else {
            60.0 * ((r - g) / d + 4.0)
        };
        let s = if max == 0.0 { 0.0 } else { d / max };
        Hsv {
            h: h.round() as u16 % 360,
            s: (s * 100.0).round() as u8,
            v: (max * 100.0).round() as u8,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preset {
    pub mode: u8,
    pub static_brightness: u8,
    pub breathing_brightness: u8,
    pub phantom_brightness: u8,
    pub rainbow_brightness: u8,
    pub radar_brightness: u8,
    pub breathing_speed: u8,
    pub phantom_speed: u8,
    pub rainbow_speed: u8,
    pub radar_speed: u8,
    pub phantom_hue: u8,
    pub rainbow_hue: u8,
    /// In [`ZONES`] order.
    pub colors: [Hsv; 5],
}

impl Preset {
    /// Brightness field that applies to the current mode.
    pub fn brightness_mut(&mut self) -> &mut u8 {
        match self.mode {
            2 => &mut self.breathing_brightness,
            3 => &mut self.rainbow_brightness,
            4 => &mut self.phantom_brightness,
            5 => &mut self.radar_brightness,
            _ => &mut self.static_brightness,
        }
    }

    /// Speed field for the current mode (Static/Off have none).
    pub fn speed_mut(&mut self) -> Option<&mut u8> {
        match self.mode {
            2 => Some(&mut self.breathing_speed),
            3 => Some(&mut self.rainbow_speed),
            4 => Some(&mut self.phantom_speed),
            5 => Some(&mut self.radar_speed),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LightProfile {
    pub presets: Vec<Preset>,
    /// bit0: wake on raise.
    pub flags: u8,
    pub standby_time: u8,
    pub dock_led_mode: u8,
    pub dock_led_brightness: u8,
    pub report_rate: u8,
}

impl LightProfile {
    pub fn from_bytes(b: &[u8]) -> Result<Self, Error> {
        if b.len() != LIGHT_PROFILE_LEN {
            return Err(Error::BadLength {
                expected: LIGHT_PROFILE_LEN,
                got: b.len(),
            });
        }
        let presets = b[..PRESETS * PRESET_LEN]
            .chunks(PRESET_LEN)
            .map(|p| {
                let mut colors = [Hsv::default(); 5];
                for (z, c) in colors.iter_mut().enumerate() {
                    let o = 12 + z * 4;
                    *c = Hsv {
                        h: u16::from_be_bytes([p[o], p[o + 1]]),
                        s: p[o + 2],
                        v: p[o + 3],
                    };
                }
                Preset {
                    mode: p[0],
                    static_brightness: p[1],
                    breathing_brightness: p[2],
                    phantom_brightness: p[3],
                    rainbow_brightness: p[4],
                    radar_brightness: p[5],
                    breathing_speed: p[6],
                    phantom_speed: p[7],
                    rainbow_speed: p[8],
                    radar_speed: p[9],
                    phantom_hue: p[10],
                    rainbow_hue: p[11],
                    colors,
                }
            })
            .collect();
        let g = &b[PRESETS * PRESET_LEN..];
        Ok(LightProfile {
            presets,
            flags: g[0],
            standby_time: g[1],
            dock_led_mode: g[2],
            dock_led_brightness: g[3],
            report_rate: g[4],
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(LIGHT_PROFILE_LEN);
        for p in &self.presets {
            out.extend_from_slice(&[
                p.mode,
                p.static_brightness,
                p.breathing_brightness,
                p.phantom_brightness,
                p.rainbow_brightness,
                p.radar_brightness,
                p.breathing_speed,
                p.phantom_speed,
                p.rainbow_speed,
                p.radar_speed,
                p.phantom_hue,
                p.rainbow_hue,
            ]);
            for c in p.colors {
                out.extend_from_slice(&c.h.to_be_bytes());
                out.push(c.s);
                out.push(c.v);
            }
        }
        out.extend_from_slice(&[
            self.flags,
            self.standby_time,
            self.dock_led_mode,
            self.dock_led_brightness,
            self.report_rate,
        ]);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let bytes: Vec<u8> = (0..LIGHT_PROFILE_LEN).map(|i| (i * 7 + 3) as u8).collect();
        assert_eq!(LightProfile::from_bytes(&bytes).unwrap().to_bytes(), bytes);
    }

    #[test]
    fn hsv_rgb() {
        assert_eq!(
            Hsv {
                h: 0,
                s: 100,
                v: 100
            }
            .to_rgb(),
            [255, 0, 0]
        );
        assert_eq!(
            Hsv::from_rgb([0, 0, 255]),
            Hsv {
                h: 240,
                s: 100,
                v: 100
            }
        );
    }
}

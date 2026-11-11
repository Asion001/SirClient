//! Response-curve presets, ported from GameSir Connect so the points we
//! write match what the official app writes.
//!
//! Curve kinds: 0 linear, 1 relaxed (slow start), 2 aggressive (fast
//! start), 3 custom (points edited by hand). For kinds 0..=2 the three
//! middle points are a cubic Bézier sampled at ¼, ½ and ¾ between the
//! dead-zone bounds; `curvature` (0..=100) bends it.

use super::profile::{Curve, Deadzone};

pub const LINEAR: u8 = 0;
pub const RELAXED: u8 = 1;
pub const AGGRESSIVE: u8 = 2;
pub const CUSTOM: u8 = 3;

type P = (f32, f32);

fn bezier(t: f32, p0: P, c1: P, c2: P, p3: P) -> P {
    let u = 1.0 - t;
    let f = |a: f32, b: f32, c: f32, d: f32| {
        u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d
    };
    (f(p0.0, c1.0, c2.0, p3.0), f(p0.1, c1.1, c2.1, p3.1))
}

/// Round half away from zero like JavaScript's Math.round for positives.
fn round(v: f32) -> f32 {
    (v + 0.5).floor()
}

/// Middle points in percent (0..=100 of full travel) for a preset.
pub fn preset_points(dz: &Deadzone, kind: u8, curvature: u8) -> [P; 3] {
    let p0 = (dz.inner as f32 / 10.0, dz.anti_inner as f32 / 10.0);
    let p3 = (dz.outer as f32 / 10.0, dz.anti_outer as f32 / 10.0);
    let c = curvature.min(100) as f32 / 100.0;
    let (hx, hy) = ((p3.0 - p0.0) / 2.0, (p3.1 - p0.1) / 2.0);
    let (c1, c2) = match kind {
        RELAXED => (
            (p0.0 + hx * c, p0.1 + hy * (1.0 - c)),
            (p3.0 - hx * (1.0 - c), p3.1 - hy * c),
        ),
        AGGRESSIVE => (
            (p0.0 + hx * (1.0 - c), p0.1 + hy * c),
            (p3.0 - hx * (1.0 - c), p3.1 - hy * c),
        ),
        _ => (p0, p3),
    };
    let mut out = [(0.0, 0.0); 3];
    for (i, t) in [0.25, 0.5, 0.75].into_iter().enumerate() {
        let (x, y) = bezier(t, p0, c1, c2, p3);
        out[i] = (round(x), round(y));
    }
    out
}

/// Percent point to the 0..=255 byte the firmware stores, scaled between
/// the dead-zone bounds.
fn to_byte(v: f32, lo: u16, hi: u16) -> u8 {
    let lo = round(lo as f32 / 10.0);
    let hi = round(hi as f32 / 10.0);
    if hi <= lo {
        return 0;
    }
    round((v - lo) / (hi - lo) * 255.0).clamp(0.0, 255.0) as u8
}

/// Percent value of a stored byte.
pub fn from_byte(b: u8, lo: u16, hi: u16) -> f32 {
    let lo = round(lo as f32 / 10.0);
    let hi = round(hi as f32 / 10.0);
    b as f32 / 255.0 * (hi - lo) + lo
}

/// Recompute a 5-point (stick / motion) curve after its kind, curvature or
/// dead zone changed. Custom curves keep their middle points.
pub fn refresh_stick(curve: &mut Curve, dz: &Deadzone) {
    curve.enabled = 1;
    curve.points[0] = (0, 0);
    curve.points[4] = (255, 255);
    if curve.kind == CUSTOM {
        return;
    }
    for (i, (x, y)) in preset_points(dz, curve.kind, curve.curvature)
        .into_iter()
        .enumerate()
    {
        curve.points[i + 1] = (
            to_byte(x, dz.inner, dz.outer),
            to_byte(y, dz.anti_inner, dz.anti_outer),
        );
    }
}

/// Same for a 3-point trigger curve (points 0 and 4 are fixed at 0 / 255).
pub fn refresh_trigger(curve: &mut Curve, dz: &Deadzone) {
    refresh_stick(curve, dz);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dz() -> Deadzone {
        Deadzone {
            enabled: 1,
            inner: 0,
            outer: 1000,
            anti_inner: 0,
            anti_outer: 1000,
        }
    }

    #[test]
    fn linear_is_diagonal() {
        let mut c = Curve {
            kind: LINEAR,
            curvature: 100,
            ..Default::default()
        };
        refresh_stick(&mut c, &dz());
        // Same sampling as GameSir Connect: on the diagonal, not evenly spaced.
        assert_eq!(
            c.points,
            [(0, 0), (41, 41), (128, 128), (214, 214), (255, 255)]
        );
    }

    #[test]
    fn relaxed_is_below_diagonal() {
        let mut c = Curve {
            kind: RELAXED,
            curvature: 100,
            ..Default::default()
        };
        refresh_stick(&mut c, &dz());
        assert!(c.points[1].1 < c.points[1].0);
        let mut a = Curve {
            kind: AGGRESSIVE,
            curvature: 100,
            ..Default::default()
        };
        refresh_stick(&mut a, &dz());
        assert!(a.points[1].1 > a.points[1].0);
    }

    #[test]
    fn custom_untouched() {
        let mut c = Curve {
            kind: CUSTOM,
            points: [(0, 0), (10, 200), (20, 210), (30, 220), (255, 255)],
            ..Default::default()
        };
        let before = c.points;
        refresh_stick(&mut c, &dz());
        assert_eq!(c.points, before);
    }
}

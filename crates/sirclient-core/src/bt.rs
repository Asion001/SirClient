//! G7 Pro XBox over Bluetooth.
//!
//! Over Bluetooth the controller presents itself as a Microsoft "Xbox
//! Bluetooth Gamepad" (`045E:02FD`) with the standard Xbox One S HID
//! descriptor and no GameSir configuration channel. GameSir Nexus has no
//! Bluetooth support either, so settings stay dongle/cable only. What the
//! link does carry is live input, battery and the four rumble motors, which
//! [`BtLink`] maps onto the session's state reports and vibrate command.

use crate::Error;
use crate::protocol::g7pro::commands::{STATE_REPORT_ID, cmd};
use crate::session::Link;

pub const MS_VID: u16 = 0x045E;
/// Xbox One S / Series-style Bluetooth gamepad PID the G7 Pro XBox uses.
pub const BT_PID: u16 = 0x02FD;

const REPORT_INPUT: u8 = 0x01;
const REPORT_GUIDE: u8 = 0x02;
const REPORT_RUMBLE: u8 = 0x03;
const REPORT_BATTERY: u8 = 0x04;

/// Live-only link over the Bluetooth HID interface.
pub struct BtLink<D> {
    dev: D,
    guide: bool,
    battery: u8,
    charging: u8,
}

/// The pieces of a HID device [`BtLink`] needs (lets tests use a fake).
pub trait HidIo: Send {
    fn write(&mut self, data: &[u8]) -> Result<(), Error>;
    fn read(&mut self, buf: &mut [u8], timeout_ms: i32) -> Result<usize, Error>;
}

impl HidIo for hidapi::HidDevice {
    fn write(&mut self, data: &[u8]) -> Result<(), Error> {
        hidapi::HidDevice::write(self, data)?;
        Ok(())
    }
    fn read(&mut self, buf: &mut [u8], timeout_ms: i32) -> Result<usize, Error> {
        Ok(self.read_timeout(buf, timeout_ms)?)
    }
}

impl<D: HidIo> BtLink<D> {
    pub fn new(dev: D) -> Self {
        BtLink {
            dev,
            guide: false,
            battery: 0,
            charging: 0,
        }
    }

    /// Rebuild an Xbox BT input report as the 8K's 64-byte state report.
    fn state_report(&self, r: &[u8]) -> Option<Vec<u8>> {
        if r.len() < 16 {
            return None;
        }
        let u16_at = |i: usize| u16::from_le_bytes([r[i], r[i + 1]]);
        let mut s = vec![0u8; 64];
        s[0] = STATE_REPORT_ID;
        // Sticks: 0..=65535 with y growing downward, as on the 8K.
        for (k, i) in [1usize, 3, 5, 7].into_iter().enumerate() {
            let v = u16_at(i);
            s[1 + k] = (v >> 8) as u8;
            s[26 + k] = v as u8;
        }
        // Hat: 1..=8 clockwise from up, anything else centred (this pad
        // sends 9); 8K: 0..=7, 8+ centred.
        let hat = match r[13] & 0x0F {
            h @ 1..=8 => h - 1,
            _ => 0x0F,
        };
        let b0 = r[14];
        let b1 = r[15];
        let bit = |b: u8, n: u8| (b >> n) & 1;
        // 8K byte 5 bits 4..7: X A B Y.
        s[5] = hat | bit(b0, 3) << 4 | bit(b0, 0) << 5 | bit(b0, 1) << 6 | bit(b0, 4) << 7;
        let lt = (u16_at(9) >> 2) as u8;
        let rt = (u16_at(11) >> 2) as u8;
        // 8K byte 6: LB RB LT RT View Menu LS RS.
        s[6] = bit(b0, 6)
            | bit(b0, 7) << 1
            | ((lt > 0x20) as u8) << 2
            | ((rt > 0x20) as u8) << 3
            | bit(b1, 2) << 4
            | bit(b1, 3) << 5
            | bit(b1, 5) << 6
            | bit(b1, 6) << 7;
        // 8K byte 7: bit0 Home, bit1 Screenshot (share).
        let share = r.get(16).copied().unwrap_or(0) & 1;
        s[7] = (self.guide as u8 | bit(b1, 4)) | share << 1;
        s[8] = lt;
        s[9] = rt;
        s[35] = self.charging;
        s[36] = self.battery;
        // Physical = mapped: the pad remaps internally.
        let mapped: Vec<u8> = s[0..11].to_vec();
        s[53..64].copy_from_slice(&mapped);
        Some(s)
    }
}

/// Xbox BT battery report byte: bits 0..1 level (0 empty .. 3 full),
/// bit 4 charging.
pub fn battery_percent(b: u8) -> (u8, u8) {
    let pct = [10, 40, 70, 100][(b & 0x03) as usize];
    (pct, (b >> 4) & 1)
}

/// Output report 3: enable mask, LT, RT, left, right (0..=100), duration
/// (10 ms units), start delay, loop count.
pub fn rumble_report(left: u8, right: u8, lt: u8, rt: u8) -> [u8; 9] {
    let pct = |v: u8| (v as u16 * 100 / 255) as u8;
    [
        REPORT_RUMBLE,
        0x0F,
        pct(lt),
        pct(rt),
        pct(left),
        pct(right),
        0xFF,
        0x00,
        0x00,
    ]
}

impl<D: HidIo> Link for BtLink<D> {
    fn write(&mut self, report: &[u8]) -> Result<(), Error> {
        if report.get(1) == Some(&cmd::VIBRATE) {
            let at = |i: usize| report.get(i).copied().unwrap_or(0);
            return self.dev.write(&rumble_report(at(4), at(5), at(6), at(7)));
        }
        Ok(()) // no configuration channel over Bluetooth
    }

    fn read(&mut self, buf: &mut [u8], timeout_ms: i32) -> Result<usize, Error> {
        let mut r = [0u8; 64];
        let n = self.dev.read(&mut r, timeout_ms)?;
        if n == 0 {
            return Ok(0);
        }
        let r = &r[..n];
        match r[0] {
            REPORT_GUIDE if n >= 2 => self.guide = r[1] & 1 != 0,
            REPORT_BATTERY if n >= 2 => (self.battery, self.charging) = battery_percent(r[1]),
            REPORT_INPUT => {
                if let Some(s) = self.state_report(r) {
                    let k = s.len().min(buf.len());
                    buf[..k].copy_from_slice(&s[..k]);
                    return Ok(k);
                }
            }
            _ => {}
        }
        Ok(0)
    }

    fn has_config(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::g7pro::State;

    struct Fake(Vec<Vec<u8>>, Vec<Vec<u8>>);
    impl HidIo for Fake {
        fn write(&mut self, d: &[u8]) -> Result<(), Error> {
            self.1.push(d.to_vec());
            Ok(())
        }
        fn read(&mut self, buf: &mut [u8], _: i32) -> Result<usize, Error> {
            let Some(r) = self.0.pop() else { return Ok(0) };
            buf[..r.len()].copy_from_slice(&r);
            Ok(r.len())
        }
    }

    #[test]
    fn input_and_battery() {
        let mut input = vec![
            0x01, 0, 0x80, 0, 0x80, 0xff, 0xff, 0, 0x80, 0xff, 0x03, 0, 0, 1, 0x01, 0x08, 0,
        ];
        input.resize(17, 0);
        let mut l = BtLink::new(Fake(vec![input, vec![0x04, 0x13]], vec![]));
        let mut buf = [0u8; 64];
        assert_eq!(l.read(&mut buf, 1).unwrap(), 0); // battery
        let n = l.read(&mut buf, 1).unwrap();
        let s = State::parse(&buf[..n]).unwrap();
        assert!(s.mapped.buttons.a);
        assert!(s.mapped.buttons.menu);
        assert!(s.mapped.buttons.dpad_up);
        assert_eq!(s.mapped.lt, 255);
        assert!(s.mapped.right.0 > 0.99);
        assert_eq!((s.battery, s.charge_state), (100, 1));
    }

    #[test]
    fn rumble_maps_to_report_3() {
        let mut l = BtLink::new(Fake(vec![], vec![]));
        l.write(&[0x0F, cmd::VIBRATE, 0x66, 0x55, 255, 0, 0, 255])
            .unwrap();
        l.write(&[0x0F, cmd::READ_FIRMWARE]).unwrap();
        assert_eq!(l.dev.1, vec![vec![0x03, 0x0F, 0, 100, 100, 0, 0xFF, 0, 0]]);
    }
}

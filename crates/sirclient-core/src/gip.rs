//! Xbox-edition controllers over raw USB (Xbox Game Input Protocol).
//!
//! The G7 Pro XBox is a licensed Xbox controller. Its "app mode" shows up as
//! `3537:10BA` (cable) or `3537:10BB` (dongle) with a vendor-class GIP
//! interface, which Linux binds to `xpad`. GameSir Nexus configures it by
//! sending GIP message `0x0F` whose payload is the same command set the
//! G7 Pro 8K uses over HID (`[cmd, slot, offset hi, offset lo, len, data]`,
//! padded to 60 bytes). Replies come back as GIP message `0x10` with a
//! 60-byte payload laid out like the HID replies.
//!
//! Two quirks matter (seen on Linux, where `xpad` does not send `F2`):
//!
//! * Shortly after the pad links, the dongle drops to a plain HID mode
//!   (`3537:1022`) unless the host sends the `0xF2` "app present" command,
//!   which Nexus does on connect. [`GipLink::open`] sends it right away and
//!   the session heartbeat keeps sending it.
//! * From the HID fallback a USB port reset brings the dongle back into
//!   GIP mode without replugging; [`GipLink::open`] does that when needed.
//!
//! While SirClient holds the interface on Linux, `xpad` is detached, so the
//! pad does not work as a gamepad; it is reattached when the link is dropped.
//! macOS has no driver for the GIP interface, so libusb claims it directly.

use crate::transport::HidInfo;

/// Product IDs of the G7 Pro XBox in GIP (app) mode.
pub const GIP_PIDS: [u16; 2] = [0x10BA, 0x10BB];
/// Product ID of its plain-HID fallback mode.
pub const HID_FALLBACK_PID: u16 = 0x1022;

/// Device paths for GIP devices start with this prefix.
pub const PATH_PREFIX: &str = "gip:";

/// GIP message carrying a configuration command (host to device).
const MSG_COMMAND: u8 = 0x0F;
/// GIP message carrying a configuration reply (device to host).
const MSG_REPLY: u8 = 0x10;
/// Payload size of command and reply messages.
const PAYLOAD: usize = 60;
/// Reply type the firmware streams continuously: the live controller state
/// (parsed by [`crate::protocol::g7pro::State::parse_xbox`]).
const STATE_STREAM: u8 = crate::protocol::g7pro::state::XBOX_STATE_ID;

pub fn is_gip_path(path: &str) -> bool {
    path.starts_with(PATH_PREFIX)
}

/// Wrap a HID-style command report (`[0x0F, cmd, ...]`, any length) into a
/// GIP command message.
pub fn encode_command(report: &[u8], seq: u8) -> [u8; 4 + PAYLOAD] {
    let mut out = [0u8; 4 + PAYLOAD];
    out[0] = MSG_COMMAND;
    out[1] = 0x00;
    out[2] = seq;
    out[3] = PAYLOAD as u8;
    let body = report.get(1..).unwrap_or(&[]);
    let n = body.len().min(PAYLOAD);
    out[4..4 + n].copy_from_slice(&body[..n]);
    out
}

/// Turn a GIP reply message into the HID-style report the session parses
/// (`[0x10, reply code, ...]`). Returns `None` for anything else.
pub fn decode_reply(packet: &[u8]) -> Option<Vec<u8>> {
    if packet.len() < 5 || packet[0] != MSG_REPLY {
        return None;
    }
    let len = (packet[3] as usize).min(packet.len() - 4);
    reply_from_payload(&packet[4..4 + len])
}

/// Turn the payload of a GIP reply message (`0x10`) into what the session
/// parses: the state stream as is, anything else as a HID-style report.
pub fn reply_from_payload(payload: &[u8]) -> Option<Vec<u8>> {
    match payload.first() {
        None => None,
        Some(&STATE_STREAM) => Some(payload.to_vec()),
        Some(_) => {
            let mut out = Vec::with_capacity(1 + payload.len());
            out.push(MSG_REPLY);
            out.extend_from_slice(payload);
            Some(out)
        }
    }
}

/// GIP rumble (message `0x09`): grip motors and trigger motors, each
/// 0..=255 (sent as 0..=100 %).
pub fn rumble(left: u8, right: u8, lt: u8, rt: u8, seq: u8) -> [u8; 13] {
    let pct = |v: u8| (v as u16 * 100 / 255) as u8;
    [
        0x09,
        0x00,
        seq,
        0x09,
        0x00,
        0x0F, // all four motors
        pct(lt),
        pct(rt),
        pct(left),
        pct(right),
        0xFF, // duration: until the next rumble message
        0x00,
        0x00,
    ]
}

/// The 8K's vibrate command becomes a standard GIP rumble message.
fn rumble_for(report: &[u8], seq: u8) -> Option<[u8; 13]> {
    if report.get(1) != Some(&crate::protocol::g7pro::commands::cmd::VIBRATE) {
        return None;
    }
    let at = |i: usize| report.get(i).copied().unwrap_or(0);
    Some(rumble(at(4), at(5), at(6), at(7), seq))
}

/// GIP acknowledgement for a packet that asked for one (flag `0x10`).
fn ack_for(packet: &[u8]) -> Option<[u8; 13]> {
    if packet.len() < 4 || packet[1] & 0x10 == 0 {
        return None;
    }
    Some([
        0x01, 0x20, packet[2], 0x09, 0x00, packet[0], 0x20, packet[3], 0, 0, 0, 0, 0,
    ])
}

/// Raw USB through libusb. Works wherever libusb can claim the GIP
/// interface: Linux (after detaching `xpad`), macOS (no system driver binds
/// it) and Windows when the interface has WinUSB bound.
mod usb {
    use std::time::{Duration, Instant};

    use std::sync::OnceLock;

    use rusb::{Context, DeviceHandle, UsbContext};

    use super::*;
    use crate::Error;
    use crate::protocol::g7pro::commands::Geometry;
    use crate::registry::GAMESIR_VID;
    use crate::session::Link;
    use crate::transport::Bus;

    const IFACE: u8 = 0;
    const EP_OUT: u8 = 0x02;
    const EP_IN: u8 = 0x82;

    fn usb(e: rusb::Error) -> Error {
        Error::Usb(e.to_string())
    }

    /// Shared libusb context, or `None` when libusb can't start (no
    /// `/dev/bus/usb`, e.g. in a sandbox or container). rusb's
    /// `GlobalContext` panics in that case, which took down the device-scan
    /// thread along with the plain HID devices it was listing.
    fn ctx() -> Option<&'static Context> {
        static CTX: OnceLock<Option<Context>> = OnceLock::new();
        CTX.get_or_init(|| match Context::new() {
            Ok(c) => Some(c),
            Err(e) => {
                log::warn!("libusb unavailable, GIP devices won't be listed: {e}");
                None
            }
        })
        .as_ref()
    }

    fn path_of(d: &rusb::Device<Context>) -> String {
        format!("{PATH_PREFIX}{:03}:{:03}", d.bus_number(), d.address())
    }

    fn find(pids: &[u16]) -> Option<rusb::Device<Context>> {
        ctx()?.devices().ok()?.iter().find(|d| {
            d.device_descriptor()
                .is_ok_and(|dd| dd.vendor_id() == GAMESIR_VID && pids.contains(&dd.product_id()))
        })
    }

    /// GIP-mode GameSir devices, as entries for the device list.
    pub(super) fn enumerate() -> Vec<HidInfo> {
        let Some(Ok(list)) = ctx().map(|c| c.devices()) else {
            return vec![];
        };
        list.iter()
            .filter_map(|d| {
                let dd = d.device_descriptor().ok()?;
                (dd.vendor_id() == GAMESIR_VID && GIP_PIDS.contains(&dd.product_id())).then(|| {
                    HidInfo {
                        path: path_of(&d),
                        vid: dd.vendor_id(),
                        pid: dd.product_id(),
                        usage_page: 0,
                        usage: 0,
                        interface: IFACE as i32,
                        bus: Bus::Usb,
                        manufacturer: "GameSir".into(),
                        product: "GameSir-G7 Pro".into(),
                        serial: String::new(),
                    }
                })
            })
            .collect()
    }

    /// Reset a controller sitting in the HID fallback so it comes back in
    /// GIP mode. Returns once a GIP device shows up (or after `wait`).
    pub fn reset_hid_fallback(wait: Duration) -> Result<bool, Error> {
        let Some(dev) = find(&[HID_FALLBACK_PID]) else {
            return Ok(false);
        };
        let h = dev.open().map_err(usb)?;
        // Re-enumeration makes the reset report "not found"; that is fine.
        let _ = h.reset();
        drop(h);
        let end = Instant::now() + wait;
        while Instant::now() < end {
            if find(&GIP_PIDS).is_some() {
                return Ok(true);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(false)
    }

    pub struct UsbGipLink {
        handle: DeviceHandle<Context>,
        reattach: bool,
        seq: u8,
        path: String,
    }

    impl UsbGipLink {
        /// Open the first G7 Pro XBox in GIP mode, resetting it out of the
        /// HID fallback first if that is how it is connected.
        pub fn open() -> Result<UsbGipLink, Error> {
            if find(&GIP_PIDS).is_none() {
                reset_hid_fallback(Duration::from_secs(5))?;
            }
            let dev = find(&GIP_PIDS)
                .ok_or_else(|| Error::Usb("no G7 Pro XBox in app (GIP) mode found".into()))?;
            let path = path_of(&dev);
            let handle = dev.open().map_err(usb)?;
            // xpad may still be probing right after enumeration: retry.
            let mut reattach = false;
            let end = Instant::now() + Duration::from_secs(2);
            loop {
                let r = (|| {
                    if handle.kernel_driver_active(IFACE).unwrap_or(false) {
                        handle.detach_kernel_driver(IFACE)?;
                        reattach = true;
                    }
                    handle.claim_interface(IFACE)
                })();
                match r {
                    Ok(()) => break,
                    Err(rusb::Error::Busy) if Instant::now() < end => {
                        std::thread::sleep(Duration::from_millis(25))
                    }
                    Err(e) => {
                        // Don't leave the pad without its gamepad driver.
                        if reattach {
                            let _ = handle.attach_kernel_driver(IFACE);
                        }
                        return Err(usb(e));
                    }
                }
            }
            // On Linux the pad's driver is xpad. If none was bound (an earlier
            // run was killed before it could hand the pad back), still attach
            // it on close so the gamepad works again.
            let needs_power_on = !reattach;
            let reattach = reattach || cfg!(target_os = "linux");
            let mut link = UsbGipLink {
                handle,
                reattach,
                seq: 0,
                path,
            };
            if needs_power_on {
                // No system driver started the pad (macOS, Windows with
                // WinUSB): send the GIP power-on that `xpad` would have.
                let seq = link.next_seq();
                link.raw_write(&[0x05, 0x20, seq, 0x01, 0x00])?;
            }
            // Tell the firmware a configurator is attached before the dongle
            // gives up on GIP mode.
            link.write(&[0x0F, 0xF2, 0x01])?;
            log::info!("opened G7 Pro XBox over GIP at {}", link.path);
            Ok(link)
        }

        pub fn path(&self) -> &str {
            &self.path
        }

        fn next_seq(&mut self) -> u8 {
            self.seq = self.seq.wrapping_add(1).max(1);
            self.seq
        }

        fn raw_write(&self, pkt: &[u8]) -> Result<(), Error> {
            self.handle
                .write_interrupt(EP_OUT, pkt, Duration::from_millis(200))
                .map_err(usb)?;
            Ok(())
        }
    }

    impl Link for UsbGipLink {
        fn write(&mut self, report: &[u8]) -> Result<(), Error> {
            let seq = self.next_seq();
            // The 8K's vibrate command becomes a standard GIP rumble.
            if let Some(r) = rumble_for(report, seq) {
                return self.raw_write(&r);
            }
            self.raw_write(&encode_command(report, seq))
        }

        fn read(&mut self, buf: &mut [u8], timeout_ms: i32) -> Result<usize, Error> {
            let end = Instant::now() + Duration::from_millis(timeout_ms.max(1) as u64);
            let mut pkt = [0u8; 64];
            loop {
                let left = end.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    return Ok(0);
                }
                let n = match self.handle.read_interrupt(EP_IN, &mut pkt, left) {
                    Ok(n) => n,
                    Err(rusb::Error::Timeout) => return Ok(0),
                    Err(e) => return Err(usb(e)),
                };
                let p = &pkt[..n];
                if let Some(a) = ack_for(p) {
                    let _ = self
                        .handle
                        .write_interrupt(EP_OUT, &a, Duration::from_millis(100));
                }
                if let Some(r) = decode_reply(p) {
                    let n = r.len().min(buf.len());
                    buf[..n].copy_from_slice(&r[..n]);
                    return Ok(n);
                }
            }
        }

        fn geometry(&self) -> Geometry {
            Geometry::GIP
        }
    }

    impl Drop for UsbGipLink {
        fn drop(&mut self) {
            let _ = self.handle.release_interface(IFACE);
            if self.reattach {
                let _ = self.handle.attach_kernel_driver(IFACE);
            }
        }
    }
}

pub use usb::{UsbGipLink, reset_hid_fallback};

#[cfg(windows)]
#[path = "gip_winrt.rs"]
mod winrt;

/// GIP-mode G7 Pro XBox controllers, as entries for the device list.
pub fn enumerate() -> Vec<HidInfo> {
    #[allow(unused_mut)]
    let mut out = usb::enumerate();
    #[cfg(windows)]
    for d in winrt::enumerate() {
        if !out.iter().any(|u| u.pid == d.pid) {
            out.push(d);
        }
    }
    out
}

/// Configuration link to a G7 Pro XBox, whichever way this OS reaches it.
pub struct GipLink {
    inner: Box<dyn crate::session::Link>,
    path: String,
}

impl GipLink {
    /// Open the first G7 Pro XBox.
    ///
    /// Linux and macOS use raw USB. Windows goes through the system GIP
    /// stack (`Windows.Gaming.Input.Custom`), as GameSir Nexus does, and
    /// falls back to raw USB for a pad bound to WinUSB.
    pub fn open() -> Result<GipLink, crate::Error> {
        #[cfg(windows)]
        {
            match winrt::WinGipLink::open(std::time::Duration::from_secs(3)) {
                Ok(Some(l)) => {
                    let path = l.path().to_string();
                    return Ok(GipLink {
                        inner: Box::new(l),
                        path,
                    });
                }
                Ok(None) => log::info!("Windows GIP stack reported no G7 Pro XBox"),
                Err(e) if usb::enumerate().is_empty() => return Err(e),
                Err(e) => log::warn!("{e}; trying raw USB"),
            }
        }
        let l = UsbGipLink::open()?;
        let path = l.path().to_string();
        Ok(GipLink {
            inner: Box::new(l),
            path,
        })
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

impl crate::session::Link for GipLink {
    fn write(&mut self, report: &[u8]) -> Result<(), crate::Error> {
        self.inner.write(report)
    }
    fn read(&mut self, buf: &mut [u8], timeout_ms: i32) -> Result<usize, crate::Error> {
        self.inner.read(buf, timeout_ms)
    }
    fn config_report_ids(&self) -> Option<Vec<u8>> {
        self.inner.config_report_ids()
    }
    fn geometry(&self) -> crate::protocol::g7pro::commands::Geometry {
        self.inner.geometry()
    }
    fn has_config(&self) -> bool {
        self.inner.has_config()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_framing() {
        let p = encode_command(&[0x0F, 0x04, 0x01, 0x00, 0x37, 0x37], 9);
        assert_eq!(
            &p[..10],
            &[0x0F, 0x00, 9, 60, 0x04, 0x01, 0x00, 0x37, 0x37, 0]
        );
        assert_eq!(p.len(), 64);
    }

    #[test]
    fn reply_framing() {
        let mut pkt = vec![0x10, 0x00, 0x20, 0x3C, 0x0C, 0x03];
        pkt.resize(64, 0);
        let r = decode_reply(&pkt).unwrap();
        assert_eq!(&r[..3], &[0x10, 0x0C, 0x03]);
        assert_eq!(r.len(), 61);
        let mut st = pkt.clone();
        st[4] = 0xE0;
        st[3] = 60;
        st[4 + 29] = 100; // battery
        let r = decode_reply(&st).unwrap();
        let s = crate::protocol::g7pro::State::parse_xbox(&r).unwrap();
        assert_eq!(s.battery, 100);
        assert!(decode_reply(&[0x20, 0x00, 0x01, 0x0e]).is_none());
    }

    #[test]
    fn ack() {
        assert!(ack_for(&[0x02, 0x20, 0x01, 0x1c]).is_none());
        let a = ack_for(&[0x02, 0x30, 0x05, 0x1c]).unwrap();
        assert_eq!(&a[..8], &[0x01, 0x20, 0x05, 0x09, 0x00, 0x02, 0x20, 0x1c]);
    }
}

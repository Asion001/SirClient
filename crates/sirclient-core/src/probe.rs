//! Raw HID inspection, used to find out what a controller exposes over a
//! given connection (for example Bluetooth, which GameSir Connect never
//! talks to).

use std::time::{Duration, Instant};

use crate::Error;
use crate::protocol::g7pro::commands::{self, Reply};
use crate::session::Link;
use crate::transport::{REPORT_LEN, pad_report};

#[derive(Clone, Debug, serde::Serialize)]
pub struct Captured {
    pub at_ms: u64,
    pub outgoing: bool,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct ProbeResult {
    pub log: Vec<Captured>,
    /// Firmware version, if the collection answered the G7 handshake.
    pub firmware: Option<String>,
}

impl ProbeResult {
    pub fn answered(&self) -> bool {
        self.firmware.is_some()
    }
}

/// Listen to input reports without sending anything.
pub fn listen(link: &mut dyn Link, duration: Duration) -> Result<ProbeResult, Error> {
    run(link, duration, false)
}

/// Send the G7 heartbeat and firmware-version request, then listen.
/// Only use on collections that may be a GameSir configuration interface:
/// it writes output report 0x0F.
pub fn handshake(link: &mut dyn Link, duration: Duration) -> Result<ProbeResult, Error> {
    run(link, duration, true)
}

fn run(link: &mut dyn Link, duration: Duration, send: bool) -> Result<ProbeResult, Error> {
    let start = Instant::now();
    let mut res = ProbeResult::default();
    if send {
        for c in [commands::heartbeat(false), commands::read_firmware()] {
            let r = pad_report(&c);
            link.write(&r)?;
            res.log.push(Captured {
                at_ms: 0,
                outgoing: true,
                bytes: c,
            });
        }
    }
    let mut buf = [0u8; REPORT_LEN];
    let mut last_state: Option<Vec<u8>> = None;
    while start.elapsed() < duration && res.log.len() < 200 {
        let n = link.read(&mut buf, 20)?;
        if n == 0 {
            continue;
        }
        let bytes = buf[..n].to_vec();
        // Collapse identical repeated reports (idle controllers stream them).
        if last_state.as_ref() == Some(&bytes) {
            continue;
        }
        last_state = Some(bytes.clone());
        if let Some(Reply::Firmware { controller, .. }) = commands::parse_reply(&bytes) {
            res.firmware = Some(controller);
        }
        res.log.push(Captured {
            at_ms: start.elapsed().as_millis() as u64,
            outgoing: false,
            bytes,
        });
    }
    Ok(res)
}

pub fn hex(bytes: &[u8], max: usize) -> String {
    let mut s: Vec<String> = bytes.iter().take(max).map(|b| format!("{b:02X}")).collect();
    if bytes.len() > max {
        s.push("…".into());
    }
    s.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockG7;

    #[test]
    fn handshake_against_mock() {
        let mut m = MockG7::new();
        let r = handshake(&mut m, Duration::from_millis(100)).unwrap();
        assert_eq!(r.firmware.as_deref(), Some("2.1.9"));
    }
}

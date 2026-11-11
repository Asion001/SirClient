//! G7 Pro 8K wire format.
//!
//! Host-to-device commands are output reports with report ID `0x0F`, padded
//! to 64 bytes. Byte 1 is the command. Device replies come back on the same
//! collection with the reply code in byte 1. Live controller state arrives
//! as input report `0x12`.

/// Report ID of every host command.
pub const REPORT_ID: u8 = 0x0F;
/// Input report carrying live controller state.
pub const STATE_REPORT_ID: u8 = 0x12;
/// Payload bytes per read/write chunk (64 - 6 header bytes).
pub const CHUNK: usize = 58;

/// Size of a button/stick/motion profile.
pub const PROFILE_LEN: usize = 1070;
/// Size of the lighting profile.
pub const LIGHT_PROFILE_LEN: usize = 165;

/// Sizes that differ between the HID G7 Pro 8K and the Xbox edition
/// (which tunnels the same commands through GIP message `0x0F`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Geometry {
    pub profile_len: usize,
    pub light_len: usize,
    /// Payload bytes per read/write chunk.
    pub chunk: usize,
}

impl Geometry {
    /// G7 Pro 8K over HID.
    pub const HID: Geometry = Geometry {
        profile_len: PROFILE_LEN,
        light_len: LIGHT_PROFILE_LEN,
        chunk: CHUNK,
    };
    /// G7 Pro XBox over GIP: 60-byte payloads, 5 header bytes.
    pub const GIP: Geometry = Geometry {
        profile_len: 480,
        light_len: 511,
        chunk: 55,
    };

    pub fn slot_len(self, slot: Slot) -> usize {
        match slot {
            Slot::Light => self.light_len,
            _ => self.profile_len,
        }
    }
}

/// Profile slots understood by the firmware.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Slot {
    /// Profiles 1..=4.
    Profile(u8),
    /// Layer active while the Shift button is held.
    Shift,
    /// Lighting presets and global light settings.
    Light,
}

impl Slot {
    pub const ALL: [Slot; 6] = [
        Slot::Profile(1),
        Slot::Profile(2),
        Slot::Profile(3),
        Slot::Profile(4),
        Slot::Shift,
        Slot::Light,
    ];

    pub fn number(self) -> u8 {
        match self {
            Slot::Profile(n) => n,
            Slot::Shift => 5,
            Slot::Light => 32,
        }
    }

    pub fn from_number(n: u8) -> Option<Slot> {
        match n {
            1..=4 => Some(Slot::Profile(n)),
            5 => Some(Slot::Shift),
            32 => Some(Slot::Light),
            _ => None,
        }
    }

    pub fn label(self) -> String {
        match self {
            Slot::Profile(n) => format!("Profile {n}"),
            Slot::Shift => "Shift".into(),
            Slot::Light => "Lighting".into(),
        }
    }
}

/// Command codes (byte 1 of a host report).
pub mod cmd {
    pub const WRITE_PROFILE: u8 = 0x03;
    pub const READ_PROFILE: u8 = 0x04;
    pub const SWITCH_PROFILE: u8 = 0x07;
    pub const READ_FIRMWARE: u8 = 0x09;
    pub const READ_CURRENT_PROFILE: u8 = 0x0B;
    pub const LIGHT_PREVIEW: u8 = 0x0D;
    pub const REFRESH_PROFILE: u8 = 0x10;
    pub const MACRO_RECORD: u8 = 0x16;
    /// Puts the controller into firmware-update mode. Not exposed.
    pub const ENTER_DFU: u8 = 0x17;
    pub const VIBRATE: u8 = 0x20;
    pub const SET_STICK_CENTER: u8 = 0xE2;
    pub const HEARTBEAT: u8 = 0xF2;
    pub const CALIBRATE: u8 = 0xFE;
}

/// Reply codes (byte 1 of a device report).
pub mod reply {
    pub const READ_PROFILE: u8 = 0x05;
    pub const ACK: u8 = 0x06;
    pub const FIRMWARE: u8 = 0x0A;
    pub const CURRENT_PROFILE: u8 = 0x0C;
    pub const RGB: u8 = 0x0E;
    pub const PROFILE_CHANGED: u8 = 0x0F;
    pub const REFRESH_PROFILE: u8 = 0x11;
    pub const MACRO_STATUS: u8 = 0x17;
    pub const RGB_ALT: u8 = 0x18;
    pub const USB_MODE_CHANGED: u8 = 0xE0;
    pub const STICK_CENTER: u8 = 0xE3;
    pub const HEARTBEAT: u8 = 0xF2;
}

/// `ProfileChanged` uses this slot number to say the active profile changed.
pub const CURRENT_PROFILE_MARKER: u8 = 48;

fn header(cmd: u8) -> Vec<u8> {
    vec![REPORT_ID, cmd]
}

pub fn heartbeat(test_mode: bool) -> Vec<u8> {
    vec![REPORT_ID, cmd::HEARTBEAT, test_mode as u8]
}

pub fn read_firmware() -> Vec<u8> {
    header(cmd::READ_FIRMWARE)
}

pub fn read_current_profile() -> Vec<u8> {
    header(cmd::READ_CURRENT_PROFILE)
}

pub fn switch_profile(slot: Slot) -> Vec<u8> {
    vec![REPORT_ID, cmd::SWITCH_PROFILE, slot.number()]
}

/// Commands that read `len` bytes of a slot starting at `offset`.
pub fn read_range(command: u8, slot: Slot, offset: usize, len: usize) -> Vec<Vec<u8>> {
    read_range_in(Geometry::HID, command, slot, offset, len)
}

/// [`read_range`] for a given device geometry.
pub fn read_range_in(
    geo: Geometry,
    command: u8,
    slot: Slot,
    offset: usize,
    len: usize,
) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < len {
        let off = offset + pos;
        let n = (len - pos).min(geo.chunk);
        out.push(vec![
            REPORT_ID,
            command,
            slot.number(),
            (off >> 8) as u8,
            off as u8,
            n as u8,
        ]);
        pos += n;
    }
    out
}

/// Commands that read a whole slot.
pub fn read_slot(slot: Slot) -> Vec<Vec<u8>> {
    read_slot_in(Geometry::HID, slot)
}

/// [`read_slot`] for a given device geometry.
pub fn read_slot_in(geo: Geometry, slot: Slot) -> Vec<Vec<u8>> {
    read_range_in(geo, cmd::READ_PROFILE, slot, 0, geo.slot_len(slot))
}

/// Commands that write `data` into a slot starting at `offset`.
pub fn write_range(slot: Slot, offset: usize, data: &[u8]) -> Vec<Vec<u8>> {
    write_range_in(Geometry::HID, slot, offset, data)
}

/// [`write_range`] for a given device geometry.
pub fn write_range_in(geo: Geometry, slot: Slot, offset: usize, data: &[u8]) -> Vec<Vec<u8>> {
    data.chunks(geo.chunk)
        .enumerate()
        .map(|(i, chunk)| {
            let off = offset + i * geo.chunk;
            let mut p = vec![
                REPORT_ID,
                cmd::WRITE_PROFILE,
                slot.number(),
                (off >> 8) as u8,
                off as u8,
                chunk.len() as u8,
            ];
            p.extend_from_slice(chunk);
            p
        })
        .collect()
}

/// Run both rumble motors. Values are 0..=255.
pub fn vibrate(left: u8, right: u8) -> Vec<u8> {
    vec![REPORT_ID, cmd::VIBRATE, 0x66, 0x55, left, right]
}

/// [`vibrate`] plus the trigger motors (bytes 6 and 7). Only the XBox
/// edition (through [`crate::gip`], as a GIP rumble) drives those; leave
/// them 0 for the 8K.
pub fn vibrate_all(left: u8, right: u8, lt: u8, rt: u8) -> Vec<u8> {
    let mut v = vibrate(left, right);
    if lt != 0 || rt != 0 {
        v.extend_from_slice(&[lt, rt]);
    }
    v
}

/// Preview a lighting animation frame on the controller.
pub fn light_preview(playing: bool, frame: u8) -> Vec<u8> {
    vec![REPORT_ID, cmd::LIGHT_PREVIEW, playing as u8, frame]
}

/// Ask for the RGB state (answered inside the state report).
pub fn read_rgb() -> Vec<u8> {
    header(cmd::LIGHT_PREVIEW)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalibrationPart {
    Sticks,
    Triggers,
    SticksAndTriggers,
    Motion,
}

/// Calibrate the given parts: `kind` 0 starts, 1 finishes (saves) and
/// 2 exits without saving.
/// The mode the firmware is in comes back in the state report.
pub fn calibrate(kind: u8, part: CalibrationPart) -> Vec<u8> {
    let mask = match part {
        CalibrationPart::Sticks => 0b0011,
        CalibrationPart::Triggers => 0b1100,
        CalibrationPart::SticksAndTriggers => 0b1111,
        CalibrationPart::Motion => 0b1_0000,
    };
    vec![REPORT_ID, cmd::CALIBRATE, kind, mask]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StickSide {
    Left = 0,
    Right = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nudge {
    Left = 0,
    Right = 1,
    Up = 2,
    Down = 3,
}

/// Nudge a stick's resting centre by `amount` steps.
pub fn set_stick_center(side: StickSide, dir: Nudge, amount: u16) -> Vec<u8> {
    let mut p = vec![0u8; 64];
    p[0] = REPORT_ID;
    p[1] = cmd::SET_STICK_CENTER;
    p[2] = side as u8;
    p[3] = dir as u8;
    p[4] = (amount >> 8) as u8;
    p[5] = amount as u8;
    p
}

/// Start/stop on-controller macro recording for a key code.
pub fn macro_record(key_code: u8, state: u8) -> Vec<u8> {
    vec![REPORT_ID, cmd::MACRO_RECORD, key_code, state]
}

/// Decoded device reply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reply {
    Ack,
    /// Device is busy; the command must be resent.
    Busy,
    ProfileData {
        slot: u8,
        offset: usize,
        data: Vec<u8>,
    },
    RefreshData {
        slot: u8,
        offset: usize,
        data: Vec<u8>,
    },
    Firmware {
        controller: String,
        dongle: String,
    },
    CurrentProfile(u8),
    ProfileChanged {
        slot: u8,
        offset: usize,
        len: usize,
    },
    StickCenter {
        ok: bool,
    },
    UsbModeChanged,
    Heartbeat,
    Rgb,
    MacroStatus,
    Other(u8),
}

impl Reply {
    /// Reply code that acknowledges `command`, if the command expects one.
    pub fn expected_for(command: u8) -> Option<u8> {
        Some(match command {
            cmd::WRITE_PROFILE | cmd::SWITCH_PROFILE => reply::ACK,
            cmd::READ_PROFILE => reply::READ_PROFILE,
            cmd::READ_FIRMWARE => reply::FIRMWARE,
            cmd::READ_CURRENT_PROFILE => reply::CURRENT_PROFILE,
            cmd::REFRESH_PROFILE => reply::REFRESH_PROFILE,
            cmd::SET_STICK_CENTER => reply::STICK_CENTER,
            _ => return None,
        })
    }

    pub fn code(&self) -> u8 {
        match self {
            Reply::Ack | Reply::Busy => reply::ACK,
            Reply::ProfileData { .. } => reply::READ_PROFILE,
            Reply::RefreshData { .. } => reply::REFRESH_PROFILE,
            Reply::Firmware { .. } => reply::FIRMWARE,
            Reply::CurrentProfile(_) => reply::CURRENT_PROFILE,
            Reply::ProfileChanged { .. } => reply::PROFILE_CHANGED,
            Reply::StickCenter { .. } => reply::STICK_CENTER,
            Reply::UsbModeChanged => reply::USB_MODE_CHANGED,
            Reply::Heartbeat => reply::HEARTBEAT,
            Reply::Rgb => reply::RGB,
            Reply::MacroStatus => reply::MACRO_STATUS,
            Reply::Other(c) => *c,
        }
    }
}

/// Firmware versions are ASCII digits; the app shows them dot-separated.
fn version(bytes: &[u8]) -> String {
    let digits: Vec<String> = bytes
        .iter()
        .filter(|b| b.is_ascii_graphic())
        .map(|b| (*b as char).to_string())
        .collect();
    digits.join(".")
}

fn data_reply(r: &[u8]) -> (u8, usize, Vec<u8>) {
    let slot = r[2];
    let offset = ((r[3] as usize) << 8) | r[4] as usize;
    let len = (r[5] as usize).min(r.len().saturating_sub(6));
    (slot, offset, r[6..6 + len].to_vec())
}

/// Decode a non-state input report. Returns `None` for state reports and
/// reports too short to be replies.
pub fn parse_reply(r: &[u8]) -> Option<Reply> {
    if r.len() < 2 || r[0] == STATE_REPORT_ID {
        return None;
    }
    let at = |i: usize| r.get(i).copied().unwrap_or(0);
    Some(match r[1] {
        reply::ACK if at(2) == 1 => Reply::Busy,
        reply::ACK => Reply::Ack,
        reply::READ_PROFILE if r.len() >= 6 => {
            let (slot, offset, data) = data_reply(r);
            Reply::ProfileData { slot, offset, data }
        }
        reply::REFRESH_PROFILE if r.len() >= 6 => {
            let (slot, offset, data) = data_reply(r);
            Reply::RefreshData { slot, offset, data }
        }
        // The Xbox edition answers with UTF-16 digits: "0249" "0153".
        reply::FIRMWARE if at(3) == 0 && at(2).is_ascii_digit() => {
            let digits: Vec<u8> = r[2..].chunks(2).map(|c| c[0]).take(8).collect();
            let trim = |d: &[u8]| {
                let start = d.iter().position(|&b| b != b'0').unwrap_or(d.len());
                version(&d[start..])
            };
            Reply::Firmware {
                controller: trim(&digits[..digits.len().min(4)]),
                dongle: trim(digits.get(4..).unwrap_or(&[])),
            }
        }
        reply::FIRMWARE => Reply::Firmware {
            controller: version(r.get(4..9).unwrap_or(&[])),
            dongle: version(r.get(12..17).unwrap_or(&[])),
        },
        reply::CURRENT_PROFILE => Reply::CurrentProfile(at(2)),
        reply::PROFILE_CHANGED => Reply::ProfileChanged {
            slot: at(2),
            offset: ((at(3) as usize) << 8) | at(4) as usize,
            len: ((at(5) as usize) << 8) | at(6) as usize,
        },
        reply::STICK_CENTER => Reply::StickCenter { ok: at(2) == 0 },
        reply::USB_MODE_CHANGED => Reply::UsbModeChanged,
        reply::HEARTBEAT => Reply::Heartbeat,
        reply::RGB | reply::RGB_ALT => Reply::Rgb,
        reply::MACRO_STATUS => Reply::MacroStatus,
        other => Reply::Other(other),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_slot_chunks_cover_profile() {
        let cmds = read_slot(Slot::Profile(2));
        assert_eq!(cmds.len(), PROFILE_LEN.div_ceil(CHUNK));
        let total: usize = cmds.iter().map(|c| c[5] as usize).sum();
        assert_eq!(total, PROFILE_LEN);
        let last = cmds.last().unwrap();
        let off = ((last[3] as usize) << 8) | last[4] as usize;
        assert_eq!(off + last[5] as usize, PROFILE_LEN);
        assert_eq!(&cmds[0][..3], &[0x0F, 0x04, 2]);
    }

    #[test]
    fn write_range_header() {
        let data = vec![7u8; 70];
        let cmds = write_range(Slot::Light, 0x100, &data);
        assert_eq!(cmds.len(), 2);
        assert_eq!(&cmds[0][..6], &[0x0F, 0x03, 32, 0x01, 0x00, 58]);
        assert_eq!(&cmds[1][..6], &[0x0F, 0x03, 32, 0x01, 58, 12]);
    }

    #[test]
    fn parse_firmware() {
        let mut r = vec![0u8; 64];
        r[0] = 0x0F;
        r[1] = reply::FIRMWARE;
        r[4..9].copy_from_slice(b"219\0\0");
        r[12..17].copy_from_slice(b"134\0\0");
        assert_eq!(
            parse_reply(&r),
            Some(Reply::Firmware {
                controller: "2.1.9".into(),
                dongle: "1.3.4".into()
            })
        );
    }

    #[test]
    fn parse_xbox_firmware() {
        // GIP reply payload behind report byte 0x10, as GipLink delivers it.
        let mut r = vec![0x10, reply::FIRMWARE];
        r.extend("02490153".encode_utf16().flat_map(|c| c.to_le_bytes()));
        r.resize(61, 0);
        assert_eq!(
            parse_reply(&r),
            Some(Reply::Firmware {
                controller: "2.4.9".into(),
                dongle: "1.5.3".into()
            })
        );
    }

    #[test]
    fn gip_chunks() {
        let cmds = read_slot_in(Geometry::GIP, Slot::Profile(1));
        assert_eq!(cmds.len(), 9);
        assert_eq!(&cmds[0][..6], &[0x0F, 0x04, 1, 0, 0, 55]);
        let last = cmds.last().unwrap();
        assert_eq!(&last[..6], &[0x0F, 0x04, 1, 0x01, 0xB8, 40]);
        let w = write_range_in(Geometry::GIP, Slot::Light, 0, &[1u8; 60]);
        assert_eq!(w.len(), 2);
        assert_eq!(w[1][5], 5);
    }

    #[test]
    fn busy_ack() {
        assert_eq!(parse_reply(&[0x0F, 0x06, 1]), Some(Reply::Busy));
        assert_eq!(parse_reply(&[0x0F, 0x06, 0]), Some(Reply::Ack));
    }
}

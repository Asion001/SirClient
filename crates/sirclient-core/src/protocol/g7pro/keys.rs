//! Key codes used by G7 Pro 8K remaps, macros and motion triggers.
//!
//! Names follow GameSir Connect: `CRO`/`CIR`/`SQU`/`TRI` are A/B/X/Y,
//! `FL1`/`FR1`/`FL2`/`FR2` are the back buttons L4/R4/L5/R5, `KB_*` are
//! keyboard keys and `MS_*` mouse buttons.

include!("keys_table.rs");

/// Code that means "not connected / no key".
pub const NC: u8 = 255;

pub fn name(code: u8) -> Option<&'static str> {
    KEY_CODES.iter().find(|(_, c)| *c == code).map(|(n, _)| *n)
}

pub fn code(name: &str) -> Option<u8> {
    KEY_CODES.iter().find(|(n, _)| *n == name).map(|(_, c)| *c)
}

/// Xbox-style label the G7 Pro prints on its buttons.
pub fn display_name(name: &str) -> String {
    let s = match name {
        "CRO" => "A",
        "CIR" => "B",
        "SQU" => "X",
        "TRI" => "Y",
        "Up" => "D-pad Up",
        "Under" => "D-pad Down",
        "Left" => "D-pad Left",
        "Right" => "D-pad Right",
        "L1" => "LB",
        "R1" => "RB",
        "L2" => "LT",
        "R2" => "RT",
        "L3" => "LS click",
        "R3" => "RS click",
        "FL1" => "L4",
        "FR1" => "R4",
        "FL2" => "L5",
        "FR2" => "R5",
        "PS" => "Home",
        "Sel" => "View",
        "Sta" => "Menu",
        "Camera" => "Screenshot",
        "NC" => "Disabled",
        other => {
            if let Some(k) = other.strip_prefix("KB_") {
                return format!("Key {k}");
            }
            if let Some(m) = other.strip_prefix("MS_") {
                return format!("Mouse {}", m.replace('_', " ").to_lowercase());
            }
            other
        }
    };
    s.to_string()
}

pub fn display(code: u8) -> String {
    match name(code) {
        Some(n) => display_name(n),
        None if code == 0 => "—".into(),
        None => format!("0x{code:02x}"),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Gamepad,
    Keyboard,
    Mouse,
    Special,
}

pub fn group(code: u8) -> Group {
    match code {
        50..=155 => Group::Keyboard,
        200..=206 => Group::Mouse,
        230 | 231 | 255 => Group::Special,
        _ => Group::Gamepad,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn lookups() {
        assert_eq!(super::code("CRO"), Some(9));
        assert_eq!(super::name(255), Some("NC"));
        assert_eq!(super::display(9), "A");
    }
}

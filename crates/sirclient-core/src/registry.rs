//! Static table of GameSir devices known to GameSir Connect 1.14.5.
//!
//! Every controller exposes a vendor-defined HID collection used for
//! configuration. The table maps (VID, PID, usage page) to a model and the
//! protocol family that speaks to it. Only some families are implemented;
//! the rest are still detected so the UI can tell the user what it found.

/// GameSir USB vendor ID.
pub const GAMESIR_VID: u16 = 0x3537;

/// Protocol family. Names follow the controller model each family was first
/// seen on in GameSir Connect.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Protocol {
    /// G7 Pro 8K / G7 Pro CE (implemented).
    G7Pro,
    /// G7 Pro XBox: the G7 Pro command set tunnelled through Xbox GIP over
    /// raw USB (see [`crate::gip`]).
    G7ProXbox,
    /// The G7 Pro XBox over Bluetooth: a standard Xbox BT gamepad, live
    /// input and rumble only (see [`crate::bt`]).
    XboxBluetooth,
    /// Cyclone 2.
    C2,
    /// K2 8K.
    K2,
    /// Tarantula CE.
    T3CE,
    /// Tegenaria Lite.
    T3Lite,
    /// Nova Pro (Windows app drives it through a vendor DLL).
    T4NPro,
    /// Tarantula Pro (Windows app drives it through a vendor DLL).
    T3Pro,
    /// Nova 2 Lite (Windows app drives it through a vendor DLL).
    T4N2,
    /// Nova Vision (Windows app drives it through a vendor DLL).
    N2S,
    /// T3XS (Windows app drives it through a vendor DLL).
    T3XS,
}

impl Protocol {
    /// Whether SirClient can configure devices of this family.
    pub fn is_implemented(self) -> bool {
        matches!(
            self,
            Protocol::G7Pro | Protocol::XboxBluetooth | Protocol::G7ProXbox
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Connection {
    /// USB cable.
    Wired,
    /// 2.4 GHz dongle.
    Wireless,
    /// Bluetooth (only live input and rumble on the XBox edition).
    Bluetooth,
}

#[derive(Clone, Copy, Debug)]
pub struct ModeInfo {
    pub pid: u16,
    pub connection: Connection,
    /// HID usage page of the configuration collection.
    pub usage_page: u16,
    /// Colour/edition name used by the official app.
    pub variant: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub struct ModelInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub protocol: Protocol,
    /// The official Windows app talks to this model through a closed DLL
    /// instead of plain HID, so its protocol still needs separate work.
    pub vendor_dll: bool,
    pub modes: &'static [ModeInfo],
}

pub static MODELS: &[ModelInfo] = &[
    // Not in GameSir Connect: configured by GameSir Nexus over Xbox GIP.
    ModelInfo {
        id: "G7ProXbox",
        name: "G7 Pro XBox",
        protocol: Protocol::G7ProXbox,
        vendor_dll: false,
        modes: &[
            ModeInfo {
                pid: 0x10BA,
                connection: Connection::Wired,
                usage_page: 0,
                variant: "App mode (GIP)",
            },
            ModeInfo {
                pid: 0x10BB,
                connection: Connection::Wireless,
                usage_page: 0,
                variant: "App mode (GIP)",
            },
            // Plain HID fallback the dongle drops to on hosts that don't
            // keep it in app mode. Its config collection does not answer;
            // SirClient resets the port to get back to GIP mode.
            ModeInfo {
                pid: 0x1022,
                connection: Connection::Wireless,
                usage_page: 0xfff0,
                variant: "HID fallback",
            },
        ],
    },
    ModelInfo {
        id: "T4N2",
        name: "Nova 2 Lite",
        protocol: Protocol::T4N2,
        vendor_dll: true,
        modes: &[
            ModeInfo {
                pid: 0x100f,
                connection: Connection::Wired,
                usage_page: 0xffff,
                variant: "White",
            },
            ModeInfo {
                pid: 0x1098,
                connection: Connection::Wireless,
                usage_page: 0xffff,
                variant: "T4N2",
            },
        ],
    },
    ModelInfo {
        id: "N2S",
        name: "NOVA VISION",
        protocol: Protocol::N2S,
        vendor_dll: true,
        modes: &[
            ModeInfo {
                pid: 0x1141,
                connection: Connection::Wired,
                usage_page: 0xffff,
                variant: "White",
            },
            ModeInfo {
                pid: 0x10bd,
                connection: Connection::Wireless,
                usage_page: 0xffff,
                variant: "N2S",
            },
        ],
    },
    ModelInfo {
        id: "T3CE",
        name: "Tarantula CE",
        protocol: Protocol::T3CE,
        vendor_dll: false,
        modes: &[
            ModeInfo {
                pid: 0x103d,
                connection: Connection::Wired,
                usage_page: 0xfff0,
                variant: "Default",
            },
            ModeInfo {
                pid: 0x090d,
                connection: Connection::Wired,
                usage_page: 0xfff0,
                variant: "Default",
            },
        ],
    },
    ModelInfo {
        id: "G7ProCE",
        name: "G7 Pro 8K",
        protocol: Protocol::G7Pro,
        vendor_dll: false,
        modes: &[
            ModeInfo {
                pid: 0x10b7,
                connection: Connection::Wired,
                usage_page: 0xfff0,
                variant: "Gold",
            },
            ModeInfo {
                pid: 0x10b9,
                connection: Connection::Wireless,
                usage_page: 0xfff0,
                variant: "Gold",
            },
            ModeInfo {
                pid: 0x10cd,
                connection: Connection::Wired,
                usage_page: 0xfff0,
                variant: "Gold",
            },
            ModeInfo {
                pid: 0x10ce,
                connection: Connection::Wireless,
                usage_page: 0xfff0,
                variant: "Gold",
            },
            ModeInfo {
                pid: 0x10c5,
                connection: Connection::Wired,
                usage_page: 0xfff0,
                variant: "Aimlabs",
            },
            ModeInfo {
                pid: 0x10c6,
                connection: Connection::Wireless,
                usage_page: 0xfff0,
                variant: "Aimlabs",
            },
            ModeInfo {
                pid: 0x10fd,
                connection: Connection::Wired,
                usage_page: 0xfff0,
                variant: "GoldenApple",
            },
            ModeInfo {
                pid: 0x10fe,
                connection: Connection::Wireless,
                usage_page: 0xfff0,
                variant: "GoldenApple",
            },
            ModeInfo {
                pid: 0x10c7,
                connection: Connection::Wired,
                usage_page: 0xfff0,
                variant: "Nioh",
            },
            ModeInfo {
                pid: 0x10c8,
                connection: Connection::Wireless,
                usage_page: 0xfff0,
                variant: "Nioh",
            },
            ModeInfo {
                pid: 0x10c9,
                connection: Connection::Wired,
                usage_page: 0xfff0,
                variant: "NBA2K",
            },
            ModeInfo {
                pid: 0x10ca,
                connection: Connection::Wireless,
                usage_page: 0xfff0,
                variant: "NBA2K",
            },
            ModeInfo {
                pid: 0x10cb,
                connection: Connection::Wired,
                usage_page: 0xfff0,
                variant: "Gold",
            },
            ModeInfo {
                pid: 0x10cc,
                connection: Connection::Wireless,
                usage_page: 0xfff0,
                variant: "Gold",
            },
            ModeInfo {
                pid: 0x1032,
                connection: Connection::Wired,
                usage_page: 0xfff0,
                variant: "Crown",
            },
            ModeInfo {
                pid: 0x1033,
                connection: Connection::Wireless,
                usage_page: 0xfff0,
                variant: "Crown",
            },
            ModeInfo {
                pid: 0x1034,
                connection: Connection::Wired,
                usage_page: 0xfff0,
                variant: "Gold",
            },
            ModeInfo {
                pid: 0x1035,
                connection: Connection::Wireless,
                usage_page: 0xfff0,
                variant: "Gold",
            },
            ModeInfo {
                pid: 0x10fb,
                connection: Connection::Wired,
                usage_page: 0xfff0,
                variant: "GradientBlue",
            },
            ModeInfo {
                pid: 0x10fc,
                connection: Connection::Wireless,
                usage_page: 0xfff0,
                variant: "GradientBlue",
            },
        ],
    },
    ModelInfo {
        id: "K2_8K",
        name: "K2 8K",
        protocol: Protocol::K2,
        vendor_dll: false,
        modes: &[
            ModeInfo {
                pid: 0x108e,
                connection: Connection::Wired,
                usage_page: 0xfff0,
                variant: "Black",
            },
            ModeInfo {
                pid: 0x109f,
                connection: Connection::Wireless,
                usage_page: 0xfff0,
                variant: "Black",
            },
        ],
    },
    ModelInfo {
        id: "C2",
        name: "CYCLONE 2",
        protocol: Protocol::C2,
        vendor_dll: false,
        modes: &[
            ModeInfo {
                pid: 0x101d,
                connection: Connection::Wired,
                usage_page: 0xfff0,
                variant: "White",
            },
            ModeInfo {
                pid: 0x102a,
                connection: Connection::Wireless,
                usage_page: 0xfff0,
                variant: "White",
            },
            ModeInfo {
                pid: 0x1053,
                connection: Connection::Wired,
                usage_page: 0xfff0,
                variant: "White",
            },
            ModeInfo {
                pid: 0x100b,
                connection: Connection::Wireless,
                usage_page: 0xfff0,
                variant: "White",
            },
        ],
    },
    ModelInfo {
        id: "T3Pro",
        name: "Tarantula Pro",
        protocol: Protocol::T3Pro,
        vendor_dll: true,
        modes: &[
            ModeInfo {
                pid: 0x1050,
                connection: Connection::Wired,
                usage_page: 0xffff,
                variant: "Black",
            },
            ModeInfo {
                pid: 0x103e,
                connection: Connection::Wireless,
                usage_page: 0xfff0,
                variant: "Black",
            },
        ],
    },
    ModelInfo {
        id: "T4NPro",
        name: "NovaPro",
        protocol: Protocol::T4NPro,
        vendor_dll: true,
        modes: &[
            ModeInfo {
                pid: 0x100e,
                connection: Connection::Wired,
                usage_page: 0xffff,
                variant: "Black",
            },
            ModeInfo {
                pid: 0x103e,
                connection: Connection::Wireless,
                usage_page: 0xfff0,
                variant: "Black",
            },
        ],
    },
    ModelInfo {
        id: "T3Lite",
        name: "Tegenaria Lite",
        protocol: Protocol::T3Lite,
        vendor_dll: false,
        modes: &[ModeInfo {
            pid: 0x1093,
            connection: Connection::Wired,
            usage_page: 0xfff0,
            variant: "White",
        }],
    },
    ModelInfo {
        id: "T3XS",
        name: "T3XS",
        protocol: Protocol::T3XS,
        vendor_dll: true,
        modes: &[
            ModeInfo {
                pid: 0x105a,
                connection: Connection::Wired,
                usage_page: 0xffff,
                variant: "White",
            },
            ModeInfo {
                pid: 0x10be,
                connection: Connection::Wireless,
                usage_page: 0xffff,
                variant: "White",
            },
        ],
    },
];

/// Find the model and mode for a VID/PID pair.
/// The G7 Pro XBox over Bluetooth, which uses Microsoft's IDs.
pub static XBOX_BLUETOOTH: ModelInfo = ModelInfo {
    id: "G7ProXboxBT",
    name: "G7 Pro XBox (Bluetooth)",
    protocol: Protocol::XboxBluetooth,
    vendor_dll: false,
    modes: &[ModeInfo {
        pid: crate::bt::BT_PID,
        connection: Connection::Bluetooth,
        usage_page: 0x0001,
        variant: "Xbox Bluetooth Gamepad",
    }],
};

pub fn lookup(vid: u16, pid: u16) -> Option<(&'static ModelInfo, &'static ModeInfo)> {
    if vid == crate::bt::MS_VID && pid == crate::bt::BT_PID {
        return Some((&XBOX_BLUETOOTH, &XBOX_BLUETOOTH.modes[0]));
    }
    if vid != GAMESIR_VID {
        return None;
    }
    MODELS.iter().find_map(|m| {
        m.modes
            .iter()
            .find(|mode| mode.pid == pid)
            .map(|mode| (m, mode))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn g7_pro_wired_and_wireless() {
        let (m, mode) = lookup(GAMESIR_VID, 0x10b7).unwrap();
        assert_eq!(m.protocol, Protocol::G7Pro);
        assert_eq!(mode.connection, Connection::Wired);
        let (_, mode) = lookup(GAMESIR_VID, 0x10b9).unwrap();
        assert_eq!(mode.connection, Connection::Wireless);
        assert_eq!(mode.usage_page, 0xfff0);
    }

    #[test]
    fn g7_pro_xbox_modes() {
        for pid in [0x10ba, 0x10bb, 0x1022] {
            let (m, _) = lookup(GAMESIR_VID, pid).unwrap();
            assert_eq!(m.protocol, Protocol::G7ProXbox);
        }
        let (_, mode) = lookup(GAMESIR_VID, 0x1022).unwrap();
        assert_eq!(mode.usage_page, 0xfff0);
    }

    #[test]
    fn unknown_vendor() {
        assert!(lookup(0x045e, 0x10b7).is_none());
    }
}

//! Core library for SirClient, an unofficial cross-platform configurator
//! for GameSir controllers.
//!
//! * [`transport`] enumerates and opens HID collections (USB, 2.4 GHz
//!   dongle and Bluetooth).
//! * [`registry`] knows which GameSir models exist and which protocol each
//!   one speaks.
//! * [`protocol`] holds the wire formats; [`session`] runs the
//!   request/acknowledge loop against a device.
//! * [`mock`] simulates a G7 Pro so the UI can run without hardware.

pub mod diag;
pub mod mock;
pub mod probe;
pub mod protocol;
pub mod registry;
pub mod session;
pub mod transport;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("HID error: {0}")]
    Hid(#[from] hidapi::HidError),
    #[error("invalid device path")]
    InvalidPath,
    #[error("expected {expected} bytes, got {got}")]
    BadLength { expected: usize, got: usize },
}

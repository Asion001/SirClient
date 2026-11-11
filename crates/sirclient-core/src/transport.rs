//! HID enumeration and raw I/O.

use std::ffi::CString;

use hidapi::{BusType, HidApi, HidDevice};

use crate::Error;
use crate::registry::{self, ModeInfo, ModelInfo};

/// Size of every report the GameSir configuration protocol uses.
pub const REPORT_LEN: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize)]
pub enum Bus {
    Usb,
    Bluetooth,
    Other,
}

impl From<BusType> for Bus {
    fn from(b: BusType) -> Self {
        match b {
            BusType::Usb => Bus::Usb,
            BusType::Bluetooth => Bus::Bluetooth,
            _ => Bus::Other,
        }
    }
}

/// One HID top-level collection (a physical device usually has several).
#[derive(Clone, Debug, serde::Serialize)]
pub struct HidInfo {
    /// Platform path, used to open the collection.
    pub path: String,
    pub vid: u16,
    pub pid: u16,
    pub usage_page: u16,
    pub usage: u16,
    pub interface: i32,
    pub bus: Bus,
    pub manufacturer: String,
    pub product: String,
    pub serial: String,
}

impl HidInfo {
    pub fn is_vendor_page(&self) -> bool {
        self.usage_page >= 0xff00
    }

    /// Known GameSir model for this VID/PID, if any.
    pub fn model(&self) -> Option<(&'static ModelInfo, &'static ModeInfo)> {
        registry::lookup(self.vid, self.pid)
    }

    /// True when this is the configuration collection of a known model.
    pub fn is_config_interface(&self) -> bool {
        match self.model() {
            // Some backends (older Linux hidraw) report usage page 0.
            Some((_, mode)) => self.usage_page == mode.usage_page || self.usage_page == 0,
            None => false,
        }
    }

    /// A GameSir vendor-defined collection whose product ID is not in the
    /// registry (e.g. a controller that enumerates differently on Linux).
    /// It may still speak a known protocol.
    pub fn is_unknown_gamesir(&self) -> bool {
        self.vid == registry::GAMESIR_VID && self.model().is_none() && self.is_vendor_page()
    }

    /// Short label for lists.
    pub fn label(&self) -> String {
        match self.model() {
            Some((m, mode)) => format!("{} ({:?})", m.name, mode.connection),
            None if !self.product.is_empty() => {
                format!("{} ({:04x}:{:04x})", self.product, self.vid, self.pid)
            }
            None => format!("{:04x}:{:04x}", self.vid, self.pid),
        }
    }
}

/// Owns the hidapi context.
pub struct Hid {
    api: HidApi,
}

impl Hid {
    pub fn new() -> Result<Self, Error> {
        Ok(Self {
            api: HidApi::new()?,
        })
    }

    /// Rescan and list every HID collection on the system.
    pub fn enumerate(&mut self) -> Result<Vec<HidInfo>, Error> {
        self.api.refresh_devices()?;
        let mut out: Vec<HidInfo> = self
            .api
            .device_list()
            .map(|d| HidInfo {
                path: d.path().to_string_lossy().into_owned(),
                vid: d.vendor_id(),
                pid: d.product_id(),
                usage_page: d.usage_page(),
                usage: d.usage(),
                interface: d.interface_number(),
                bus: d.bus_type().into(),
                manufacturer: d.manufacturer_string().unwrap_or_default().to_string(),
                product: d.product_string().unwrap_or_default().to_string(),
                serial: d.serial_number().unwrap_or_default().to_string(),
            })
            .collect();
        out.sort_by(|a, b| (a.vid, a.pid, &a.path).cmp(&(b.vid, b.pid, &b.path)));
        Ok(out)
    }

    /// Configuration collections of known GameSir models.
    pub fn gamesir_devices(&mut self) -> Result<Vec<HidInfo>, Error> {
        Ok(self
            .enumerate()?
            .into_iter()
            .filter(HidInfo::is_config_interface)
            .collect())
    }

    pub fn open(&self, path: &str) -> Result<HidDevice, Error> {
        let c = CString::new(path).map_err(|_| Error::InvalidPath)?;
        Ok(self.api.open_path(&c)?)
    }
}

/// Pad a command to a full 64-byte output report. Byte 0 is the report ID.
pub fn pad_report(bytes: &[u8]) -> [u8; REPORT_LEN] {
    let mut buf = [0u8; REPORT_LEN];
    let n = bytes.len().min(REPORT_LEN);
    buf[..n].copy_from_slice(&bytes[..n]);
    buf
}

enum ServiceCmd {
    Rescan,
    Open(
        String,
        crossbeam_channel::Sender<Result<hidapi::HidDevice, Error>>,
    ),
}

/// Runs hidapi on its own thread.
///
/// On macOS, hidapi's enumeration pumps the calling thread's CoreFoundation
/// run loop, which re-enters the GUI event loop if called from the main
/// thread. Keeping every enumerate/open call on this thread avoids that on
/// all platforms.
pub struct HidService {
    tx: crossbeam_channel::Sender<ServiceCmd>,
    lists: crossbeam_channel::Receiver<Result<Vec<HidInfo>, String>>,
}

impl HidService {
    /// Start the service; it rescans every `interval` and calls `notify`
    /// after each scan.
    pub fn spawn(
        interval: std::time::Duration,
        notify: std::sync::Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded::<ServiceCmd>();
        let (list_tx, lists) = crossbeam_channel::unbounded();
        std::thread::Builder::new()
            .name("sirclient-hid".into())
            .spawn(move || {
                let mut hid = match Hid::new() {
                    Ok(h) => h,
                    Err(e) => {
                        let _ = list_tx.send(Err(e.to_string()));
                        notify();
                        // Keep answering opens with the error.
                        while let Ok(cmd) = rx.recv() {
                            if let ServiceCmd::Open(_, reply) = cmd {
                                let _ = reply.send(Err(Error::InvalidPath));
                            }
                        }
                        return;
                    }
                };
                loop {
                    let scan = match rx.recv_timeout(interval) {
                        Ok(ServiceCmd::Rescan) => true,
                        Ok(ServiceCmd::Open(path, reply)) => {
                            let _ = reply.send(hid.open(&path));
                            false
                        }
                        Err(crossbeam_channel::RecvTimeoutError::Timeout) => true,
                        Err(crossbeam_channel::RecvTimeoutError::Disconnected) => return,
                    };
                    if scan {
                        let _ = list_tx.send(hid.enumerate().map_err(|e| e.to_string()));
                        notify();
                    }
                }
            })
            .expect("spawn hid thread");
        let s = HidService { tx, lists };
        s.rescan();
        s
    }

    pub fn rescan(&self) {
        let _ = self.tx.send(ServiceCmd::Rescan);
    }

    /// Newest finished scan, if one arrived since the last call.
    pub fn latest(&self) -> Option<Result<Vec<HidInfo>, String>> {
        self.lists.try_iter().last()
    }

    /// Open a collection by path (blocks until the service thread answers).
    pub fn open(&self, path: &str) -> Result<hidapi::HidDevice, Error> {
        let (reply, rx) = crossbeam_channel::bounded(1);
        self.tx
            .send(ServiceCmd::Open(path.to_string(), reply))
            .map_err(|_| Error::InvalidPath)?;
        rx.recv_timeout(std::time::Duration::from_secs(5))
            .map_err(|_| Error::InvalidPath)?
    }
}

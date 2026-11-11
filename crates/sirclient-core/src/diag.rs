//! Plain-text diagnostic report for bug reports (`sirclient --diagnose`).
//!
//! It answers "why isn't my controller detected?": what hidapi sees, and on
//! Linux what the USB bus sees, which kernel driver owns each interface and
//! whether the hidraw nodes can be opened by this user.

use std::fmt::Write;

use crate::registry::GAMESIR_VID;
use crate::transport::{Hid, HidInfo};

/// Microsoft's vendor ID; Xbox-mode controllers may enumerate with it.
const MICROSOFT_VID: u16 = 0x045E;

fn interesting(vid: u16) -> bool {
    vid == GAMESIR_VID || vid == MICROSOFT_VID
}

pub fn report(app_version: &str) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "SirClient {app_version} diagnostic report");
    let _ = writeln!(
        out,
        "os: {} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    let _ = writeln!(out);

    let _ = writeln!(out, "== HID collections (hidapi) and GIP devices ==");
    match Hid::new().and_then(|mut h| h.enumerate().map(|l| (h, l))) {
        Err(e) => {
            let _ = writeln!(out, "hidapi failed: {e}");
        }
        Ok((hid, list)) => {
            let _ = writeln!(out, "{} collections in total", list.len());
            let relevant: Vec<&HidInfo> = list.iter().filter(|d| interesting(d.vid)).collect();
            if relevant.is_empty() {
                let _ = writeln!(
                    out,
                    "no GameSir (3537) or Microsoft (045e) HID collections found"
                );
            }
            for d in relevant {
                let model = match d.model() {
                    Some((m, mode)) => format!(
                        "{} {:?} (config page {:04x})",
                        m.name, mode.connection, mode.usage_page
                    ),
                    None if d.vid == GAMESIR_VID => "unknown GameSir product id".into(),
                    None => "-".into(),
                };
                let _ = writeln!(
                    out,
                    "{:04x}:{:04x} page {:04x}/{:04x} iface {} bus {:?} config={} | {} | {} {} | {}",
                    d.vid,
                    d.pid,
                    d.usage_page,
                    d.usage,
                    d.interface,
                    d.bus,
                    d.is_config_interface(),
                    model,
                    d.manufacturer,
                    d.product,
                    d.path
                );
                if crate::gip::is_gip_path(&d.path) {
                    let _ = writeln!(out, "    raw USB (Xbox GIP); not opened here");
                } else if d.vid == GAMESIR_VID {
                    let res = match hid.open(&d.path) {
                        Ok(dev) if d.is_vendor_page() => format!(
                            "ok, report ids {:02x?}",
                            crate::transport::device_report_ids(&dev, d.usage_page)
                        ),
                        Ok(_) => "ok".to_string(),
                        Err(e) => e.to_string(),
                    };
                    let _ = writeln!(out, "    open: {res}");
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    linux::append(&mut out);

    out
}

#[cfg(target_os = "linux")]
mod linux {
    use std::fmt::Write;
    use std::fs;
    use std::path::Path;

    fn read(p: &Path) -> String {
        fs::read_to_string(p)
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    }

    fn driver(p: &Path) -> String {
        fs::read_link(p.join("driver"))
            .ok()
            .and_then(|l| l.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "(none)".into())
    }

    pub fn append(out: &mut String) {
        let _ = writeln!(out);
        let _ = writeln!(out, "== USB devices (sysfs) ==");
        let mut found = false;
        if let Ok(rd) = fs::read_dir("/sys/bus/usb/devices") {
            let mut devs: Vec<_> = rd.flatten().map(|e| e.path()).collect();
            devs.sort();
            for dev in &devs {
                let vid = read(&dev.join("idVendor"));
                if vid != "3537" && vid != "045e" {
                    continue;
                }
                found = true;
                let _ = writeln!(
                    out,
                    "{}:{} {} {} ({})",
                    vid,
                    read(&dev.join("idProduct")),
                    read(&dev.join("manufacturer")),
                    read(&dev.join("product")),
                    dev.file_name().unwrap_or_default().to_string_lossy()
                );
                // Interfaces are siblings named "<dev>:<config>.<n>".
                let name = dev
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                for itf in devs.iter().filter(|p| {
                    p.file_name()
                        .is_some_and(|n| n.to_string_lossy().starts_with(&format!("{name}:")))
                }) {
                    let _ = writeln!(
                        out,
                        "    iface {} class {} sub {} proto {} driver {}",
                        read(&itf.join("bInterfaceNumber")),
                        read(&itf.join("bInterfaceClass")),
                        read(&itf.join("bInterfaceSubClass")),
                        read(&itf.join("bInterfaceProtocol")),
                        driver(itf)
                    );
                }
            }
        }
        if !found {
            let _ = writeln!(out, "no USB device with vendor 3537 or 045e");
        }

        let _ = writeln!(out);
        let _ = writeln!(out, "== hidraw nodes ==");
        if let Ok(rd) = fs::read_dir("/sys/class/hidraw") {
            let mut nodes: Vec<_> = rd.flatten().map(|e| e.path()).collect();
            nodes.sort();
            for n in nodes {
                let uevent = read(&n.join("device/uevent"));
                let hid_id = uevent
                    .lines()
                    .find_map(|l| l.strip_prefix("HID_ID="))
                    .unwrap_or("")
                    .to_lowercase();
                let hid_name = uevent
                    .lines()
                    .find_map(|l| l.strip_prefix("HID_NAME="))
                    .unwrap_or("");
                if !hid_id.contains(":00003537:") && !hid_id.contains(":0000045e:") {
                    continue;
                }
                let dev = format!(
                    "/dev/{}",
                    n.file_name().unwrap_or_default().to_string_lossy()
                );
                let access = match fs::OpenOptions::new().read(true).write(true).open(&dev) {
                    Ok(_) => "read/write ok".to_string(),
                    Err(e) => format!("cannot open: {e}"),
                };
                let _ = writeln!(out, "{dev} {hid_id} \"{hid_name}\" -> {access}");
            }
        }
        let rules = Path::new("/etc/udev/rules.d/70-sirclient.rules").exists();
        let _ = writeln!(out, "udev rule installed: {rules}");
    }
}

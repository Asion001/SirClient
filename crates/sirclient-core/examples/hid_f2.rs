//! Send the GIP "app present" command (`F2 01`) to the G7 Pro XBox dongle
//! while it sits in its HID fallback (`3537:1022`), through every HID
//! collection of it, and watch whether the dongle changes mode.
//!
//!     cargo run -p sirclient-core --example hid_f2

use std::time::{Duration, Instant};

fn main() {
    let api = hidapi::HidApi::new().expect("hidapi");
    #[cfg(target_os = "macos")]
    api.set_open_exclusive(false);
    let list: Vec<_> = api
        .device_list()
        .filter(|d| d.vendor_id() == 0x3537 && d.product_id() == 0x1022)
        .collect();
    if list.is_empty() {
        println!("no 3537:1022 collection");
        return;
    }
    for d in &list {
        println!(
            "collection page {:04x} usage {:04x} iface {} path {:?}",
            d.usage_page(),
            d.usage(),
            d.interface_number(),
            d.path()
        );
    }
    for d in &list {
        let Ok(dev) = d.open_device(&api) else {
            println!("  open failed for page {:04x}", d.usage_page());
            continue;
        };
        let mut desc = [0u8; 4096];
        if let Ok(n) = dev.get_report_descriptor(&mut desc) {
            println!("  descriptor ({n} bytes): {:02x?}", &desc[..n]);
        }
        for len in [64usize, 65] {
            let mut r = vec![0u8; len];
            r[0] = 0x0F;
            r[1] = 0xF2;
            r[2] = 0x01;
            println!(
                "  page {:04x}: write {len}-byte [0f f2 01]: {:?}",
                d.usage_page(),
                dev.write(&r)
            );
            let end = Instant::now() + Duration::from_millis(500);
            let mut buf = [0u8; 128];
            while Instant::now() < end {
                match dev.read_timeout(&mut buf, 100) {
                    Ok(0) => {}
                    Ok(n) if buf[0] != 0x01 => println!("    < {:02x?}", &buf[..n]),
                    Ok(_) => {}
                    Err(e) => {
                        println!("    read error {e}");
                        break;
                    }
                }
            }
        }
    }
    std::thread::sleep(Duration::from_secs(5));
    let mut api = api;
    let _ = api.refresh_devices();
    let pids: Vec<u16> = api
        .device_list()
        .filter(|d| d.vendor_id() == 0x3537)
        .map(|d| d.product_id())
        .collect();
    println!("GameSir HID PIDs after 5 s: {pids:04x?}");
}

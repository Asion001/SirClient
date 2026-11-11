//! Low-level GIP probe for bringing up a new OS: open the G7 Pro XBox with
//! libusb, print its descriptors and configuration, then log every packet
//! it sends while a few init messages go out.
//!
//!     cargo run -p sirclient-core --example gip_raw [--wait] [--reset] [--set-config] [--no-power-on]
//!
//! `--wait` waits up to 60 s for the pad to appear in GIP mode (plug the
//! dongle in while it waits) and sends the init right away.

use std::time::{Duration, Instant};

use rusb::UsbContext;

const VID: u16 = 0x3537;
const PIDS: [u16; 2] = [0x10BA, 0x10BB];

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let set_config = args.iter().any(|a| a == "--set-config");
    let reset = args.iter().any(|a| a == "--reset");
    let wait = args.iter().any(|a| a == "--wait");
    let power_on = !args.iter().any(|a| a == "--no-power-on");

    let ctx = rusb::Context::new().expect("libusb");
    let pids_now = |ctx: &rusb::Context| -> Vec<u16> {
        ctx.devices()
            .map(|l| {
                l.iter()
                    .filter_map(|d| d.device_descriptor().ok())
                    .filter(|dd| dd.vendor_id() == VID)
                    .map(|dd| dd.product_id())
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut from_fallback = false;
    if wait {
        println!("waiting for 3537:10BA/10BB (plug the dongle in now)...");
        let t0 = Instant::now();
        let mut last = Vec::new();
        while t0.elapsed() < Duration::from_secs(60) {
            let p = pids_now(&ctx);
            if p != last {
                println!("  {:>5} ms: {p:04x?}", t0.elapsed().as_millis());
                last = p.clone();
            }
            if p.iter().any(|x| PIDS.contains(x)) {
                from_fallback = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    let now = pids_now(&ctx);
    println!("GameSir PIDs now: {now:04x?}");
    if !now.iter().any(|p| PIDS.contains(p)) && now.contains(&0x1022) {
        // The dongle dropped to its HID fallback: reset it back to GIP.
        let d = ctx
            .devices()
            .unwrap()
            .iter()
            .find(|d| {
                d.device_descriptor()
                    .is_ok_and(|dd| dd.product_id() == 0x1022)
            })
            .unwrap();
        match d.open() {
            Ok(h) => println!("reset 1022: {:?}", h.reset()),
            Err(e) => println!("open 1022 for reset: {e}"),
        }
        let t0 = Instant::now();
        let mut last = Vec::new();
        while t0.elapsed() < Duration::from_secs(6) {
            let p = pids_now(&ctx);
            if p != last {
                println!("  {:>5} ms: {p:04x?}", t0.elapsed().as_millis());
                last = p.clone();
            }
            if p.iter().any(|x| PIDS.contains(x)) {
                from_fallback = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    let dev = ctx
        .devices()
        .expect("device list")
        .iter()
        .find(|d| {
            d.device_descriptor()
                .is_ok_and(|dd| dd.vendor_id() == VID && PIDS.contains(&dd.product_id()))
        })
        .expect("no 3537:10BA/10BB device");
    let dd = dev.device_descriptor().unwrap();
    println!(
        "device {:04x}:{:04x} bus {} addr {} class {:02x}/{:02x}/{:02x} configs {}",
        dd.vendor_id(),
        dd.product_id(),
        dev.bus_number(),
        dev.address(),
        dd.class_code(),
        dd.sub_class_code(),
        dd.protocol_code(),
        dd.num_configurations()
    );
    for c in 0..dd.num_configurations() {
        let Ok(cd) = dev.config_descriptor(c) else {
            continue;
        };
        println!(
            "config {} ({} interfaces)",
            cd.number(),
            cd.num_interfaces()
        );
        for i in cd.interfaces() {
            for alt in i.descriptors() {
                println!(
                    "  iface {} alt {} class {:02x}/{:02x}/{:02x}",
                    alt.interface_number(),
                    alt.setting_number(),
                    alt.class_code(),
                    alt.sub_class_code(),
                    alt.protocol_code()
                );
                for ep in alt.endpoint_descriptors() {
                    println!(
                        "    ep {:02x} {:?} {:?} max {} interval {}",
                        ep.address(),
                        ep.direction(),
                        ep.transfer_type(),
                        ep.max_packet_size(),
                        ep.interval()
                    );
                }
            }
        }
    }

    let mut h = dev.open().expect("open");
    if reset {
        // A fresh enumeration makes the pad send its GIP announce again.
        println!("reset: {:?}", h.reset());
        drop(h);
        std::thread::sleep(Duration::from_millis(1500));
        let dev = ctx
            .devices()
            .expect("device list")
            .iter()
            .find(|d| {
                d.device_descriptor()
                    .is_ok_and(|dd| dd.vendor_id() == VID && PIDS.contains(&dd.product_id()))
            })
            .expect("pad gone after reset");
        println!(
            "after reset: bus {} addr {}",
            dev.bus_number(),
            dev.address()
        );
        h = dev.open().expect("open after reset");
    }
    println!("active configuration: {:?}", h.active_configuration());
    if set_config {
        println!(
            "set_active_configuration(1): {:?}",
            h.set_active_configuration(1)
        );
        println!("active configuration: {:?}", h.active_configuration());
    }
    println!("kernel driver active: {:?}", h.kernel_driver_active(0));
    println!("claim_interface(0): {:?}", h.claim_interface(0));

    let mut seq = 0u8;
    let mut send = |h: &rusb::DeviceHandle<rusb::Context>, label: &str, mut pkt: Vec<u8>| {
        seq = seq.wrapping_add(1).max(1);
        if pkt.len() > 2 {
            pkt[2] = seq;
        }
        let r = h.write_interrupt(0x02, &pkt, Duration::from_millis(500));
        println!("> {label}: {:02x?} -> {r:?}", pkt);
    };
    let listen = |h: &rusb::DeviceHandle<rusb::Context>, ms: u64| {
        let end = Instant::now() + Duration::from_millis(ms);
        let mut buf = [0u8; 64];
        let mut n_state = 0;
        let mut timeouts = 0;
        while Instant::now() < end {
            match h.read_interrupt(0x82, &mut buf, Duration::from_millis(100)) {
                Ok(n) => {
                    let p = &buf[..n];
                    // Don't flood the log with the live state stream.
                    if p.first() == Some(&0x10) && p.get(4) == Some(&0xE0) {
                        n_state += 1;
                        if n_state > 2 {
                            continue;
                        }
                    }
                    if p.first() == Some(&0x20) {
                        n_state += 1;
                        if n_state > 2 {
                            continue;
                        }
                    }
                    println!("< {:02x?}", p);
                    // Acknowledge packets that ask for it.
                    if n >= 4 && p[1] & 0x10 != 0 {
                        let ack = [
                            0x01, 0x20, p[2], 0x09, 0x00, p[0], 0x20, p[3], 0, 0, 0, 0, 0,
                        ];
                        let r = h.write_interrupt(0x02, &ack, Duration::from_millis(200));
                        println!("> ack {:02x?} -> {r:?}", ack);
                    }
                }
                Err(rusb::Error::Timeout) => timeouts += 1,
                Err(e) => {
                    println!("read error: {e}");
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        }
        if n_state > 2 {
            println!("  (+{} more state packets)", n_state - 2);
        }
        println!("  ({timeouts} empty 100 ms reads)");
    };

    if !from_fallback {
        println!("-- listening before any message");
        listen(&h, 1500);
    }
    if power_on {
        send(&h, "power on", vec![0x05, 0x20, 0, 0x01, 0x00]);
        listen(&h, if from_fallback { 200 } else { 1500 });
    }
    let cmd = |body: &[u8]| {
        let mut p = vec![0x0F, 0x00, 0, 60];
        p.extend_from_slice(body);
        p.resize(64, 0);
        p
    };
    send(&h, "F2 01 (app present)", cmd(&[0xF2, 0x01]));
    listen(&h, 1000);
    send(&h, "0B (current profile)", cmd(&[0x0B]));
    listen(&h, 1000);
    send(&h, "09 (firmware)", cmd(&[0x09]));
    listen(&h, 1500);
    send(&h, "F2 01 again", cmd(&[0xF2, 0x01]));
    listen(&h, 3000);
    println!("GameSir PIDs at the end: {:04x?}", pids_now(&ctx));
    let _ = h.release_interface(0);
}

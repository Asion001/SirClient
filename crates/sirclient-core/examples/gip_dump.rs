//! Read firmware, the active profile and every slot from a G7 Pro XBox over
//! GIP and print them.
//!
//!     cargo run -p sirclient-core --example gip_dump

fn main() {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use sirclient_core::gip::GipLink;
    use sirclient_core::session::{Event, Session};

    env_logger::init();
    let link = GipLink::open().expect("open G7 Pro XBox");
    println!("opened {}", link.path());
    // The session reads everything on start.
    let s = Session::spawn(Box::new(link), Arc::new(|| {}));
    let end = Instant::now() + Duration::from_secs(10);
    let mut slots = 0;
    while Instant::now() < end && slots < 6 {
        for e in s.events() {
            match e {
                Event::Firmware { controller, dongle } => {
                    println!("firmware: controller {controller}, dongle {dongle}")
                }
                Event::CurrentProfile(n) => println!("current profile: {n}"),
                Event::SlotLoaded(slot, bytes) => {
                    slots += 1;
                    println!("{slot:?}: {} bytes", bytes.len());
                    for (i, row) in bytes.chunks(16).enumerate() {
                        let hex: Vec<String> = row.iter().map(|b| format!("{b:02x}")).collect();
                        println!("  {:03x}: {}", i * 16, hex.join(" "));
                    }
                }
                Event::Warning(w) => println!("warning: {w}"),
                Event::Disconnected(w) => {
                    println!("disconnected: {w}");
                    return;
                }
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    // Let the current-profile reply arrive.
    std::thread::sleep(Duration::from_millis(300));
    for e in s.events() {
        if let Event::CurrentProfile(n) = e {
            println!("current profile: {n}");
        }
    }
}

//! Windows: reach the G7 Pro XBox through `Windows.Gaming.Input.Custom`.
//!
//! On Windows `xboxgip.sys` owns GIP controllers, so the USB interface can't
//! be opened directly. GameSir Nexus instead registers a custom game
//! controller factory for the pad's GIP interface GUIDs. Windows then hands
//! the factory a [`GipGameControllerProvider`] for each matching controller,
//! which can send GIP messages (`SendMessage(Command, 0x0F, payload)`), and
//! delivers every GIP message from the pad to the object the factory returned
//! (`IGipGameControllerInputSink::OnMessageReceived`). The driver builds the
//! GIP header, so only the 60-byte payload crosses this API.
//!
//! The factory registration lasts for the life of the process, so it is done
//! once and every provider it sees is kept in a process-wide list.

use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use windows::Foundation::TypedEventHandler;
use windows::Gaming::Input::Custom::{
    GameControllerFactoryManager, GipGameControllerProvider, GipMessageClass,
    ICustomGameControllerFactory, ICustomGameControllerFactory_Impl, IGameControllerInputSink_Impl,
    IGameControllerProvider, IGipGameControllerInputSink, IGipGameControllerInputSink_Impl,
};
use windows::Gaming::Input::{Headset, IGameController, IGameController_Impl};
use windows::System::{User, UserChangedEventArgs};
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
use windows::core::{Error as WinError, GUID, IInspectable, Interface, Ref, implement};

use super::{GIP_PIDS, MSG_COMMAND, MSG_REPLY, PATH_PREFIX, reply_from_payload};
use crate::Error;
use crate::protocol::g7pro::commands::Geometry;
use crate::registry::GAMESIR_VID;
use crate::session::Link;

/// GIP interface GUIDs GameSir Nexus registers its factory for. The G7 Pro
/// XBox declares one of them in its GIP metadata.
const GIP_INTERFACES: [GUID; 3] = [
    GUID::from_u128(0x082e402c_07df_45e1_a5ab_a3127af197b5),
    GUID::from_u128(0x332054cc_a34b_41d5_a34a_a6a6711ec4b3),
    GUID::from_u128(0xb8f31fe7_7386_40e9_a9f8_2f21263acfb7),
];

fn win(e: WinError) -> Error {
    Error::Usb(format!("Windows GIP: {e}"))
}

/// A controller Windows handed to the factory.
struct Pad {
    id: u64,
    provider: GipGameControllerProvider,
    vid: u16,
    pid: u16,
    /// Replies from the pad, as HID-style reports (`[0x10, code, ...]`).
    replies: Receiver<Vec<u8>>,
    /// Cleared when Windows reports the controller removed.
    connected: Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Default)]
struct Registry {
    pads: Vec<Arc<Pad>>,
    next_id: u64,
}

fn registry() -> &'static Mutex<Registry> {
    static R: OnceLock<Mutex<Registry>> = OnceLock::new();
    R.get_or_init(Default::default)
}

/// Register the factory once per process. Errors are kept and returned to
/// every caller.
fn ensure_registered() -> Result<(), Error> {
    static DONE: OnceLock<Result<(), String>> = OnceLock::new();
    DONE.get_or_init(|| {
        // Already initialised (in either apartment) is fine.
        let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
        let factory: ICustomGameControllerFactory = Factory.into();
        let mut ok = false;
        let mut last = String::new();
        for g in GIP_INTERFACES {
            match GameControllerFactoryManager::RegisterCustomFactoryForGipInterface(&factory, g) {
                Ok(()) => ok = true,
                Err(e) => {
                    log::warn!("RegisterCustomFactoryForGipInterface({g:?}): {e}");
                    last = e.to_string();
                }
            }
        }
        if ok {
            log::info!("registered GIP controller factory");
            Ok(())
        } else {
            Err(last)
        }
    })
    .clone()
    .map_err(|e| {
        Error::Usb(format!(
            "Windows GIP: registering the controller factory failed: {e}"
        ))
    })
}

fn live_pads() -> Vec<Arc<Pad>> {
    let r = registry().lock().unwrap();
    r.pads
        .iter()
        .filter(|p| {
            p.connected.load(std::sync::atomic::Ordering::Relaxed)
                && p.provider.IsConnected().unwrap_or(false)
        })
        .cloned()
        .collect()
}

fn is_g7_xbox(p: &Pad) -> bool {
    p.vid == GAMESIR_VID && GIP_PIDS.contains(&p.pid)
}

/// Wait up to `wait` for Windows to report a G7 Pro XBox to the factory.
fn find_pad(wait: Duration) -> Result<Option<Arc<Pad>>, Error> {
    ensure_registered()?;
    let end = Instant::now() + wait;
    loop {
        if let Some(p) = live_pads().into_iter().find(|p| is_g7_xbox(p)) {
            return Ok(Some(p));
        }
        if Instant::now() >= end {
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[implement(ICustomGameControllerFactory)]
struct Factory;

impl ICustomGameControllerFactory_Impl for Factory_Impl {
    fn CreateGameController(
        &self,
        provider: Ref<IGameControllerProvider>,
    ) -> windows::core::Result<IInspectable> {
        let provider: GipGameControllerProvider = provider.ok()?.cast()?;
        let vid = provider.HardwareVendorId().unwrap_or(0);
        let pid = provider.HardwareProductId().unwrap_or(0);
        log::info!("Windows GIP controller {vid:04x}:{pid:04x}");
        let (tx, rx) = crossbeam_channel::unbounded();
        let connected = Arc::new(std::sync::atomic::AtomicBool::new(true));
        {
            let mut r = registry().lock().unwrap();
            r.next_id += 1;
            let id = r.next_id;
            r.pads
                .retain(|p| p.connected.load(std::sync::atomic::Ordering::Relaxed));
            r.pads.push(Arc::new(Pad {
                id,
                provider,
                vid,
                pid,
                replies: rx,
                connected: connected.clone(),
            }));
        }
        let sink: IGipGameControllerInputSink = Controller { tx, connected }.into();
        sink.cast()
    }

    fn OnGameControllerAdded(&self, _value: Ref<IGameController>) -> windows::core::Result<()> {
        Ok(())
    }

    fn OnGameControllerRemoved(&self, _value: Ref<IGameController>) -> windows::core::Result<()> {
        // Liveness is checked through the provider's IsConnected.
        Ok(())
    }
}

/// The object Windows feeds the pad's GIP messages to.
#[implement(IGipGameControllerInputSink, IGameController)]
struct Controller {
    tx: Sender<Vec<u8>>,
    connected: Arc<std::sync::atomic::AtomicBool>,
}

impl IGameControllerInputSink_Impl for Controller_Impl {
    fn OnInputResumed(&self, _timestamp: u64) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnInputSuspended(&self, _timestamp: u64) -> windows::core::Result<()> {
        Ok(())
    }
}

impl IGipGameControllerInputSink_Impl for Controller_Impl {
    fn OnKeyReceived(
        &self,
        _timestamp: u64,
        _key_code: u8,
        _is_pressed: bool,
    ) -> windows::core::Result<()> {
        Ok(())
    }

    fn OnMessageReceived(
        &self,
        _timestamp: u64,
        _message_class: GipMessageClass,
        message_id: u8,
        _sequence_id: u8,
        message_buffer: &[u8],
    ) -> windows::core::Result<()> {
        if message_id == MSG_REPLY
            && let Some(r) = reply_from_payload(message_buffer)
        {
            let _ = self.tx.send(r);
        }
        Ok(())
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        self.connected
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }
}

// Windows asks the factory's object for IGameController; it carries no
// state SirClient needs, so the members are inert.
impl IGameController_Impl for Controller_Impl {
    fn HeadsetConnected(
        &self,
        _value: Ref<TypedEventHandler<IGameController, Headset>>,
    ) -> windows::core::Result<i64> {
        Ok(0)
    }
    fn RemoveHeadsetConnected(&self, _token: i64) -> windows::core::Result<()> {
        Ok(())
    }
    fn HeadsetDisconnected(
        &self,
        _value: Ref<TypedEventHandler<IGameController, Headset>>,
    ) -> windows::core::Result<i64> {
        Ok(0)
    }
    fn RemoveHeadsetDisconnected(&self, _token: i64) -> windows::core::Result<()> {
        Ok(())
    }
    fn UserChanged(
        &self,
        _value: Ref<TypedEventHandler<IGameController, UserChangedEventArgs>>,
    ) -> windows::core::Result<i64> {
        Ok(0)
    }
    fn RemoveUserChanged(&self, _token: i64) -> windows::core::Result<()> {
        Ok(())
    }
    fn Headset(&self) -> windows::core::Result<Headset> {
        Err(windows::core::Error::from_hresult(
            windows::Win32::Foundation::E_NOTIMPL,
        ))
    }
    fn IsWireless(&self) -> windows::core::Result<bool> {
        Ok(false)
    }
    fn User(&self) -> windows::core::Result<User> {
        Err(windows::core::Error::from_hresult(
            windows::Win32::Foundation::E_NOTIMPL,
        ))
    }
}

/// G7 Pro XBox pads Windows has reported to the factory, for the device
/// list. The first call registers the factory; pads appear on later scans.
pub fn enumerate() -> Vec<crate::transport::HidInfo> {
    if let Err(e) = ensure_registered() {
        log::debug!("{e}");
        return vec![];
    }
    live_pads()
        .into_iter()
        .filter(|p| is_g7_xbox(p))
        .map(|p| crate::transport::HidInfo {
            path: format!("{PATH_PREFIX}win:{}", p.id),
            vid: p.vid,
            pid: p.pid,
            usage_page: 0,
            usage: 0,
            interface: 0,
            bus: crate::transport::Bus::Usb,
            manufacturer: "GameSir".into(),
            product: "GameSir-G7 Pro".into(),
            serial: String::new(),
        })
        .collect()
}

/// Configuration link to one G7 Pro XBox through the Windows GIP stack.
pub struct WinGipLink {
    pad: Arc<Pad>,
    path: String,
}

impl WinGipLink {
    /// Open the first G7 Pro XBox Windows reports, waiting up to `wait` for
    /// it to show up after the factory is registered.
    pub fn open(wait: Duration) -> Result<Option<WinGipLink>, Error> {
        let Some(pad) = find_pad(wait)? else {
            return Ok(None);
        };
        // Drop replies queued before this link was opened.
        while pad.replies.try_recv().is_ok() {}
        let path = format!("{PATH_PREFIX}win:{}", pad.id);
        let mut link = WinGipLink { pad, path };
        link.write(&[MSG_COMMAND, 0xF2, 0x01])?;
        log::info!("opened G7 Pro XBox over Windows GIP");
        Ok(Some(link))
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

impl Link for WinGipLink {
    fn write(&mut self, report: &[u8]) -> Result<(), Error> {
        if let Some(r) = super::rumble_for(report, 0) {
            // The driver adds the GIP header; send the rumble body only.
            return self
                .pad
                .provider
                .SendMessage(GipMessageClass::Command, r[0], &r[4..])
                .map_err(win);
        }
        let pkt = super::encode_command(report, 0);
        self.pad
            .provider
            .SendMessage(GipMessageClass::Command, MSG_COMMAND, &pkt[4..])
            .map_err(win)
    }

    fn read(&mut self, buf: &mut [u8], timeout_ms: i32) -> Result<usize, Error> {
        let t = Duration::from_millis(timeout_ms.max(1) as u64);
        match self.pad.replies.recv_timeout(t) {
            Ok(r) => {
                let n = r.len().min(buf.len());
                buf[..n].copy_from_slice(&r[..n]);
                Ok(n)
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                if self
                    .pad
                    .connected
                    .load(std::sync::atomic::Ordering::Relaxed)
                {
                    Ok(0)
                } else {
                    Err(Error::Usb("controller disconnected".into()))
                }
            }
            Err(_) => Err(Error::Usb("controller disconnected".into())),
        }
    }

    fn geometry(&self) -> Geometry {
        Geometry::GIP
    }
}

# G7 Pro XBox over GIP

Recovered from GameSir Nexus 2.5.8 (static analysis of
`HJC.GameSir.Nexus2_0.dll` plus its logs) and checked against a real
controller (pad firmware 2.4.9, dongle 1.5.3) on Linux.

## Modes and product IDs

| PID      | What                                                         |
|----------|--------------------------------------------------------------|
| `0x10BA` | App mode over the cable: Xbox GIP, vendor class `FF/47/D0`   |
| `0x10BB` | App mode over the 2.4 GHz dongle                             |
| `0x100A` | XInput (Xbox 360) mode, seen on Windows before app mode      |
| `0x1022` | Plain HID fallback (gamepad + keyboard/mouse + `0xFFF0`)     |

On Linux the dongle enumerates as `0x10BB` (bound to `xpad`). About two
seconds after the pad links, it re-enumerates as `0x1022` unless the host
sends the `F2` command below. The `0xFFF0` collection of `0x1022` accepts
output report `0x0F` but never answers. A USB port reset
(`libusb_reset_device`) brings it back to `0x10BB` without replugging.

## Framing

GIP header: `[message, flags, sequence, length]`.

* Host to device: message `0x0F`, flags `0x00`, a 60-byte payload.
* Device to host: message `0x10`, flags `0x00`, a 60-byte payload.

The payload is the G7 Pro 8K HID command with the report ID removed:

```
command:  [cmd, slot, offset_hi, offset_lo, length, data…]   zero padded to 60
reply:    [code, slot, offset_hi, offset_lo, length, data…]
```

Chunks carry at most 55 data bytes (60 − 5 header bytes), against 58 on HID.

## Commands seen in Nexus

| cmd  | Payload                        | Reply                                    |
|------|--------------------------------|------------------------------------------|
| `F2` | `[F2, 01]`                     | none; keeps app mode, starts `E0` stream |
| `0B` | `[0B]`                         | `[0C, current_profile]`                  |
| `09` | `[09]`                         | `[0A, UTF-16 "0249" "0153"]` (pad, dongle firmware) |
| `04` | read `[04, slot, off, off, n]` | `[05, slot, off, off, n, data]`          |
| `03` | write, 60 bytes                | `[06, 00]` ok, `[06, 01]` busy           |
| `07` | `[07, slot]`                   | `[06, 00]`                               |
| `10` | re-read after a change         | `[11, …]` like `05`                      |
| —    | —                              | `[0F, slot, off, off, len_hi, len_lo]` profile changed on the pad |
| `E2` | stick-centre test              | —                                        |
| —    | —                              | `[E0, …]` raw sticks/sensors, streamed   |

GIP message `0x09` is the standard Xbox rumble command (Nexus uses it for
the motor test).

## Slots

Slots are numbered as on the 8K: profiles 1–4, Shift `5`, Light `0x20`.
Profiles are 480 bytes and the light slot is 511 bytes. The full layout and
its translation to the 8K profile live in
[`xbox.rs`](../../crates/sirclient-core/src/protocol/g7pro/xbox.rs); it was
mapped from Nexus logs, which print each raw slot next to the decoded
settings, and checked against 30 logged profiles.

Live state arrives as reply type `E0`: the 8K state report with the battery,
active light animation and zone colours moved up (bytes 28–45) and the
physical inputs at 49.

## Reaching the pad on each OS

* **Linux:** raw USB with libusb. `xpad` is detached from interface 0 while
  SirClient holds it. Endpoints `0x02` out and `0x82` in.
* **macOS: not working.** No system driver binds the GIP interface. The
  dongle shows up as `0x10BB` only until the pad links. About 1.2 s after
  the pad's GIP announce it re-enumerates as `0x1022`, whose HID
  descriptor is a plain gamepad with no `0xFFF0` collection. The wired pad
  comes up as `0x1022` straight away (bcdDevice `0x0249`). Commands do
  work inside that window: `0B` got its `0C` reply. None of these kept it
  in GIP mode (tested 2026-11-11 with `examples/gip_raw.rs`):
  * an `F2` heartbeat every 250 ms
  * SET_CONFIGURATION
  * GIP power-on before or after the announce
  * identify (`04`) and LED-on (`0A`)
  * SET_INTERFACE(1, 0)
  * libusb reset and an IOUSBHost port reset (both come back as `0x1022`)

  The likely fix is a DriverKit extension that claims the pad at plug-in.
  Until then the XBox edition is marked unsupported on macOS (Bluetooth
  live input still works).
* **Windows:** `xboxgip.sys` owns the pad, so it can't be opened directly.
  Nexus ships a small WinRT component (`D4XGaming.Devices.dll`) that calls
  `GameControllerFactoryManager.RegisterCustomFactoryForGipInterface` for
  three GIP interface GUIDs:

  ```
  082E402C-07DF-45E1-A5AB-A3127AF197B5
  332054CC-A34B-41D5-A34A-A6A6711EC4B3
  B8F31FE7-7386-40E9-A9F8-2F21263ACFB7
  ```

  Windows then gives the factory a `GipGameControllerProvider` per pad.
  Commands go out as `SendMessage(GipMessageClass.Command, 0x0F, payload)`
  (the driver adds the GIP header), and replies arrive at the object the
  factory returned, through `IGipGameControllerInputSink.OnMessageReceived`
  (message `0x10`, payload only). SirClient does the same from Rust
  (`gip_winrt.rs`), falling back to libusb if the pad is bound to WinUSB.
  In Nexus logs the pad first shows up as XInput `0x100A` and moves to
  `0x10BB` once Nexus asks the user to press a button to enter app mode.

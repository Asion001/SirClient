# G7 Pro 8K configuration protocol

Reverse engineered from GameSir Connect 1.14.5 (Windows, Electron). The
official app calls the device "G7ProCE". SirClient's implementation is in
[`crates/sirclient-core/src/protocol/g7pro`](../../crates/sirclient-core/src/protocol/g7pro).

> Status: derived from the official app's code and exercised against the
> simulated controller in `mock.rs`. Confirm on real hardware before relying
> on a field, and please report anything that differs.

## Transport

- USB vendor ID `0x3537`. Product IDs depend on edition and connection
  (wired cable vs 2.4 GHz dongle); see [devices.md](../devices.md).
- Configuration uses the HID collection with **usage page `0xFFF0`**. The
  gamepad collection on the same device is not used for configuration.
- Every report is **64 bytes**. Byte 0 is the report ID.
- Host → device: report ID `0x0F`, byte 1 = command.
- Device → host: byte 1 = reply code. Report ID `0x12` is the live state
  stream (see below).
- **Heartbeat:** `0F F2 00` once per second keeps the controller in
  configuration mode.
- **Flow control:** one command in flight. Commands that expect a reply are
  resent every 200 ms, up to 10 times. `06 01` (ack with byte 2 = 1) means
  "busy": keep waiting and resend.

## Commands

| cmd  | name                 | payload (after `0F cmd`)                       | reply |
|------|----------------------|------------------------------------------------|-------|
| `03` | write profile        | slot, offset hi, offset lo, len (≤58), data…   | `06`  |
| `04` | read profile         | slot, offset hi, offset lo, len (≤58)          | `05`  |
| `07` | switch profile       | slot (1–4)                                     | `06`  |
| `09` | read firmware        | —                                              | `0A`  |
| `0B` | read current profile | —                                              | `0C`  |
| `0D` | light preview / RGB  | playing (0/1), frame                           | —     |
| `10` | refresh profile      | same as `04`                                   | `11`  |
| `16` | macro record         | key code, state                                | —     |
| `17` | enter firmware update| `55 88` — **not used by SirClient**            | —     |
| `20` | vibrate              | `66 55` left right (0–255)                     | —     |
| `E2` | nudge stick centre   | side (0 L, 1 R), dir (0 L 1 R 2 U 3 D), amount hi, lo | `E3` |
| `F2` | heartbeat            | test mode (0)                                  | —     |
| `FE` | calibrate            | kind (0 start, 1 finish, 2 exit), part mask    | —     |

Calibration part mask: bit 0 LS, bit 1 RS, bit 2 LT, bit 3 RT, bit 4 gyro.

## Replies

| code | meaning | layout |
|------|---------|--------|
| `05` | profile data | slot, offset hi, offset lo, len, data from byte 6 |
| `06` | ack | byte 2 = 1 means busy |
| `0A` | firmware | ASCII digits at bytes 4–8 (controller) and 12–16 (dongle); `"219"` → 2.1.9 |
| `0C` | current profile | byte 2 = profile number |
| `0F` | profile changed on the controller | slot, offset hi, lo, length hi, lo. Slot 48 = "active profile changed" |
| `11` | refresh data | as `05` |
| `E0` | USB mode changed | — |
| `E3` | stick centre result | byte 2 = 0 on success |

When the controller reports `0F` (profile changed), the app re-reads the
changed range with `10` (refresh).

## Slots

| slot | content | size |
|------|---------|------|
| 1–4  | profiles | 1070 bytes |
| 5    | Shift layer (active while Shift is held) | 1070 bytes |
| 32   | lighting | 165 bytes |

Reads and writes go in chunks of up to 58 bytes. When saving, the official
app only writes the byte range between the first and last changed byte.

## Profile layout (1070 bytes)

Multi-byte values are big-endian.

| offset | size    | section |
|-------:|--------:|---------|
| 0      | 32      | name, UTF-8, zero padded |
| 32     | 32      | function data |
| 64     | 16 × 7  | buttons: Up, Down, Left, Right, LB, RB, LS, RS, A, B, X, Y, Home, View, Menu, Screenshot |
| 176    | 4 × 169 | back buttons L4, R4, L5, R5 (with macros) |
| 852    | 2 × 32  | triggers LT, RT |
| 916    | 2 × 36  | sticks left, right |
| 988    | 2 × 41  | motion: aim, tilt |

**Function data (32):** motors left, right, LT, RT (0/25/50/75/100); audio
enable, volume, mixer, mic mute, mic sensitivity; shift enable, shift value,
D-pad diagonal lock, Xinput ABXY swap, Switch ABXY swap, report rate
(0 250 Hz, 1 500, 2 1000, 4 4000, 5 8000), motion precision, stick
resolution (UI shows `4 - raw`), LS anti-jitter, RS anti-jitter (1–21; UI
shows `raw - 11`); 13 reserved bytes.

**Button (7):** turbo enable, turbo rate (0–30 Hz), remap enable, key 1,
key 2, key 3, toggle.

**Back button (169):** button (7), macro flags (bit 0 enabled, bit 1 trigger
on press, bit 2 repeat), cycle time (u16), step count, then 32 steps of
key (u8), hold ms (u16), delay ms (u16). The last step has no delay field.

**Dead zone (9):** enable, inner, outer, anti inner, anti outer (u16 each,
0–1000 = tenths of a percent).

**Curve (13):** enable, kind (0 linear, 1 relaxed, 2 aggressive, 3 custom),
curvature (0–100), then five (input, output) byte pairs. Each byte is
0–255 scaled between the dead-zone bounds. For kinds 0–2 the middle three
points are a cubic Bézier sampled at ¼, ½ and ¾ (see `curve.rs`).

**Trigger (32):** turbo enable, turbo rate, dead zone (9), remap enable,
key ×3, toggle, hair trigger (bit 7 on, bit 0 mode 1, bit 1 mode 2), hair
start, hair end, curve (13).

**Stick (36):** enable, square output, dead zone (9), curve (13), map:
enable, flip X, flip Y, axis ratio, mouse speed, output (1 left stick,
2 right stick, 3 four keys, 4 mouse), cross, up, down, left, right, centre
key.

**Motion (41):** activation (0 off, 1 hold button, 2 toggle by button,
3 always), trigger key ×3 (255 = none), active axis (bit 0 roll, bit 1 yaw,
both = combined), dead zone (9), curve (13), map: enable, flip X, flip Y,
flip Z, axis ratio, mouse speed, output, cross, up, down, left, right,
centre, stick sensitivity. Only one of aim/tilt is active at a time.

## Lighting slot (165 bytes)

Five presets of 32 bytes, then five global bytes.

**Preset:** mode (0 off, 1 static, 2 breathing, 3 rainbow, 4 phantom,
5 radar), brightness for static/breathing/phantom/rainbow/radar, speed for
breathing/phantom/rainbow/radar, phantom hue, rainbow hue, then five zones
(lower left, upper left, upper right, lower right, home) as hue (u16,
0–360), saturation, value (0–100).

**Global:** flags (bit 0 wake on lift), standby time, dock LED mode, dock
LED brightness, report rate.

## State report (`0x12`)

| byte | content |
|------|---------|
| 1–4  | LX LY RX RY high bytes (low bytes at 26–29) |
| 5    | low nibble: hat (0 up, clockwise to 7; 8+ centred); bits 4–7: X A B Y |
| 6    | LB RB LT RT View Menu LS RS (bits 0–7) |
| 7    | bit 0 Home, bit 1 Screenshot, bit 3 L4, bit 4 R4, bit 5 M |
| 8, 9 | LT, RT analog |
| 10   | bit 0 L5, bit 1 R5, bit 2 L6, bit 3 R6 |
| 31   | calibration mode (1 = not calibrating) |
| 32   | calibration progress bits (same order as the part mask) |
| 35   | charge state |
| 36   | battery % |
| 37   | active lighting preset |
| 38–52| current RGB of the five zones |
| 53   | key being recorded as a macro (0 = not recording) |
| 53–63| same layout as bytes 0–10, before remapping (physical buttons) |

## Key codes

The full table is in [`keys_table.rs`](../../crates/sirclient-core/src/protocol/g7pro/keys_table.rs),
generated from the official app. Highlights: 1–4 D-pad, 5/6 LB/RB, 7/8
LS/RS click, 9–12 A B X Y, 13 Home, 14 View, 15 Menu, 16 Screenshot,
17/18 L4/R4, 19/20 LT/RT, 21–28 stick directions, 31/32 L5/R5, 50–155
keyboard, 200–206 mouse, 230 mute, 231 Shift, 255 none.

## Bluetooth

GameSir Connect never talks to a controller over Bluetooth: its device
table only lists USB product IDs. Whether the G7 Pro exposes the `0xFFF0`
collection over Bluetooth is unknown. The **HID devices & Bluetooth** page
lists every collection with its bus, and **Probe config** sends `0F F2 00`
and `0F 09` and reports whether a firmware reply comes back.

# Supported devices

All GameSir controllers use USB vendor ID `0x3537`. This table comes from
GameSir Connect 1.14.5 and matches [`registry.rs`](../crates/sirclient-core/src/registry.rs).

| Model | Protocol family | Status | Connection | Product IDs | Config usage page |
|---|---|---|---|---|---|
| Nova 2 Lite | T4N2 | Detected; vendor DLL protocol, needs research | Wired | `0x100F` | `0xFFFF` |
| Nova 2 Lite | T4N2 | Detected; vendor DLL protocol, needs research | Wireless | `0x1098` | `0xFFFF` |
| NOVA VISION | N2S | Detected; vendor DLL protocol, needs research | Wired | `0x1141` | `0xFFFF` |
| NOVA VISION | N2S | Detected; vendor DLL protocol, needs research | Wireless | `0x10BD` | `0xFFFF` |
| Tarantula CE | T3CE | Detected; protocol not ported yet | Wired | `0x090D`, `0x103D` | `0xFFF0` |
| G7 Pro 8K | G7Pro | **Supported** | Wired | `0x1032`, `0x1034`, `0x10B7`, `0x10C5`, `0x10C7`, `0x10C9`, `0x10CB`, `0x10CD`, `0x10FB`, `0x10FD` | `0xFFF0` |
| G7 Pro 8K | G7Pro | **Supported** | Wireless | `0x1033`, `0x1035`, `0x10B9`, `0x10C6`, `0x10C8`, `0x10CA`, `0x10CC`, `0x10CE`, `0x10FC`, `0x10FE` | `0xFFF0` |
| K2 8K | K2 | Detected; protocol not ported yet | Wired | `0x108E` | `0xFFF0` |
| K2 8K | K2 | Detected; protocol not ported yet | Wireless | `0x109F` | `0xFFF0` |
| CYCLONE 2 | C2 | Detected; protocol not ported yet | Wired | `0x101D`, `0x1053` | `0xFFF0` |
| CYCLONE 2 | C2 | Detected; protocol not ported yet | Wireless | `0x100B`, `0x102A` | `0xFFF0` |
| Tarantula Pro | T3Pro | Detected; vendor DLL protocol, needs research | Wired | `0x1050` | `0xFFFF` |
| Tarantula Pro | T3Pro | Detected; vendor DLL protocol, needs research | Wireless | `0x103E` | `0xFFF0` |
| NovaPro | T4NPro | Detected; vendor DLL protocol, needs research | Wired | `0x100E` | `0xFFFF` |
| NovaPro | T4NPro | Detected; vendor DLL protocol, needs research | Wireless | `0x103E` | `0xFFF0` |
| Tegenaria Lite | T3Lite | Detected; protocol not ported yet | Wired | `0x1093` | `0xFFF0` |
| T3XS | T3XS | Detected; vendor DLL protocol, needs research | Wired | `0x105A` | `0xFFFF` |
| T3XS | T3XS | Detected; vendor DLL protocol, needs research | Wireless | `0x10BE` | `0xFFFF` |

**G7 Pro, `0x1022`:** not in GameSir Connect. Seen on Linux, where the
controller enumerates as a plain HID gamepad (interface 0) plus a keyboard,
media-key, mouse and `0xFFF0` configuration collection sharing interface 1.
SirClient treats it as a wired G7 Pro and filters that shared stream by the
configuration collection's report IDs.

Several product IDs per model are colour or collaboration editions
(Gold, Crown, Nioh, NBA 2K and so on). Their protocol is identical.

## Porting another model

1. Find the model's `Proxy` class in GameSir Connect's `main.js` (the family
   names above are the proxy names the official app uses for each model).
2. Add a module under `crates/sirclient-core/src/protocol/<family>/` with the
   command builders, reply parser and profile layout, plus round-trip tests.
3. Mark the family as implemented in `Protocol::is_implemented` and add pages
   or reuse the G7 Pro pages where the layout matches.

Models marked "vendor DLL" are driven by closed Windows libraries
(`sapjoyuniapis_*.dll`) in the official app, so their protocol has to be
recovered from those DLLs or from USB captures instead.

The G7 SE/HE and the dongles have no configuration interface in GameSir
Connect; the app only updates their firmware.


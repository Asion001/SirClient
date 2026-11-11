# SirClient — English (United States). This is the reference locale:
# every other locale falls back to it for messages it does not define.
# See docs/TRANSLATING.md.

# Shown in the language picker, in the language itself.
language-name = English

## App shell

app-unofficial = Unofficial, open source
sidebar-devices = Devices
sidebar-configure = Configure
sidebar-language = Language
sidebar-advanced = Advanced
nav-overview = Overview
nav-buttons = Buttons & macros
nav-sticks = Sticks, triggers & motion
nav-lighting = Lighting
nav-devices = HID devices & Bluetooth

device-demo = Demo device
device-demo-sub = Simulated G7 Pro, no hardware
device-none = No GameSir controller found. Connect it with a cable or its 2.4 GHz dongle.
device-unsupported = Detected · not supported yet
device-unknown-sub = Unknown model { $id } · click to try the G7 Pro protocol
device-disconnect = Disconnect
conn-wired = Wired · USB
conn-wireless = Wireless · 2.4 GHz

no-device = No controller connected
loading = Reading settings from the controller…
loading-no-answer = The controller is not answering. It may use a different protocol, or another app may be using it. Try HID devices & Bluetooth → Probe config, or run `sirclient --diagnose` and attach the output to an issue.
hint-udev = On Linux, install the udev rule shipped with SirClient (70-sirclient.rules), then unplug and replug the controller.
empty-title = No controller connected
empty-body = Plug in a G7 Pro 8K with its cable or 2.4 GHz dongle, or pick Demo device in the sidebar to explore the app.

apply = Apply to controller
revert = Revert
unsaved-changes = { $count ->
    [one] 1 unsaved change
   *[other] { $count } unsaved changes
}
profile-n = Profile { $n }
profile-shift = Shift

toast-open-failed = Could not open the controller: { $error }
toast-disconnected = Controller disconnected
toast-lost = Lost connection: { $error }
toast-saved = Saved { $slot }
toast-changed-on-device = { $slot } was changed on the controller
toast-stick-center-failed = The controller rejected the stick centre change

## Overview

stat-connection = Connection
stat-connection-sub = All settings synced
stat-syncing = Syncing with the controller…
stat-firmware = Firmware
stat-dongle = Dongle { $version }
stat-battery = Battery
stat-charging = Charging
stat-polling = Polling rate

live-input = Live input
live-mapped = After remap
live-physical = Physical
live-waiting = Waiting for input…
left-stick = Left stick
right-stick = Right stick

vibration = Vibration
motor-left = Left grip
motor-right = Right grip
motor-lt = Left trigger
motor-rt = Right trigger
level-off = Off
test-rumble = Test rumble
calibrate = Calibrate…
calibrate-help = Choose what to calibrate, move it through its full range, then press Finish.
calibrate-active = Calibrating
calibrate-idle = Not calibrating
calibrate-sticks = Start: sticks
calibrate-triggers = Start: triggers
calibrate-motion = Start: gyro
calibrate-finish = Finish
calibrate-exit = Cancel

shortcuts-title = Controller shortcuts
shortcuts-sub = Work on the controller itself, even without SirClient running
shortcut-unknown = Combo unknown
shortcut-not-documented = Not listed in GameSir Connect; check the G7 Pro manual.
shortcut-profile = Switch profile
shortcut-remap = Remap a button on the controller
shortcut-shift = Hold to use the Shift layer
shortcut-shift-note = Assign Shift to a button under Buttons & macros first.
shortcut-1000hz = Fall back to 1000 Hz
shortcut-1000hz-note = If the PC stops detecting the controller at 4000 or 8000 Hz.
shortcut-light = Switch lighting preset
shortcut-light-note = From GameSir Connect; confirm on the G7 Pro.

## Buttons & macros

remap = Remap
swap-abxy = Swap A/B and X/Y
col-button = Button
col-sends = Sends
col-turbo = Turbo
col-toggle = Toggle
map-default = Default
map-add-key = Add a key pressed at the same time
turbo-off = Off
keys-gamepad = Controller
keys-keyboard = Keyboard
keys-mouse = Mouse
keys-special = Special

back-buttons = Back buttons
action-single = Single key
action-macro = Macro
macro-key = Key
macro-hold = Hold
macro-wait = Then wait
macro-remove = Remove step
macro-add = Add step
macro-repeat = Repeat while held
macro-cycle = Pause between repeats
macro-on-press = Trigger on press
macro-used = { $used } of { $max } steps used

## Sticks, triggers & motion

tab-sticks = Sticks
tab-triggers = Triggers
tab-motion = Motion
square-output = Square output (raw)
curve-linear = Linear
curve-relaxed = Relaxed
curve-aggressive = Aggressive
curve-custom = Custom
curve-strength = Curve strength
curve-input = input
curve-output = output
dz-inner = Inner dead zone
dz-outer = Outer dead zone
dz-anti = Anti dead zone
anti-jitter = Anti-jitter
acts-as = Acts as
out-left = Left stick
out-right = Right stick
out-keys = Four keys
out-mouse = Mouse
invert-x = Invert X
invert-y = Invert Y
mouse-speed = Mouse speed

hair-trigger = Hair trigger
hair-mode-1 = Instant press
hair-mode-2 = Rapid re-press
hair-start = Starts at
hair-end = Full at

motion-aim = Gyro aim
motion-tilt = Tilt
motion-activation = Activation
motion-off = Off
motion-hold = While a button is held
motion-toggle = Toggled by a button
motion-always = Always on
motion-button = Button
motion-axis = Axis
axis-yaw = Yaw
axis-roll = Roll
axis-both = Both
motion-sensitivity = Sensitivity
motion-note = Only one motion mode can be active at a time.

## Lighting

lighting-subtitle = Shared by all profiles
preset-n = Preset { $n }
zones = Zones
zones-same = Use the first colour everywhere
zone-lower-left = Lower left
zone-upper-left = Upper left
zone-upper-right = Upper right
zone-lower-right = Lower right
zone-home = Home button
effect = Effect
light-off = Off
light-static = Static
light-breathing = Breathing
light-rainbow = Rainbow
light-phantom = Phantom
light-radar = Radar
brightness = Brightness
speed = Speed
all-presets = All presets
wake-on-lift = Wake lights when the controller is lifted
standby-time = Lights off after (raw)
live-colors = Now on the controller

## HID devices & Bluetooth

devices-subtitle = Every HID collection on this computer. Use it to see what a controller exposes over each connection.
devices-controllers-only = Controllers only
devices-rescan = Rescan
devices-technical = Technical details
devices-none = No matching HID devices.
col-device = Device
col-bus = Bus
col-role = Role
role-config = Configuration
role-config-unsupported = Configuration (not supported yet)
role-joystick = Joystick
role-gamepad = Gamepad
role-keyboard = Keyboard
role-mouse = Mouse
role-media = Media keys
role-vendor = Vendor-defined
probe-listen = Listen
probe-handshake = Probe config
probe-title = Probe
probe-help = Listen records the reports a collection sends. Probe config also sends the G7 firmware-version request; an answer means the collection accepts configuration commands. Use it on the Bluetooth entries to check whether the controller can be configured wirelessly.
probe-of = Probe: { $device }
probe-running = Listening for 1.5 seconds…
probe-listened = Captured { $count } distinct reports.
probe-ok = The controller answered with firmware { $version }, so it can be configured over this connection.
probe-no-answer = No answer. This connection does not accept configuration commands.
probe-empty = (nothing received)
probe-raw = Raw reports ({ $count })
probe-error = Could not open the device: { $error }

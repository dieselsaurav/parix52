# Vendored RMK

Upstream: https://github.com/rmk-rs/rmk at the commit in `UPSTREAM_REV`
(`775a767`, 2026-10-06; rmk 0.9.0 plus the unreleased 0.10 changes). The
crates `rmk`, `rmk-macro`, `rmk-config` and `rmk-types` are copied here and
used as path dependencies. Every change against upstream is listed below and
marked `PARIX PATCH` in the source where it is a change rather than an
addition. Regenerate the list with

    diff -rq <upstream>/rmk/src vendor/rmk/rmk/src   (and the other three crates)

## Display bus cannot stall the keys

`rmk/src/display/mod.rs`, `rmk-macro/src/codegen/display.rs`.

The nRF I2C driver has no timeout and the display task subscribes to key
events. With no display module fitted (the module carries the pull-ups) or a
wedged bus, a transfer never ends, the key queue fills and the matrix stops
publishing. Internal pull-ups on SDA and SCL; `init` and `flush` run under a
250 ms timeout; after a timeout the panel is left alone for 5 s. Proven on
hardware on the first base (keys froze after eight presses without it).

## Fixed Bluetooth addresses from keyboard.toml on nRF

`rmk-macro/src/codegen/chip/chip_init.rs`, `rmk-macro/src/codegen/split/central.rs`,
`rmk/src/split/ble/central.rs`.

Upstream derives every nRF address from FICR and ignores `ble_addr`. Here a
configured address wins, for both halves, and the central seeds its
peripheral slot from the right half's configured address: it never has to
scan for "any RMK peripheral with id 0" (which could be another keyboard),
and a connect timeout falls back to the configured address instead of
clearing the slot.

## A key on the right half wakes the right half

`rmk/src/split/ble/peripheral.rs`.

The central tells the peripheral to sleep and only the central can tell it to
wake. If the central was switched off while the peripheral slept, its lights
and display stayed dark through every key press until a central reconnected.
A local key press now clears the peripheral's own sleep state.

## Cirque Pinnacle trackpad, wheel axis, automatic mouse layer

`rmk/src/input_device/pinnacle.rs` (new), `rmk-macro/src/codegen/input_device/pinnacle.rs`
(new), `rmk-config/src/lib.rs`, `rmk-config/src/resolved/{hardware.rs,build_constants.rs}`,
`rmk-macro/src/codegen/{chip/bind_interrupt.rs,input_device/mod.rs,split/peripheral.rs}`,
`rmk/src/input_device/{mod.rs,pointing.rs,pmw3610.rs,pmw33xx.rs}`.

See `PINNACLE_PATCH.md`. The pointing path gains a wheel axis (a driver's
`take_wheel`, accumulated by `PointingDevice`, carried on the event's Z axis
and sent as a wheel report), an automatic layer held for a timeout after any
motion (`auto_layer`, `auto_layer_timeout`), and mouse reports that are
dropped rather than queued when the report channel is full, so a streaming
pointer can never delay a key release.

## Switched VCC rail while asleep

`rmk/src/display/mod.rs`, `rmk/src/input_device/pinnacle.rs`,
`rmk/src/split/ble/central.rs`.

`src/rgb.rs` turns the module's VCC output off (P0.13) while the keyboard
sleeps, which is the only way to stop the LEDs' standby current. The OLED and
the trackpad share that rail: the display draws nothing while asleep and
initialises the panel again on waking; the Pinnacle driver parks chip select
low for the sleep and reconfigures the pad on waking. The split link's
peripheral latency while asleep is 25 (0.5 s) instead of 200 (4 s) so the
right half hears the wake-up promptly.

## Dropped since the first base (b982049)

Upstream now carries its own version of: the PHY/connection-parameter retry
loops that sleep and give up, rebooting on a BLE runner error, a modifiers-
only `WM(No, ...)` key held as a modifier (upstream registers it like a
modifier action), the stranded tap-hold key (the tap-hold engine was
rewritten), the per-page erase counter in nrf-mpsl. The whitelist-advertising
patch (profile stealing between two bonded hosts) is not carried over; the
keyboard is used with one computer at a time. The storage capacity check and
the bootloader-jump watchdog guard are gone with the Bootloader key.

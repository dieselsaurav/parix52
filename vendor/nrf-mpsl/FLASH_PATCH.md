# nrf-mpsl flash timeslot driver, patched

Vendored from haobogu/nrf-sdc rev ffe59a7, `nrf-mpsl` only; `nrf-mpsl-sys`
(the Nordic binaries) stays a git dependency at the same rev.

## Why

On the Parix 52 central every runtime flash write -- RMK saving the host's
notification subscriptions on each Bluetooth connection, a Vial edit, a bond
-- ended in a watchdog reset about ten seconds after the write. The driver
runs flash work inside MPSL timeslots. It asked for an 8.5 ms slot at normal
priority for a write and an 11 ms one for a partial erase, with 1 ms of slack.
A keyboard holding two 7.5 ms connection intervals plus advertising never has
an 8.5 ms gap, so every request was refused and fell into the timeslot
callback's retry path, which `assert!`s on the re-request result, and an
overstayed slot `panic!`s. Both run in interrupt context: a hard fault, then
the watchdog.

This went unnoticed because RMK's default 2-sector store is full after the
first boot on any large keymap, so runtime writes fail early and never reach
the driver. Give the store room and the driver starts being used.

## What changed (src/flash.rs)

- Write slot 1.5 ms (was 7.5), erase in 2 ms partials (was 10), normal
  priority request timeout 100 ms (was 30). Work now fits between connection
  events without pre-empting the radio.
- The BLOCKED/CANCELLED re-request no longer asserts; an error is handed to
  the waiting task. An OVERSTAYED slot reports an error instead of panicking.

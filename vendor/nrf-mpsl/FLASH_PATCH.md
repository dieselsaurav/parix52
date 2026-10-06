# nrf-mpsl flash timeslot driver, patched

`nrf-mpsl` 0.4.0 from crates.io, copied here and used through
`[patch.crates-io]`. Only `src/flash.rs` is changed; `nrf-mpsl-sys` (Nordic's
binaries) stays the crates.io dependency.

## Why

On the Parix 52 central every runtime flash write (RMK saving the host's
notification subscriptions on each Bluetooth connection, a Vial edit, a bond)
ended in a watchdog reset about ten seconds after the write. The driver runs
flash work inside MPSL timeslots. It asked for an 8.5 ms slot at normal
priority for a write and an 11 ms one for a partial erase, with 1 ms of slack.
A keyboard holding two 7.5 ms connection intervals plus advertising never has
an 8.5 ms gap, so every request was refused and fell into the timeslot
callback's retry path, which ends in a panic on an overstayed slot. A panic in
interrupt context is a hard fault, then the watchdog.

## What changed (src/flash.rs)

- Write slot 1.5 ms (was 7.5), erase in 2 ms partials (was 10), normal
  priority request timeout 100 ms (was 30). Work fits between connection
  events without pre-empting the radio.
- OVERSTAYED reports an error to the waiting task instead of panicking. (The
  BLOCKED/CANCELLED path already reports an error in 0.4.0, and the per-page
  elapsed counter is reset upstream now; both were patches on the 0.3 base.)

Proven on the first base (2026-09-25): the reboot-on-write stopped with these
slot sizes. Not re-measured on 0.4.0.

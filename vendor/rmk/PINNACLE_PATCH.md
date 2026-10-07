# Cirque Pinnacle trackpad driver (added, not upstream)

Vendored RMK gains a `pinnacle` input device: Cirque GlidePoint circle
trackpads (TM040040 / TM035035) over hardware SPI in relative mode. Upstream
RMK (checked 2026-09-30) has no Cirque driver.

- `rmk/src/input_device/pinnacle.rs` -- driver: RAP framing (read 0xA0|addr +
  fillers, write 0x80|addr), firmware-id check, reset, sleep enable, taps and
  scroll disabled at the pad, curved-overlay tuning through ERA (ADC
  attenuation 2x, wide-Z minimums 0x04/0x03) as in Cirque's example, 4-byte
  relative packet with PS/2-style sign bits. `ActiveHigh` wraps the DR pin
  because `PointingDevice` waits for an active-low motion pin.
- `rmk-config/src/lib.rs` -- `PinnacleConfig`, `input_device.pinnacle`.
- `rmk-macro/src/codegen/input_device/pinnacle.rs` -- device + processor
  expansion (nRF52 SPIM, mode 1, 2 MHz), SPIM interrupt lines; hooked into
  `input_device/mod.rs`, `chip/bind_interrupt.rs`, `split/peripheral.rs`
  exactly where pmw33xx is.

TOML:

    [[split.peripheral.input_device.pinnacle]]
    name = "trackpad"
    curved_overlay = true      # -303 overlay
    # dr = "P0_09"             # optional data-ready pin; polls without it
    proc_invert_y = true       # direction unverified on hardware
    [split.peripheral.input_device.pinnacle.spi]
    instance = "TWISPI0"
    sck = "P0_20"; mosi = "P0_17"; miso = "P1_06"; cs = "P0_06"

Working on hardware since 2026-10-01 (DR pin, absolute mode with scroll ring, auto mouse layer); re-ported onto upstream 775a767 on 2026-10-06.

## Gestures (absolute mode)

Decided once per touch, in `absolute_to_motion`:

- **Tap** (`tap`, `tap_term_ms`): a touch no longer than the term that stays
  within about 1.5 mm of where it landed is a left click, sent as its own
  pointing event (`Axis::Button`, a button mask) and turned into a press and a
  release by the pointing processor.
- **Scroll ring** (`ring_width_percent`): a touch that lands in the outer band
  is undecided, and the cursor held still, until it has travelled an eighth of
  the radius. Travel within 50 degrees of the radial direction makes it a
  cursor move; anything more tangential makes it a scroll. The numbers are
  QMK's (`cirque_pinnacle_gestures.c`: trigger 16 of 128, 50 degrees); QMK's
  ring is a third of the radius, this one a fifth.
- **Glide** (`glide`, `glide_friction`, `glide_trigger`): if a cursor move ends
  with the finger still moving faster than the trigger, the driver keeps
  producing motion after the lift, decelerating at a constant rate
  (`p = v0 t - friction t^2 / 2`, one step per 10 ms), as QMK's cursor glide
  does, with QMK's defaults (friction 0.4, trigger 10). The starting velocity
  is the cursor speed smoothed over the last packets. While a glide runs the
  device is polled on the timer instead of waiting for the data-ready pin; a
  new touch ends it.

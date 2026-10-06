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

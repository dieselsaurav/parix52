# parix52-rmk

[RMK](https://github.com/HaoboGu/rmk) (Rust) firmware for the **Parix 52** — a 52-key
Voyager-spaced wireless Choc split. nRF52840 Pro Micro (nice!nano v2 or SuperMini), BLE
split, SSD1306 OLEDs, 26 per-key SK6812MINI-E per half, battery reporting, Vial.

Hardware lives in [dieselsaurav/paraboard](https://github.com/dieselsaurav/paraboard) —
`paraboard` is the design source, Parix 52 is the keyboard. Ported from
[parix-rmk-corne](https://github.com/dieselsaurav/parix-rmk-corne), which carries the
same MCU, displays and RGB approach; what changed is the matrix, the pins and the keymap.

## Hardware map

Read off the fabbed boards' netlist, not from the schematic's intent.

| | Pro Micro | nRF52840 |
|---|---|---|
| Rows R0–R4 | 4, 5, 6, 7, 8 | P0.22, P0.24, P1.00, P0.11, P1.04 |
| Columns C0–C5 | 21, 20, 19, 18, 15, 14 | P0.31, P0.29, P0.02, P1.15, P1.13, P1.11 |
| RGB data in | 0 | **P0.08** (the Corne uses P0.06) |
| OLED SDA / SCL | 2, 3 | P0.17, P0.20 |

5 rows × 6 columns per half, COL2ROW. **C0 is the outer column on both halves**, so the
right half lists its column pins in reverse and the combined 5 × 12 matrix reads left to
right. Row 4 is the thumb row and uses only two columns: the near-grid thumb is on **C5**,
the far 1.5u thumb on **C4**. Left half = central.

## Layout

The Corne's seven Miryoku-style layers, carried over with its home-row mods and all 14
combos intact — rows 1–3 here are the Corne's three rows token for token, which is what
keeps the combos bound. Two things had to change:

- **A number row on top** (row 0). `F1`–`F12` sit on it in the FUN layer.
- **Two thumbs a side instead of three.** Kept: `Space`=NAV and `Escape`=MEDIA on the
  left, `Backspace`=NUM and `Enter`=SYM on the right. MEDIA had to survive, since every
  Bluetooth key lives there. The two layer-taps that lost their thumbs moved to the number
  row's outer corners: hold **`` ` ``** for MOUSE, hold **`-`** for FUN, each on the hand
  opposite the layer it opens.

This is a starting keymap, not a considered one — it is editable live in
[Vial](https://vial.rocks). Unlock by holding the two left thumb keys. BLE keys on MEDIA:
User0-3 = profile BT0-BT3, User6 = clear bond, User7 = toggle USB/BLE output.

The keymap SVG/HTML renderer in `scripts/keymap_docs.py` still assumes the Corne's
geometry and has **not** been ported.

## Patched RMK (vendor/)

`vendor/rmk` is upstream RMK at rev `b982049`, pristine except for the
**per-profile whitelist-advertising patch** (see `vendor/rmk/YUYUDHAN_PATCH.md`):

- a bonded profile advertises *filtered* — other computers can't steal the
  connection slot and wedge the keyboard on the wrong LTK;
- filtered advertising is gated on split-link health, so the right half can
  always reconnect (stock accept-list contention starves the split link).

Wired via `[patch."https://github.com/HaoboGu/rmk"]` in `Cargo.toml`.

## Displays

- **Left (central):** ZMK-built-in-style status — output (USB/BT profile),
  battery gauge, layer name. Event-driven — zero idle cost.
- **Right (peripheral):** Parix screen — glitching P keycap logo (byte-exact
  port of the ZMK `custom_status_screen.c`), split-link ✓/✗, battery.

If content is upside-down for your mount, flip `rotation = 0` → `180` in
`config/keyboard.toml`.

## RGB

RMK has no native WS2812/SK6812 support yet, so `src/rgb.rs` drives the 26-LED chain per
half (one under every key, no underglow, data on P0.08) from the nRF52840 PWM engine: a
dim static color per active layer, off while sleeping, and TG(7) (DISPOFF) as the kill
switch. The LEDs hang off VCC with no power mosfet, so they idle at roughly 0.5–1 mA each
even when dark — about 13–26 mA per half. Real "off" is the power switch.

## Building

Cloud: push to GitHub — CI uploads `parix52-rmk-central.uf2` /
`parix52-rmk-peripheral.uf2`.

Local:

```sh
rustup target add thumbv7em-none-eabihf
cargo install cargo-make
cargo make uf2          # outputs build/parix52-rmk-{central,peripheral}.uf2
```

## Settings reset (ZMK `settings_reset` equivalent)

`parix52-rmk-reset.uf2` (built alongside the other two by `cargo make uf2`)
is a single role-free binary, safe on **either half** — just like ZMK's
`settings_reset` shield. It erases RMK's storage region (0xA0000–0xC0000:
BLE host bonds, split-pairing state, Vial remaps, selected profile) and
signals on the nice!nano's blue LED: slow blink = erased, fast blink =
failed. Flash it, wait for the slow blink, then flash the normal firmware
back. For host-pairing trouble alone, try **User6** on MEDIA first — it
clears just the current profile's bond at runtime.

If you ever override `start_addr` / `num_sectors` under `[storage]`, update
the constants in `src/reset.rs` to match.

## Flashing

1. Double-tap reset on the **left** half → drag `parix52-rmk-central.uf2` onto the `NICENANO` drive.
2. Same on the **right** half with `parix52-rmk-peripheral.uf2`.

Reflash the **central** for keymap changes; the peripheral only needs
reflashing when firmware code changes.

## ⚠️ Going back to ZMK

RMK ≥ 0.7 replaces the Nordic SoftDevice BLE stack. **To return to ZMK you
must first re-flash the
[nice!nano bootloader](https://nicekeyboards.com/docs/nice-nano/troubleshooting#my-nicenano-seems-to-be-acting-up-and-i-want-to-re-flash-the-bootloader)**,
then flash ZMK as usual.

## Not ported (RMK gaps)

- **RGB underglow keycodes** (`rgb_ug` TOG/EFF/HUI/SAI) — replaced by the
  layer-color driver above; TG(7) doubles as the RGB toggle.
- **nice!view** — unsupported; this port drives the SSD1306 OLEDs. With
  nice!views fitted the keyboard works but screens stay blank.
- **`ext_power` toggle, ZMK-style deep sleep, ZMK Studio** — not in RMK
  (Vial replaces Studio for live keymap editing).

#![no_main]
#![no_std]

use rmk::macros::rmk_peripheral;

mod bitmaps;
mod layer_names;
// The right OLED gave way to the trackpad; the Parix screen stays for a
// board that fits one again.
#[allow(dead_code)]
mod parix;
mod rgb;
mod rgb_map;

#[rmk_peripheral(id = 0)]
mod keyboard_peripheral {
    use crate::rgb::RgbProcessor;
    use crate::rgb_map::Side;

    /// SK6812MINI-E chain on the right half (26 LEDs, data on P0.08).
    #[register_processor]
    fn rgb_underglow() -> RgbProcessor {
        let mut rgb = RgbProcessor::new(p.PWM0, p.P0_08, p.P0_13, Side::Right);
        rgb.show().await; // paint the BASE colors at boot
        rgb
    }
}

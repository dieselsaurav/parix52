#![no_main]
#![no_std]

use rmk::macros::rmk_central;

mod bitmaps;
mod layer_names;
mod rgb;
mod rgb_map;
mod status;

#[rmk_central]
mod keyboard_central {
    use crate::rgb::RgbProcessor;
    use crate::rgb_map::Side;

    /// SK6812MINI-E chain on the left half (26 LEDs, data on P0.08).
    #[register_processor]
    fn rgb_underglow() -> RgbProcessor {
        let mut rgb = RgbProcessor::new(p.PWM0, p.P0_08, p.P0_13, Side::Left);
        rgb.show().await; // paint the BASE colors at boot
        rgb
    }
}

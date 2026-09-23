#![no_main]
#![no_std]

use rmk::macros::rmk_peripheral;

mod bitmaps;
mod layer_names;
mod parix;
mod rgb;

#[rmk_peripheral(id = 0)]
mod keyboard_peripheral {
    use crate::rgb::RgbProcessor;

    /// SK6812MINI-E chain on the right half (26 LEDs, data on P0.08).
    #[register_processor(event)]
    fn rgb_underglow() -> RgbProcessor {
        let mut rgb = RgbProcessor::new(p.PWM0, p.P0_08);
        rgb.show().await; // paint the BASE color at boot
        rgb
    }
}

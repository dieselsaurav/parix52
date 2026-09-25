#![no_main]
#![no_std]

use rmk::macros::rmk_central;

mod bitmaps;
mod crash;
mod layer_names;
mod rgb;
mod status;

#[rmk_central]
mod keyboard_central {
    use crate::rgb::RgbProcessor;

    /// SK6812MINI-E chain on the left half (26 LEDs, data on P0.08).
    #[register_processor(event)]
    fn rgb_underglow() -> RgbProcessor {
        // The crash reporter rides on this constructor because it is
        // inlined into main, where `spawner` is in scope.
        if let Ok(task) = crate::crash::report_task() {
            spawner.spawn(task);
        }
        let mut rgb = RgbProcessor::new(p.PWM0, p.P0_08);
        rgb.show().await; // paint the BASE color at boot
        rgb
    }
}

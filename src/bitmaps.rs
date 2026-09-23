//! Shared SSD1306 page-format bitmap helper used by both halves' OLED
//! renderers.
//!
//! Each binary (`central`, `peripheral`) declares `mod bitmaps;` in its entry
//! point so this module is compiled into both.

use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;

/// Draw an SSD1306 page-format bitmap onto an embedded-graphics DrawTarget.
///
/// `data[page * cols + col]`, bit `b` → pixel at
/// `(col + offset_x, page*8 + b + offset_y)`.  LSB (bit 0) = top row of the
/// page.  Copied verbatim from rmk `display/renderers/logo.rs` (private there).
#[allow(dead_code)] // used by the peripheral (parix.rs) build only
pub fn draw_page_format_frame<D: DrawTarget<Color = BinaryColor>>(
    display: &mut D,
    data: &[u8],
    cols: usize,
    offset_x: i32,
    offset_y: i32,
) {
    let pages = data.len() / cols;
    for page in 0..pages {
        for col in 0..cols {
            let byte = data[page * cols + col];
            if byte == 0 {
                continue;
            }
            for bit in 0..8u32 {
                if byte & (1 << bit) != 0 {
                    let x = col as i32 + offset_x;
                    let y = page as i32 * 8 + bit as i32 + offset_y;
                    Pixel(Point::new(x, y), BinaryColor::On).draw(display).ok();
                }
            }
        }
    }
}

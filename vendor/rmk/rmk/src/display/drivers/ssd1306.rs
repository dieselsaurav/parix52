//! [`DisplayDriver`] implementation for SSD1306 OLED displays.

use display_interface::AsyncWriteOnlyDataCommand;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use ssd1306::mode::{BufferedGraphicsModeAsync, DisplayConfigAsync};
use ssd1306::size::DisplaySizeAsync;

use super::super::DisplayDriver;

impl<DI, SIZE> DisplayDriver for ssd1306::Ssd1306Async<DI, SIZE, BufferedGraphicsModeAsync<SIZE>>
where
    DI: AsyncWriteOnlyDataCommand,
    SIZE: DisplaySizeAsync,
    Self: DrawTarget<Color = BinaryColor> + DisplayConfigAsync,
{
    async fn init(&mut self) {
        DisplayConfigAsync::init(self).await.ok();
        // PARIX PATCH: the panel's own default is its "normal" level, which
        // is harsh on a desk at night. Applied on every init, since the panel
        // is initialised again each time its supply returns from a sleep.
        self.apply_brightness().await;
    }

    async fn apply_brightness(&mut self) {
        let level = match super::super::display_brightness() {
            0 => ssd1306::prelude::Brightness::DIMMEST,
            1 => ssd1306::prelude::Brightness::DIM,
            2 => ssd1306::prelude::Brightness::NORMAL,
            3 => ssd1306::prelude::Brightness::BRIGHT,
            _ => ssd1306::prelude::Brightness::BRIGHTEST,
        };
        self.set_brightness(level).await.ok();
    }

    async fn flush(&mut self) {
        ssd1306::Ssd1306Async::flush(self).await.ok();
    }
}

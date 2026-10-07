//! The left half's OLED: takes RMK's render context and hands the drawing
//! (`status_view`) what it shows. 128 x 32 SSD1306, landscape. Event-driven,
//! no `render_interval`: the screen is redrawn only when something on it
//! changes. If it reads upside-down for your mount, set `rotation = 180` in
//! config/keyboard.toml.

use core::fmt::Write as _;

use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use rmk::display::{DisplayRenderer, RenderContext};
use rmk::heapless::String;
use rmk::types::battery::{BatteryStatus, ChargeState};
use rmk::types::ble::BleState;
use rmk::types::connection::ConnectionType;

use crate::layer_names::{DISPLAY_OFF_LAYER, LAYER_NAMES};
use crate::status_view::{Active, Bt, View, draw};

/// True while USB power (VBUS) is present on this half.
fn usb_powered() -> bool {
    embassy_nrf::pac::POWER.usbregstatus().read().vbusdetect()
}

fn percent(status: &BatteryStatus) -> Option<u8> {
    match status {
        BatteryStatus::Available { level, .. } => *level,
        BatteryStatus::Unavailable => None,
    }
}

fn charging(status: &BatteryStatus) -> bool {
    matches!(
        status,
        BatteryStatus::Available {
            charge_state: ChargeState::Charging,
            ..
        }
    )
}

/// Panel brightness until the brightness keys (Space + 9 / 0) are first
/// used, 0 (dimmest) to 4 (brightest); after that the level they left is
/// remembered. The panel's own default is 2, which is glaring in a dim room.
/// The range of these small OLEDs is narrow: 0 is "noticeably dimmer", not
/// "faint".
const OLED_BRIGHTNESS: u8 = 0;

pub struct StatusRenderer;

impl Default for StatusRenderer {
    fn default() -> Self {
        // The renderer is built before the panel is first initialised.
        rmk::display::set_display_brightness(OLED_BRIGHTNESS);
        Self
    }
}

impl DisplayRenderer<BinaryColor> for StatusRenderer {
    fn render<D: DrawTarget<Color = BinaryColor>>(&mut self, ctx: &RenderContext, display: &mut D) {
        display.clear(BinaryColor::Off).ok();
        if ctx.sleeping || ctx.layer == DISPLAY_OFF_LAYER {
            return;
        }

        let mut layer_buf: String<8> = String::new();
        let layer: &str = match LAYER_NAMES.get(ctx.layer as usize) {
            Some(name) => name,
            None => {
                let _ = write!(layer_buf, "L{}", ctx.layer);
                &layer_buf
            }
        };

        let bt = match ctx.ble_status.state {
            BleState::Connected => Bt::On,
            _ if ctx.ble_status.bonded => Bt::Searching,
            _ => Bt::Unpaired,
        };
        let active = match ctx.active_output {
            Some(ConnectionType::Usb) => Active::Usb,
            Some(ConnectionType::Ble) => Active::Bluetooth,
            None => Active::Nowhere,
        };

        let m = ctx.modifiers;
        draw(
            display,
            &View {
                layer,
                usb: ctx.usb_connected,
                profile: ctx.ble_status.profile,
                bt,
                active,
                prefer_bluetooth: ctx.preferred_output == ConnectionType::Ble,
                left: percent(&ctx.battery),
                right: ctx.peripheral_batteries.first().and_then(|b| percent(b)),
                right_linked: ctx.peripherals_connected.first().copied().unwrap_or(false),
                // The left knows at once; the right says so with its battery level.
                left_charging: usb_powered() || charging(&ctx.battery),
                right_charging: ctx.peripheral_batteries.first().is_some_and(|b| charging(b)),
                cmd: m.left_gui() || m.right_gui(),
                opt: m.left_alt() || m.right_alt(),
                ctrl: m.left_ctrl() || m.right_ctrl(),
                shift: m.left_shift() || m.right_shift(),
                caps: ctx.caps_lock,
            },
        );
    }
}

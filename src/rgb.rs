//! WS2812 RGB driver — event processor driving the Parix 52's LED chain from the
//! nRF52840 PWM engine (DMA sequence at 800 kHz), since RMK has no native
//! WS2812/underglow support yet.
//!
//! Wiring (from the paraboard netlist, read off the fabbed boards): 26
//! SK6812MINI-E per half, one under every key and no underglow, fed from
//! VCC, data in on net RGB_DI = pro_micro D0 = P0.08, GRB wire order. The
//! SK6812 takes the same 800 kHz one-wire protocol as the WS2812. (GRB was
//! confirmed on the right half 2026-10-01: sent as RGB, letters came out
//! green and digits red. A green Q on the left was a chain fault, not order.)
//!
//! Behavior:
//!   - a colour per key for the active layer, from `rgb_map` (dim palette,
//!     battery first)
//!   - when the keyboard sleeps (RMK's idle sleep, `keyboard.toml`), the
//!     module's VCC output is switched off at P0.13, the way ZMK's ext_power
//!     does it. Every SK6812 draws 0.5-1 mA dark, about 15 mA a half, and
//!     only removing their supply stops that. The rail also feeds the OLED
//!     (left) and the trackpad (right); each re-initialises when it wakes.
//!   - layer 7 (DISPOFF, TG(7) on MEDIA) doubles as the RGB kill switch

use embassy_nrf::Peri;
use embassy_nrf::gpio::{Level, Output, OutputDrive, Pin as GpioPin};
use embassy_nrf::pwm::{
    Config, Instance, Prescaler, SequenceConfig, SequencePwm, SingleSequenceMode, SingleSequencer,
};
use embassy_time::{Duration, Timer};
use rmk::event::{LayerChangeEvent, SleepStateEvent};
use rmk::macros::processor;

use crate::rgb_map::{self, NUM_LEDS, Side};

/// PWM ticks (16 MHz) per WS2812 bit: 20 ticks = 1.25 us = 800 kHz.
const BIT_TICKS: u16 = 20;
/// Compare value for a WS2812 "0" bit (~0.375 us high). Bit 15 = polarity.
const DUTY_ZERO: u16 = 0x8000 | 6;
/// Compare value for a "1" bit (~0.75 us high). 12, not 13: the SK6812 allows
/// at most 0.75 us for a one, and 13 ticks (0.81 us) was just outside it.
const DUTY_ONE: u16 = 0x8000 | 12;
/// All-low slots appended for the >50 us WS2812 reset latch.
const RESET_SLOTS: usize = 64; // 80 us at 1.25 us/slot, the SK6812 minimum
const BUF_LEN: usize = NUM_LEDS * 24 + RESET_SLOTS;

/// Time the switched rail needs to come up before the LEDs take a frame
/// (ZMK's nice!nano ext-power uses the same 50 ms).
const RAIL_SETTLE: Duration = Duration::from_millis(50);
/// Wait between hearing the wake-up and switching the rail on.
const RAIL_ON_DELAY: Duration = Duration::from_millis(10);

#[processor(subscribe = [LayerChangeEvent, SleepStateEvent])]
pub struct RgbProcessor {
    pwm: SequencePwm<'static>,
    buf: [u16; BUF_LEN],
    side: Side,
    layer: u8,
    /// The keyboard is asleep: lights dark and the rail off.
    sleeping: bool,
    /// P0.13, high = the module's VCC output is on.
    rail: Output<'static>,
}

impl RgbProcessor {
    pub fn new(
        pwm: Peri<'static, impl Instance>,
        pin: Peri<'static, impl GpioPin>,
        rail: Peri<'static, impl GpioPin>,
        side: Side,
    ) -> Self {
        let mut config = Config::default();
        config.prescaler = Prescaler::Div1; // 16 MHz
        config.max_duty = BIT_TICKS;
        let pwm = SequencePwm::new_1ch(pwm, pin, config).expect("ws2812 pwm init");

        Self {
            pwm,
            buf: [0x8000; BUF_LEN],
            side,
            layer: 0,
            sleeping: false,
            rail: Output::new(rail, Level::High, OutputDrive::Standard),
        }
    }

    /// Encode each key's color into the PWM duty buffer (GRB wire order).
    fn fill_buffer(&mut self) {
        let mut i = 0;
        for led in 0..NUM_LEDS {
            let (r, g, b) = if self.sleeping {
                (0, 0, 0)
            } else {
                rgb_map::color(self.side, self.layer, led)
            };
            for byte in [g, r, b] {
                for bit in (0..8).rev() {
                    self.buf[i] = if (byte >> bit) & 1 == 1 { DUTY_ONE } else { DUTY_ZERO };
                    i += 1;
                }
            }
        }
        // Trailing slots stay at 0% duty for the reset latch.
        for slot in self.buf[NUM_LEDS * 24..].iter_mut() {
            *slot = 0x8000;
        }
    }

    /// Push the buffer out to the strip and wait for the DMA sequence to end
    /// (26 LEDs x 30 us + 50 us reset ~= 0.8 ms).
    pub async fn show(&mut self) {
        self.fill_buffer();
        let sequencer = SingleSequencer::new(&mut self.pwm, &self.buf, SequenceConfig::default());
        if sequencer.start(SingleSequenceMode::Times(1)).is_ok() {
            embassy_time::Timer::after_millis(2).await;
        }
    }

    async fn on_layer_change_event(&mut self, event: LayerChangeEvent) {
        if event.0 != self.layer {
            self.layer = event.0;
            if !self.sleeping {
                self.show().await;
            }
        }
    }

    /// The lights follow the keyboard's sleep state, which is one state for
    /// both halves: the central decides it and tells the right half, so a
    /// key on either half lights both. (A lights timer of its own per half
    /// was tried and removed: a half that saw no keys went dark while the
    /// other was in use, and nothing told it to come back.)
    async fn on_sleep_state_event(&mut self, event: SleepStateEvent) {
        if event.0 == self.sleeping {
            return;
        }
        if event.0 {
            // Dark frame first, then the supply: the data line idles low, so
            // nothing back-feeds the chain once the rail is gone.
            self.sleeping = true;
            self.show().await;
            self.rail.set_low();
        } else {
            // A few milliseconds first: whatever hangs off the rail has to
            // have its inputs set before it powers up. The trackpad reads
            // its chip select at power-up to choose SPI or I2C, and its
            // driver raises that line as soon as it hears this same event.
            Timer::after(RAIL_ON_DELAY).await;
            self.rail.set_high();
            Timer::after(RAIL_SETTLE).await;
            self.sleeping = false;
            self.show().await;
        }
    }
}

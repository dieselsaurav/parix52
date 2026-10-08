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
//!     battery first); the base layer is dark unless switched on from the
//!     keyboard, which also sets its colour and the overall brightness
//!   - after a minute without a key or a pointer move the lights go dark
//!     (the left half decides and tells the right); the supply stays on,
//!     so a touch on the trackpad brings them back
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
use embassy_time::{Duration, Instant, Timer};
use rmk::event::{LayerChangeEvent, LightEvent, SleepStateEvent};
use rmk::key_light::{LightSettings, light_settings};
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

/// How long the rainbow takes to go once round. A frame is drawn every
/// 40 ms (`poll_interval` below).
const RAINBOW_PERIOD_MS: u64 = 6000;

#[processor(subscribe = [LayerChangeEvent, SleepStateEvent, LightEvent], poll_interval = 40)]
pub struct RgbProcessor {
    pwm: SequencePwm<'static>,
    buf: [u16; BUF_LEN],
    side: Side,
    layer: u8,
    /// Base light on/off, its colour and the brightness, set from the
    /// keyboard (Space + Y U O P); the left half sends them to the right.
    light: LightSettings,
    /// A light key was just pressed with the base light on: show the base
    /// layer as it will look, in place of the layer the key sits on, until
    /// the layer changes (the layer key is let go). Otherwise the change
    /// could only be seen after releasing Space.
    preview: bool,
    /// When the rainbow started. Each half keeps its own clock; both restart
    /// it on the events they receive together (a light key, a wake-up), so
    /// the sweep runs on across the gap between the halves.
    anim_start: Instant,
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
            light: light_settings(),
            preview: false,
            anim_start: Instant::now(),
            sleeping: false,
            rail: Output::new(rail, Level::High, OutputDrive::Standard),
        }
    }

    /// Encode each key's color into the PWM duty buffer (GRB wire order).
    fn fill_buffer(&mut self) {
        // Taken afresh for every frame: an event published before this
        // processor subscribed (the stored settings, read at start) is not
        // delivered, and the next frame must not show stale settings.
        self.light = light_settings();
        let layer = if self.preview { 0 } else { self.layer };
        let phase = (self.anim_start.elapsed().as_millis() % RAINBOW_PERIOD_MS * 256 / RAINBOW_PERIOD_MS) as u8;
        let mut i = 0;
        for led in 0..NUM_LEDS {
            let (r, g, b) = if self.sleeping || self.light.idle {
                (0, 0, 0)
            } else {
                rgb_map::color(self.side, layer, led, self.light, phase)
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

    /// The frame timer: only the rainbow, and only while it can be seen,
    /// needs a new frame. (MEDIA is included for its colour key, which
    /// shows the rainbow too.)
    async fn poll(&mut self) {
        let visible = self.preview || self.layer == 0 || self.layer == 3;
        let light = light_settings();
        if !self.sleeping && !light.idle && visible && rgb_map::animated(light) {
            self.show().await;
        }
    }

    async fn on_layer_change_event(&mut self, event: LayerChangeEvent) {
        if event.0 != self.layer || self.preview {
            self.layer = event.0;
            self.preview = false;
            if !self.sleeping {
                self.show().await;
            }
        }
    }

    async fn on_light_event(&mut self, event: LightEvent) {
        let new = LightSettings::from_bits(event.0);
        // Lights going idle or coming back is not a light key: no preview.
        let setting_changed = LightSettings { idle: self.light.idle, ..new } != self.light;
        self.light = new;
        if setting_changed {
            self.preview = self.light.base_on;
        }
        self.anim_start = Instant::now();
        if !self.sleeping {
            self.show().await;
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
            self.rail.set_high();
            Timer::after(RAIL_SETTLE).await;
            self.sleeping = false;
            self.anim_start = Instant::now();
            self.show().await;
        }
    }
}

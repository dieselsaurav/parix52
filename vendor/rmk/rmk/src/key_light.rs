//! PARIX PATCH: settings for the per-key lights, changed from the keyboard.
//!
//! RMK has no lighting support; the board's own processor drives the LEDs.
//! What lives here is the part that needs RMK: four `User` keys change the
//! settings, the central keeps them in the store and sends them over the
//! split link, and every change is published as a [`LightEvent`] on both
//! halves.

use core::sync::atomic::{AtomicU16, Ordering};

use crate::event::{LightEvent, publish_event};

/// Brightness steps: 0 (dimmest) to `BRIGHTNESS_MAX`.
pub const BRIGHTNESS_MAX: u8 = 7;
/// Looks the base layer cycles through (colours and animations); the board
/// gives them their meaning.
pub const BASE_COLORS: u8 = 9;

/// `User` key ids (keyboard.toml `User12`..`User15`).
pub(crate) const KEY_BASE_TOGGLE: u8 = 12;
pub(crate) const KEY_DIMMER: u8 = 13;
pub(crate) const KEY_BRIGHTER: u8 = 14;
pub(crate) const KEY_NEXT_COLOR: u8 = 15;

/// The user-data slot the settings are kept in (slot 0 is the screen's).
#[cfg(feature = "storage")]
const LIGHT_SLOT: u8 = 1;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct LightSettings {
    /// The base layer is lit. Held layers are lit either way.
    pub base_on: bool,
    /// 0 (dimmest) to [`BRIGHTNESS_MAX`], for every light.
    pub brightness: u8,
    /// Which of the [`BASE_COLORS`] the base layer shows.
    pub base_color: u8,
    /// Nothing has been pressed or touched for [`LIGHTS_IDLE_SECS`]: every
    /// light is dark, though the keyboard is still awake. Not a setting: the
    /// sleep manager raises it and any activity clears it.
    pub idle: bool,
}

impl LightSettings {
    /// Base layer dark, first colour, a brightness two steps under the top.
    pub const DEFAULT: Self = Self {
        base_on: false,
        brightness: BRIGHTNESS_MAX - 2,
        base_color: 0,
        idle: false,
    };

    /// The low byte holds the settings that are stored; bit 8 is `idle`.
    pub const fn from_bits(bits: u16) -> Self {
        let brightness = ((bits >> 4) & 0x07) as u8;
        Self {
            base_on: bits & 0x80 != 0,
            brightness: if brightness > BRIGHTNESS_MAX { BRIGHTNESS_MAX } else { brightness },
            base_color: (bits & 0x0f) as u8 % BASE_COLORS,
            idle: bits & IDLE_BIT != 0,
        }
    }

    pub const fn to_bits(self) -> u16 {
        (self.idle as u16) << 8 | (self.base_on as u16) << 7 | (self.brightness as u16) << 4 | self.base_color as u16
    }
}

const IDLE_BIT: u16 = 1 << 8;

/// How long without a key or a pointer move before the lights go dark. The
/// keyboard itself sleeps later (`split_central_sleep_timeout_seconds`).
pub(crate) const LIGHTS_IDLE_SECS: u64 = 60;

static SETTINGS: AtomicU16 = AtomicU16::new(LightSettings::DEFAULT.to_bits());

/// The settings in force on this half.
pub fn light_settings() -> LightSettings {
    LightSettings::from_bits(SETTINGS.load(Ordering::Relaxed))
}

/// Take new settings and tell everyone on this half.
pub(crate) fn apply(bits: u16) {
    let bits = LightSettings::from_bits(bits).to_bits();
    if SETTINGS.swap(bits, Ordering::Relaxed) != bits {
        publish_event(LightEvent::new(bits));
    }
}

/// The sleep manager's word on whether the lights are idle (central only;
/// the split link carries it to the other half with the settings).
pub(crate) fn set_idle(idle: bool) {
    let mut settings = light_settings();
    settings.idle = idle;
    apply(settings.to_bits());
}

/// One of the light keys was pressed (central only).
pub(crate) async fn key_pressed(id: u8) {
    let mut settings = light_settings();
    match id {
        KEY_BASE_TOGGLE => settings.base_on = !settings.base_on,
        KEY_DIMMER => settings.brightness = settings.brightness.saturating_sub(1),
        KEY_BRIGHTER => settings.brightness = (settings.brightness + 1).min(BRIGHTNESS_MAX),
        KEY_NEXT_COLOR => settings.base_color = (settings.base_color + 1) % BASE_COLORS,
        _ => return,
    }
    if settings == light_settings() {
        return;
    }
    apply(settings.to_bits());
    #[cfg(feature = "storage")]
    crate::storage::store_user_data(LIGHT_SLOT, &[settings.to_bits() as u8]).await.ok();
}

/// Read back what the keys last left (central, once, at start).
#[cfg(feature = "storage")]
pub(crate) async fn load() {
    let read = crate::storage::read_user_data(LIGHT_SLOT);
    if let Ok(Some(data)) = embassy_time::with_timeout(embassy_time::Duration::from_secs(2), read).await
        && let Some(bits) = data.first()
    {
        // The stored byte has no idle bit; keep the one in force.
        apply(*bits as u16 | (SETTINGS.load(Ordering::Relaxed) & IDLE_BIT));
    }
}

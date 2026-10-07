//! PARIX PATCH: settings for the per-key lights, changed from the keyboard.
//!
//! RMK has no lighting support; the board's own processor drives the LEDs.
//! What lives here is the part that needs RMK: four `User` keys change the
//! settings, the central keeps them in the store and sends them over the
//! split link, and every change is published as a [`LightEvent`] on both
//! halves.

use core::sync::atomic::{AtomicU8, Ordering};

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
}

impl LightSettings {
    /// Base layer dark, first colour, a brightness two steps under the top.
    pub const DEFAULT: Self = Self {
        base_on: false,
        brightness: BRIGHTNESS_MAX - 2,
        base_color: 0,
    };

    pub const fn from_bits(bits: u8) -> Self {
        let brightness = (bits >> 4) & 0x07;
        Self {
            base_on: bits & 0x80 != 0,
            brightness: if brightness > BRIGHTNESS_MAX { BRIGHTNESS_MAX } else { brightness },
            base_color: (bits & 0x0f) % BASE_COLORS,
        }
    }

    pub const fn to_bits(self) -> u8 {
        (self.base_on as u8) << 7 | self.brightness << 4 | self.base_color
    }
}

static SETTINGS: AtomicU8 = AtomicU8::new(LightSettings::DEFAULT.to_bits());

/// The settings in force on this half.
pub fn light_settings() -> LightSettings {
    LightSettings::from_bits(SETTINGS.load(Ordering::Relaxed))
}

/// Take new settings and tell everyone on this half.
pub(crate) fn apply(bits: u8) {
    let bits = LightSettings::from_bits(bits).to_bits();
    if SETTINGS.swap(bits, Ordering::Relaxed) != bits {
        publish_event(LightEvent::new(bits));
    }
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
    crate::storage::store_user_data(LIGHT_SLOT, &[settings.to_bits()]).await.ok();
}

/// Read back what the keys last left (central, once, at start).
#[cfg(feature = "storage")]
pub(crate) async fn load() {
    let read = crate::storage::read_user_data(LIGHT_SLOT);
    if let Ok(Some(data)) = embassy_time::with_timeout(embassy_time::Duration::from_secs(2), read).await
        && let Some(bits) = data.first()
    {
        apply(*bits);
    }
}

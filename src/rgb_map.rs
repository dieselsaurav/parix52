//! Per-key, per-layer colours: what each LED shows on each layer.
//!
//! The colour grids below are written in the SAME order as the `[[layer]]`
//! grids in config/keyboard.toml (4 rows of 12, then the 4 thumbs), so a
//! colour sits where its key sits. They MUST be kept in step with the keymap
//! by hand: a key moved in keyboard.toml, or remapped live from Vial, keeps
//! the colour of its position, not of its function.
//!
//! A key that does nothing on a layer is dark, so a held layer shows only the
//! keys that are live on it. The key that is held to reach a layer stays lit
//! in that layer's colour.

use rmk::key_light::LightSettings;

/// One colour, (r, g, b).
pub type Rgb = (u8, u8, u8);

/// Which half this binary drives.
#[allow(dead_code)]
#[derive(Clone, Copy)]
pub enum Side {
    Left,
    Right,
}

/// Number of LEDs on one half: one per key, 26.
pub const NUM_LEDS: usize = 26;
/// Keys on the whole keyboard, both halves.
const NUM_KEYS: usize = 52;
const NUM_LAYERS: usize = 8;

/// Highest value any colour channel may take (of 255). 26 LEDs at full white
/// draw about 1.6 A per half (paraboard docs/BOM.md), which neither the
/// module's regulator nor the cell can supply; the build fails on a palette
/// entry above this.
const MAX_CHANNEL: u8 = 32;

// ── Palette ─────────────────────────────────────────────────────────────────
// Three-letter names so the grids below line up. The keycaps are white with
// shine-through legends: an unlit key shows no legend at all. Every hue uses
// one or two channels; a three-channel mix at these low levels comes out
// uneven from LED to LED. White is the exception and keeps its channels equal.
const ___: Rgb = (0, 0, 0); //    dark: the key does nothing here
const WHT: Rgb = (26, 26, 26); // white: light, brightness and output keys on MEDIA
const NAV: Rgb = (0, 10, 30); //  blue: NAV layer, arrows, Bluetooth profiles
const NUM: Rgb = (0, 28, 0); //   green: NUM layer, digits
const MED: Rgb = (16, 0, 30); //  violet: MEDIA layer, transport, volume
const SYM: Rgb = (30, 10, 0); //  orange: SYM layer, symbols
const FUN: Rgb = (30, 0, 0); //   red: FUN layer, F-keys, destructive keys
const MOU: Rgb = (0, 22, 22); //  cyan: MOUSE layer, pointer, buttons
const MOD: Rgb = (24, 20, 0); //  yellow: modifiers
const EDT: Rgb = (30, 0, 10); //  magenta: editing keys (Tab, Esc, clipboard...)

/// Index = layer number, then key in keyboard.toml grid order.
#[rustfmt::skip]
const COLORS: [[Rgb; NUM_KEYS]; NUM_LAYERS] = [
    // 0 BASE. Only the six layer-tap keys are named here (two top corners,
    // four thumbs), in the colour of the layer they hold; every other key
    // takes the base colour chosen from the keyboard. All of it shows only
    // while the base light is switched on (see `color`).
    [
        MOU, ___, ___, ___, ___, ___,    ___, ___, ___, ___, ___, FUN,
        ___, ___, ___, ___, ___, ___,    ___, ___, ___, ___, ___, ___,
        ___, ___, ___, ___, ___, ___,    ___, ___, ___, ___, ___, ___,
        ___, ___, ___, ___, ___, ___,    ___, ___, ___, ___, ___, ___,
                            MED, NAV,    SYM, NUM,
    ],
    // 1 NAV (hold Tab, second left thumb).
    [
        ___, ___, ___, ___, ___, ___,    ___, ___, ___, ___, ___, ___,
        ___, ___, ___, ___, ___, ___,    EDT, EDT, EDT, EDT, EDT, ___,
        ___, MOD, MOD, MOD, MOD, ___,    NAV, NAV, NAV, NAV, MOD, ___,
        ___, ___, ___, ___, ___, ___,    EDT, NAV, NAV, NAV, NAV, ___,
                            ___, NAV,    EDT, EDT,
    ],
    // 2 NUM (hold Backspace, second right thumb).
    [
        ___, ___, ___, ___, ___, ___,    ___, ___, ___, ___, ___, ___,
        ___, SYM, NUM, NUM, NUM, SYM,    ___, ___, ___, ___, ___, ___,
        ___, SYM, NUM, NUM, NUM, SYM,    ___, MOD, MOD, MOD, MOD, ___,
        ___, SYM, NUM, NUM, NUM, SYM,    ___, ___, ___, ___, ___, ___,
                            NUM, SYM,    ___, NUM,
    ],
    // 3 MEDIA (hold Space, first left thumb), one subject per row. Number
    // row: the two screens, yellow. Top row: the key lights, white; the first
    // two show the current setting instead (see `color`). Home row: all off
    // in red, then media. Bottom row: output toggle, four Bluetooth profiles.
    [
        ___, ___, ___, ___, ___, ___,    ___, MOD, MOD, MOD, MOD, ___,
        ___, ___, ___, ___, ___, ___,    WHT, WHT, ___, WHT, WHT, ___,
        ___, MOD, MOD, MOD, MOD, ___,    FUN, MED, MED, MED, MED, ___,
        ___, ___, ___, ___, ___, ___,    WHT, NAV, NAV, NAV, NAV, ___,
                            MED, ___,    MED, MED,
    ],
    // 4 SYM (hold Enter, first right thumb).
    [
        ___, ___, ___, ___, ___, ___,    ___, ___, ___, ___, ___, ___,
        ___, SYM, SYM, SYM, SYM, SYM,    ___, ___, ___, ___, ___, ___,
        ___, SYM, SYM, SYM, SYM, SYM,    ___, MOD, MOD, MOD, MOD, ___,
        ___, SYM, SYM, SYM, SYM, SYM,    ___, ___, ___, ___, ___, ___,
                            SYM, SYM,    SYM, ___,
    ],
    // 5 FUN (hold Minus, top right).
    [
        FUN, FUN, FUN, FUN, FUN, FUN,    FUN, FUN, FUN, FUN, FUN, FUN,
        ___, FUN, FUN, FUN, FUN, EDT,    ___, ___, ___, ___, ___, ___,
        ___, FUN, FUN, FUN, FUN, EDT,    ___, MOD, MOD, MOD, MOD, ___,
        ___, FUN, FUN, FUN, FUN, EDT,    ___, ___, ___, ___, ___, ___,
                            EDT, EDT,    ___, ___,
    ],
    // 6 MOUSE (hold Grave, top left). Pointer cyan, scroll blue.
    [
        MOU, ___, ___, ___, ___, ___,    ___, ___, ___, ___, ___, ___,
        ___, ___, ___, ___, ___, ___,    ___, ___, ___, ___, ___, ___,
        ___, MOD, MOD, MOD, MOD, ___,    MOU, MOU, MOU, MOU, ___, ___,
        ___, ___, ___, ___, ___, ___,    NAV, NAV, NAV, NAV, ___, ___,
                            ___, ___,    MOU, MOU,
    ],
    // 7 DISPOFF: everything dark (RGB kill switch, same toggle as the OLEDs).
    [___; NUM_KEYS],
];

/// Chain position -> (matrix row, column net Cn), first LED after RGB_DI
/// first. Read off the `led_din`/`led_dout` nets in paraboard
/// ergogen/config.yaml: the chain starts at the bottom of the outer column,
/// snakes up and down the six columns towards the inner one, then takes the
/// two thumbs. The right board is the mirror image with the same nets, so
/// its chain starts at the bottom of ITS outer column.
const CHAIN: [(u8, u8); NUM_LEDS] = [
    (3, 0), (2, 0), (1, 0), (0, 0), // outer, upwards
    (0, 1), (1, 1), (2, 1), (3, 1), // pinky, downwards
    (3, 2), (2, 2), (1, 2), (0, 2), // ring, upwards
    (0, 3), (1, 3), (2, 3), (3, 3), // middle, downwards
    (3, 4), (2, 4), (1, 4), (0, 4), // index, upwards
    (0, 5), (1, 5), (2, 5), (3, 5), // inner, downwards
    (4, 5), (4, 4), //                 home thumb, outer thumb
];

/// Position in a colour grid of the key under LED `led` of `side`.
///
/// Column net Cn is matrix column n on the left and 11 - n on the right
/// (the right half's `col_pins` are listed inner column first). The thumbs
/// follow `matrix_map`, which lists them (4,5) (4,4) | (4,7) (4,6).
const fn key_index(side: Side, led: usize) -> usize {
    let (row, net) = CHAIN[led];
    let col = match side {
        Side::Left => net,
        Side::Right => 11 - net,
    };
    match (row, col) {
        (4, 5) => 48,
        (4, 4) => 49,
        (4, 7) => 50,
        (4, 6) => 51,
        _ => row as usize * 12 + col as usize,
    }
}

/// What the base layer's plain keys can be lit in; Space + U steps through
/// them. Even white first, then round the colour wheel.
const BASE_COLORS: [Rgb; rmk::key_light::BASE_COLORS as usize] = [
    (20, 20, 20), // white
    (0, 10, 30),  // blue
    (0, 22, 22),  // cyan
    (0, 28, 0),   // green
    (24, 20, 0),  // yellow
    (30, 10, 0),  // orange
    (30, 0, 0),   // red
    (30, 0, 10),  // magenta
];

/// Share of a colour that each brightness step lets through, in eighths.
const BRIGHTNESS_EIGHTHS: [u16; rmk::key_light::BRIGHTNESS_MAX as usize + 1] = [1, 2, 3, 5, 8];

const BASE_LAYER: usize = 0;
const MEDIA_LAYER: usize = 3;
/// On MEDIA, the keys for base on/off (Space + Y) and the base colour (Space + U).
const KEY_BASE_TOGGLE: usize = 12 + 6;
const KEY_BASE_COLOR: usize = 12 + 7;

fn dimmed((r, g, b): Rgb, brightness: u8) -> Rgb {
    let eighths = BRIGHTNESS_EIGHTHS[(brightness as usize).min(BRIGHTNESS_EIGHTHS.len() - 1)];
    // A channel that is lit at all stays lit: rounding down to 0 would
    // change the hue at the dimmest steps.
    let scale = |c: u8| if c == 0 { 0 } else { ((c as u16 * eighths) / 8).max(1) as u8 };
    (scale(r), scale(g), scale(b))
}

/// Colour of LED `led` (chain position) of `side` while `layer` is on top.
pub fn color(side: Side, layer: u8, led: usize, light: LightSettings) -> Rgb {
    let layer = (layer as usize).min(NUM_LAYERS - 1);
    let key = key_index(side, led);
    let base_color = BASE_COLORS[light.base_color as usize % BASE_COLORS.len()];
    let named = COLORS[layer][key];
    let rgb = match (layer, key) {
        (BASE_LAYER, _) if !light.base_on => ___,
        (BASE_LAYER, _) if named == ___ => base_color,
        // The two base-light keys show what they are set to.
        (MEDIA_LAYER, KEY_BASE_COLOR) => base_color,
        (MEDIA_LAYER, KEY_BASE_TOGGLE) if !light.base_on => FUN,
        _ => named,
    };
    dimmed(rgb, light.brightness)
}

// Build-time checks: the 52 LEDs of the two chains land on 52 different keys,
// and no colour is brighter than the cap.
const _: () = {
    let mut seen = [false; NUM_KEYS];
    let mut led = 0;
    while led < NUM_LEDS {
        let left = key_index(Side::Left, led);
        let right = key_index(Side::Right, led);
        assert!(!seen[left], "two LEDs map to the same key");
        seen[left] = true;
        assert!(!seen[right], "two LEDs map to the same key");
        seen[right] = true;
        led += 1;
    }

    let mut layer = 0;
    while layer < NUM_LAYERS {
        let mut key = 0;
        while key < NUM_KEYS {
            let (r, g, b) = COLORS[layer][key];
            assert!(
                r <= MAX_CHANNEL && g <= MAX_CHANNEL && b <= MAX_CHANNEL,
                "colour brighter than MAX_CHANNEL"
            );
            key += 1;
        }
        layer += 1;
    }
};

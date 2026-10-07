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
const WHT: Rgb = (26, 26, 26); // white: brightness and output keys on MEDIA
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
    // 0 BASE: dark. The legends are shine-through, so the base layer shows
    // plain white caps; lights and legends appear only while a layer is held.
    [___; NUM_KEYS],
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
    // 3 MEDIA (hold Space, first left thumb). Row 1: displays/RGB off, the
    // computer's screen brightness, the OLED's brightness. Row 3: output toggle, four Bluetooth profiles, clear bond.
    [
        ___, ___, ___, ___, ___, ___,    ___, ___, ___, ___, ___, ___,
        ___, ___, ___, ___, ___, ___,    FUN, WHT, WHT, WHT, WHT, ___,
        ___, MOD, MOD, MOD, MOD, ___,    ___, MED, MED, MED, MED, ___,
        ___, ___, ___, ___, ___, ___,    WHT, NAV, NAV, NAV, NAV, FUN,
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

/// Colour of LED `led` (chain position) of `side` while `layer` is on top.
pub fn color(side: Side, layer: u8, led: usize) -> Rgb {
    COLORS[(layer as usize).min(NUM_LAYERS - 1)][key_index(side, led)]
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

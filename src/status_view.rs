//! The left half's status screen as a drawing, free of firmware types so it
//! can be rendered on a computer too (128 x 32, landscape):
//!
//! ```text
//!   NAV                   USB [BT2]
//!   [mods] [A]          L 87%  R 64%
//!                       ======  ====
//! ```
//!
//!   top-left      layer name, large
//!   top-right     the links to the computer, `USB` when one is plugged in
//!                 and `BT` with the profile number: plain when connected,
//!                 `..` paired and looking for its computer, `?` nothing
//!                 paired. The one in the filled box is where typing goes.
//!   bottom-left   the modifiers held now, as their Mac symbols in home-row
//!                 order (Cmd Opt Ctrl Shift), and a boxed A for Caps Lock
//!   bottom-right  both batteries, left then right, each with a bar under it;
//!                 a bolt in place of the letter while that half is on USB
//!                 power, `--` for a right half that is not linked

use core::fmt::Write as _;

use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Line, PrimitiveStyle, Rectangle};
use heapless::String;
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};
use u8g2_fonts::{FontRenderer, fonts};

/// The Bluetooth profile in use and how it stands.
#[derive(Clone, Copy, PartialEq)]
pub enum Bt {
    /// Connected to its computer.
    On,
    /// Paired, not connected: looking for its computer.
    Searching,
    /// Nothing paired on this profile: open for pairing.
    Unpaired,
}

/// Where the keystrokes are going right now.
#[derive(Clone, Copy, PartialEq)]
pub enum Active {
    Usb,
    Bluetooth,
    /// Nowhere: no computer on USB and the profile is not connected.
    Nowhere,
}

/// Everything the screen shows.
pub struct View<'a> {
    pub layer: &'a str,
    /// A computer is connected over USB.
    pub usb: bool,
    /// Bluetooth profile number, 0-based, and its state.
    pub profile: u8,
    pub bt: Bt,
    pub active: Active,
    /// Left battery, percent.
    pub left: Option<u8>,
    /// Right battery, percent; `None` when unknown.
    pub right: Option<u8>,
    pub right_linked: bool,
    /// Each half on USB power, i.e. charging: a bolt replaces its letter.
    pub left_charging: bool,
    pub right_charging: bool,
    pub cmd: bool,
    pub opt: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub caps: bool,
}

const ON: BinaryColor = BinaryColor::On;

fn line<D: DrawTarget<Color = BinaryColor>>(d: &mut D, a: (i32, i32), b: (i32, i32)) {
    Line::new(Point::new(a.0, a.1), Point::new(b.0, b.1))
        .into_styled(PrimitiveStyle::with_stroke(ON, 1))
        .draw(d)
        .ok();
}

fn frame<D: DrawTarget<Color = BinaryColor>>(d: &mut D, x: i32, y: i32, w: u32, h: u32) {
    Rectangle::new(Point::new(x, y), Size::new(w, h))
        .into_styled(PrimitiveStyle::with_stroke(ON, 1))
        .draw(d)
        .ok();
}

/// The four modifier symbols, each in a 9 x 9 cell with its corner at (x, y).
fn cmd_icon<D: DrawTarget<Color = BinaryColor>>(d: &mut D, x: i32, y: i32) {
    frame(d, x + 2, y + 2, 5, 5);
    frame(d, x, y, 3, 3);
    frame(d, x + 6, y, 3, 3);
    frame(d, x, y + 6, 3, 3);
    frame(d, x + 6, y + 6, 3, 3);
}

fn opt_icon<D: DrawTarget<Color = BinaryColor>>(d: &mut D, x: i32, y: i32) {
    line(d, (x, y + 2), (x + 3, y + 2));
    line(d, (x + 3, y + 2), (x + 6, y + 7));
    line(d, (x + 6, y + 7), (x + 8, y + 7));
    line(d, (x + 5, y + 2), (x + 8, y + 2));
}

fn ctrl_icon<D: DrawTarget<Color = BinaryColor>>(d: &mut D, x: i32, y: i32) {
    line(d, (x, y + 6), (x + 4, y + 2));
    line(d, (x + 4, y + 2), (x + 8, y + 6));
}

fn shift_icon<D: DrawTarget<Color = BinaryColor>>(d: &mut D, x: i32, y: i32) {
    line(d, (x + 4, y), (x, y + 4));
    line(d, (x + 4, y), (x + 8, y + 4));
    line(d, (x, y + 4), (x + 2, y + 4));
    line(d, (x + 6, y + 4), (x + 8, y + 4));
    line(d, (x + 2, y + 4), (x + 2, y + 8));
    line(d, (x + 6, y + 4), (x + 6, y + 8));
    line(d, (x + 2, y + 8), (x + 6, y + 8));
}

fn bolt<D: DrawTarget<Color = BinaryColor>>(d: &mut D, x: i32, y: i32) {
    line(d, (x + 3, y), (x, y + 4));
    line(d, (x, y + 4), (x + 4, y + 4));
    line(d, (x + 4, y + 4), (x + 1, y + 9));
}

/// One battery: a label (or the bolt), the percentage, and a bar under it.
/// The block is 30 px wide and ends at `right`.
fn battery<D: DrawTarget<Color = BinaryColor>>(d: &mut D, right: i32, label: char, charging: bool, level: Option<u8>) {
    let small = FontRenderer::new::<fonts::u8g2_font_6x12_tr>();
    let on = FontColor::Transparent(ON);
    let left = right - 30;
    let mut text: String<6> = String::new();
    match level {
        Some(pct) => {
            let _ = write!(text, "{}%", pct.min(100));
        }
        None => {
            let _ = write!(text, "--");
        }
    }
    if charging {
        bolt(d, left + 1, 19);
    } else {
        let mut l: String<2> = String::new();
        let _ = l.push(label);
        small
            .render_aligned(l.as_str(), Point::new(left, 18), VerticalPosition::Top, HorizontalAlignment::Left, on, d)
            .ok();
    }
    small
        .render_aligned(text.as_str(), Point::new(right, 18), VerticalPosition::Top, HorizontalAlignment::Right, on, d)
        .ok();
    if let Some(pct) = level {
        let len = (pct.min(100) as i32 * 30) / 100;
        if len > 0 {
            line(d, (left, 31), (left + len - 1, 31));
        }
    }
}

/// A word on the top line with its left edge at `x`; `active` draws it
/// dark on a filled box.
fn tag<D: DrawTarget<Color = BinaryColor>>(d: &mut D, x: i32, text: &str, active: bool) {
    let small = FontRenderer::new::<fonts::u8g2_font_6x12_tr>();
    let w = text.len() as u32 * 6;
    let color = if active {
        Rectangle::new(Point::new(x - 2, 0), Size::new(w + 3, 13))
            .into_styled(PrimitiveStyle::with_fill(ON))
            .draw(d)
            .ok();
        FontColor::Transparent(BinaryColor::Off)
    } else {
        FontColor::Transparent(ON)
    };
    small
        .render_aligned(text, Point::new(x, 1), VerticalPosition::Top, HorizontalAlignment::Left, color, d)
        .ok();
}

pub fn draw<D: DrawTarget<Color = BinaryColor>>(d: &mut D, v: &View) {
    let small = FontRenderer::new::<fonts::u8g2_font_6x12_tr>();
    let big = FontRenderer::new::<fonts::u8g2_font_logisoso16_tr>();
    let on = FontColor::Transparent(ON);

    // Top-left: layer.
    big.render_aligned(v.layer, Point::new(0, 0), VerticalPosition::Top, HorizontalAlignment::Left, on, d)
        .ok();

    // Top-right: the links, and which one the typing is on. Both are named
    // when both exist; the one in the filled box carries the keystrokes.
    let mut bt: String<8> = String::new();
    let _ = match v.bt {
        Bt::On => write!(bt, "BT{}", v.profile + 1),
        Bt::Searching => write!(bt, "BT{}..", v.profile + 1),
        Bt::Unpaired => write!(bt, "BT{}?", v.profile + 1),
    };
    let bt_w = bt.len() as i32 * 6;
    let bt_x = 126 - bt_w;
    tag(d, bt_x, bt.as_str(), v.active == Active::Bluetooth);
    if v.usb {
        tag(d, bt_x - 6 - 18, "USB", v.active == Active::Usb);
    }

    // Bottom-left: held modifiers in home-row order, then Caps Lock.
    if v.cmd {
        cmd_icon(d, 0, 20);
    }
    if v.opt {
        opt_icon(d, 12, 20);
    }
    if v.ctrl {
        ctrl_icon(d, 24, 20);
    }
    if v.shift {
        shift_icon(d, 36, 20);
    }
    if v.caps {
        frame(d, 49, 18, 10, 13);
        small
            .render_aligned("A", Point::new(51, 19), VerticalPosition::Top, HorizontalAlignment::Left, on, d)
            .ok();
    }

    // Bottom-right: batteries.
    battery(d, 93, 'L', v.left_charging, v.left);
    battery(
        d,
        127,
        'R',
        v.right_linked && v.right_charging,
        if v.right_linked { v.right } else { None },
    );
}

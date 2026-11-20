//! Colours and measures of the HUD, taken from the validated mock-up (variant 2, decision D-12:
//! rules `.v2` and `.dark` of `maquettes/outils/generer.mjs`). Sizes are egui points, i.e. pixels at
//! 100 % Windows scaling.

use eframe::egui::{Color32, epaint::Shadow};

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

// Panel ----------------------------------------------------------------------------------------
pub const PANEL: Color32 = rgb(0x0f1114);
/// 1 px border, mid grey: visible on white as on black backgrounds.
pub const EDGE: Color32 = rgb(0x5d6876);
/// 1 px ring outside the border: keeps the edge visible on light and mid-tone backgrounds.
pub const RING: Color32 = Color32::BLACK;
pub const BAR_LINE: Color32 = rgb(0x2a2f37);
pub const GRIP: Color32 = rgb(0x7f8996);
pub const TEXT: Color32 = rgb(0xf3f5f8);
pub const TEXT_2: Color32 = rgb(0xaab3bf);
pub const TEXT_BUTTON: Color32 = rgb(0xd3d9e1);
pub const HOVER: Color32 = rgb(0x1d2128);
pub const PRESS: Color32 = rgb(0x262b33);

// Macro rows -----------------------------------------------------------------------------------
pub const ROW: Color32 = rgb(0x181b20);
pub const ROW_LINE: Color32 = rgb(0x262b33);
pub const STRIPE: Color32 = rgb(0x6b7685);
pub const CONTROL: Color32 = rgb(0x2b313b);
pub const CONTROL_HOVER: Color32 = rgb(0x363d48);
pub const CONTROL_PRESS: Color32 = rgb(0x232830);
pub const CONTROL_EDGE: Color32 = rgb(0x6b7685);
pub const ROW_HOVER: Color32 = rgb(0x22262d);

// Active macro: the whole row turns green ------------------------------------------------------
pub const ON: Color32 = rgb(0x34d27c);
pub const ON_INK: Color32 = rgb(0x04140a);
pub const ON_LINE: Color32 = rgb(0x1f9b57);
pub const ON_HOVER: Color32 = rgb(0x2bbd6c);
pub const ON_INK_HOVER: Color32 = rgb(0x0e2c18);

// Macro « À venir » ----------------------------------------------------------------------------
pub const SOON_ROW: Color32 = rgb(0x14161a);
pub const SOON_STRIPE: Color32 = rgb(0x30353d);
pub const SOON_TEXT: Color32 = rgb(0x7a838f);
pub const SOON_CHEVRON: Color32 = rgb(0x4f5762);

// Settings under a row -------------------------------------------------------------------------
pub const OPTIONS: Color32 = rgb(0x14171b);
pub const SEGMENT_TEXT: Color32 = rgb(0xe6eaf0);
pub const SEGMENT_HOVER: Color32 = rgb(0x1d2128);

// Keycap ---------------------------------------------------------------------------------------
pub const KEY: Color32 = rgb(0xe9edf2);
pub const KEY_LIP: Color32 = rgb(0x9aa4b1);
pub const KEY_INK: Color32 = rgb(0x111418);
pub const KEY_OFF: Color32 = rgb(0x2d323a);
pub const KEY_OFF_LIP: Color32 = rgb(0x1f2329);
pub const KEY_OFF_INK: Color32 = rgb(0xa3abb6);

// Messages and shortcut capture ----------------------------------------------------------------
pub const INFO: Color32 = rgb(0x6cb8ff);
pub const INFO_BG: Color32 = rgb(0x10233a);
pub const INFO_TEXT: Color32 = rgb(0xb3d7ff);
pub const WARN: Color32 = rgb(0xffb547);
pub const WARN_BG: Color32 = rgb(0x2b2112);
pub const WARN_TITLE: Color32 = rgb(0xfff1d6);
pub const WARN_TEXT: Color32 = rgb(0xffe2b0);
pub const WARN_BUTTON: Color32 = rgb(0x3a2c14);
pub const WARN_BUTTON_HOVER: Color32 = rgb(0x4d3a1a);
/// The « ! » inside the warning sign.
pub const SIGN_INK: Color32 = rgb(0x1a1206);
pub const ERROR: Color32 = rgb(0xff6b6b);
pub const ERROR_BG: Color32 = rgb(0x3a1616);
pub const ERROR_TITLE: Color32 = rgb(0xffe3e3);
pub const ERROR_TEXT: Color32 = rgb(0xffc9c9);
/// Keyboard focus ring.
pub const FOCUS: Color32 = rgb(0x6cb8ff);

// Measures -------------------------------------------------------------------------------------
/// Width of the panel, its 1 px border included (the black ring adds 1 px on each side).
pub const PANEL_WIDTH: f32 = 288.0;
pub const RADIUS: u8 = 8;
/// Radius inside the 1 px border.
pub const INNER_RADIUS: u8 = RADIUS - 1;
pub const BAR_HEIGHT: f32 = 32.0;
pub const ROW_HEIGHT: f32 = 50.0;
pub const CHIP_HEIGHT: f32 = 34.0;

/// Line boxes (CSS line-height 1.25, rounded to whole pixels).
pub const LINE_12: f32 = 15.0;
pub const LINE_14: f32 = 18.0;
/// Line box of message lines (line-height 1.35).
pub const LINE_MESSAGE: f32 = 16.0;

/// Drop shadow of the mock-up (`0 6px 16px rgba(0,0,0,.5)`). Painted in the renders; in the
/// real window it would need a transparent margin that swallows clicks around the HUD.
pub const SHADOW: Shadow = Shadow { offset: [0, 6], blur: 16, spread: 0, color: Color32::from_black_alpha(128) };
/// Room the shadow needs around the HUD: left/right, top, bottom.
#[cfg(test)]
pub const SHADOW_MARGIN: [f32; 3] = [16.0, 10.0, 22.0];

// Font weights (Inter variable font, `wght` axis).
pub const REGULAR: f32 = 400.0;
pub const MEDIUM: f32 = 500.0;
pub const SEMIBOLD: f32 = 600.0;
pub const BOLD: f32 = 700.0;
pub const EXTRABOLD: f32 = 800.0;

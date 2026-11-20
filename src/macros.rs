//! The macros MacroHUD knows, in display order (D-05: the HUD is designed for several).

use crate::contract::{MacroId, MacroOptions, MouseButton};
use crate::hotkey::Hotkey;

/// What does not change about a macro.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MacroDef {
    pub id: MacroId,
    /// « Maintenir le clic ».
    pub name: &'static str,
    /// One line, for the HUD.
    pub description: &'static str,
    /// False for a macro « à venir », shown greyed out.
    pub available: bool,
    pub default_hotkey: Hotkey,
    pub default_options: MacroOptions,
}

/// « Maintenir le clic » (D-04): its hotkey presses a mouse button and keeps it down; the hotkey
/// again, or a real click on that button, releases it.
pub const HOLD_CLICK: MacroDef = MacroDef {
    id: MacroId::HOLD_CLICK,
    name: "Maintenir le clic",
    description: "Garde un bouton de la souris enfoncé ; le raccourci ou un vrai clic le relâche.",
    available: true,
    default_hotkey: Hotkey::F8,
    default_options: MacroOptions::HoldClick { button: MouseButton::Left },
};

/// All macros, in display order.
pub const CATALOG: &[MacroDef] = &[HOLD_CLICK];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_macro_holds_the_left_button_with_f8() {
        let first = CATALOG[0];
        assert_eq!(first.id, MacroId::HOLD_CLICK);
        assert_eq!(first.default_hotkey, Hotkey::F8);
        assert_eq!(first.default_options, MacroOptions::HoldClick { button: MouseButton::Left });
        assert!(first.available);
    }

    #[test]
    fn ids_and_default_hotkeys_are_distinct() {
        for (i, a) in CATALOG.iter().enumerate() {
            for b in &CATALOG[i + 1..] {
                assert_ne!(a.id, b.id);
                assert_ne!(a.default_hotkey, b.default_hotkey);
            }
        }
    }
}

//! Capture of a new hotkey: the next key pressed, with the modifiers held at that moment.
//! Échap cancels.
//!
//! It is fed with every keyboard event (from the low-level keyboard hook, on Windows) and decides
//! which ones to swallow: the keys typed for the capture reach no application and trigger no other
//! shortcut (not even the current hotkey). It only swallows the release of a key whose press it
//! swallowed, so no key ever stays stuck down for the system.

use crate::hotkey::{Hotkey, Modifier, Modifiers, Vk, modifier_of, vk};

/// How a capture ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The user pressed this key or combination.
    Chosen(Hotkey),
    /// The user pressed Échap.
    Cancelled,
}

/// What to do with one keyboard event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decision {
    /// Keep the event from the system and the applications.
    pub swallow: bool,
    /// The capture ended with this event.
    pub outcome: Option<Outcome>,
}

impl Decision {
    const PASS: Decision = Decision { swallow: false, outcome: None };
    const SWALLOW: Decision = Decision { swallow: true, outcome: None };
}

/// One capture, from the moment the HUD asks for a new hotkey until the last swallowed key is released.
#[derive(Clone, Debug)]
pub struct KeyCapture {
    /// Modifiers already held when the capture started (their press reached the system).
    held_at_start: Modifiers,
    /// Keys whose press was swallowed: their releases must be swallowed too.
    swallowed: Vec<Vk>,
    finished: bool,
}

impl KeyCapture {
    pub fn new(held_at_start: Modifiers) -> Self {
        KeyCapture { held_at_start, swallowed: Vec::new(), finished: false }
    }

    /// Whether the capture still waits for a key.
    #[cfg(test)]
    pub fn is_capturing(&self) -> bool {
        !self.finished
    }

    /// Whether keyboard events are still needed: capturing, or swallowed keys not released yet.
    pub fn needs_events(&self) -> bool {
        !self.finished || !self.swallowed.is_empty()
    }

    /// Ends the capture without a result (cancelled from the HUD, or too long). Releases of keys
    /// already swallowed are still swallowed.
    pub fn finish(&mut self) {
        self.finished = true;
    }

    /// The modifiers held now, as far as the capture knows.
    fn modifiers(&self) -> Modifiers {
        let mut m = self.held_at_start;
        for key in &self.swallowed {
            if let Some(modifier) = modifier_of(*key) {
                m.set(modifier, true);
            }
        }
        m
    }

    fn swallow_key(&mut self, key: Vk) {
        if !self.swallowed.contains(&key) {
            self.swallowed.push(key);
        }
    }

    /// Handles one keyboard event (`down`: pressed or repeated; otherwise released).
    pub fn on_key(&mut self, key: Vk, down: bool) -> Decision {
        // Characters typed by other programs (VK_PACKET) and invalid codes are not keys.
        if key == 0 || key >= 0xFF || key == vk::PACKET {
            return Decision::PASS;
        }
        if !down {
            if let Some(pos) = self.swallowed.iter().position(|k| *k == key) {
                self.swallowed.remove(pos);
                return Decision::SWALLOW;
            }
            // A key pressed before the capture: its release goes through.
            if let Some(modifier) = modifier_of(key) {
                self.release_held_at_start(modifier);
            }
            return Decision::PASS;
        }
        if self.finished {
            // Auto-repeat of a swallowed key, still held: keep it away from the applications.
            return if self.swallowed.contains(&key) { Decision::SWALLOW } else { Decision::PASS };
        }
        self.swallow_key(key);
        if modifier_of(key).is_some() {
            return Decision::SWALLOW;
        }
        self.finished = true;
        let outcome =
            if key == vk::ESCAPE { Outcome::Cancelled } else { Outcome::Chosen(Hotkey::new(self.modifiers(), key)) };
        Decision { swallow: true, outcome: Some(outcome) }
    }

    fn release_held_at_start(&mut self, modifier: Modifier) {
        self.held_at_start.set(modifier, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(capture: &mut KeyCapture, events: &[(Vk, bool)]) -> Vec<Decision> {
        events.iter().map(|(key, down)| capture.on_key(*key, *down)).collect()
    }

    #[test]
    fn a_single_key_is_chosen_and_both_its_events_are_swallowed() {
        let mut c = KeyCapture::new(Modifiers::NONE);
        let d = run(&mut c, &[(vk::F9, true)]);
        assert_eq!(d[0].outcome, Some(Outcome::Chosen(Hotkey::new(Modifiers::NONE, vk::F9))));
        assert!(d[0].swallow);
        assert!(!c.is_capturing());
        assert!(c.needs_events(), "its release is still to swallow");
        let d = run(&mut c, &[(vk::F9, true), (vk::F9, true), (vk::F9, false)]);
        assert!(d.iter().all(|d| d.swallow && d.outcome.is_none()), "repeats and release swallowed");
        assert!(!c.needs_events());
    }

    #[test]
    fn a_combination_keeps_its_modifiers_and_swallows_every_release() {
        let mut c = KeyCapture::new(Modifiers::NONE);
        let d = run(&mut c, &[(vk::LCONTROL, true), (vk::LMENU, true), (vk::KEY_M, true)]);
        assert!(d.iter().all(|d| d.swallow));
        let expected = Hotkey::new(Modifiers::NONE.with(Modifier::Ctrl).with(Modifier::Alt), vk::KEY_M);
        assert_eq!(d[2].outcome, Some(Outcome::Chosen(expected)));
        let d = run(&mut c, &[(vk::KEY_M, false), (vk::LMENU, false), (vk::LCONTROL, false)]);
        assert!(d.iter().all(|d| d.swallow));
        assert!(!c.needs_events());
    }

    #[test]
    fn escape_cancels() {
        let mut c = KeyCapture::new(Modifiers::NONE);
        let d = run(&mut c, &[(vk::ESCAPE, true), (vk::ESCAPE, false)]);
        assert_eq!(d[0].outcome, Some(Outcome::Cancelled));
        assert!(d[0].swallow && d[1].swallow);
        assert!(!c.needs_events());
        let mut c = KeyCapture::new(Modifiers::NONE);
        let d = run(&mut c, &[(vk::LSHIFT, true), (vk::ESCAPE, true)]);
        assert_eq!(d[1].outcome, Some(Outcome::Cancelled), "Échap cancels, even with a modifier");
    }

    #[test]
    fn a_modifier_released_before_the_key_does_not_count() {
        let mut c = KeyCapture::new(Modifiers::NONE);
        let d = run(&mut c, &[(vk::LCONTROL, true), (vk::LCONTROL, false), (vk::F7, true)]);
        assert_eq!(d[2].outcome, Some(Outcome::Chosen(Hotkey::new(Modifiers::NONE, vk::F7))));
    }

    #[test]
    fn altgr_reads_as_ctrl_alt() {
        // AltGr arrives as a left Ctrl press followed by a right Alt press.
        let mut c = KeyCapture::new(Modifiers::NONE);
        let d = run(&mut c, &[(vk::LCONTROL, true), (vk::RMENU, true), (vk::KEY_E, true)]);
        let expected = Hotkey::new(Modifiers::NONE.with(Modifier::Ctrl).with(Modifier::Alt), vk::KEY_E);
        assert_eq!(d[2].outcome, Some(Outcome::Chosen(expected)));
    }

    #[test]
    fn keys_held_before_the_capture_are_never_swallowed_on_release() {
        // Ctrl was already down when the capture started: its press reached the system, so its
        // release must reach it too, or Ctrl would stay stuck.
        let mut c = KeyCapture::new(Modifiers::NONE.with(Modifier::Ctrl));
        let d = run(&mut c, &[(vk::F5, true), (vk::LCONTROL, false), (vk::F5, false)]);
        assert_eq!(d[0].outcome, Some(Outcome::Chosen(Hotkey::new(Modifiers::NONE.with(Modifier::Ctrl), vk::F5))));
        assert!(!d[1].swallow, "release of a key pressed before the capture");
        assert!(d[2].swallow);
        // A release without any press seen (key held before the capture) goes through too.
        let mut c = KeyCapture::new(Modifiers::NONE);
        assert!(!c.on_key(vk::KEY_A, false).swallow);
        assert!(c.is_capturing());
    }

    #[test]
    fn modifiers_held_at_start_count_until_released() {
        let mut c = KeyCapture::new(Modifiers::NONE.with(Modifier::Shift));
        run(&mut c, &[(vk::LSHIFT, false)]);
        let d = c.on_key(vk::F6, true);
        assert_eq!(d.outcome, Some(Outcome::Chosen(Hotkey::new(Modifiers::NONE, vk::F6))));
    }

    #[test]
    fn finishing_early_keeps_swallowing_pending_releases_only() {
        let mut c = KeyCapture::new(Modifiers::NONE);
        assert!(c.on_key(vk::LCONTROL, true).swallow);
        c.finish();
        assert!(!c.is_capturing());
        assert!(c.needs_events());
        let d = c.on_key(vk::KEY_B, true);
        assert_eq!(d, Decision::PASS, "after the end, new keys go through");
        assert!(c.on_key(vk::LCONTROL, false).swallow);
        assert!(!c.on_key(vk::KEY_B, false).swallow);
        assert!(!c.needs_events());
    }

    #[test]
    fn injected_characters_are_ignored() {
        let mut c = KeyCapture::new(Modifiers::NONE);
        assert_eq!(c.on_key(vk::PACKET, true), Decision::PASS);
        assert_eq!(c.on_key(0, true), Decision::PASS);
        assert!(c.is_capturing());
    }
}

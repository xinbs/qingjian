//! 中英模式：Shift 切出的英文直接交给应用，Caps Lock 保留英文候选路径。

#[derive(Debug, Clone, Copy)]
pub(super) struct InputMode {
    enabled: bool,

    english: bool,

    direct: bool,

    caps: bool,
}

impl Default for InputMode {
    fn default() -> Self {
        Self {
            enabled: true,
            english: false,
            direct: false,
            caps: false,
        }
    }
}

impl InputMode {
    /// Caps Lock 状态变化时恢复它的既有行为；Shift 状态不会随焦点变化重置。
    pub fn observe_caps(&mut self, caps: bool) {
        if self.caps != caps {
            self.caps = caps;
            self.english = self.enabled && caps;
            self.direct = false;
        }
    }

    pub fn set_enabled(&mut self, enabled: bool, caps: bool) {
        self.observe_caps(caps);
        if self.enabled != enabled {
            self.enabled = enabled;
            self.english = enabled && caps;
            self.direct = false;
        }
    }

    pub fn toggle(&mut self, caps: bool) -> bool {
        self.observe_caps(caps);
        if self.enabled {
            self.english = !self.english;
            self.direct = self.english;
        }
        self.english
    }

    pub fn english(&self) -> bool {
        self.english
    }

    pub fn direct(&self) -> bool {
        self.english && self.direct
    }
}

#[cfg(test)]
mod tests {
    use super::InputMode;

    #[test]
    fn shift_toggles_direct_english_and_keeps_it_between_reads() {
        let mut mode = InputMode::default();
        assert!(mode.toggle(false));
        assert!(mode.direct());
        mode.observe_caps(false);
        assert!(mode.english());
        assert!(mode.direct());
        assert!(!mode.toggle(false));
        assert!(!mode.direct());
    }

    #[test]
    fn caps_changes_restore_caps_behavior_and_shift_can_toggle_both_ways() {
        let mut mode = InputMode::default();
        mode.observe_caps(true);
        assert!(mode.english());
        assert!(!mode.direct());
        assert!(!mode.toggle(true));
        mode.observe_caps(true);
        assert!(!mode.english());
        assert!(mode.toggle(true));
        assert!(mode.direct());
        mode.observe_caps(false);
        assert!(!mode.english());
        assert!(!mode.direct());
    }

    #[test]
    fn disabling_english_clears_shift_mode_and_blocks_both_switches() {
        let mut mode = InputMode::default();
        mode.toggle(false);
        mode.set_enabled(false, false);
        assert!(!mode.english());
        assert!(!mode.toggle(false));
        mode.observe_caps(true);
        assert!(!mode.english());
        assert!(!mode.toggle(true));
        mode.set_enabled(true, true);
        assert!(mode.english());
        assert!(!mode.direct());
    }
}

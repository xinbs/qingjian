//! 修饰键事件里的 Shift 单击：左右键都可用，组合键、点击与会话切换取消待切换状态。

#[derive(Debug, Default, Clone, Copy)]
pub struct ShiftTap {
    /// 已观察到按下的左右 Shift，避免两个 Shift 同时按着时误判释放。
    pressed: u8,

    /// 这次按下后没有发生其他输入的 Shift 键码。
    pending: Option<u16>,
}

impl ShiftTap {
    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub fn cancel(&mut self) {
        self.pending = None;
    }

    pub fn flags_changed(
        &mut self,
        key: u16,
        shift: bool,
        other_modifiers: bool,
        enabled: bool,
    ) -> bool {
        let bit = match key {
            56 => 1,
            60 => 2,
            _ => {
                self.cancel();
                return false;
            }
        };
        let was_down = self.pressed & bit != 0;
        if shift && !was_down {
            let alone = self.pressed == 0;
            self.pressed |= bit;
            self.pending = (alone && !other_modifiers && enabled).then_some(key);
            false
        } else {
            self.pressed &= !bit;
            if !shift {
                self.pressed = 0;
            }
            let tap = self.pending.take() == Some(key);
            tap && was_down && !shift && !other_modifiers && enabled
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ShiftTap;

    #[test]
    fn either_shift_fires_once_on_release() {
        for key in [56, 60] {
            let mut tap = ShiftTap::default();
            assert!(!tap.flags_changed(key, true, false, true));
            assert!(tap.flags_changed(key, false, false, true));
            assert!(!tap.flags_changed(key, false, false, true));
        }
    }

    #[test]
    fn typing_or_clicking_cancels_the_switch() {
        let mut tap = ShiftTap::default();
        tap.flags_changed(56, true, false, true);
        tap.cancel();
        assert!(!tap.flags_changed(56, false, false, true));
    }

    #[test]
    fn modifier_combinations_do_not_fire_after_the_other_key_is_released() {
        let mut tap = ShiftTap::default();
        tap.flags_changed(56, true, false, true);
        tap.flags_changed(55, true, true, true);
        tap.flags_changed(55, true, false, true);
        assert!(!tap.flags_changed(56, false, false, true));
        tap.flags_changed(60, true, true, true);
        assert!(!tap.flags_changed(60, false, false, true));
    }

    #[test]
    fn overlapping_shifts_do_not_toggle() {
        for (first, second, release_first) in [
            (56, 60, true),
            (56, 60, false),
            (60, 56, true),
            (60, 56, false),
        ] {
            let mut tap = ShiftTap::default();
            tap.flags_changed(first, true, false, true);
            tap.flags_changed(second, true, false, true);
            let (release, last) = if release_first {
                (first, second)
            } else {
                (second, first)
            };
            assert!(!tap.flags_changed(release, true, false, true));
            assert!(!tap.flags_changed(last, false, false, true));
            tap.flags_changed(first, true, false, true);
            assert!(tap.flags_changed(first, false, false, true));
        }
    }

    #[test]
    fn release_without_an_observed_press_and_disabled_switch_do_not_fire() {
        let mut tap = ShiftTap::default();
        assert!(!tap.flags_changed(56, false, false, true));
        tap.flags_changed(56, true, false, true);
        assert!(!tap.flags_changed(56, false, false, false));
        tap.flags_changed(56, true, false, false);
        assert!(!tap.flags_changed(56, false, false, true));
        tap.flags_changed(56, true, false, true);
        tap = ShiftTap::default();
        assert!(!tap.flags_changed(56, false, false, true));
    }
}

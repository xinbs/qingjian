//! 当前修饰键与中英模式：模式在进程内共用，切换应用不重置 Shift 切出的直通英文。

use objc2_app_kit::{NSEvent, NSEventModifierFlags};
use std::cell::RefCell;

use super::input_mode::InputMode;

thread_local! {
    static MODE: RefCell<InputMode> = RefCell::new(InputMode::default());
}

pub fn set_english_enabled(enabled: bool) {
    let caps = caps_lock_on();
    MODE.with(|mode| mode.borrow_mut().set_enabled(enabled, caps));
}

/// Caps Lock 改变时同步模式；读取不会重置 Shift 的选择。
pub fn english_mode() -> bool {
    let caps = caps_lock_on();
    MODE.with(|mode| {
        let mut mode = mode.borrow_mut();
        mode.observe_caps(caps);
        mode.english()
    })
}

pub fn direct_english() -> bool {
    english_mode();
    MODE.with(|mode| mode.borrow().direct())
}

pub fn toggle_english_mode() -> bool {
    let caps = caps_lock_on();
    MODE.with(|mode| mode.borrow_mut().toggle(caps))
}

/// Caps Lock 亮着：视为英文模式，字母默认小写、按住 Shift 才大写、标点不转全角。
pub fn caps_lock_on() -> bool {
    NSEvent::modifierFlags_class().contains(NSEventModifierFlags::CapsLock)
}

/// Shift 正按着。读的是此刻的硬件状态而不是事件自带的标志，但 Shift 是按住不放的键，处理按键时它几乎总还按着。
/// macOS 上 Caps Lock 亮着时按住 Shift 送来的仍是大写（不像 Windows 会反转），所以英文模式的大小写只能靠它判断。
pub fn shift_down() -> bool {
    NSEvent::modifierFlags_class().contains(NSEventModifierFlags::Shift)
}

//! 数字开头的即时计算：等号触发结果，求值复用表达式模式的安全解析器。

use crate::shortcut;

/// 限制待计算输入长度，避免异常按键流无限积累。
pub(crate) const MAX_INPUT_BYTES: usize = 128;

pub(crate) fn is_calculation(text: &str) -> bool {
    text.as_bytes().first().is_some_and(u8::is_ascii_digit)
        && text.len() <= MAX_INPUT_BYTES
        && text.chars().all(is_character)
}

pub(crate) fn is_character(c: char) -> bool {
    c.is_ascii_digit()
        || matches!(
            c,
            '+' | '-' | '*' | 'x' | '/' | '%' | '^' | '.' | '(' | ')' | '='
        )
}

pub(crate) fn result(text: &str) -> Option<String> {
    if !is_calculation(text) {
        return None;
    }
    shortcut::evaluate(text.strip_suffix('=')?)
}

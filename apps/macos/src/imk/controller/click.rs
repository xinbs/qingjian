//! 鼠标点候选窗口：点候选与按它的数字键一样上屏，点译词与按译词键一样上屏它的那条译词。
//!
//! 候选窗口不知道是哪个会话，所以在 `activateServer:` 时记下当前控制器，点击时用它的客户端上屏。

use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{Message, msg_send};
use qingjian_render::HitTarget;

use super::QingjianInputController;
use crate::host;
use crate::imk::{TextClient, catch_panic, recover_from_panic};

thread_local! {
    /// 最近激活的会话；`deactivateServer:` 时清掉。
    static ACTIVE: RefCell<Option<Retained<QingjianInputController>>> = const { RefCell::new(None) };
}

/// 候选窗口的点击回调（host 建窗口时注册）。
pub fn click(target: Option<HitTarget>) {
    let Some(controller) = ACTIVE.with(|active| active.borrow().clone()) else {
        tracing::debug!("没有激活的会话，点击不处理");
        return;
    };
    controller.cancel_shift_tap();
    let Some(target) = target else {
        return;
    };
    if catch_panic("click", || controller.click(target)).is_none() {
        recover_from_panic(None);
    }
}

impl QingjianInputController {
    pub(super) fn set_active(&self) {
        ACTIVE.with(|active| *active.borrow_mut() = Some(self.retain()));
    }

    pub(super) fn clear_active(&self) {
        ACTIVE.with(|active| {
            let mut active = active.borrow_mut();
            if active
                .as_deref()
                .is_some_and(|current| std::ptr::eq(current, self))
            {
                *active = None;
            }
        });
    }

    fn click(&self, target: HitTarget) {
        // SAFETY: IMKInputController 的 `client` 返回当前会话的文本客户端（实现 IMKTextInput 的代理），可能为 nil
        let client: Option<Retained<AnyObject>> = unsafe { msg_send![self, client] };
        let Some(client) = client else {
            return;
        };
        let client = TextClient::new(&client);
        let composing = host::with(|h| !h.engine.composition().is_empty()).unwrap_or(false);
        if !composing {
            return;
        }
        let offset = match target {
            HitTarget::Candidate(offset) => offset,
            HitTarget::Translation { candidate, .. } => candidate,
        };
        let index = host::with(|h| h.session.index_in_view(offset)).flatten();
        let english_first = host::with(|h| {
            index
                .and_then(|index| h.session.candidate(index))
                .is_some_and(|candidate| h.english_translation_first(&candidate))
        })
        .unwrap_or(false);
        match displayed_target(target, english_first) {
            HitTarget::Candidate(_) => {
                if let Some(index) = index {
                    self.commit_index(index, client);
                }
            }
            HitTarget::Translation { sense, .. } => {
                self.commit_translation_at(index, sense, client);
            }
        }
    }
}

/// 前置显示时，主文本是英文译文，旁注是中文；鼠标语义始终跟随可见文字。
fn displayed_target(target: HitTarget, english_first: bool) -> HitTarget {
    if !english_first {
        return target;
    }
    match target {
        HitTarget::Candidate(candidate) => HitTarget::Translation {
            candidate,
            sense: 0,
        },
        HitTarget::Translation { candidate, .. } => HitTarget::Candidate(candidate),
    }
}

#[cfg(test)]
mod tests {
    use super::displayed_target;
    use qingjian_render::HitTarget;

    #[test]
    fn english_first_clicks_follow_the_visible_language() {
        assert_eq!(
            displayed_target(HitTarget::Candidate(2), true),
            HitTarget::Translation {
                candidate: 2,
                sense: 0
            }
        );
        assert_eq!(
            displayed_target(
                HitTarget::Translation {
                    candidate: 2,
                    sense: 0
                },
                true
            ),
            HitTarget::Candidate(2)
        );
    }

    #[test]
    fn normal_display_and_missing_translations_keep_click_behavior() {
        for target in [
            HitTarget::Candidate(2),
            HitTarget::Translation {
                candidate: 2,
                sense: 1,
            },
        ] {
            assert_eq!(displayed_target(target, false), target);
        }
    }
}

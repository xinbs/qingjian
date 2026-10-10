//! 鼠标点候选窗口：按位图渲染器给的点击区域认出点中的候选或译词，交给 host 注册的回调上屏。
//! AppKit 逐项绘制的旧路径没有点击区域，点了不动。

use objc2::DefinedClass;
use objc2_app_kit::NSEvent;
use qingjian_render::HitTarget;

use super::CandidateView;

/// 每次鼠标按下时调的回调；空白处传 `None`，让输入控制器也能取消修饰键单击。
pub type ClickHandler = Box<dyn Fn(Option<HitTarget>)>;

impl CandidateView {
    pub fn set_click_handler(&self, handler: ClickHandler) {
        *self.ivars().on_click.borrow_mut() = Some(handler);
    }

    pub(super) fn clicked(&self, event: &NSEvent) {
        let point = self.convertPoint_fromView(event.locationInWindow(), None);
        // 先放掉位图的借用：回调里上屏会重画这个视图
        let target = self
            .ivars()
            .bitmap
            .borrow()
            .as_ref()
            .and_then(|bitmap| bitmap.hit(point));
        if target.is_some() {
            tracing::debug!(?target, "点击候选窗口");
        }
        if let Some(handler) = &*self.ivars().on_click.borrow() {
            handler(target);
        }
    }
}

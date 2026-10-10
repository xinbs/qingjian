//! 鼠标点候选窗口：按位图渲染器给的点击区域认出点中的候选或译词，交给 host 注册的回调上屏。
//! AppKit 绘制只为计算结果行提供点击区域。

use objc2::DefinedClass;
use objc2_app_kit::NSEvent;
use qingjian_platform::LayoutMode;
use qingjian_render::HitTarget;

use super::CandidateView;

/// 每次鼠标按下时调的回调；空白处传 `None`，让输入控制器也能取消修饰键单击。
pub type ClickHandler = Box<dyn Fn(Option<HitTarget>)>;

impl CandidateView {
    pub fn accepts_clicks(&self) -> bool {
        self.uses_bitmap() || self.ivars().frame.borrow().calculation
    }

    pub fn set_click_handler(&self, handler: ClickHandler) {
        *self.ivars().on_click.borrow_mut() = Some(handler);
    }

    pub(super) fn clicked(&self, event: &NSEvent) {
        let point = self.convertPoint_fromView(event.locationInWindow(), None);
        // 先放掉位图的借用：回调里上屏会重画这个视图
        let target = if self.uses_bitmap() {
            self.ivars()
                .bitmap
                .borrow()
                .as_ref()
                .and_then(|bitmap| bitmap.hit(point))
        } else {
            let frame = self.ivars().frame.borrow();
            if frame.calculation && !frame.rows.is_empty() {
                let height = match self.ivars().layout.get() {
                    LayoutMode::Vertical => self.columns(&frame.rows).row_height,
                    LayoutMode::Horizontal => self.items(&frame.rows).1,
                };
                result_hit(
                    point.x,
                    point.y,
                    self.bounds().size.width,
                    self.theme().padding,
                    height,
                )
            } else {
                None
            }
        };
        if target.is_some() {
            tracing::debug!(?target, "点击候选窗口");
        }
        if let Some(handler) = &*self.ivars().on_click.borrow() {
            handler(target);
        }
    }
}

fn result_hit(x: f64, y: f64, width: f64, padding: f64, height: f64) -> Option<HitTarget> {
    (x >= padding / 2.0 && x < width - padding / 2.0 && y >= padding && y < padding + height)
        .then_some(HitTarget::Candidate(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculation_click_excludes_footer_and_padding() {
        assert_eq!(
            result_hit(30.0, 20.0, 200.0, 10.0, 30.0),
            Some(HitTarget::Candidate(0))
        );
        for (x, y) in [(30.0, 45.0), (30.0, 5.0), (0.0, 20.0), (199.0, 20.0)] {
            assert_eq!(result_hit(x, y, 200.0, 10.0, 30.0), None);
        }
    }
}

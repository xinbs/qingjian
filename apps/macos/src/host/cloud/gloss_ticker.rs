//! 候选译文定时器的 Objective-C 回调。

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_foundation::{NSObject, NSObjectProtocol};

define_class!(
    // SAFETY: NSObject 没有子类化要求；没有实现 Drop。
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ()]
    pub(super) struct GlossTicker;

    impl GlossTicker {
        #[unsafe(method(fire:))]
        fn fire(&self, _timer: Option<&AnyObject>) {
            if crate::imk::catch_panic("候选翻译请求", || crate::host::with(|h| h.start_candidate_gloss())).is_none() {
                crate::imk::recover_from_panic(None);
            }
        }

        #[unsafe(method(poll:))]
        fn poll(&self, _timer: Option<&AnyObject>) {
            if crate::imk::catch_panic("候选翻译轮询", || crate::host::with(|h| h.poll_candidate_gloss())).is_none() {
                crate::imk::recover_from_panic(None);
            }
        }
    }

    unsafe impl NSObjectProtocol for GlossTicker {}
);

impl GlossTicker {
    pub(super) fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

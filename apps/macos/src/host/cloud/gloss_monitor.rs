//! 候选译文的防抖与轮询；同一高亮项重画时保持原来的等待时间。

use std::time::{Duration, Instant};

use objc2::rc::Retained;
use objc2::{MainThreadMarker, sel};
use objc2_foundation::NSTimer;

use super::gloss_ticker::GlossTicker;

const DEBOUNCE: f64 = 0.5;
const POLL_INTERVAL: f64 = 0.1;

pub struct GlossMonitor {
    debounce: Option<Retained<NSTimer>>,

    poll: Option<Retained<NSTimer>>,

    focus: Option<(String, usize)>,

    since: Option<Instant>,

    max_wait: Duration,

    mtm: MainThreadMarker,
}

impl GlossMonitor {
    pub fn new(mtm: MainThreadMarker) -> Self {
        Self {
            debounce: None,
            poll: None,
            focus: None,
            since: None,
            max_wait: Duration::ZERO,
            mtm,
        }
    }

    /// 候选与位置不变时不重复计时，云联想或重排的重画不会推迟翻译。
    pub fn schedule(&mut self, text: &str, index: usize) -> bool {
        if self
            .focus
            .as_ref()
            .is_some_and(|(word, i)| word == text && *i == index)
        {
            return false;
        }
        self.stop();
        self.focus = Some((text.to_owned(), index));
        let target = GlossTicker::new(self.mtm);
        self.debounce = Some(unsafe {
            NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
                DEBOUNCE,
                &target,
                sel!(fire:),
                None,
                false,
            )
        });
        true
    }

    pub fn start_polling(&mut self, max_wait: Duration) {
        self.debounce = None;
        self.since = Some(Instant::now());
        self.max_wait = max_wait;
        let target = GlossTicker::new(self.mtm);
        self.poll = Some(unsafe {
            NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
                POLL_INTERVAL,
                &target,
                sel!(poll:),
                None,
                true,
            )
        });
    }

    pub fn expired(&self) -> bool {
        self.since
            .is_some_and(|since| since.elapsed() >= self.max_wait)
    }

    pub fn stop(&mut self) {
        if let Some(timer) = self.debounce.take() {
            timer.invalidate();
        }
        if let Some(timer) = self.poll.take() {
            timer.invalidate();
        }
        self.focus = None;
        self.since = None;
    }
}

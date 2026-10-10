//! 候选译文的异步呈现；翻译资格与查表都交给 Engine。

use std::time::Duration;

use crate::host::Host;
use crate::imk::{modifiers, secure_input};

impl Host {
    pub(in crate::host) fn schedule_candidate_gloss(&mut self) {
        if self.engine.composition().is_empty()
            || modifiers::english_mode()
            || secure_input::enabled()
            || self.translation.is_some()
            || self.notice.is_some()
        {
            self.cancel_candidate_gloss();
            return;
        }
        let index = self.session.highlighted;
        let Some(candidate) = self.session.candidate(index) else {
            self.cancel_candidate_gloss();
            return;
        };
        if candidate.translation.is_some() {
            self.cancel_candidate_gloss();
            return;
        }
        if self.gloss_monitor.schedule(&candidate.text, index) {
            self.engine.cancel_candidate_gloss();
        }
    }

    pub fn cancel_candidate_gloss(&mut self) {
        self.gloss_monitor.stop();
        self.engine.cancel_candidate_gloss();
    }

    pub fn start_candidate_gloss(&mut self) {
        // 安全输入状态可能在停键之后改变，发请求前再查一次。
        if secure_input::enabled()
            || modifiers::english_mode()
            || self.engine.composition().is_empty()
        {
            self.cancel_candidate_gloss();
            return;
        }
        let Some(candidate) = self.session.candidate(self.session.highlighted) else {
            self.cancel_candidate_gloss();
            return;
        };
        if !self.engine.request_candidate_gloss(&candidate) {
            self.gloss_monitor.stop();
            return;
        }
        tracing::debug!("候选云翻译已请求");
        let timeout = self
            .settings
            .config()
            .predict
            .request_timeout_ms()
            .saturating_add(1000);
        self.gloss_monitor
            .start_polling(Duration::from_millis(timeout));
    }

    pub fn poll_candidate_gloss(&mut self) {
        if secure_input::enabled()
            || self.engine.composition().is_empty()
            || modifiers::english_mode()
        {
            self.cancel_candidate_gloss();
            return;
        }
        let learned = self.engine.poll_glosses();
        if learned > 0 && self.engine.refresh_layout_glosses(&mut self.session.layout) {
            // 只改注释，页码与高亮留在用户正在看的位置。
            self.render();
        }
        if self.gloss_monitor.expired() {
            self.gloss_monitor.stop();
        }
    }
}

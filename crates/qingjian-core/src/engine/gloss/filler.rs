use super::FilledGloss;
use crate::candidate::Language;

/// 释义兜底的提供方。实现可以联网，接口非阻塞：`request` 只入队，`poll` 只取已到的。
pub trait GlossFiller: Send {
    /// 是否真的会去问；`false` 时 Engine 不入队。
    fn is_enabled(&self) -> bool {
        true
    }

    /// 请给 `word` 写一条 `language` 的释义。实现自行去重、攒批，问过的不再问。
    fn request(&mut self, language: Language, word: &str);

    /// 为仍在候选窗中的词或句子补译文；实现可优先处理，不必等待上屏词的攒批。
    fn request_candidate(&mut self, language: Language, text: &str) {
        self.request(language, text);
    }

    /// 当前候选已过期，取消候选翻译；上屏词的后台释义请求继续处理。
    fn cancel_candidate(&mut self) {}

    /// 取已到达的释义，没有返回空。
    fn poll(&mut self) -> Vec<FilledGloss>;
}

/// 不兜底。
#[derive(Debug, Clone, Copy, Default)]
pub struct NoGlossFiller;

impl GlossFiller for NoGlossFiller {
    fn is_enabled(&self) -> bool {
        false
    }

    fn request(&mut self, _language: Language, _word: &str) {}

    fn poll(&mut self) -> Vec<FilledGloss> {
        Vec::new()
    }
}

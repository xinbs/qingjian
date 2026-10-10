//! 尚未上屏的候选翻译请求，过期时不再发送。

use std::time::{Duration, Instant};

use qingjian_core::Language;

const MAX_QUEUE_WAIT: Duration = Duration::from_secs(2);

#[derive(Clone)]
pub(in crate::gloss) struct CandidateRequest {
    pub language: Language,

    pub word: String,

    created: Instant,
}

impl CandidateRequest {
    pub fn new(language: Language, word: &str) -> Self {
        Self {
            language,
            word: word.to_owned(),
            created: Instant::now(),
        }
    }

    pub fn expired(&self) -> bool {
        self.created.elapsed() > MAX_QUEUE_WAIT
    }

    pub fn same_text(&self, other: &Self) -> bool {
        self.language == other.language && self.word == other.word
    }
}

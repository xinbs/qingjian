//! 释义请求去重与失败冷却；失败不永久阻止后续重试。

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use qingjian_core::Language;

const RETRY_COOLDOWN: Duration = Duration::from_secs(10);
pub(super) const MAX_SUCCESSES: usize = 4096;

#[derive(Default)]
pub(super) struct RequestCache {
    successes: HashSet<(Language, String)>,

    failures: HashMap<(Language, String), Instant>,
}

impl RequestCache {
    pub fn allows(&mut self, language: Language, word: &str, now: Instant) -> bool {
        self.failures.retain(|_, deadline| *deadline > now);
        let key = (language, word.to_owned());
        !self.successes.contains(&key) && !self.failures.contains_key(&key)
    }

    pub fn record(&mut self, language: Language, word: &str, success: bool, now: Instant) {
        let key = (language, word.to_owned());
        if success {
            if self.successes.len() >= MAX_SUCCESSES {
                self.successes.clear();
            }
            self.failures.remove(&key);
            self.successes.insert(key);
        } else {
            self.failures.insert(key, now + RETRY_COOLDOWN);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failures_can_retry_after_cooldown_but_successes_are_deduplicated() {
        let mut cache = RequestCache::default();
        let now = Instant::now();
        cache.record(Language::English, "关于这件事", false, now);
        assert!(!cache.allows(Language::English, "关于这件事", now));
        assert!(cache.allows(Language::English, "关于这件事", now + RETRY_COOLDOWN));
        cache.record(Language::English, "关于这件事", true, now);
        assert!(!cache.allows(Language::English, "关于这件事", now + RETRY_COOLDOWN));
        assert!(cache.allows(Language::Japanese, "关于这件事", now));
    }
}

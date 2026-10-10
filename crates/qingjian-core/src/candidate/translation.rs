//! 候选词在单一学习语言下的译文、来源与义项。

use serde::{Deserialize, Serialize};

use super::language::Language;
use super::sense::Sense;
use super::translation_source::TranslationSource;

/// 候选词在单一学习语言下的翻译。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Translation {
    /// 译文所属的学习语言。
    pub language: Language,

    /// 译文来源；旧序列化数据缺省为随包释义。
    #[serde(default)]
    pub source: TranslationSource,

    /// 释义，按常用程度排序，最多 [`Self::MAX_SENSES`] 条。
    senses: Vec<Sense>,
}

impl Translation {
    /// 释义上限。超出的部分在构造时丢弃，保证候选框不会退化成迷你词典。
    pub const MAX_SENSES: usize = 2;

    pub fn new(language: Language, mut senses: Vec<Sense>) -> Self {
        senses.truncate(Self::MAX_SENSES);
        Self {
            language,
            source: TranslationSource::Bundled,
            senses,
        }
    }

    pub fn senses(&self) -> &[Sense] {
        &self.senses
    }

    pub fn senses_mut(&mut self) -> &mut [Sense] {
        &mut self.senses
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::candidate::PartOfSpeech;

    #[test]
    fn old_serialized_translation_defaults_to_bundled() {
        let translation: Translation = toml::from_str("language = 'English'\nsenses = []").unwrap();
        assert_eq!(translation.source, TranslationSource::Bundled);
        let cloud: Translation =
            toml::from_str("language = 'English'\nsource = 'cloud'\nsenses = []").unwrap();
        assert_eq!(cloud.source, TranslationSource::Cloud);
    }

    #[test]
    fn keeps_at_most_two_senses() {
        let senses = ["develop", "development", "exploit"]
            .into_iter()
            .map(|text| Sense {
                part_of_speech: Some(PartOfSpeech::Verb),
                text: text.to_owned(),
                reading: None,
                fresh: false,
            })
            .collect();
        let translation = Translation::new(Language::English, senses);
        assert_eq!(translation.senses().len(), Translation::MAX_SENSES);
    }
}

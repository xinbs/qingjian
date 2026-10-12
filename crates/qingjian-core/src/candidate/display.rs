//! 英文译文前置的展示资格；不改变候选内容、排序或提交行为。

use super::{Candidate, CandidateKind, Language, Sense, TranslationSource};

impl Candidate {
    /// 中文候选已有英文释义时，提供可放在主显示位置的第一义项。
    pub fn english_display_translation(&self) -> Option<(&Sense, TranslationSource)> {
        if !matches!(
            self.kind,
            CandidateKind::Chinese
                | CandidateKind::Cloud
                | CandidateKind::Code
                | CandidateKind::Sentence
                | CandidateKind::Generated
        ) {
            return None;
        }
        let translation = self.translation.as_ref()?;
        if translation.language != Language::English {
            return None;
        }
        let sense = translation.senses().first()?;
        (!sense.text.trim().is_empty()).then_some((sense, translation.source))
    }
}

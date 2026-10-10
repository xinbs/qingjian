//! 候选译文标注与异步兜底入口；查表与联网请求分开，不阻塞输入。

use std::time::Instant;

use crate::candidate::{Candidate, CandidateKind, CandidateLayout, CandidateList, Language};
use crate::engine::Engine;
use crate::engine::annotation::AnnotationReport;

impl Engine {
    /// 给候选补上译文。与 [`Self::query`] 分开调用，平台层可以先画候选再补画译文。
    pub fn annotate(&self, list: &mut CandidateList) -> AnnotationReport {
        let start = Instant::now();
        let mut hits = 0;
        for candidate in &mut list.items {
            self.annotate_candidate(candidate);
            hits += usize::from(candidate.translation.is_some());
        }
        AnnotationReport {
            total: list.items.len(),
            hits,
            elapsed: start.elapsed(),
        }
    }

    /// 为缺少译文的当前候选入队；壳在选中项稳定后调用，返回是否交给了兜底提供方。
    /// 去重由提供方负责。私密输入、关闭学习语言或已有译文时不联网。
    pub fn request_candidate_gloss(&mut self, candidate: &Candidate) -> bool {
        let language = self.translator.language();
        if self.private
            || language == Language::Chinese
            || !self.gloss_filler.is_enabled()
            || candidate.text.trim().is_empty()
            || !matches!(
                candidate.kind,
                CandidateKind::Chinese
                    | CandidateKind::Cloud
                    | CandidateKind::Code
                    | CandidateKind::Sentence
                    | CandidateKind::Generated
            )
            || candidate
                .translation
                .as_ref()
                .is_some_and(|translation| translation.language == language)
        {
            return false;
        }
        let text = self.gloss_lookup_text(candidate);
        if self.translator.translate(&text).is_some() {
            return false;
        }
        self.gloss_filler.request_candidate(language, &text);
        true
    }

    /// 输入或选中项改变时取消过期的候选翻译，不影响上屏词的后台释义。
    pub fn cancel_candidate_gloss(&mut self) {
        self.gloss_filler.cancel_candidate();
    }

    /// 后台译文入表后只补当前布局的标注，不重查、重排或改变格子位置。
    pub fn refresh_layout_glosses(&self, layout: &mut CandidateLayout) -> bool {
        let mut changed = false;
        for candidate in layout.candidates_mut() {
            let before = candidate.translation.clone();
            self.annotate_candidate(candidate);
            changed |= before != candidate.translation;
        }
        changed
    }

    fn annotate_candidate(&self, candidate: &mut Candidate) {
        if matches!(
            candidate.kind,
            CandidateKind::Custom(_) | CandidateKind::Shortcut | CandidateKind::Emoji
        ) {
            candidate.translation = None;
            return;
        }
        let translator = if candidate.kind == CandidateKind::English {
            &self.english_translator
        } else {
            &self.translator
        };
        let language = translator.language();
        if language == Language::Chinese && candidate.kind != CandidateKind::English {
            candidate.translation = None;
            return;
        }
        // 重画不能擦掉仍属于当前语言的云译文；切换学习语言时则移除旧标注。
        let existing = candidate
            .translation
            .take()
            .filter(|translation| translation.language == language);
        let text = self.gloss_lookup_text(candidate);
        candidate.translation = existing.or_else(|| {
            translator.translate(&text).or_else(|| {
                (candidate.kind == CandidateKind::English)
                    .then(|| translator.translate(&text.to_ascii_lowercase()))
                    .flatten()
            })
        });
        if candidate.kind != CandidateKind::English
            && let Some(translation) = &mut candidate.translation
        {
            self.mark_fresh(translation);
        }
    }

    fn gloss_lookup_text(&self, candidate: &Candidate) -> String {
        if self.traditional
            && let Some(simplified) = self.traditional_map.borrow().get(&candidate.text)
        {
            return simplified.clone();
        }
        candidate.text.clone()
    }
}

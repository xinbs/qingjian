//! 候选行的展示投影；交换译文位置时不修改会话中的原候选。

use super::Tone;
use qingjian_core::{Candidate, TranslationSource};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// 显示用序号文本，如 `1`。
    pub index: String,

    /// 候选词。
    pub text: String,

    /// 右侧 annotation，按顺序绘制；没有译文时为空。
    pub annotation: Vec<(String, Tone)>,

    /// 来自云联想：词前画一个小云朵，与本地候选区分。
    pub cloud: bool,
}

impl Row {
    /// 仅交换展示位置；会话始终保留原中文候选，键盘选词照常提交中文。
    pub fn with_english_first(position: usize, candidate: &Candidate, enabled: bool) -> Self {
        let mut row = Self::from_candidate(position, candidate);
        if enabled && let Some((sense, source)) = candidate.english_display_translation() {
            row.text = sense.text.clone();
            row.annotation = vec![(candidate.text.clone(), Tone::Gloss)];
            row.cloud = source == TranslationSource::Cloud;
        }
        row
    }

    pub fn from_candidate(position: usize, candidate: &Candidate) -> Self {
        let mut annotation = Vec::new();
        // 读音（问字模式答案的带声调拼音）放在最前
        if let Some(reading) = &candidate.reading {
            annotation.push((reading.clone(), Tone::Gloss));
        }
        if let Some(translation) = &candidate.translation {
            for (i, sense) in translation.senses().iter().enumerate() {
                if i > 0 || !annotation.is_empty() {
                    annotation.push((" · ".to_owned(), Tone::Separator));
                }
                if let Some(pos) = sense.part_of_speech {
                    annotation.push((format!("{pos} "), Tone::Pos));
                }
                // 日文译词按汉字段注平假名（開発(かいはつ)する），假名淡色
                let tone = if translation.source == TranslationSource::Cloud {
                    Tone::CloudGloss
                } else if sense.fresh {
                    Tone::Fresh
                } else {
                    Tone::Gloss
                };
                for segment in sense.furigana() {
                    annotation.push((segment.text, tone));
                    if let Some(reading) = segment.reading {
                        annotation.push((format!("({reading})"), Tone::Faint));
                    }
                }
            }
        }
        Self {
            index: (position + 1).to_string(),
            text: if matches!(candidate.kind, qingjian_core::CandidateKind::Custom(_)) {
                qingjian_core::CustomPhrase::preview(&candidate.text, 60)
            } else {
                candidate.text.clone()
            },
            annotation,
            cloud: false,
        }
    }
}

//! 候选窗口的一行：序号、候选词、annotation 片段。只是 Core 输出的展示形态，不含任何排序或查词。

use qingjian_core::{Candidate, TranslationSource};

/// annotation 片段的种类，决定颜色；青简渲染器的主题还按它挑着画。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// 译文。
    Gloss,

    /// 云端译文，包括个人释义缓存；颜色优先于生词强调色。
    CloudGloss,

    /// 生词的译文（用户还没在候选里见过几轮，`Sense::fresh`），用强调色。
    Fresh,

    /// 其余的淡色片段（假名注音），最浅。
    Faint,

    /// 词性，与 `Faint` 同色。
    Pos,

    /// 义项之间的分隔，与 `Faint` 同色。
    Separator,
}

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

#[cfg(test)]
mod tests {
    use super::{Row, Tone};
    use qingjian_core::{
        Candidate, CandidateKind, Language, PartOfSpeech, Sense, Translation, TranslationSource,
    };

    #[test]
    fn phrase_tag_is_independent_from_cloud_and_fresh_colors() {
        let mut candidate = Candidate {
            text: "关于这件事".to_owned(),
            kind: CandidateKind::Sentence,
            syllables: Vec::new(),
            reading: None,
            translation: Some(Translation::new(
                Language::English,
                vec![Sense {
                    part_of_speech: Some(PartOfSpeech::Phrase),
                    text: "About this matter".to_owned(),
                    reading: None,
                    fresh: false,
                }],
            )),
            aux_code: None,
        };
        let local = Row::from_candidate(0, &candidate);
        assert_eq!(local.annotation[0], ("phr. ".to_owned(), Tone::Pos));
        assert_eq!(local.annotation[1].1, Tone::Gloss);
        candidate.translation.as_mut().unwrap().senses_mut()[0].fresh = true;
        assert_eq!(
            Row::from_candidate(0, &candidate).annotation[1].1,
            Tone::Fresh
        );
        candidate.translation.as_mut().unwrap().source = TranslationSource::Cloud;
        assert_eq!(
            Row::from_candidate(0, &candidate).annotation[1].1,
            Tone::CloudGloss
        );
        assert!(candidate.translation.as_ref().unwrap().senses()[0].fresh);
        assert!(!Row::from_candidate(0, &candidate).cloud);
    }
}

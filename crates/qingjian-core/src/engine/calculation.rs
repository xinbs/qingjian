//! 即时计算门面：平台只判断按键是否交给缓冲区，不承担算式语法或求值。

use std::time::{Duration, Instant};

use super::{Engine, Query, Timings};
use crate::calculator;
use crate::candidate::{Candidate, CandidateKind, CandidateList};

impl Engine {
    /// 中英文模式下数字开头进入计算缓冲区；已输入等号时数字恢复为选词键。
    /// 普通标点不能单独开启计算，以免影响中文标点和英文直输段。
    pub fn calculation_accepts(&self, c: char) -> bool {
        if (self.zhuyin && !self.english_mode) || self.aux_code.is_some() {
            return false;
        }
        let text = self.composition.text();
        if text.is_empty() {
            return c.is_ascii_digit();
        }
        self.is_calculating()
            && text.len() < calculator::MAX_INPUT_BYTES
            && !(text.ends_with('=') && self.composition.cursor() == text.len())
            && calculator::is_character(c)
            && (c != '=' || !text.contains('='))
    }

    /// 当前整段缓冲区是否为数字开头的计算输入，包括尚未输入等号的普通数字。
    pub fn is_calculating(&self) -> bool {
        (!self.zhuyin || self.english_mode) && calculator::is_calculation(self.composition.text())
    }

    /// 回车提交完整等式；无可用结果时保持输入，让平台按原文上屏。
    pub fn commit_calculation_with_expression(&mut self) -> Option<String> {
        if !self.is_calculating() {
            return None;
        }
        let result = calculator::result(self.composition.text())?;
        let candidate = Candidate {
            text: format!("{}{result}", self.raw_preedit().text),
            kind: CandidateKind::Shortcut,
            syllables: Vec::new(),
            reading: None,
            translation: None,
            aux_code: None,
        };
        Some(self.commit(&candidate))
    }

    pub(super) fn query_calculation(&self, start: Instant) -> Query {
        let text = self.composition.text();
        let items = calculator::result(text)
            .map(|result| Candidate {
                text: result,
                kind: CandidateKind::Shortcut,
                syllables: Vec::new(),
                reading: None,
                translation: None,
                aux_code: None,
            })
            .into_iter()
            .collect();
        Query {
            candidates: CandidateList { items },
            tail: text.to_owned(),
            text: text.to_owned(),
            cursor: self.composition.cursor(),
            timings: Timings {
                parse: start.elapsed(),
                lookup: Duration::ZERO,
                rank: Duration::ZERO,
            },
            ..Query::default()
        }
    }
}

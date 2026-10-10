//! 算式结果的小框；表达式保留在输入位置，只有可用结果才显示。

use qingjian_core::Candidate;

use crate::candidates::{Frame, Row};
use crate::host::Host;

impl Host {
    pub(super) fn render_calculation(&mut self) {
        self.cancel_candidate_gloss();
        self.engine.note_displayed(std::iter::empty());
        let frame = calculation_frame(self.session.layout.candidate(0), self.input_mode());
        self.window.show(frame, self.anchor);
    }
}

fn calculation_frame(candidate: Option<&Candidate>, mode: qingjian_render::Mode) -> Frame {
    let Some(candidate) = candidate else {
        return Frame::default();
    };
    let mut row = Row::from_candidate(0, candidate);
    row.index.clear();
    Frame {
        calculation: true,
        rows: vec![row],
        highlighted: 0,
        footer: Some("点击或 1 使用 · ↵ 原文".to_owned()),
        mode,
        ..Frame::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qingjian_core::{CandidateKind, Engine};
    use qingjian_dictionary::Dictionary;

    #[test]
    fn only_the_result_is_shown_without_preedit_or_candidate_number() {
        let mut engine = Engine::new(Dictionary::parse("").unwrap());
        engine.set_english_mode(true);
        engine.set_input("1+1=");
        let query = engine.query().unwrap();
        let frame = calculation_frame(
            query.candidates.items.first(),
            qingjian_render::Mode::default(),
        );
        assert!(frame.calculation);
        assert!(frame.preedit.is_none());
        assert_eq!(frame.rows.len(), 1);
        assert_eq!(frame.rows[0].text, "2");
        assert!(frame.rows[0].index.is_empty());
        assert_eq!(query.candidates.items[0].kind, CandidateKind::Shortcut);
        assert!(!frame.is_empty());
        assert!(frame.footer.as_deref().unwrap().contains("原文"));
    }

    #[test]
    fn incomplete_or_invalid_calculations_do_not_show_a_result_box() {
        let mut engine = Engine::new(Dictionary::parse("").unwrap());
        for expression in ["123", "1+1", "1/0="] {
            engine.set_input(expression);
            let query = engine.query().unwrap();
            let frame = calculation_frame(
                query.candidates.items.first(),
                qingjian_render::Mode::default(),
            );
            assert!(frame.is_empty());
            assert_eq!(engine.take_raw(), expression);
        }
    }
}

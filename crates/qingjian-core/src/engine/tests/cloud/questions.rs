//! 云联想的问字行为测试。

use super::super::*;

#[test]
fn question_mode_asks_the_cloud_and_shows_answers_unvalidated() {
    let submitted = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let answer = CloudWord {
        text: "森".into(),
        syllables: Vec::new(),
        reading: Some("sēn".into()),
    };
    let restated = CloudWord {
        text: "木木木是什么字".into(),
        syllables: Vec::new(),
        reading: None,
    };
    let predictor = EchoPredictor {
        submitted: submitted.clone(),
        replies: vec![Prediction {
            sequence: 1,
            words: vec![restated, answer],
            sentence: None,
        }],
        sentence: true,
    };
    let mut engine = self::engine().with_predictor(Box::new(predictor));
    // 缺省 `?` 不是入口：开了开关才进问字
    assert!(!engine.takes_question_mark());
    engine.set_mode_keys(ModeKeys {
        question_mark: true,
        ..ModeKeys::default()
    });
    assert!(engine.takes_question_mark());
    engine.push('?');
    assert!(engine.bare_question());
    assert!(engine.question_mode());
    for c in "mumumu".chars() {
        engine.push(c);
    }
    assert!(!engine.bare_question());
    let query = engine.query().unwrap();
    assert!(query.candidates.items.is_empty());
    assert_eq!(query.marked_text(), "?mu'mu'mu");
    assert_eq!(query.marked_cursor(), 9);

    assert_eq!(engine.request_prediction(None, &[]), Some(1));
    let request = submitted.borrow()[0].clone();
    assert_eq!(request.kind, PredictionKind::Question);
    assert_eq!(request.pinyin, "mu'mu'mu");
    assert_eq!(request.letters, "mumumu");
    assert!(!request.want_sentence);

    // 答案的拼音与敲的字母对不上，但问字模式不校验；复述问题的「答案」剔掉
    let prediction = engine.poll_prediction().unwrap();
    assert_eq!(prediction.words.len(), 1);
    assert_eq!(prediction.words[0].text, "森");
    let answer = prediction
        .words
        .into_iter()
        .next()
        .unwrap()
        .into_candidate();
    assert_eq!(answer.reading.as_deref(), Some("sēn"));
    assert_eq!(engine.commit(&answer), "森");
    assert!(engine.composition().is_empty());
}

#[test]
fn question_key_answers_code_points_locally_and_keeps_question_mark_alias() {
    let submitted = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let mut engine = engine().with_predictor(Box::new(EchoPredictor {
        submitted: submitted.clone(),
        replies: Vec::new(),
        sentence: false,
    }));
    engine.set_input("u4e00");
    assert!(engine.question_mode());
    assert!(engine.unicode_entry());
    let query = engine.query().unwrap();
    assert_eq!(query.candidates.items[0].text, "一");
    assert_eq!(query.candidates.items[0].kind, CandidateKind::Shortcut);
    assert_eq!(query.tail, "u4e00");
    // 码点本地就答，不问云端
    assert!(
        engine
            .request_prediction(None, &query.candidates.items)
            .is_none()
    );
    assert!(submitted.borrow().is_empty());

    engine.set_input("usangemu");
    assert!(engine.question_mode());
    assert!(!engine.unicode_entry());
    let query = engine.query().unwrap();
    assert!(query.candidates.items.is_empty());
    let body = query.tail.strip_prefix('u').unwrap().to_owned();
    assert!(!body.is_empty());

    // `?` 缺省不是入口：`?sangemu` 是英文直输段而不是问题
    engine.set_input("?sangemu");
    assert!(!engine.question_mode() && engine.raw_mode());
    // 开了开关才是别名：同一个问题、同样的切分，只是前缀不同
    engine.set_mode_keys(ModeKeys {
        question_mark: true,
        ..ModeKeys::default()
    });
    assert!(engine.question_mode());
    assert_eq!(engine.query().unwrap().tail, format!("?{body}"));

    // 换成 i 问字、v 表达式后 u 就是普通字母
    engine.set_mode_keys(ModeKeys {
        expression: 'v',
        question: 'i',
        question_mark: false,
    });
    engine.set_input("u4e00");
    assert!(!engine.question_mode());
    engine.set_input("i4e00");
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "一");
    // 非法组合退回缺省
    engine.set_mode_keys(ModeKeys {
        expression: 'u',
        question: 'u',
        question_mark: false,
    });
    assert_eq!(engine.mode_keys(), ModeKeys::default());
}

#[test]
fn bare_question_restores_punctuation_once_in_each_mode() {
    for (english, full_width, expected) in [
        (false, false, "?"),
        (false, true, "？"),
        (true, false, "?"),
        (true, true, "?"),
    ] {
        let mut engine = engine();
        engine.set_full_width_punctuation(full_width);
        engine.push('?');
        assert_eq!(
            engine.restore_bare_question(english).as_deref(),
            Some(expected)
        );
        assert!(engine.composition().is_empty());
        assert_eq!(engine.history().text(), expected);
        assert_eq!(engine.passthrough_pending, expected);
        assert_eq!(engine.restore_bare_question(english), None);
        assert_eq!(engine.history().text(), expected);
        assert_eq!(engine.passthrough_pending, expected);
    }
}

#[test]
fn restoring_question_preserves_other_compositions() {
    for input in ["", "nihao", "?nihao"] {
        let mut engine = engine();
        engine.set_input(input);
        assert_eq!(engine.restore_bare_question(false), None);
        assert_eq!(engine.composition().text(), input);
        assert!(engine.history().text().is_empty());
        assert!(engine.passthrough_pending.is_empty());
    }
}

//! 即时算式候选与按键分派回归：数字原文、等号触发、错误处理和完整上屏。

use super::{CountingLearner, EchoPredictor, engine};
use crate::candidate::CandidateKind;
use crate::engine::Learner;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[test]
fn calculation_starts_with_digits_and_waits_for_equals() {
    let mut engine = engine();
    assert!(engine.calculation_accepts('1'));
    for c in ['(', '-', '.', '+', 'x', '='] {
        assert!(!engine.calculation_accepts(c));
    }
    for c in "1+1".chars() {
        assert!(engine.calculation_accepts(c));
        engine.push(c);
    }
    assert!(engine.is_calculating());
    let query = engine.query().unwrap();
    assert_eq!(query.marked_text(), "1+1");
    assert!(query.candidates.items.is_empty());
    assert!(engine.calculation_accepts('='));
    engine.push('=');
    let query = engine.query().unwrap();
    assert_eq!(query.marked_text(), "1+1=");
    assert_eq!(query.candidates.items.len(), 1);
    assert_eq!(query.candidates.items[0].text, "2");
    assert_eq!(query.candidates.items[0].kind, CandidateKind::Shortcut);
    assert!(!engine.calculation_accepts('1'));
}

#[test]
fn calculation_respects_input_modes_and_candidate_selection() {
    let mut engine = engine();
    engine.set_input("kaifa");
    assert!(!engine.calculation_accepts('1'));
    assert!(!engine.is_calculating());
    engine.clear();
    engine.set_english_mode(true);
    assert!(engine.calculation_accepts('1'));
    engine.set_input("1+1=");
    assert!(engine.is_calculating());
    engine.set_english_mode(false);
    engine.set_zhuyin_mode(true);
    assert!(!engine.calculation_accepts('1'));
    assert!(!engine.is_calculating());
}

#[test]
fn calculation_results_cover_precedence_unary_and_decimals() {
    let mut engine = engine();
    for (input, expected) in [
        ("1+2*3=", "7"),
        ("2*(3+4)=", "14"),
        ("1+-3=", "-2"),
        ("0.1+0.2=", "0.3"),
        ("7%3=", "1"),
        ("2^3^2=", "512"),
        ("2^-2=", "0.25"),
        ("1+-2^2=", "-3"),
    ] {
        engine.set_input(input);
        assert_eq!(engine.query().unwrap().candidates.items[0].text, expected);
    }
}

#[test]
fn invalid_calculations_remain_available_as_original_text() {
    let mut engine = engine();
    for input in ["1/0=", "1%=", "1+(2=", "1.2.3=", "2^9999=", "1+1=="] {
        engine.set_input(input);
        let query = engine.query().unwrap();
        assert!(query.candidates.items.is_empty(), "{input}");
        assert_eq!(query.marked_text(), input);
        assert_eq!(engine.take_raw(), input);
    }
}

#[test]
fn calculation_can_be_edited_and_committed_without_learning() {
    let mut engine = engine().with_learner(Box::new(CountingLearner(HashMap::new())));
    engine.set_input("1+1=");
    engine.move_cursor_left();
    engine.move_cursor_left();
    assert!(engine.calculation_accepts('2'));
    engine.push('2');
    let query = engine.query().unwrap();
    assert_eq!(query.marked_text(), "1+21=");
    assert_eq!(query.marked_cursor(), 3);
    let candidate = &query.candidates.items[0];
    assert_eq!(engine.commit(candidate), "22");
    assert!(engine.composition().is_empty());
    assert_eq!(engine.learner.weight("22"), 0);
}

#[test]
fn calculations_never_request_cloud_predictions() {
    let submitted = Rc::new(RefCell::new(Vec::new()));
    let mut engine = engine().with_predictor(Box::new(EchoPredictor {
        submitted: submitted.clone(),
        replies: Vec::new(),
        sentence: true,
    }));
    for input in ["1", "1+1", "1+1="] {
        engine.set_input(input);
        let query = engine.query().unwrap();
        assert_eq!(
            engine.request_prediction(None, &query.candidates.items),
            None
        );
    }
    assert!(submitted.borrow().is_empty());
}

#[test]
fn excessive_calculation_input_does_not_accept_more_keys() {
    let mut engine = engine();
    engine.set_input(&"1".repeat(128));
    assert!(!engine.calculation_accepts('+'));
    engine.set_input(&format!("1+{}1{}=", "(".repeat(40), ")".repeat(40)));
    assert!(engine.query().unwrap().candidates.items.is_empty());
}

#[test]
fn english_calculations_offer_result_and_preserve_original_expression() {
    let mut engine = engine();
    engine.set_english_mode(true);
    for c in "1+1=".chars() {
        assert!(engine.calculation_accepts(c));
        engine.push(c);
    }
    let query = engine.query().unwrap();
    assert_eq!(query.marked_text(), "1+1=");
    assert_eq!(
        query
            .candidates
            .items
            .iter()
            .map(|c| c.text.as_str())
            .collect::<Vec<_>>(),
        ["2"]
    );
    assert!(
        query
            .candidates
            .items
            .iter()
            .all(|c| c.kind == CandidateKind::Shortcut)
    );
    assert_eq!(engine.take_raw(), "1+1=");
    assert!(engine.composition().is_empty());
    engine.set_input("1+1=");
    assert_eq!(
        engine.commit(&engine.query().unwrap().candidates.items[0]),
        "2"
    );
    assert!(engine.composition().is_empty());
}

#[test]
fn english_calculation_overrides_english_candidates_and_zhuyin_keys() {
    let mut engine = engine();
    engine.set_zhuyin_mode(true);
    engine.set_english_mode(true);
    assert!(engine.calculation_accepts('2'));
    engine.set_input("2X3=");
    let query = engine.query().unwrap();
    assert_eq!(query.candidates.items[0].text, "6");
    engine.move_cursor_left();
    engine.move_cursor_left();
    assert_eq!(engine.take_raw(), "2X3=");
    assert!(engine.composition().is_empty());
    engine.set_input("123");
    assert!(engine.query().unwrap().candidates.items.is_empty());
    assert_eq!(engine.take_raw(), "123");
}

#[test]
fn english_calculations_do_not_request_cloud_or_learn_words() {
    let submitted = Rc::new(RefCell::new(Vec::new()));
    let mut engine = engine()
        .with_learner(Box::new(CountingLearner(HashMap::new())))
        .with_predictor(Box::new(EchoPredictor {
            submitted: submitted.clone(),
            replies: Vec::new(),
            sentence: true,
        }));
    engine.set_english_mode(true);
    engine.set_input("2*(3+4)=");
    let query = engine.query().unwrap();
    assert_eq!(
        engine.request_prediction(None, &query.candidates.items),
        None
    );
    assert!(submitted.borrow().is_empty());
    assert_eq!(engine.commit(&query.candidates.items[0]), "14");
    assert_eq!(engine.learner.weight("14"), 0);
}

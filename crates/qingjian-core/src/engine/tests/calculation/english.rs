//! 英文模式即时算式与原式保留回归。

use super::super::{CountingLearner, EchoPredictor, engine};
use crate::candidate::CandidateKind;
use crate::engine::Learner;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

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

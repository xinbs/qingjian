//! 回车提交完整等式，保留输入原式且不触发词语学习。

use super::super::{CountingLearner, engine};
use crate::engine::Learner;
use std::collections::HashMap;

#[test]
fn calculation_return_keeps_expression_and_appends_result_in_both_modes() {
    let mut engine = engine();
    for english in [false, true] {
        engine.set_english_mode(english);
        engine.set_input("1+333=");
        engine.query().unwrap();
        assert_eq!(
            engine.commit_calculation_with_expression().as_deref(),
            Some("1+333=334")
        );
        assert!(engine.composition().is_empty());
        assert_eq!(engine.commit_calculation_with_expression(), None);
    }
}

#[test]
fn complete_equation_uses_current_input_even_with_cursor_in_the_middle() {
    let mut engine = engine();
    engine.set_english_mode(true);
    engine.set_input("2X3=");
    engine.query().unwrap();
    engine.move_cursor_left();
    engine.move_cursor_left();
    assert_eq!(
        engine.commit_calculation_with_expression().as_deref(),
        Some("2X3=6")
    );
    engine.set_input("1+1=");
    engine.query().unwrap();
    engine.backspace();
    engine.backspace();
    engine.push('2');
    engine.push('=');
    assert_eq!(
        engine.commit_calculation_with_expression().as_deref(),
        Some("1+2=3")
    );
}

#[test]
fn invalid_or_unfinished_calculation_stays_available_for_raw_commit() {
    let mut engine = engine();
    for input in ["123", "1+333", "1/0=", "nihao", "", "v1+2"] {
        engine.set_input(input);
        assert_eq!(engine.commit_calculation_with_expression(), None);
        assert_eq!(engine.take_raw(), input);
    }
}

#[test]
fn complete_equations_do_not_learn_words_and_result_only_selection_is_unchanged() {
    let mut engine = engine().with_learner(Box::new(CountingLearner(HashMap::new())));
    engine.set_english_mode(true);
    engine.set_input("1+333=");
    assert_eq!(
        engine.commit_calculation_with_expression().as_deref(),
        Some("1+333=334")
    );
    assert_eq!(engine.learner.weight("1+333=334"), 0);
    engine.set_input("1+333=");
    let result = engine.query().unwrap().candidates.items.remove(0);
    assert_eq!(engine.commit(&result), "334");
    assert_eq!(engine.learner.weight("334"), 0);
    engine.set_input("1+333=");
    assert_eq!(engine.take_raw(), "1+333=");
}

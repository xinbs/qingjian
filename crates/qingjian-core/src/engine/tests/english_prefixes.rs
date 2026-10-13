//! 英文词首字母不能触发中文模式的问字或表达式入口。

use super::engine;
use crate::{CandidateKind, ModeKeys};
use qingjian_dictionary::WordList;

#[test]
fn typing_update_keeps_english_mode_and_unsegmented_marked_text() {
    let mut engine = engine().with_english(WordList::parse("update\tupdate\t5000\n").unwrap());
    engine.set_english_mode(true);
    for letter in "update".chars() {
        // macOS 组词分派：已经进入问字时才切到中文问题处理。
        let question = !engine.composition().is_empty() && engine.question_mode();
        assert!(
            !question,
            "英文缓冲区 {} 误入问字",
            engine.composition().text()
        );
        engine.set_english_mode(!question);
        engine.push(letter);
        let query = engine.query().unwrap();
        assert_eq!(query.marked_text(), engine.composition().text());
        assert!(!query.marked_text().contains('\''));
    }
    let query = engine.query().unwrap();
    assert_eq!(query.candidates.items[0].text, "update");
    assert_eq!(query.candidates.items[0].kind, CandidateKind::English);
    assert_eq!(engine.commit(&query.candidates.items[0]), "update");
}

#[test]
fn english_u_and_v_words_do_not_trigger_chinese_mode_shortcuts() {
    let mut engine = engine();
    engine.set_english_mode(true);
    for word in ["u", "update", "user", "u4e00", "v", "version", "value"] {
        engine.set_input(word);
        assert!(!engine.question_mode(), "{word}");
        assert!(!engine.expression_mode(), "{word}");
        assert!(!engine.unicode_entry(), "{word}");
    }
}

#[test]
fn chinese_shortcuts_and_explicit_question_mark_still_work() {
    let mut engine = engine();
    engine.set_input("u4e00");
    assert!(engine.question_mode());
    assert!(engine.unicode_entry());
    engine.set_input("v1+2");
    assert!(engine.expression_mode());
    engine.set_mode_keys(ModeKeys {
        question_mark: true,
        ..ModeKeys::default()
    });
    engine.set_english_mode(true);
    engine.set_input("?4e00");
    assert!(engine.question_mode());
    assert!(engine.unicode_entry());
}

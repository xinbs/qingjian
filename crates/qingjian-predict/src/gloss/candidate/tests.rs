//! 候选整句译文的长度、语言及严格响应校验。

use super::prompt::{parse_reply, system_prompt, user_prompt};
use qingjian_core::Language;

#[test]
fn translates_complete_sentence_with_punctuation_and_over_old_limits() {
    let word = "关于这件事，我们明天再详细讨论吧";
    let translation = "About this matter, let's discuss it in more detail tomorrow.";
    assert!(translation.len() > 40);
    assert!(translation.split_whitespace().count() > 4);
    let content = serde_json::json!({"w":word,"t":translation}).to_string();
    let result = parse_reply(&content, Language::English, word).unwrap();
    assert_eq!(result.translation.senses()[0].text, translation);
}

#[test]
fn rejects_unknown_source_explanations_invalid_json_and_controls() {
    let word = "关于这件事";
    for content in [
        r#"{"w":"别的句子","t":"About this matter"}"#,
        r#"{"w":"关于这件事","t":"About this matter","explanation":"说明"}"#,
        r#"{"w":"关于这件事","t":"About this matter\nexplanation"}"#,
        r#"{"w":"关于这件事","t":"关于这件事"}"#,
        "not JSON",
    ] {
        assert!(parse_reply(content, Language::English, word).is_none());
    }
    let oversized = serde_json::json!({"w":word,"t":"a".repeat(513)}).to_string();
    assert!(parse_reply(&oversized, Language::English, word).is_none());
}

#[test]
fn retains_sentence_translation_in_all_learning_languages() {
    let word = "关于这件事，我们明天再讨论吧";
    for (language, text) in [
        (
            Language::Japanese,
            "この件については、明日また話し合いましょう。",
        ),
        (
            Language::Spanish,
            "Sobre este asunto, hablemos de nuevo mañana.",
        ),
        (
            Language::Vietnamese,
            "Về chuyện này, chúng ta hãy thảo luận lại vào ngày mai.",
        ),
    ] {
        let content = serde_json::json!({"w":word,"t":text}).to_string();
        let result = parse_reply(&content, language, word).unwrap();
        assert_eq!(result.translation.language, language);
        assert_eq!(result.translation.senses()[0].text, text);
        assert!(system_prompt(language).contains("完整"));
    }
    assert!(user_prompt(word).contains(word));
}

#[test]
fn allows_common_technical_sentence_punctuation() {
    let word = "服务成功率百分之九十九，费用五欧元";
    let text = "The success rate (API [v1]) is 99%; cost = €5 + $2 & £1. #test @home";
    let content = serde_json::json!({"w":word,"t":text}).to_string();
    assert_eq!(
        parse_reply(&content, Language::English, word)
            .unwrap()
            .translation
            .senses()[0]
            .text,
        text
    );
}

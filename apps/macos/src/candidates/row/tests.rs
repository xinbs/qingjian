//! 英文译文前置的显示、回退与正常中文提交回归。

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

fn candidate() -> Candidate {
    Candidate {
        text: "请客".to_owned(),
        kind: CandidateKind::Chinese,
        syllables: vec!["qing".to_owned(), "ke".to_owned()],
        reading: None,
        translation: Some(Translation::new(
            Language::English,
            vec![
                Sense {
                    part_of_speech: Some(PartOfSpeech::Verb),
                    text: "treat someone to a meal".to_owned(),
                    reading: None,
                    fresh: false,
                },
                Sense {
                    part_of_speech: Some(PartOfSpeech::Verb),
                    text: "pick up the tab".to_owned(),
                    reading: None,
                    fresh: false,
                },
            ],
        )),
        aux_code: None,
    }
}

#[test]
fn english_first_only_changes_display_and_keeps_keyboard_commit_chinese() {
    let original = candidate();
    let snapshot = original.clone();
    let row = Row::with_english_first(0, &original, true);
    assert_eq!(row.text, "treat someone to a meal");
    assert_eq!(row.annotation, vec![("请客".to_owned(), Tone::Gloss)]);
    assert_eq!(row.index, "1");
    assert!(!row.cloud);
    assert_eq!(original, snapshot);
    let mut engine = qingjian_core::Engine::new(
        qingjian_core::dictionary::Dictionary::parse("请客\tqing ke\t1000\n").unwrap(),
    );
    engine.set_input("qingke");
    assert_eq!(engine.commit(&original), "请客");
    assert!(engine.composition().is_empty());
    engine.set_input("qingke");
    assert_eq!(
        engine.commit_translation(&original, 0).as_deref(),
        Some("treat someone to a meal")
    );
    assert!(engine.composition().is_empty());
    engine.set_input("qingke");
    assert_eq!(
        engine.commit_translation(&original, 1).as_deref(),
        Some("pick up the tab")
    );
    assert!(engine.composition().is_empty());
}

#[test]
fn disabling_english_first_restores_both_senses_and_cloud_color_survives() {
    let mut candidate = candidate();
    assert_eq!(
        Row::with_english_first(0, &candidate, false),
        Row::from_candidate(0, &candidate)
    );
    candidate.translation.as_mut().unwrap().source = TranslationSource::Cloud;
    let row = Row::with_english_first(0, &candidate, true);
    assert!(row.cloud);
    assert_eq!(row.annotation, vec![("请客".to_owned(), Tone::Gloss)]);
    assert_eq!(
        candidate.translation.as_ref().unwrap().language,
        Language::English
    );
}

#[test]
fn missing_gloss_other_languages_and_non_chinese_candidates_do_not_swap() {
    let original = candidate();
    let mut cases = vec![];
    let mut missing = original.clone();
    missing.translation = None;
    cases.push(missing);
    let mut empty = original.clone();
    empty.translation.as_mut().unwrap().senses_mut()[0]
        .text
        .clear();
    cases.push(empty);
    let mut japanese = original.clone();
    japanese.translation.as_mut().unwrap().language = Language::Japanese;
    cases.push(japanese);
    for kind in [
        CandidateKind::English,
        CandidateKind::Emoji,
        CandidateKind::Shortcut,
    ] {
        let mut other = original.clone();
        other.kind = kind;
        cases.push(other);
    }
    for candidate in cases {
        assert!(candidate.english_display_translation().is_none());
        assert_eq!(
            Row::with_english_first(0, &candidate, true),
            Row::from_candidate(0, &candidate)
        );
    }
}

#[test]
fn cloud_and_complete_sentence_glosses_swap_without_losing_the_original() {
    for kind in [
        CandidateKind::Cloud,
        CandidateKind::Sentence,
        CandidateKind::Generated,
        CandidateKind::Code,
    ] {
        let mut candidate = candidate();
        candidate.kind = kind;
        candidate.text = "关于这件事，我们明天再讨论吧".to_owned();
        candidate.translation.as_mut().unwrap().senses_mut()[0].text =
            "Let's discuss this matter tomorrow.".to_owned();
        let row = Row::with_english_first(0, &candidate, true);
        assert_eq!(row.text, "Let's discuss this matter tomorrow.");
        assert_eq!(row.annotation[0].0, candidate.text);
    }
}

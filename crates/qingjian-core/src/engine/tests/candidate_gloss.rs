//! 候选未上屏时的释义兜底、布局补译文与异步结果隔离。

use super::{LearningTranslator, MemoryFiller, engine};
use crate::candidate::{
    Candidate, CandidateKind, CandidateLayout, CandidateList, Language, Sense, Translation,
};
use crate::engine::{FilledGloss, NoTranslator};

fn sentence() -> Candidate {
    Candidate {
        text: "关于这件事".into(),
        kind: CandidateKind::Sentence,
        syllables: ["guan", "yu", "zhe", "jian", "shi"]
            .map(str::to_owned)
            .into(),
        reading: None,
        translation: None,
        aux_code: None,
    }
}

fn translation(text: &str) -> Translation {
    Translation::new(
        Language::English,
        vec![Sense {
            part_of_speech: None,
            text: text.into(),
            reading: None,
            fresh: false,
        }],
    )
}

#[test]
fn cancelling_candidate_gloss_is_forwarded_without_polling_committed_results() {
    let filler = MemoryFiller::default();
    let cancelled = filler.cancelled.clone();
    let ready = filler.ready.clone();
    let mut engine = engine()
        .with_translator(Box::new(LearningTranslator::default()))
        .with_gloss_filler(Box::new(filler));
    ready.lock().unwrap().push(FilledGloss {
        word: "关于这件事".into(),
        translation: translation("About this matter"),
    });
    engine.cancel_candidate_gloss();
    assert_eq!(*cancelled.lock().unwrap(), 1);
    assert_eq!(engine.poll_glosses(), 1);
}

#[test]
fn uncommitted_sentence_requests_a_gloss_and_can_commit_the_reply() {
    let filler = MemoryFiller::default();
    let requested = filler.requested.clone();
    let ready = filler.ready.clone();
    let mut engine = engine()
        .with_translator(Box::new(LearningTranslator::default()))
        .with_gloss_filler(Box::new(filler));
    engine.set_input("guanyuzhejianshi");
    let mut layout = CandidateLayout::new(vec![sentence()], 9, 2);
    assert!(engine.request_candidate_gloss(layout.candidate(0).unwrap()));
    assert_eq!(*requested.lock().unwrap(), ["关于这件事"]);
    assert_eq!(engine.composition().text(), "guanyuzhejianshi");
    assert!(!engine.refresh_layout_glosses(&mut layout));
    ready.lock().unwrap().push(FilledGloss {
        word: "关于这件事".into(),
        translation: translation("About this matter"),
    });
    assert_eq!(engine.poll_glosses(), 1);
    assert!(engine.refresh_layout_glosses(&mut layout));
    assert!(!engine.refresh_layout_glosses(&mut layout));
    assert!(!engine.request_candidate_gloss(layout.candidate(0).unwrap()));
    assert_eq!(
        engine
            .commit_translation(layout.candidate(0).unwrap(), 0)
            .as_deref(),
        Some("About this matter")
    );
    assert!(engine.composition().is_empty());
}

#[test]
fn only_missing_allowed_candidates_are_requested() {
    let filler = MemoryFiller::default();
    let requested = filler.requested.clone();
    let mut engine = engine()
        .with_translator(Box::new(LearningTranslator::default()))
        .with_gloss_filler(Box::new(filler));
    let known = Candidate {
        text: "开发".into(),
        ..sentence()
    };
    assert!(!engine.request_candidate_gloss(&known));
    let translated = Candidate {
        translation: Some(translation("Already available")),
        ..sentence()
    };
    assert!(!engine.request_candidate_gloss(&translated));
    let blank = Candidate {
        text: " ".into(),
        ..sentence()
    };
    assert!(!engine.request_candidate_gloss(&blank));
    for kind in [
        CandidateKind::English,
        CandidateKind::Shortcut,
        CandidateKind::Emoji,
        CandidateKind::Custom(1),
    ] {
        assert!(!engine.request_candidate_gloss(&Candidate { kind, ..sentence() }));
    }
    engine.set_private(true);
    assert!(!engine.request_candidate_gloss(&sentence()));
    engine.set_private(false);
    engine.set_translator(Box::new(NoTranslator));
    assert!(!engine.request_candidate_gloss(&sentence()));
    assert!(requested.lock().unwrap().is_empty());
    let mut disabled = super::engine().with_translator(Box::new(LearningTranslator::default()));
    assert!(!disabled.request_candidate_gloss(&sentence()));
}

#[test]
fn all_chinese_sources_can_request_candidate_translation() {
    let filler = MemoryFiller::default();
    let requested = filler.requested.clone();
    let mut engine = engine()
        .with_translator(Box::new(LearningTranslator::default()))
        .with_gloss_filler(Box::new(filler));
    for kind in [
        CandidateKind::Chinese,
        CandidateKind::Cloud,
        CandidateKind::Code,
        CandidateKind::Sentence,
        CandidateKind::Generated,
    ] {
        assert!(engine.request_candidate_gloss(&Candidate { kind, ..sentence() }));
    }
    assert_eq!(requested.lock().unwrap().len(), 5);
}

#[test]
fn refreshed_glosses_preserve_cell_order_cloud_slots_and_pages() {
    let filler = MemoryFiller::default();
    let ready = filler.ready.clone();
    let mut engine = engine()
        .with_translator(Box::new(LearningTranslator::default()))
        .with_gloss_filler(Box::new(filler));
    let others = ["本地甲", "本地乙", "本地丙", "本地丁"];
    let mut local = vec![sentence()];
    local.extend(others.map(|text| Candidate {
        text: text.into(),
        ..sentence()
    }));
    let mut layout = CandidateLayout::new(local, 3, 1);
    layout.set_cloud(vec![Candidate {
        text: "云端候选".into(),
        ..sentence()
    }]);
    let before: Vec<_> = layout
        .cells()
        .iter()
        .map(|cell| cell.candidate().unwrap().text.clone())
        .collect();
    let pages = layout.pages();
    for (word, text) in [
        ("关于这件事", "About this matter"),
        ("云端候选", "Cloud candidate"),
        ("旧候选", "Old candidate"),
    ] {
        ready.lock().unwrap().push(FilledGloss {
            word: word.into(),
            translation: translation(text),
        });
    }
    assert_eq!(engine.poll_glosses(), 3);
    assert!(engine.refresh_layout_glosses(&mut layout));
    assert_eq!(layout.pages(), pages);
    assert_eq!(layout.page_size(), 3);
    assert_eq!(layout.cloud().len(), 1);
    let after: Vec<_> = layout
        .cells()
        .iter()
        .map(|cell| cell.candidate().unwrap().text.clone())
        .collect();
    assert_eq!(before, after);
    assert_eq!(
        layout
            .candidate(2)
            .unwrap()
            .translation
            .as_ref()
            .unwrap()
            .senses()[0]
            .text,
        "Cloud candidate"
    );
    assert!(
        layout.local()[1..]
            .iter()
            .all(|candidate| candidate.translation.is_none())
    );
}

#[test]
fn annotation_keeps_existing_cloud_translation_until_language_changes() {
    let mut engine = engine().with_translator(Box::new(LearningTranslator::default()));
    let mut list = CandidateList {
        items: vec![Candidate {
            translation: Some(translation("Existing cloud translation")),
            ..sentence()
        }],
    };
    assert_eq!(engine.annotate(&mut list).hits, 1);
    assert_eq!(
        list.items[0].translation.as_ref().unwrap().senses()[0].text,
        "Existing cloud translation"
    );
    engine.set_translator(Box::new(NoTranslator));
    let mut layout = CandidateLayout::new(list.items, 9, 1);
    assert!(engine.refresh_layout_glosses(&mut layout));
    assert!(layout.candidate(0).unwrap().translation.is_none());
}

#[test]
fn shortcuts_and_emoji_have_no_translation_annotation() {
    let engine = engine().with_translator(Box::new(LearningTranslator::default()));
    let mut list = CandidateList {
        items: [CandidateKind::Shortcut, CandidateKind::Emoji]
            .map(|kind| Candidate {
                text: "开发".into(),
                kind,
                translation: Some(translation("develop")),
                ..sentence()
            })
            .into(),
    };
    assert_eq!(engine.annotate(&mut list).hits, 0);
    assert!(
        list.items
            .iter()
            .all(|candidate| candidate.translation.is_none())
    );
}

#[test]
fn private_and_changed_language_discard_arriving_glosses() {
    let filler = MemoryFiller::default();
    let ready = filler.ready.clone();
    let mut engine = engine()
        .with_translator(Box::new(LearningTranslator::default()))
        .with_gloss_filler(Box::new(filler));
    ready.lock().unwrap().push(FilledGloss {
        word: sentence().text,
        translation: translation("About this matter"),
    });
    engine.set_private(true);
    assert_eq!(engine.poll_glosses(), 0);
    engine.set_private(false);
    assert_eq!(engine.poll_glosses(), 0);
    let mut layout = CandidateLayout::new(vec![sentence()], 9, 1);
    assert!(!engine.refresh_layout_glosses(&mut layout));
    ready.lock().unwrap().push(FilledGloss {
        word: sentence().text,
        translation: translation("About this matter"),
    });
    engine.set_translator(Box::new(NoTranslator));
    assert_eq!(engine.poll_glosses(), 0);
    engine.set_translator(Box::new(LearningTranslator::default()));
    assert!(!engine.refresh_layout_glosses(&mut layout));
}

#[test]
fn traditional_candidate_uses_simplified_lookup_key_without_changing_display() {
    let filler = MemoryFiller::default();
    let requested = filler.requested.clone();
    let ready = filler.ready.clone();
    let mut engine = engine()
        .with_translator(Box::new(LearningTranslator::default()))
        .with_gloss_filler(Box::new(filler));
    engine.set_traditional_mode(true);
    engine
        .traditional_map
        .borrow_mut()
        .insert("關於這件事".into(), "关于这件事".into());
    let traditional = Candidate {
        text: "關於這件事".into(),
        ..sentence()
    };
    assert!(engine.request_candidate_gloss(&traditional));
    assert_eq!(*requested.lock().unwrap(), ["关于这件事"]);
    ready.lock().unwrap().push(FilledGloss {
        word: "关于这件事".into(),
        translation: translation("About this matter"),
    });
    assert_eq!(engine.poll_glosses(), 1);
    let mut layout = CandidateLayout::new(vec![traditional], 9, 1);
    assert!(engine.refresh_layout_glosses(&mut layout));
    assert_eq!(layout.candidate(0).unwrap().text, "關於這件事");
    assert_eq!(
        layout
            .candidate(0)
            .unwrap()
            .translation
            .as_ref()
            .unwrap()
            .senses()[0]
            .text,
        "About this matter"
    );
}

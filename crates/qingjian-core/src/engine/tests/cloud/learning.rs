//! 云联想的学习与整句提交测试。

use super::super::*;

#[test]
fn accepted_sentence_completion_feeds_the_personal_ngram_and_can_be_retracted() {
    let shared = Arc::new(Mutex::new((Vec::new(), sentence::UserNgram::default())));
    let mut engine = Engine::new(Dictionary::parse(SAMPLE).unwrap())
        .with_language_model(Box::new(SentenceModel))
        .with_learner(Box::new(WordLearner {
            shared: shared.clone(),
            ..WordLearner::default()
        }));
    engine.set_input("kaifa");
    assert_eq!(
        engine.accept_prediction("开发输入法很好用。"),
        "开发输入法很好用。"
    );
    {
        let ngram = &shared.lock().unwrap().1;
        assert_eq!(ngram.pair(None, "开发"), 1);
        assert_eq!(ngram.pair(Some("开发"), "输入法"), 1);
        assert_eq!(ngram.pair(Some("输入法"), "很"), 1);
        assert_eq!(ngram.pair(Some("很"), "好用"), 1);
    }
    // 句尾是句号：下一个词按句首记
    assert_eq!(engine.chain.previous(), None);

    // 整句退格删光、同一段拼音重新选词：撤销这句记的转移
    for _ in 0.."开发输入法很好用。".chars().count() {
        engine.note_backspace();
    }
    engine.set_input("kaifa");
    let query = engine.query().unwrap();
    let first = query.candidates.items[0].clone();
    engine.commit(&first);
    let ngram = &shared.lock().unwrap().1;
    assert_eq!(ngram.pair(Some("开发"), "输入法"), 0);
    assert_eq!(ngram.pair(Some("很"), "好用"), 0);
}

#[test]
fn committing_a_cloud_word_learns_it_and_it_ranks_first_next_time() {
    let mut engine = engine().with_learner(Box::new(WordLearner::default()));
    engine.set_input("zt");
    assert!(engine.query().unwrap().candidates.items.is_empty());
    let word = Candidate {
        text: "账套".into(),
        kind: CandidateKind::Cloud,
        syllables: vec!["zhang".into(), "tao".into()],
        reading: None,
        translation: None,
        aux_code: None,
    };
    assert_eq!(engine.commit(&word), "账套");
    assert!(engine.composition().is_empty());

    engine.set_input("zhangtao");
    let all = texts_of(&engine);
    assert_eq!(all[0], "账套");
    engine.set_input("zt");
    assert_eq!(texts_of(&engine)[0], "账套");

    // 词库里已有的云端词不重复记
    engine.set_input("kaifa");
    let mut kaifa = engine.query().unwrap().candidates.items[0].clone();
    kaifa.kind = CandidateKind::Cloud;
    engine.commit(&kaifa);
    engine.set_input("kaifa");
    assert_eq!(texts_of(&engine).iter().filter(|t| *t == "开发").count(), 1);
}

#[test]
fn traditional_mode_preserves_original_text_across_queries() {
    let mut engine = engine()
        .with_predictor(Box::new(EchoPredictor {
            submitted: std::rc::Rc::new(std::cell::RefCell::new(Vec::new())),
            sentence: false,
            replies: vec![Prediction {
                sequence: 1,
                words: vec![cloud("凯发", &["kai", "fa"])],
                sentence: None,
            }],
        }))
        .with_learner(Box::new(WordLearner::default()));

    engine.set_traditional_mode(true);
    engine.set_input("kaifa");
    engine.request_prediction(None, &[]);
    let prediction = engine.poll_prediction().unwrap();
    let cloud_text = prediction.words[0].text.clone();

    engine.query().unwrap(); // 第二次 query() 不应清空云端词的映射

    let word = Candidate {
        text: cloud_text,
        kind: CandidateKind::Cloud,
        syllables: vec!["kai".into(), "fa".into()],
        reading: None,
        translation: None,
        aux_code: None,
    };
    assert_eq!(engine.commit(&word), "凱發");
    // 检查词库里学到的是简体「凯发」
    assert!(engine.learner().weight("凯发") > 0);
    assert_eq!(engine.learner().weight("凱發"), 0);
}

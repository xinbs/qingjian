//! 云联想的返回校验测试。

use super::super::*;

#[test]
fn stale_predictions_are_dropped_and_accept_clears_composition() {
    let submitted = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let mut engine = engine().with_predictor(Box::new(EchoPredictor {
        submitted,
        sentence: false,
        replies: vec![
            Prediction {
                sequence: 2,
                words: vec![
                    cloud("开花", &["kai", "hua"]),
                    cloud("凯发", &["kai", "fa"]),
                ],
                sentence: Some("开发输入法".into()),
            },
            Prediction {
                sequence: 1,
                words: Vec::new(),
                sentence: Some("旧结果".into()),
            },
        ],
    }));
    engine.set_input("kaifa");
    engine.request_prediction(None, &[]);
    engine.request_prediction(None, &[]);
    let prediction = engine.poll_prediction().unwrap();
    assert_eq!(prediction.sequence, 2);
    assert_eq!(prediction.sentence.as_deref(), Some("开发输入法"));
    // 开花 的拼音对不上 kaifa，被过滤
    assert_eq!(prediction.words, [cloud("凯发", &["kai", "fa"])]);
    assert_eq!(engine.poll_prediction(), None);

    // 取消后连当前序号的结果也不要
    engine.request_prediction(None, &[]);
    engine.cancel_prediction();
    assert_eq!(engine.poll_prediction(), None);

    assert_eq!(engine.accept_prediction("开发输入法"), "开发输入法");
    assert!(engine.composition().is_empty());
    assert_eq!(engine.history().text(), "开发输入法");
}

#[test]
fn cloud_words_tolerate_typos_but_not_unrelated_words() {
    let submitted = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let mut engine = engine().with_predictor(Box::new(EchoPredictor {
        submitted,
        sentence: false,
        replies: vec![Prediction {
            sequence: 1,
            words: vec![
                cloud("这个东西吗", &["zhe", "ge", "dong", "xi", "ma"]),
                cloud("知道", &["zhi", "dao"]),
            ],
            sentence: None,
        }],
    }));
    engine.set_input("zhgdoima");
    engine.request_prediction(None, &[]);
    let prediction = engine.poll_prediction().unwrap();
    assert_eq!(
        prediction.words,
        [cloud("这个东西吗", &["zhe", "ge", "dong", "xi", "ma"])]
    );
    // 云端词上屏吃掉整段（按音节对不上的）拼音
    let word = Candidate {
        text: "这个东西吗".into(),
        kind: CandidateKind::Cloud,
        syllables: prediction.words[0].syllables.clone(),
        reading: None,
        translation: None,
        aux_code: None,
    };
    assert_eq!(engine.commit(&word), "这个东西吗");
    assert!(engine.composition().is_empty());
}

#[test]
fn cloud_words_are_validated_against_abbreviated_pinyin() {
    let submitted = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let mut engine = engine().with_predictor(Box::new(EchoPredictor {
        submitted,
        sentence: false,
        replies: vec![Prediction {
            sequence: 1,
            words: vec![
                cloud("账套", &["zhang", "tao"]),
                cloud("张涛涛", &["zhang", "tao", "tao"]),
                cloud("知道", &["zhi", "dao"]),
                cloud("不是音节", &["zx", "tq", "a", "b"]),
            ],
            sentence: None,
        }],
    }));
    engine.set_input("zt");
    engine.request_prediction(None, &[]);
    let prediction = engine.poll_prediction().unwrap();
    assert_eq!(prediction.words, [cloud("账套", &["zhang", "tao"])]);
}

#[test]
fn cloud_words_are_learned_with_the_typed_reading_when_it_fits() {
    let mut engine = engine().with_learner(Box::new(WordLearner::default()));
    let cloud_word = |text: &str, syllables: &[&str]| Candidate {
        text: text.into(),
        kind: CandidateKind::Cloud,
        syllables: syllables.iter().map(|s| (*s).to_owned()).collect(),
        reading: None,
        translation: None,
        aux_code: None,
    };
    let has = |engine: &Engine, text: &str| texts_of(engine).iter().any(|t| t == text);
    // 模型把 先 的读音给成了 xia：敲的 kaixian 切得开、每个音节都是那个字的读音，按敲的学
    engine.set_input("kaixian");
    engine.commit(&cloud_word("开先", &["kai", "xia"]));
    engine.set_input("kaixian");
    assert!(has(&engine, "开先"));
    let user = engine.learner().user_words().unwrap();
    assert!(
        user.lookup(&["kai", "xian"], false)
            .iter()
            .any(|m| m.exact && m.text == "开先")
    );
    assert!(!user.lookup(&["kai", "xia"], false).iter().any(|m| m.exact));
    // 敲错了（kaixan 切不开）：模型的读音每个字都对得上，按模型的学
    engine.set_input("kaixan");
    engine.commit(&cloud_word("开想", &["kai", "xiang"]));
    engine.set_input("kaixiang");
    assert!(has(&engine, "开想"));
    // 敲错了、模型的读音又不是这个字的：不学
    engine.set_input("xiangxan");
    engine.commit(&cloud_word("想先", &["xiang", "xia"]));
    let user = engine.learner().user_words().unwrap();
    for reading in [["xiang", "xia"], ["xiang", "xian"]] {
        assert!(
            user.lookup(&reading, false)
                .iter()
                .all(|m| m.text != "想先")
        );
    }
}

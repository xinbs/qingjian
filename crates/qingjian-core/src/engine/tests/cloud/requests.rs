//! 云联想的请求参数测试。

use super::super::*;

#[test]
fn prediction_request_trims_context_and_only_fires_while_composing() {
    let submitted = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let mut engine = engine().with_predictor(Box::new(EchoPredictor {
        submitted: submitted.clone(),
        replies: Vec::new(),
        sentence: true,
    }));
    engine.set_input("kaifa");
    let query = engine.query().unwrap();
    let surrounding = SurroundingText {
        before: "我们今天一起来".into(),
        after: "吧，好不好".into(),
    };
    let sequence = engine.request_prediction(Some(surrounding), &query.candidates.items);
    assert_eq!(sequence, Some(1));
    let request = submitted.borrow()[0].clone();
    assert_eq!(request.before, "天一起来");
    assert_eq!(request.after, "吧，");
    assert_eq!(request.pinyin, "kai'fa");
    assert_eq!(request.letters, "kaifa");
    assert_eq!(request.syllables, 2);
    assert_eq!(request.candidates[0], "开发");
    assert_eq!(request.max_items, 2);
    assert!(request.want_sentence);

    // 拼音太短不发
    engine.set_input("k");
    assert_eq!(engine.request_prediction(None, &[]), None);
    engine.set_input("kaifa");
    // 没有应用上下文也照发：词候选和整句都只靠拼音；本地历史不进请求
    let kaifa = query.candidates.items[0].clone();
    engine.commit(&kaifa);
    engine.punctuate('.');
    engine.set_input("kaifa");
    assert_eq!(engine.request_prediction(None, &[]), Some(3));
    let request = submitted.borrow()[1].clone();
    assert!(request.before.is_empty());
    assert!(request.want_sentence);
    // 上屏之后不联想
    engine.clear();
    assert_eq!(engine.request_prediction(None, &[]), None);
}

#[test]
fn abbreviated_input_asks_for_words_and_the_sentence() {
    let submitted = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let mut engine = engine().with_predictor(Box::new(EchoPredictor {
        submitted: submitted.clone(),
        replies: Vec::new(),
        sentence: true,
    }));
    engine.set_input("zhsh");
    let query = engine.query().unwrap();
    assert_eq!(
        engine.request_prediction(None, &query.candidates.items),
        Some(1)
    );
    let request = submitted.borrow()[0].clone();
    assert_eq!(request.letters, "zhsh");
    assert_eq!(request.max_items, 2, "简拼也应请求猜词");
    assert!(request.want_sentence, "整句补全照常要");

    // 完整拼音照常要词
    engine.set_input("zhongshi");
    let query = engine.query().unwrap();
    assert_eq!(
        engine.request_prediction(None, &query.candidates.items),
        Some(2)
    );
    let request = submitted.borrow()[1].clone();
    assert_eq!(request.max_items, 2);
    assert!(request.want_sentence);
}

#[test]
fn cursor_in_the_middle_scopes_candidates_and_prediction_to_the_left_part() {
    let mut engine = engine();
    engine.set_input("kaifazhe");
    for _ in 0..3 {
        engine.move_cursor_left();
    }
    // kaifa|zhe：候选只看 kaifa（与单独打 kaifa 一样），zhe 只画出来
    let query = engine.query().unwrap();
    assert_eq!(query.candidates.items[0].text, "开发");
    assert_eq!(query.marked_text(), "kai'fa'zhe");
    assert_eq!(query.marked_cursor(), 6);

    // 上屏 开发 后剩 zhe，光标落到末尾，接着打就是往后加
    let kaifa = query.candidates.items[0].clone();
    assert_eq!(engine.commit(&kaifa), "开发");
    assert_eq!(engine.composition().text(), "zhe");
    assert_eq!(engine.composition().cursor(), 3);

    // 光标在开头时按整段算
    engine.set_input("kaifa");
    engine.move_cursor_home();
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "开发");
}

#[test]
fn prediction_request_uses_the_scope_only() {
    let submitted = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let predictor = EchoPredictor {
        submitted: submitted.clone(),
        replies: Vec::new(),
        sentence: false,
    };
    let mut engine = engine().with_predictor(Box::new(predictor));
    engine.set_input("kaifazhe");
    for _ in 0..3 {
        engine.move_cursor_left();
    }
    assert_eq!(engine.request_prediction(None, &[]), Some(1));
    let request = submitted.borrow()[0].clone();
    assert_eq!(request.pinyin, "kai'fa");
    assert_eq!(request.letters, "kaifa");
    assert_eq!(request.syllables, 2);
}

#[test]
fn no_predictor_never_requests() {
    let mut engine = engine();
    assert!(!engine.prediction_enabled());
    engine.set_input("kaifa");
    assert_eq!(engine.request_prediction(None, &[]), None);
}

#[test]
fn translation_requests_carry_the_text_and_target_language() {
    use std::sync::{Arc, Mutex};
    struct Recorder(Arc<Mutex<Vec<PredictionRequest>>>);
    impl Predictor for Recorder {
        fn policy(&self) -> PredictionPolicy {
            PredictionPolicy::default()
        }
        fn submit(&mut self, request: PredictionRequest) {
            self.0.lock().unwrap().push(request);
        }
        fn poll(&mut self) -> Option<Prediction> {
            None
        }
    }
    let sent = Arc::new(Mutex::new(Vec::new()));
    let mut engine = Engine::new(Dictionary::parse(SAMPLE).unwrap())
        .with_predictor(Box::new(Recorder(sent.clone())));
    assert!(engine.request_translation("  ").is_none());
    let sequence = engine.request_translation("我想去吃饭").unwrap();
    let requests = sent.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].sequence, sequence);
    assert_eq!(requests[0].kind, PredictionKind::Translate);
    assert_eq!(requests[0].text, "我想去吃饭");
    assert_eq!(requests[0].target_language, "en");
    drop(requests);
    // 外文选区译回中文
    engine.request_translation("I want to eat.").unwrap();
    assert_eq!(sent.lock().unwrap()[1].target_language, "zh");
}

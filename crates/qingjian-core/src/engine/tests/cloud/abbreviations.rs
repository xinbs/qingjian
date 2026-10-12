//! 简拼、全拼混输与错拼的猜词回归。

use super::super::{EchoPredictor, cloud, engine};
use crate::{Engine, Prediction, Predictor};

fn predictor(words: Vec<crate::CloudWord>) -> Box<dyn Predictor> {
    Box::new(EchoPredictor {
        submitted: Default::default(),
        replies: vec![Prediction {
            sequence: 1,
            words,
            sentence: None,
        }],
        sentence: false,
    })
}

#[test]
fn initials_mixed_pinyin_and_typos_can_all_guess_the_complete_idiom() {
    for input in ["oxlx", "ouxlx", "ouxinlixue", "ouximlixue", "ouxinlixeu"] {
        let expected = cloud("呕心沥血", &["ou", "xin", "li", "xue"]);
        let mut engine = engine().with_predictor(predictor(vec![
            expected.clone(),
            cloud("今天", &["jin", "tian"]),
        ]));
        engine.set_input(input);
        assert!(engine.request_prediction(None, &[]).is_some());
        let prediction = engine.poll_prediction().unwrap();
        assert_eq!(prediction.words, [expected], "{input}");
        assert_eq!(
            engine.commit(&prediction.words[0].clone().into_candidate()),
            "呕心沥血"
        );
        assert!(engine.composition().is_empty(), "{input}");
    }
}

#[test]
fn initials_do_not_accept_a_known_character_with_a_fabricated_reading() {
    let mut engine: Engine = engine().with_predictor(predictor(vec![
        cloud("开发", &["kai", "fa"]),
        // ka 是合法音节，却不是样例词库中「开」的读音；不能只凭首字母就放行。
        cloud("开发", &["ka", "fa"]),
    ]));
    engine.set_input("kf");
    engine.request_prediction(None, &[]);
    assert_eq!(
        engine.poll_prediction().unwrap().words,
        [cloud("开发", &["kai", "fa"])]
    );
}

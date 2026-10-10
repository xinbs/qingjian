//! 输入状态失效时，应通知联想器停止待发和在飞的请求。

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use super::engine;
use crate::{Prediction, PredictionPolicy, PredictionRequest, Predictor};

#[test]
fn invalid_input_and_explicit_cancellation_reach_the_predictor() {
    struct Recorder(Arc<AtomicUsize>);
    impl Predictor for Recorder {
        fn policy(&self) -> PredictionPolicy {
            PredictionPolicy::default()
        }
        fn submit(&mut self, _: PredictionRequest) {}
        fn cancel(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
        fn poll(&mut self) -> Option<Prediction> {
            None
        }
    }
    let cancelled = Arc::new(AtomicUsize::new(0));
    let mut engine = engine().with_predictor(Box::new(Recorder(cancelled.clone())));
    engine.set_input("kaifa");
    assert!(engine.request_prediction(None, &[]).is_some());
    let before = cancelled.load(Ordering::SeqCst);
    engine.set_input("k");
    assert!(engine.request_prediction(None, &[]).is_none());
    assert!(cancelled.load(Ordering::SeqCst) > before);
    let before = cancelled.load(Ordering::SeqCst);
    engine.cancel_prediction();
    assert_eq!(cancelled.load(Ordering::SeqCst), before + 1);
    let before = cancelled.load(Ordering::SeqCst);
    engine.set_private(true);
    assert_eq!(cancelled.load(Ordering::SeqCst), before + 1);
}

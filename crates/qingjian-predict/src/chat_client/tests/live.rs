//! 显式执行的 MiniMax 实际请求验证，仅使用构造输入。

use serde_json::Value;

use crate::chat_client::ChatClient;
use crate::config::PredictConfig;

/// 显式运行的真实接口验证，仅用构造的探活、问字与联想，不读取用户输入历史。
#[test]
#[ignore = "requires QINGJIAN_SMOKE_KEY and an explicitly authorized live request"]
fn minimax_live_smoke_returns_a_valid_probe_and_question_answer() {
    use qingjian_core::{PredictionKind, PredictionRequest};

    let key = std::env::var("QINGJIAN_SMOKE_KEY").expect("smoke key is required");
    let config = PredictConfig {
        base_url: "https://api.minimaxi.com/v1".to_owned(),
        model: std::env::var("QINGJIAN_SMOKE_MODEL")
            .unwrap_or_else(|_| "MiniMax-M2.7-highspeed".to_owned()),
        ..PredictConfig::default()
    };
    let client = ChatClient::new(&config, key);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let started = std::time::Instant::now();
    let reply = runtime
        .block_on(client.chat(
            "你是输入法的连通性测试。",
            "回复 JSON 对象 {\"ok\": true}，不要别的内容。",
            32,
        ))
        .unwrap();
    assert_eq!(serde_json::from_str::<Value>(&reply).unwrap()["ok"], true);
    eprintln!("探活完成：{} ms", started.elapsed().as_millis());
    let request = PredictionRequest {
        sequence: 1,
        kind: PredictionKind::Question,
        letters: "sangemu".to_owned(),
        pinyin: "san'ge'mu".to_owned(),
        syllables: 3,
        before: String::new(),
        after: String::new(),
        guess: "三个木是什么字".to_owned(),
        candidates: Vec::new(),
        max_items: 2,
        want_sentence: false,
        text: String::new(),
        target_language: String::new(),
    };
    let started = std::time::Instant::now();
    let reply = runtime.block_on(client.complete(&request)).unwrap();
    assert!(reply.words.iter().any(|word| word.text == "森"));
    eprintln!("问字完成：{} ms", started.elapsed().as_millis());
    let compose = PredictionRequest {
        kind: PredictionKind::Compose,
        letters: "jintian".to_owned(),
        pinyin: "jin'tian".to_owned(),
        syllables: 2,
        guess: "今天".to_owned(),
        candidates: vec!["今天".to_owned()],
        want_sentence: true,
        ..request
    };
    let started = std::time::Instant::now();
    let reply = runtime.block_on(client.complete(&compose)).unwrap();
    assert!(reply.sentence.is_some());
    eprintln!("整句联想完成：{} ms", started.elapsed().as_millis());
}

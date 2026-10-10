//! 聊天客户端的参数、会话头与响应回归测试。

mod live;
mod responses;

use super::*;

#[test]
fn minimax_m3_disables_thinking_without_spending_a_reasoning_budget() {
    for tokens in [32, 200] {
        let mut body = serde_json::json!({
            "model": "MiniMax-M3", "reasoning_effort": "none", "max_tokens": tokens
        });
        Provider::for_url("https://api.minimaxi.com/v1").prepare_request(
            &mut body,
            Some(&ReasoningEffort::None),
            tokens,
        );
        assert_eq!(body["thinking"]["type"], "disabled");
        assert_eq!(body["max_tokens"], tokens);
        assert_eq!(body["reasoning_split"], true);
        assert!(body.get("reasoning_effort").is_none());
    }
    for (model, effort) in [
        ("MiniMax-M2.7-highspeed", Some(ReasoningEffort::None)),
        ("MiniMax-M3.1-Flash-Preview", Some(ReasoningEffort::None)),
        ("MiniMax-M3", Some(ReasoningEffort::High)),
        ("MiniMax-M3", None),
    ] {
        let mut body = serde_json::json!({ "model": model, "max_tokens": 200 });
        Provider::for_url("https://api.minimaxi.com/v1").prepare_request(
            &mut body,
            effort.as_ref(),
            200,
        );
        assert!(body.get("thinking").is_none());
        assert_eq!(body["max_tokens"], 2248);
    }
}

#[test]
fn reasoning_effort_parses_known_values_and_ignores_the_rest() {
    assert!(matches!(
        parse_reasoning_effort("none"),
        Some(ReasoningEffort::None)
    ));
    assert!(matches!(
        parse_reasoning_effort(" High "),
        Some(ReasoningEffort::High)
    ));
    assert!(parse_reasoning_effort("").is_none());
    assert!(parse_reasoning_effort("maximum").is_none());
}

#[test]
fn zhipu_hosts_disable_thinking_with_their_own_field() {
    let mut body = serde_json::json!({ "model": "glm", "reasoning_effort": "none" });
    Provider::for_url("https://open.bigmodel.cn/api/paas/v4").prepare_request(
        &mut body,
        Some(&ReasoningEffort::None),
        200,
    );
    assert_eq!(body["thinking"]["type"], "disabled");
    assert!(body.get("reasoning_effort").is_none());
    assert_eq!(
        Provider::for_url("https://api.z.ai/api/paas/v4"),
        Provider::Zhipu
    );

    let mut body = serde_json::json!({ "model": "x", "reasoning_effort": "none" });
    Provider::for_url("https://api.deepseek.com").prepare_request(
        &mut body,
        Some(&ReasoningEffort::None),
        200,
    );
    assert_eq!(body["reasoning_effort"], "none");
    assert!(body.get("thinking").is_none());
    assert_eq!(
        Provider::for_url("https://example.com/bigmodel.cn"),
        Provider::OpenAi
    );
}

#[test]
fn preset_hosts_use_their_supported_thinking_controls() {
    for url in ["https://api.moonshot.cn/v1", "https://api.moonshot.ai/v1"] {
        let mut body = serde_json::json!({ "model": "kimi-k2.6", "reasoning_effort": "none" });
        Provider::for_url(url).prepare_request(&mut body, Some(&ReasoningEffort::None), 200);
        assert_eq!(body["thinking"]["type"], "disabled");
        assert!(body.get("reasoning_effort").is_none());
    }
    let mut body = serde_json::json!({ "model": "qwen-plus", "reasoning_effort": "none" });
    Provider::for_url("https://dashscope.aliyuncs.com/compatible-mode/v1").prepare_request(
        &mut body,
        Some(&ReasoningEffort::None),
        200,
    );
    assert_eq!(body["enable_thinking"], false);
    assert!(body.get("reasoning_effort").is_none());
    let mut body = serde_json::json!({ "model": "gpt-4.1-mini", "reasoning_effort": "none" });
    Provider::for_url("https://api.openai.com/v1").prepare_request(
        &mut body,
        Some(&ReasoningEffort::None),
        200,
    );
    assert!(body.get("reasoning_effort").is_none());
}

#[test]
fn opencode_is_recognized_by_host_only() {
    assert!(is_opencode("https://opencode.ai/zen/go/v1"));
    assert!(is_opencode("https://OpenCode.ai/zen/v1/"));
    assert!(is_opencode("https://api.opencode.ai/v1"));
    assert!(!is_opencode("https://api.deepseek.com"));
    assert!(!is_opencode("https://example.com/opencode.ai"));
    assert!(!is_opencode("not a url"));
}

#[test]
fn session_id_is_stable_within_the_process() {
    assert_eq!(*SESSION_ID, *SESSION_ID);
    assert_eq!(SESSION_ID.len(), 36);
}

/// 真发一个请求到本地端口，看 OpenCode 客户端带了会话头、默认客户端没带。
#[test]
fn opencode_client_sends_the_session_header() {
    assert_eq!(
        header_seen_by_server(http_client("https://opencode.ai/zen/go/v1")),
        Some(SESSION_ID.clone())
    );
    assert_eq!(
        header_seen_by_server(http_client("https://api.deepseek.com")),
        None
    );
}

/// 起一个只答一次的 HTTP 服务，返回请求里 `x-opencode-session` 的值。
fn header_seen_by_server(client: reqwest::Client) -> Option<String> {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut header = None;
        for line in BufReader::new(&stream).lines() {
            let line = line.unwrap();
            if line.is_empty() {
                break;
            }
            if let Some((name, value)) = line.split_once(':')
                && name.eq_ignore_ascii_case(OPENCODE_SESSION_HEADER)
            {
                header = Some(value.trim().to_owned());
            }
        }
        stream
            .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\nconnection: close\r\n\r\n")
            .unwrap();
        header
    });
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async { client.get(&url).send().await.unwrap() });
    server.join().unwrap()
}

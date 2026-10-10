//! 回放真实故障形状，验证思考正文分离、截断与网络返回的 JSON 校验。

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;

use serde_json::{Value, json};

use crate::chat_client::{ChatClient, Provider, response_content};
use crate::config::PredictConfig;
use crate::error::PredictError;

fn response(content: Option<&str>, finish_reason: &str) -> Value {
    json!({
        "id": "test", "object": "chat.completion", "created": 0, "model": "test",
        "choices": [{"index": 0, "finish_reason": finish_reason,
            "message": {"role": "assistant", "content": content,
                "reasoning_content": "private reasoning"}}]
    })
}

#[test]
fn split_reasoning_and_legacy_think_tags_yield_only_the_json_answer() {
    for content in [
        r#"{"ok":true}"#,
        "<think>thinking about {\"wrong\":true}</think>\n{\"ok\":true}",
    ] {
        let reply = response_content(&response(Some(content), "stop"), Provider::MiniMax).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&reply).unwrap(),
            json!({"ok": true})
        );
    }
}

#[test]
fn provider_service_tiers_do_not_prevent_parsing_the_answer() {
    for tier in ["standard", "vendor-specific-tier"] {
        let mut raw = response(Some(r#"{"ok":true}"#), "stop");
        raw["service_tier"] = json!(tier);
        assert_eq!(
            response_content(&raw, Provider::MiniMax).unwrap(),
            r#"{"ok":true}"#
        );
        raw["choices"][0]["finish_reason"] = json!("length");
        assert!(matches!(
            response_content(&raw, Provider::MiniMax),
            Err(PredictError::BudgetExhausted)
        ));
    }
}

#[test]
fn incomplete_and_invalid_replies_are_errors_instead_of_empty_successes() {
    assert!(matches!(
        response_content(
            &response(Some("<think>no answer</think>"), "length"),
            Provider::MiniMax
        ),
        Err(PredictError::BudgetExhausted)
    ));
    assert!(matches!(
        response_content(
            &response(Some(r#"{"ok":true}"#), "length"),
            Provider::OpenAi
        ),
        Err(PredictError::BudgetExhausted)
    ));
    assert!(matches!(
        response_content(
            &response(Some("<think>no answer</think>"), "stop"),
            Provider::MiniMax
        ),
        Err(PredictError::EmptyReply)
    ));
    for content in ["<think>unfinished", "not JSON", "[]", "true"] {
        assert!(matches!(
            response_content(&response(Some(content), "stop"), Provider::MiniMax),
            Err(PredictError::InvalidReply(_))
        ));
    }
    assert!(matches!(
        response_content(&response(None, "stop"), Provider::OpenAi),
        Err(PredictError::EmptyReply)
    ));
}

#[test]
fn minimax_is_matched_by_host_and_preserves_larger_configured_timeouts() {
    for url in [
        "https://api.minimaxi.com/v1",
        "https://api.minimax.io/v1",
        "https://api.minimax.cn/v1",
    ] {
        let config = PredictConfig {
            base_url: url.to_owned(),
            ..PredictConfig::default()
        };
        assert_eq!(Provider::for_url(url), Provider::MiniMax);
        assert_eq!(config.request_timeout_ms(), 15_000);
        assert_eq!(
            PredictConfig {
                timeout_ms: 30_000,
                ..config
            }
            .request_timeout_ms(),
            30_000
        );
    }
    for url in [
        "https://example.com/minimaxi.com",
        "https://api.minimaxi.com.example.org/v1",
        "not a url",
    ] {
        assert_eq!(Provider::for_url(url), Provider::OpenAi);
    }
    assert_eq!(PredictConfig::default().request_timeout_ms(), 5000);
}

#[test]
fn actual_http_request_uses_the_minimax_models_thinking_policy() {
    for (model, expected_tokens, disabled) in [
        ("MiniMax-M2.7-highspeed", 2080, false),
        ("MiniMax-M3", 32, true),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let config = PredictConfig {
            base_url: format!("http://{}/v1", listener.local_addr().unwrap()),
            model: model.to_owned(),
            ..PredictConfig::default()
        };
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(&stream);
            let mut length = 0;
            let mut first_line = String::new();
            reader.read_line(&mut first_line).unwrap();
            assert!(first_line.starts_with("POST /v1/chat/completions "));
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                if let Some((name, value)) = line.split_once(':')
                    && name.eq_ignore_ascii_case("content-length")
                {
                    length = value.trim().parse::<usize>().unwrap();
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let body: Value = serde_json::from_slice(&body).unwrap();
            let reply = response(Some(r#"{"ok":true}"#), "stop").to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", reply.len(), reply).unwrap();
            body
        });
        let mut client = ChatClient::new(&config, "test-key".to_owned());
        client.provider = Provider::MiniMax;
        let reply = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(client.chat("test", "return JSON", 32))
            .unwrap();
        assert_eq!(reply, r#"{"ok":true}"#);
        let body = server.join().unwrap();
        assert_eq!(body["reasoning_split"], true);
        assert_eq!(body["model"], model);
        assert_eq!(body["max_tokens"], expected_tokens);
        if disabled {
            assert_eq!(body["thinking"]["type"], "disabled");
        } else {
            assert!(body.get("thinking").is_none());
        }
        assert!(body.get("reasoning_effort").is_none());
    }
}

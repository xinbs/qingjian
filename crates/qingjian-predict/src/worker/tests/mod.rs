//! 用停住的 HTTP 请求回放旧请求挡住最新输入的问题。

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use qingjian_core::{PredictionKind, PredictionRequest};

fn request(sequence: u64) -> PredictionRequest {
    PredictionRequest {
        sequence,
        kind: PredictionKind::Compose,
        before: String::new(),
        after: String::new(),
        pinyin: format!("test{sequence}"),
        letters: format!("test{sequence}"),
        syllables: 2,
        candidates: Vec::new(),
        guess: String::new(),
        max_items: 2,
        want_sentence: true,
        text: String::new(),
        target_language: String::new(),
    }
}

fn read_body(stream: &TcpStream) -> serde_json::Value {
    stream.set_nonblocking(false).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut reader = BufReader::new(stream);
    let mut length = 0;
    loop {
        let mut line = String::new();
        assert!(reader.read_line(&mut line).unwrap() > 0);
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
    serde_json::from_slice(&body).unwrap()
}

fn respond(mut stream: TcpStream, sentence: &str) {
    let body = serde_json::json!({
        "id": "test", "object": "chat.completion", "created": 0, "model": "test",
        "choices": [{"index": 0, "finish_reason": "stop", "message": {
            "role": "assistant", "content": serde_json::json!({
                "words": [], "sentence": sentence
            }).to_string()
        }}]
    })
    .to_string();
    // 旧请求被取消后写端可能已经关闭。
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
}

mod cache;
mod debounce;
mod inflight;

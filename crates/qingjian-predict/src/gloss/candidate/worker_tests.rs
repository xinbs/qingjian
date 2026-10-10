//! 本机模拟接口验证候选翻译取消及最新槽行为，不访问真实服务。

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use qingjian_core::Language;
use tokio::sync::watch;

use super::request::CandidateRequest;
use super::worker::CandidateWorker;
use crate::chat_client::ChatClient;
use crate::config::PredictConfig;

fn accept(listener: &TcpListener) -> TcpStream {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                // macOS 接受的 socket 会继承监听端的非阻塞状态；请求还未写入时不能直接 read。
                stream.set_nonblocking(false).unwrap();
                return stream;
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline, "local request never arrived");
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(error) => panic!("accept failed: {error}"),
        }
    }
}

fn read_request(stream: &mut TcpStream) -> String {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut bytes = Vec::new();
    let mut chunk = [0; 1024];
    loop {
        let count = stream.read(&mut chunk).unwrap();
        assert!(count > 0);
        bytes.extend_from_slice(&chunk[..count]);
        let text = String::from_utf8_lossy(&bytes);
        if let Some(header_end) = text.find("\r\n\r\n") {
            let content_length: usize = text[..header_end]
                .lines()
                .find_map(|line| {
                    line.to_lowercase()
                        .strip_prefix("content-length: ")
                        .map(str::to_owned)
                })
                .unwrap()
                .parse()
                .unwrap();
            if bytes.len() >= header_end + 4 + content_length {
                return text[header_end + 4..].to_owned();
            }
        }
    }
}

fn respond(stream: &mut TcpStream, word: &str) {
    let content =
        serde_json::json!({"w":word,"t":"Let's discuss this matter tomorrow."}).to_string();
    let body = serde_json::json!({
        "id":"local-test", "object":"chat.completion", "created":0, "model":"local-test",
        "choices":[{"index":0,"finish_reason":"stop","message":{"role":"assistant","content":content}}]
    }).to_string();
    write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
}

#[test]
fn latest_queued_candidate_is_the_only_request_sent() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let config = PredictConfig {
        base_url: format!("http://{}/v1", listener.local_addr().unwrap()),
        ..PredictConfig::default()
    };
    let (requests, request_rx) = watch::channel(None);
    requests
        .send(Some(CandidateRequest::new(Language::English, "旧候选")))
        .unwrap();
    requests
        .send(Some(CandidateRequest::new(Language::English, "最新候选")))
        .unwrap();
    let (response_tx, responses) = mpsc::channel();
    let worker = CandidateWorker::new(
        request_rx,
        response_tx,
        ChatClient::new(&config, "local-key".to_owned()),
    );
    let worker = std::thread::spawn(move || worker.run().unwrap());
    let mut stream = accept(&listener);
    let body = read_request(&mut stream);
    assert!(body.contains("最新候选"));
    assert!(!body.contains("旧候选"));
    respond(&mut stream, "最新候选");
    assert_eq!(
        responses.recv_timeout(Duration::from_secs(2)).unwrap().word,
        "最新候选"
    );
    // 第一次结果即使被 Core 因语言变化或私密模式丢掉，也能取回缓存而不重发网络请求。
    requests
        .send(Some(CandidateRequest::new(Language::English, "最新候选")))
        .unwrap();
    assert_eq!(
        responses.recv_timeout(Duration::from_secs(2)).unwrap().word,
        "最新候选"
    );
    assert!(
        matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
    );
    drop(requests);
    worker.join().unwrap();
}

#[test]
fn changing_candidate_cancels_old_network_wait_and_cancel_discards_result() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let config = PredictConfig {
        base_url: format!("http://{}/v1", listener.local_addr().unwrap()),
        ..PredictConfig::default()
    };
    let (requests, request_rx) = watch::channel(None);
    let (response_tx, responses) = mpsc::channel();
    let worker = CandidateWorker::new(
        request_rx,
        response_tx,
        ChatClient::new(&config, "local-key".to_owned()),
    );
    let worker = std::thread::spawn(move || worker.run().unwrap());
    requests
        .send(Some(CandidateRequest::new(Language::English, "旧候选")))
        .unwrap();
    let mut old = accept(&listener);
    assert!(read_request(&mut old).contains("旧候选"));
    // 旧接口保持不回复；新候选仍应立即发出，而非等旧接口超时。
    requests
        .send(Some(CandidateRequest::new(Language::English, "最新候选")))
        .unwrap();
    let mut latest = accept(&listener);
    assert!(read_request(&mut latest).contains("最新候选"));
    requests.send(None).unwrap();
    drop(requests);
    worker.join().unwrap();
    assert!(responses.try_recv().is_err());
}

//! 本机接口验证跨客户端并发限制与相同在途请求合并。

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::time::Duration;

use crate::chat_client::ChatClient;
use crate::config::PredictConfig;

fn read_request(stream: &TcpStream) {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
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
            length = value.trim().parse().unwrap();
        }
    }
    reader.read_exact(&mut vec![0; length]).unwrap();
}

fn respond(mut stream: TcpStream) {
    let body = serde_json::json!({
        "id":"test", "object":"chat.completion", "created":0, "model":"test",
        "choices":[{"index":0,"finish_reason":"stop","message":{
            "role":"assistant","content":"{\"ok\":true}"
        }}]
    })
    .to_string();
    write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
}

#[test]
fn same_account_serializes_distinct_requests_and_merges_identical_requests() {
    for identical in [true, false] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let config = PredictConfig {
            base_url: format!("http://{}/v1", listener.local_addr().unwrap()),
            ..PredictConfig::default()
        };
        let first = ChatClient::new(&config, "key".to_owned());
        let second = ChatClient::new(&config, "key".to_owned());
        let (seen_tx, seen_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let stream = loop {
                if let Ok((stream, _)) = listener.accept() {
                    break stream;
                }
                std::thread::sleep(Duration::from_millis(2));
            };
            stream.set_nonblocking(false).unwrap();
            read_request(&stream);
            seen_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            assert!(
                matches!(listener.accept(), Err(e) if e.kind() == std::io::ErrorKind::WouldBlock),
                "a second HTTP request started while the first was still in flight"
            );
            respond(stream);
            if !identical {
                let deadline = std::time::Instant::now() + Duration::from_secs(2);
                let stream = loop {
                    if let Ok((stream, _)) = listener.accept() {
                        break stream;
                    }
                    assert!(std::time::Instant::now() < deadline);
                    std::thread::sleep(Duration::from_millis(2));
                };
                stream.set_nonblocking(false).unwrap();
                read_request(&stream);
                respond(stream);
            }
        });
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let one = tokio::spawn(async move { first.chat("system", "one", 32).await });
                tokio::task::spawn_blocking(move || {
                    seen_rx.recv_timeout(Duration::from_secs(2)).unwrap()
                })
                .await
                .unwrap();
                let two = tokio::spawn(async move {
                    second
                        .chat("system", if identical { "one" } else { "two" }, 32)
                        .await
                });
                tokio::time::sleep(Duration::from_millis(80)).await;
                release_tx.send(()).unwrap();
                assert_eq!(one.await.unwrap().unwrap(), "{\"ok\":true}");
                assert_eq!(two.await.unwrap().unwrap(), "{\"ok\":true}");
            });
        server.join().unwrap();
    }
}

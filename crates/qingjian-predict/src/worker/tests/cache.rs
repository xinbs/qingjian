//! 命中缓存时应沿用最新序号，不再访问 HTTP。

use std::net::TcpListener;
use std::sync::mpsc;
use std::time::Duration;

use super::{read_body, request, respond};
use crate::chat_client::ChatClient;
use crate::config::PredictConfig;
use crate::worker::Worker;

#[test]
fn cached_reply_has_the_new_sequence_without_another_http_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let server_listener = listener.try_clone().unwrap();
    let config = PredictConfig {
        base_url: format!("http://{}/v1", listener.local_addr().unwrap()),
        ..PredictConfig::default()
    };
    let server = std::thread::spawn(move || {
        let (stream, _) = server_listener.accept().unwrap();
        read_body(&stream);
        respond(stream, "缓存结果");
    });
    let (requests, incoming) = tokio::sync::watch::channel(None);
    let (outgoing, responses) = mpsc::channel();
    let worker = Worker::new(
        incoming,
        outgoing,
        ChatClient::new(&config, "test-key".to_owned()),
        Duration::from_millis(10),
    );
    let worker = std::thread::spawn(move || worker.run().unwrap());
    let mut input = request(1);
    requests.send(Some(input.clone())).unwrap();
    assert_eq!(
        responses
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .sequence,
        1
    );
    server.join().unwrap();
    input.sequence = 2;
    requests.send(Some(input)).unwrap();
    let cached = responses.recv_timeout(Duration::from_secs(1)).unwrap();
    assert_eq!(cached.sequence, 2);
    assert_eq!(cached.sentence.as_deref(), Some("缓存结果"));
    drop(requests);
    worker.join().unwrap();
    listener.set_nonblocking(true).unwrap();
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

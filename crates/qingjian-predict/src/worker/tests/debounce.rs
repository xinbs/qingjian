//! 取消防抖中的请求，不应访问网络。

use std::net::TcpListener;
use std::sync::mpsc;
use std::time::Duration;

use super::request;
use crate::chat_client::ChatClient;
use crate::config::PredictConfig;
use crate::worker::Worker;

#[test]
fn cancelling_during_debounce_sends_no_http_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let config = PredictConfig {
        base_url: format!("http://{}/v1", listener.local_addr().unwrap()),
        ..PredictConfig::default()
    };
    let (requests, incoming) = tokio::sync::watch::channel(None);
    let (outgoing, responses) = mpsc::channel();
    let worker = Worker::new(
        incoming,
        outgoing,
        ChatClient::new(&config, "test-key".to_owned()),
        Duration::from_millis(200),
    );
    let worker = std::thread::spawn(move || worker.run().unwrap());
    requests.send(Some(request(1))).unwrap();
    std::thread::sleep(Duration::from_millis(20));
    requests.send(None).unwrap();
    std::thread::sleep(Duration::from_millis(250));
    drop(requests);
    worker.join().unwrap();
    assert!(responses.try_recv().is_err());
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

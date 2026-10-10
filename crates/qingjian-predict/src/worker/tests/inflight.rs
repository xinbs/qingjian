//! 网络请求在飞时，最新输入、取消和销毁必须立即生效。

use std::net::TcpListener;
use std::sync::mpsc;
use std::time::Duration;

use super::{read_body, request, respond};
use crate::chat_client::ChatClient;
use crate::config::PredictConfig;
use crate::worker::Worker;

#[test]
fn newer_request_does_not_wait_for_stalled_http_response() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let config = PredictConfig {
        base_url: format!("http://{}/v1", listener.local_addr().unwrap()),
        ..PredictConfig::default()
    };
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let server = std::thread::spawn(move || {
        let mut old = None;
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    let body = read_body(&stream);
                    let user: serde_json::Value =
                        serde_json::from_str(body["messages"][1]["content"].as_str().unwrap())
                            .unwrap();
                    if user["letters"] == "test1" {
                        old = Some(stream);
                        started_tx.send(()).unwrap();
                    } else {
                        assert_eq!(user["letters"], "test2");
                        respond(stream, "最新结果");
                        break;
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => panic!("{error}"),
            }
            if release_rx.try_recv().is_ok() || std::time::Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        if let Some(stream) = old {
            respond(stream, "过期结果");
        }
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
    requests.send(Some(request(1))).unwrap();
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    requests.send(Some(request(2))).unwrap();
    let reply = responses.recv_timeout(Duration::from_secs(1));
    let _ = release_tx.send(());
    drop(requests);
    server.join().unwrap();
    worker.join().unwrap();
    let reply = reply.expect("latest input must not wait for the old HTTP request");
    assert_eq!(reply.sequence, 2);
    assert_eq!(reply.sentence.as_deref(), Some("最新结果"));
}

#[test]
fn cancellation_and_drop_close_the_inflight_request_without_waiting() {
    use std::io::Read;

    for cancel_first in [true, false] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let config = PredictConfig {
            base_url: format!("http://{}/v1", listener.local_addr().unwrap()),
            ..PredictConfig::default()
        };
        let (started_tx, started_rx) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            read_body(&stream);
            stream
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            started_tx.send(()).unwrap();
            let mut byte = [0];
            assert_eq!(stream.read(&mut byte).unwrap(), 0);
        });
        let (requests, incoming) = tokio::sync::watch::channel(None);
        let (outgoing, responses) = mpsc::channel();
        let worker = Worker::new(
            incoming,
            outgoing,
            ChatClient::new(&config, "test-key".to_owned()),
            Duration::from_millis(10),
        );
        let (stopped_tx, stopped_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            worker.run().unwrap();
            stopped_tx.send(()).unwrap();
        });
        requests.send(Some(request(1))).unwrap();
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        if cancel_first {
            requests.send(None).unwrap();
            server.join().unwrap();
            drop(requests);
        } else {
            drop(requests);
            server.join().unwrap();
        }
        stopped_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        worker.join().unwrap();
        assert!(responses.try_recv().is_err());
    }
}

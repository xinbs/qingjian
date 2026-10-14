//! 在途查询串行执行；输入状态的取消和销毁不等待网络，过期结果不回写。

use std::net::TcpListener;
use std::sync::mpsc;
use std::time::Duration;

use super::{read_body, request, respond};
use crate::chat_client::ChatClient;
use crate::config::PredictConfig;
use crate::worker::Worker;

#[test]
fn newer_request_waits_for_shared_slot_and_discards_old_result() {
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
            if release_rx.try_recv().is_ok() {
                respond(old.take().unwrap(), "过期结果");
            }
            if std::time::Instant::now() >= deadline {
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
    assert!(responses.recv_timeout(Duration::from_millis(80)).is_err());
    let _ = release_tx.send(());
    let reply = responses.recv_timeout(Duration::from_secs(1));
    drop(requests);
    server.join().unwrap();
    worker.join().unwrap();
    let reply = reply.expect("latest input must resume after the shared slot is released");
    assert_eq!(reply.sequence, 2);
    assert_eq!(reply.sentence.as_deref(), Some("最新结果"));
}

#[test]
fn cancellation_and_drop_stop_worker_without_waiting_for_started_http() {
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
            // Worker 结束不关闭已发送的共享请求；它继续占位到完成或超时。
            assert!(
                matches!(stream.read(&mut byte), Err(error) if matches!(error.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut))
            );
            respond(stream, "过期结果");
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
            drop(requests);
        } else {
            drop(requests);
        }
        stopped_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        worker.join().unwrap();
        server.join().unwrap();
        assert!(responses.try_recv().is_err());
    }
}

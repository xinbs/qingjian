//! 共享入口的取消、超时、账号隔离与失败重试回归。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::sync::oneshot;

use super::{RequestBroker, shared_broker};
use crate::error::PredictError;

const TIMEOUT: Duration = Duration::from_secs(2);

#[test]
fn registry_isolates_accounts_and_normalizes_trailing_slashes() {
    let one = shared_broker("https://example.test/v1/", "key-a");
    assert!(Arc::ptr_eq(
        &one,
        &shared_broker("https://example.test/v1", "key-a")
    ));
    assert!(!Arc::ptr_eq(
        &one,
        &shared_broker("https://example.test/v1", "key-b")
    ));
    assert!(!Arc::ptr_eq(
        &one,
        &shared_broker("https://other.test/v1", "key-a")
    ));
}

#[tokio::test]
async fn cancellation_keeps_started_request_but_discards_unsent_queries() {
    let broker = Arc::new(RequestBroker::default());
    let count = Arc::new(AtomicUsize::new(0));
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let first = tokio::spawn({
        let broker = Arc::clone(&broker);
        let count = Arc::clone(&count);
        async move {
            broker
                .request("first".to_owned(), TIMEOUT, async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    started_tx.send(()).unwrap();
                    release_rx.await.unwrap();
                    Ok("first-result".to_owned())
                })
                .await
        }
    });
    started_rx.await.unwrap();
    let obsolete = tokio::spawn({
        let broker = Arc::clone(&broker);
        let count = Arc::clone(&count);
        async move {
            broker
                .request("obsolete".to_owned(), TIMEOUT, async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    Ok("wrong".to_owned())
                })
                .await
        }
    });
    tokio::task::yield_now().await;
    obsolete.abort();
    assert!(obsolete.await.unwrap_err().is_cancelled());
    first.abort();
    assert!(first.await.unwrap_err().is_cancelled());
    // 旧调用线程已取消，再查相同内容仍应复用已发送的请求。
    let reuse = tokio::spawn({
        let broker = Arc::clone(&broker);
        let count = Arc::clone(&count);
        async move {
            broker
                .request("first".to_owned(), TIMEOUT, async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    Ok("duplicate".to_owned())
                })
                .await
        }
    });
    tokio::task::yield_now().await;
    assert_eq!(count.load(Ordering::SeqCst), 1);
    release_tx.send(()).unwrap();
    assert_eq!(reuse.await.unwrap().unwrap(), "first-result");
    // 新内容继续正常执行；排队的过期内容不能发出。
    let count_for_work = Arc::clone(&count);
    assert_eq!(
        broker
            .request("latest".to_owned(), TIMEOUT, async move {
                count_for_work.fetch_add(1, Ordering::SeqCst);
                Ok("latest-result".to_owned())
            })
            .await
            .unwrap(),
        "latest-result"
    );
    assert_eq!(count.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn failed_and_empty_replies_do_not_become_permanent_cache_entries() {
    let broker = Arc::new(RequestBroker::default());
    assert!(
        broker
            .request("retry".to_owned(), TIMEOUT, async {
                Err(PredictError::BudgetExhausted)
            })
            .await
            .is_err()
    );
    assert_eq!(
        broker
            .request("retry".to_owned(), TIMEOUT, async { Ok("{}".to_owned()) })
            .await
            .unwrap(),
        "{}"
    );
    assert_eq!(
        broker
            .request("retry".to_owned(), TIMEOUT, async {
                Ok("available".to_owned())
            })
            .await
            .unwrap(),
        "available"
    );
}

#[tokio::test]
async fn timeout_includes_queue_and_releases_the_concurrency_slot() {
    let broker = Arc::new(RequestBroker::default());
    let (started_tx, started_rx) = oneshot::channel();
    let first = tokio::spawn({
        let broker = Arc::clone(&broker);
        async move {
            broker
                .request("slow".to_owned(), Duration::from_millis(200), async move {
                    started_tx.send(()).unwrap();
                    std::future::pending::<Result<String, PredictError>>().await
                })
                .await
        }
    });
    started_rx.await.unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let count_for_work = Arc::clone(&count);
    assert!(
        broker
            .request("queued".to_owned(), Duration::from_millis(30), async move {
                count_for_work.fetch_add(1, Ordering::SeqCst);
                Ok("wrong".to_owned())
            })
            .await
            .is_err()
    );
    assert_eq!(count.load(Ordering::SeqCst), 0);
    assert!(first.await.unwrap().is_err());
    assert_eq!(
        broker
            .request("next".to_owned(), TIMEOUT, async { Ok("next".to_owned()) })
            .await
            .unwrap(),
        "next"
    );
}

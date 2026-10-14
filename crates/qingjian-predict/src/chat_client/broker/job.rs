//! 请求任务持有并发位直到完成或原定超时；退出时移除在途记录，失败允许重新查询。

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;

use super::request_broker::{RequestBroker, SharedResult};
use crate::error::PredictError;

pub(super) struct Job {
    broker: Arc<RequestBroker>,

    key: String,

    sender: watch::Sender<SharedResult>,
}

impl Job {
    pub(super) fn new(
        broker: Arc<RequestBroker>,
        key: String,
        sender: watch::Sender<SharedResult>,
    ) -> Self {
        Self {
            broker,
            key,
            sender,
        }
    }

    pub(super) async fn run<F>(self, timeout: Duration, work: F)
    where
        F: Future<Output = Result<String, PredictError>> + Send + 'static,
    {
        let result = tokio::time::timeout(timeout, async {
            let _permit = self
                .broker
                .permit
                .acquire()
                .await
                .expect("request permit never closes");
            if self.sender.receiver_count() == 0 {
                tracing::debug!("丢弃排队的过期云查询");
                return Err(PredictError::EmptyReply);
            }
            work.await
        })
        .await
        .unwrap_or_else(|_| Err(PredictError::Timeout(timeout.as_millis() as u64)));
        // 先移除再通知：接到失败的调用者立即重试时不会复用刚刚结束的失败。
        let mut requests = self
            .broker
            .requests
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        requests.remove(&self.key);
        self.sender.send_replace(Some(result.map_err(Arc::new)));
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        let mut requests = self
            .broker
            .requests
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if requests
            .get(&self.key)
            .is_some_and(|sender| sender.same_channel(&self.sender))
        {
            requests.remove(&self.key);
        }
    }
}

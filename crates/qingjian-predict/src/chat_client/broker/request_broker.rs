//! 每个账号仅一个 HTTP 在途请求；等待者复用相同请求，取消后不发送排队的过期内容。

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::{Semaphore, watch};

use super::executor::executor;
use super::job::Job;
use crate::error::PredictError;

pub(super) type SharedResult = Option<Result<String, Arc<PredictError>>>;

pub(in crate::chat_client) struct RequestBroker {
    pub(super) requests: Mutex<HashMap<String, watch::Sender<SharedResult>>>,

    pub(super) permit: Semaphore,
}

impl Default for RequestBroker {
    fn default() -> Self {
        Self {
            requests: Mutex::default(),
            permit: Semaphore::new(1),
        }
    }
}

impl RequestBroker {
    pub(in crate::chat_client) async fn request<F>(
        self: &Arc<Self>,
        key: String,
        timeout: Duration,
        work: F,
    ) -> Result<String, PredictError>
    where
        F: Future<Output = Result<String, PredictError>> + Send + 'static,
    {
        let runtime = executor()?;
        let mut response = {
            let mut requests = self
                .requests
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if let Some(sender) = requests.get(&key) {
                tracing::debug!("云请求复用在途查询");
                sender.subscribe()
            } else {
                let (sender, receiver) = watch::channel(None);
                requests.insert(key.clone(), sender.clone());
                let job = Job::new(Arc::clone(self), key, sender);
                runtime.spawn(job.run(timeout, work));
                receiver
            }
        };
        tokio::time::timeout(timeout, async {
            loop {
                if let Some(result) = response.borrow_and_update().clone() {
                    return result.map_err(PredictError::Shared);
                }
                response.changed().await.map_err(|_| {
                    PredictError::Runtime(std::io::Error::other("cloud request task stopped"))
                })?;
            }
        })
        .await
        .map_err(|_| PredictError::Timeout(timeout.as_millis() as u64))?
    }
}

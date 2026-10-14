//! 进程内同账号的云请求共享入口；只合并正文完全相同的在途请求。

mod executor;
mod job;
mod request_broker;

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, Weak};

pub(super) use request_broker::RequestBroker;

type Registry = HashMap<(String, String), Weak<RequestBroker>>;

// 密钥只用于进程内精确隔离账号，不落盘、不记录；创建客户端时清理失效的弱引用。
static REGISTRY: LazyLock<Mutex<Registry>> = LazyLock::new(Mutex::default);

pub(super) fn shared_broker(base_url: &str, api_key: &str) -> Arc<RequestBroker> {
    let mut registry = REGISTRY.lock().unwrap_or_else(|error| error.into_inner());
    registry.retain(|_, broker| broker.strong_count() > 0);
    let key = (
        base_url.trim_end_matches('/').to_owned(),
        api_key.to_owned(),
    );
    if let Some(broker) = registry.get(&key).and_then(Weak::upgrade) {
        return broker;
    }
    let broker = Arc::new(RequestBroker::default());
    registry.insert(key, Arc::downgrade(&broker));
    broker
}

#[cfg(test)]
mod tests;

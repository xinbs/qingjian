//! 请求执行器不依赖输入状态线程的生命周期，取消本地等待不提前释放在途并发位。

use std::future::pending;
use std::sync::{LazyLock, mpsc};

use tokio::runtime::{Builder, Handle};

use crate::error::PredictError;

static EXECUTOR: LazyLock<Result<Handle, std::io::Error>> = LazyLock::new(|| {
    let (sender, receiver) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("qingjian-cloud-requests".to_owned())
        .spawn(
            move || match Builder::new_current_thread().enable_all().build() {
                Ok(runtime) => {
                    let _ = sender.send(Ok(runtime.handle().clone()));
                    runtime.block_on(pending::<()>());
                }
                Err(error) => {
                    let _ = sender.send(Err(error));
                }
            },
        )?;
    receiver
        .recv()
        .map_err(|error| std::io::Error::other(error.to_string()))?
});

pub(super) fn executor() -> Result<&'static Handle, PredictError> {
    EXECUTOR.as_ref().map_err(|error| {
        PredictError::Runtime(std::io::Error::new(error.kind(), error.to_string()))
    })
}

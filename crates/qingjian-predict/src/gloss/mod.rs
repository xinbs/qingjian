//! 释义兜底：[`qingjian_core::GlossFiller`] 的网络实现。
//!
//! 已上屏短词攒小批次请求；候选译文独立线程立即请求完整翻译，最新槽替换时取消旧请求。
//! 成功请求在进程内去重，失败冷却后允许重试；所有通道操作均不阻塞输入。

mod candidate;
mod cloud_gloss_filler;
mod prompt;
mod request_cache;
mod worker;

pub use cloud_gloss_filler::CloudGlossFiller;

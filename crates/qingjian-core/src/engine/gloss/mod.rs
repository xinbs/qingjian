//! 释义兜底与候选翻译：查表、非阻塞请求和布局标注更新。
//! 缺少译文的候选可即时入队，上屏短词仍可后台补充；结果由壳轮询后写进个人释义表。
//! 只发送待译文本，私密输入或关闭学习语言时不请求；未接入实现时不联网。

mod candidates;
mod filled_gloss;
mod filler;

pub use filled_gloss::FilledGloss;
pub use filler::{GlossFiller, NoGlossFiller};

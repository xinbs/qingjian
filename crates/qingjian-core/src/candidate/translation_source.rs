//! 候选译文的来源，与词性和生词状态分别记录。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationSource {
    /// 随包释义表；旧协议中没有来源字段的译文也按此处理。
    #[default]
    Bundled,

    /// 云端补充的译文，包括本机个人释义表里缓存的结果。
    Cloud,
}

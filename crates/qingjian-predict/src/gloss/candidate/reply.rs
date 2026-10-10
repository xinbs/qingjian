//! 候选翻译响应只允许原文和译文两个字段。

use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CandidateReply {
    pub w: String,

    pub t: String,
}

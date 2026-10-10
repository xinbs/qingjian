//! 服务商的思考参数、输出预算与等待时间；只按 URL 主机匹配。

use async_openai::types::chat::ReasoningEffort;
use serde_json::Value;

use super::host_of;

/// MiniMax M 系列的输出额度包含思考，正文之外留一份有上限的思考预算。
const MINIMAX_REASONING_TOKENS: u32 = 2048;

/// 不能关闭思考的接口至少等 15 秒，较大的用户超时仍照用。
const MINIMAX_TIMEOUT_MS: u64 = 15_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Provider {
    OpenAi,

    OpenAiOfficial,

    Zhipu,

    Kimi,

    Qwen,

    MiniMax,
}

impl Provider {
    pub(crate) fn for_url(url: &str) -> Self {
        let host = host_of(url).unwrap_or_default();
        let matches = |domains: &[&str]| {
            domains
                .iter()
                .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
        };
        if matches(&["minimaxi.com", "minimax.io", "minimax.cn"]) {
            Self::MiniMax
        } else if matches(&["bigmodel.cn", "z.ai"]) {
            Self::Zhipu
        } else if matches(&["moonshot.cn", "moonshot.ai"]) {
            Self::Kimi
        } else if matches(&["dashscope.aliyuncs.com"]) {
            Self::Qwen
        } else if matches(&["api.openai.com"]) {
            Self::OpenAiOfficial
        } else {
            Self::OpenAi
        }
    }

    pub(crate) fn timeout_ms(self, configured: u64, model: &str, effort: &str) -> u64 {
        let direct = self.is_minimax_m3(model) && effort.trim().eq_ignore_ascii_case("none");
        if self == Self::MiniMax && !direct {
            configured.max(MINIMAX_TIMEOUT_MS)
        } else {
            configured
        }
    }

    fn is_minimax_m3(self, model: &str) -> bool {
        self == Self::MiniMax && model == "MiniMax-M3"
    }

    pub(super) fn prepare_request(
        self,
        body: &mut Value,
        effort: Option<&ReasoningEffort>,
        max_tokens: u32,
    ) {
        let Some(fields) = body.as_object_mut() else {
            return;
        };
        match self {
            Self::MiniMax => {
                // M2.x 不能关闭思考；只有明确支持关闭的 M3 使用 disabled。
                let direct = fields
                    .get("model")
                    .and_then(Value::as_str)
                    .is_some_and(|model| self.is_minimax_m3(model))
                    && matches!(effort, Some(ReasoningEffort::None));
                fields.remove("reasoning_effort");
                fields.insert("reasoning_split".to_owned(), Value::Bool(true));
                if direct {
                    fields.insert(
                        "thinking".to_owned(),
                        serde_json::json!({ "type": "disabled" }),
                    );
                }
                fields.insert(
                    "max_tokens".to_owned(),
                    Value::from(if direct {
                        max_tokens
                    } else {
                        max_tokens.saturating_add(MINIMAX_REASONING_TOKENS)
                    }),
                );
            }
            Self::Zhipu | Self::Kimi if matches!(effort, Some(ReasoningEffort::None)) => {
                fields.remove("reasoning_effort");
                fields.insert(
                    "thinking".to_owned(),
                    serde_json::json!({ "type": "disabled" }),
                );
            }
            Self::Qwen if matches!(effort, Some(ReasoningEffort::None)) => {
                fields.remove("reasoning_effort");
                fields.insert("enable_thinking".to_owned(), Value::Bool(false));
            }
            Self::OpenAiOfficial
                if fields
                    .get("model")
                    .and_then(Value::as_str)
                    .is_some_and(|model| model.starts_with("gpt-4")) =>
            {
                fields.remove("reasoning_effort");
            }
            Self::OpenAi | Self::OpenAiOfficial | Self::Zhipu | Self::Kimi | Self::Qwen => {}
        }
    }

    /// 兼容没有执行 reasoning_split 的 MiniMax 回复，只剥离正文开头完整的思考段。
    pub(super) fn content(self, content: &str) -> &str {
        if self == Self::MiniMax
            && let Some(thinking) = content.trim_start().strip_prefix("<think>")
            && let Some((_, answer)) = thinking.split_once("</think>")
        {
            answer
        } else {
            content
        }
    }
}

//! OpenAI 兼容聊天客户端：服务商参数适配与 JSON 正文校验。

mod broker;
pub(crate) mod provider;

use provider::Provider;

use std::sync::{Arc, LazyLock};
use std::time::Duration;

use async_openai::Client;
use async_openai::config::OpenAIConfig;
use async_openai::types::chat::{
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
    ChatCompletionRequestUserMessage, CreateChatCompletionRequestArgs,
    CreateChatCompletionResponse, FinishReason, ReasoningEffort, ResponseFormat,
};
use qingjian_core::PredictionRequest;
use reqwest::header::{HeaderMap, HeaderValue};

use crate::config::PredictConfig;
use crate::error::PredictError;
use crate::prompt::{self, Reply};

/// 联想回复的 token 上限：几条短句足够，防止模型长篇大论。
const MAX_TOKENS: u32 = 200;

/// 采样温度：联想要稳，不要花。
const TEMPERATURE: f32 = 0.3;

/// OpenCode Zen / Go 自 2026-09-06 起要求每个请求带这个头，值是同一会话内稳定的 ID，
/// 他们靠它把同一会话路由到同一上游复用 prompt 缓存，缺了直接 400。
const OPENCODE_SESSION_HEADER: &str = "x-opencode-session";

/// 本进程的会话 ID：输入法没有「会话」概念，一个进程算一个，进程内所有客户端共用。
static SESSION_ID: LazyLock<String> = LazyLock::new(|| uuid::Uuid::new_v4().to_string());

/// OpenAI 兼容聊天接口的封装：一个请求进、若干联想条目出。
pub struct ChatClient {
    /// 底层客户端。
    client: Client<OpenAIConfig>,

    /// 模型名。
    model: String,

    /// 超时。
    timeout: Duration,

    /// 推理强度；`None` 表示不发这个参数。
    reasoning_effort: Option<ReasoningEffort>,

    /// 服务商的思考参数与预算约定。
    provider: Provider,

    /// 同一接口与密钥的客户端共用并发位及在途请求。
    broker: Arc<broker::RequestBroker>,
}

impl ChatClient {
    pub fn new(config: &PredictConfig, api_key: String) -> Self {
        let provider = Provider::for_url(&config.base_url);
        let broker = broker::shared_broker(&config.base_url, &api_key);
        let openai = OpenAIConfig::new()
            .with_api_base(config.base_url.trim_end_matches('/'))
            .with_api_key(api_key);
        Self {
            client: Client::with_config(openai).with_http_client(http_client(&config.base_url)),
            model: config.model.clone(),
            timeout: Duration::from_millis(config.request_timeout_ms()),
            reasoning_effort: parse_reasoning_effort(&config.reasoning_effort),
            provider,
            broker,
        }
    }

    pub async fn complete(&self, request: &PredictionRequest) -> Result<Reply, PredictError> {
        let user = prompt::user_prompt(request);
        tracing::debug!(sequence = request.sequence, %user, "联想请求");
        let content = self
            .chat(prompt::system_prompt(request), &user, MAX_TOKENS)
            .await?;
        prompt::parse_reply(&content, request)
    }

    /// 一问一答：系统提示 + 用户消息，要 JSON 对象，返回正文。联想与释义兜底共用。
    pub async fn chat(
        &self,
        system: &str,
        user: &str,
        max_tokens: u32,
    ) -> Result<String, PredictError> {
        let messages: Vec<ChatCompletionRequestMessage> = vec![
            ChatCompletionRequestSystemMessage::from(system).into(),
            ChatCompletionRequestUserMessage::from(user).into(),
        ];
        let mut args = CreateChatCompletionRequestArgs::default();
        args.model(&self.model)
            .messages(messages)
            .max_tokens(max_tokens)
            .temperature(TEMPERATURE)
            .response_format(ResponseFormat::JsonObject);
        if let Some(effort) = self.reasoning_effort.clone() {
            args.reasoning_effort(effort);
        }
        let mut body = serde_json::to_value(args.build()?)?;
        self.provider
            .prepare_request(&mut body, self.reasoning_effort.as_ref(), max_tokens);
        let key = body.to_string();
        let client = self.client.clone();
        let provider = self.provider;
        self.broker
            .request(key, self.timeout, async move {
                tracing::info!("云请求开始");
                let raw: serde_json::Value = client.chat().create_byot(body).await?;
                let content = response_content(&raw, provider)?;
                tracing::debug!(%content, "模型回复");
                Ok(content)
            })
            .await
    }
}

/// 网络通了不代表正文可用：截断、思考文本和非 JSON 都要在进业务解析之前报错。
fn response_content(raw: &serde_json::Value, provider: Provider) -> Result<String, PredictError> {
    let mut answer = raw.clone();
    // 服务商的计费层级不是正文协议（MiniMax-M3 返回 standard），不交给 SDK 的闭合枚举解析。
    if let Some(fields) = answer.as_object_mut() {
        fields.remove("service_tier");
    }
    let response: CreateChatCompletionResponse = serde_json::from_value(answer)?;
    let cut_off = response
        .choices
        .iter()
        .any(|choice| choice.finish_reason == Some(FinishReason::Length));
    if cut_off {
        return Err(PredictError::BudgetExhausted);
    }
    let content = response
        .choices
        .into_iter()
        .inspect(
            |choice| tracing::debug!(finish_reason = ?choice.finish_reason, "联想回复结束原因"),
        )
        .find_map(|choice| choice.message.content.filter(|c| !c.trim().is_empty()))
        .ok_or_else(|| {
            // 正文为空时原因五花八门（思考占满额度、模型名不对、接口字段不标准），留下原始响应才查得了
            tracing::warn!(response = %truncated(raw), "接口回复里没有正文");
            PredictError::EmptyReply
        })?;
    let content = provider.content(&content).trim();
    if content.is_empty() {
        return Err(PredictError::EmptyReply);
    }
    let json = serde_json::from_str::<serde_json::Value>(content)
        .map_err(|_| PredictError::InvalidReply("expected a JSON object"))?;
    if !json.is_object() {
        return Err(PredictError::InvalidReply("expected a JSON object"));
    }
    Ok(content.to_owned())
}

/// 日志里的原始响应最多留这么多字符。
const LOGGED_RESPONSE_CHARS: usize = 2000;

fn truncated(response: &serde_json::Value) -> String {
    let text = response.to_string();
    match text.char_indices().nth(LOGGED_RESPONSE_CHARS) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text,
    }
}

fn host_of(base_url: &str) -> Option<String> {
    reqwest::Url::parse(base_url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_ascii_lowercase))
}

/// 按接口地址决定 HTTP 客户端：OpenCode 带上它要求的会话头，其他服务用默认客户端。
/// 头装不上（理论上不会）就退回默认客户端，请求照发，让服务端的报错说明问题。
fn http_client(base_url: &str) -> reqwest::Client {
    if !is_opencode(base_url) {
        return reqwest::Client::new();
    }
    let Ok(value) = HeaderValue::from_str(&SESSION_ID) else {
        return reqwest::Client::new();
    };
    let mut headers = HeaderMap::new();
    headers.insert(OPENCODE_SESSION_HEADER, value);
    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .unwrap_or_default()
}

/// 接口地址是否指向 OpenCode（`opencode.ai` 及其子域）。
fn is_opencode(base_url: &str) -> bool {
    host_of(base_url).is_some_and(|host| host == "opencode.ai" || host.ends_with(".opencode.ai"))
}

/// 配置里的推理强度字符串转成接口枚举；留空不发，认不得的值当留空并记一条警告。
fn parse_reasoning_effort(value: &str) -> Option<ReasoningEffort> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" => None,
        "none" => Some(ReasoningEffort::None),
        "minimal" => Some(ReasoningEffort::Minimal),
        "low" => Some(ReasoningEffort::Low),
        "medium" => Some(ReasoningEffort::Medium),
        "high" => Some(ReasoningEffort::High),
        "xhigh" => Some(ReasoningEffort::Xhigh),
        other => {
            tracing::warn!(value = other, "reasoning_effort 不认识，不发这个参数");
            None
        }
    }
}

#[cfg(test)]
mod tests;

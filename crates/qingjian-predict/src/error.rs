use thiserror::Error;

#[derive(Debug, Error)]
pub enum PredictError {
    #[error("no API key: set `api_key` in config or the `{0}` environment variable")]
    MissingApiKey(String),

    #[error("failed to start async runtime: {0}")]
    Runtime(#[from] std::io::Error),

    #[error("API request failed: {0}")]
    Api(#[from] async_openai::error::OpenAIError),

    #[error("API request timed out after {0} ms")]
    Timeout(u64),

    #[error("API returned no usable content")]
    EmptyReply,

    /// 输出额度耗尽，可能只有思考或不完整的 JSON。
    #[error("API reply was cut off (output token budget exhausted)")]
    BudgetExhausted,

    #[error("API returned invalid content: {0}")]
    InvalidReply(&'static str),

    #[error("failed to encode request: {0}")]
    Encode(#[from] serde_json::Error),
}

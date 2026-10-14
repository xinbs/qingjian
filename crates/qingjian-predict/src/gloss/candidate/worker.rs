//! 最新候选独立翻译；候选变化时取消网络等待，不排队发送旧候选。

use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::time::Instant;

use qingjian_core::{FilledGloss, Language};
use tokio::sync::watch;

use super::prompt;
use super::request::CandidateRequest;
use crate::chat_client::ChatClient;
use crate::error::PredictError;
use crate::gloss::request_cache::{MAX_SUCCESSES, RequestCache};

const MAX_TOKENS: u32 = 400;

pub(in crate::gloss) struct CandidateWorker {
    requests: watch::Receiver<Option<CandidateRequest>>,

    responses: Sender<FilledGloss>,

    client: ChatClient,

    cache: RequestCache,

    /// 学习语言临时切换或私密模式丢弃结果后，仍可复用成功响应。
    completed: HashMap<(Language, String), FilledGloss>,
}

impl CandidateWorker {
    pub fn new(
        requests: watch::Receiver<Option<CandidateRequest>>,
        responses: Sender<FilledGloss>,
        client: ChatClient,
    ) -> Self {
        Self {
            requests,
            responses,
            client,
            cache: RequestCache::default(),
            completed: HashMap::new(),
        }
    }

    pub fn run(mut self) -> Result<(), PredictError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        runtime.block_on(self.process());
        Ok(())
    }

    async fn process(&mut self) {
        let mut pending = None;
        loop {
            let request = match pending.take() {
                Some(request) => request,
                None => {
                    if self.requests.changed().await.is_err() {
                        return;
                    }
                    let Some(request) = self.requests.borrow_and_update().clone() else {
                        continue;
                    };
                    request
                }
            };
            if request.expired() {
                continue;
            }
            if let Some(gloss) = self
                .completed
                .get(&(request.language, request.word.clone()))
            {
                if self.responses.send(gloss.clone()).is_err() {
                    return;
                }
                continue;
            }
            if !self
                .cache
                .allows(request.language, &request.word, Instant::now())
            {
                continue;
            }
            let system = prompt::system_prompt(request.language);
            let user = prompt::user_prompt(&request.word);
            let start = Instant::now();
            let response = self.client.chat(&system, &user, MAX_TOKENS);
            tokio::pin!(response);
            loop {
                tokio::select! {
                    biased;
                    changed = self.requests.changed() => {
                        if changed.is_err() {
                            return;
                        }
                        let next = self.requests.borrow_and_update().clone();
                        if next.as_ref().is_some_and(|next| request.same_text(next)) {
                            continue;
                        }
                        // 丢弃候选的本地等待；共享入口只发送仍有人等待的查询，已发送的继续占位。
                        pending = next;
                        break;
                    }
                    result = &mut response => {
                        let filled = match result {
                            Ok(content) => prompt::parse_reply(&content, request.language, &request.word),
                            Err(error) => {
                                tracing::warn!(language = request.language.code(), %error, "候选云翻译失败");
                                None
                            }
                        };
                        self.cache.record(request.language, &request.word, filled.is_some(), Instant::now());
                        tracing::info!(
                            language = request.language.code(),
                            filled = filled.is_some(),
                            elapsed_ms = start.elapsed().as_millis(),
                            "候选云翻译完成"
                        );
                        if let Some(gloss) = filled {
                            if self.completed.len() >= MAX_SUCCESSES {
                                self.completed.clear();
                            }
                            self.completed.insert((request.language, request.word.clone()), gloss.clone());
                            if self.responses.send(gloss).is_err() {
                                return;
                            }
                        }
                        break;
                    }
                }
            }
        }
    }
}

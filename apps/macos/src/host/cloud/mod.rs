//! 云端：测试连接的起停与轮询，联想的等待 / 取消 / 轮询与结果套用到当前页。

mod gloss;
mod gloss_monitor;
mod gloss_ticker;
mod predict_monitor;
mod test_monitor;

pub(super) use gloss_monitor::GlossMonitor;
pub(super) use predict_monitor::PredictMonitor;
pub(super) use test_monitor::CloudTestMonitor;

use super::Host;
use qingjian_core::{Candidate, CandidateKind, CloudWord, Prediction};
use qingjian_predict::{ConnectionTest, PredictError};

impl Host {
    /// 清空输入日志文件；开着的话重新打开继续记。
    /// 「测试连接」：用当前配置发一条最小请求，结果回到偏好设置窗口底部的状态行。
    pub(super) fn start_cloud_test(&mut self) {
        let config = self.settings.config().predict.clone();
        match ConnectionTest::start(&config) {
            Ok(test) => {
                self.cloud_test = Some(test);
                self.cloud_test_monitor.start();
                self.preferences.set_status(&format!(
                    "正在测试云服务连接：{} · {} …",
                    config.base_url, config.model
                ));
            }
            Err(error) => {
                self.preferences.set_status(&format!(
                    "云服务连接测试没能开始：{}",
                    describe_predict_error(&error)
                ));
            }
        }
    }

    /// 轮询定时器每 0.2 秒来一次：结果到了就显示并停表；等太久也停。
    pub fn poll_cloud_test(&mut self) {
        let Some(test) = &self.cloud_test else {
            self.cloud_test_monitor.stop();
            return;
        };
        let Some(outcome) = test.poll() else {
            if self.cloud_test_monitor.expired() {
                self.cloud_test = None;
                self.cloud_test_monitor.stop();
                self.preferences
                    .set_status("云服务连接测试：等了 30 秒没有结果，请检查网络或代理");
            }
            return;
        };
        self.cloud_test = None;
        self.cloud_test_monitor.stop();
        match outcome {
            Ok(report) => self.preferences.set_status(&format!(
                "云服务连接正常：{} 在 {} ms 内回复",
                report.model,
                report.elapsed.as_millis()
            )),
            Err(error) => self.preferences.set_status(&format!(
                "云服务连接失败：{}",
                describe_predict_error(&error)
            )),
        }
    }

    /// 已发出联想请求：清掉旧结果，开始轮询。
    pub fn await_prediction(&mut self) {
        self.sentence = None;
        let config = &self.settings.config().predict;
        // 后台取消旧请求，只等当前请求的防抖与超时，再给定时器留一点余量。
        let max_wait = config
            .request_timeout_ms()
            .saturating_add(config.debounce_ms)
            .saturating_add(1000);
        self.monitor
            .start(std::time::Duration::from_millis(max_wait));
    }

    /// 作废联想：不再轮询，丢掉已到的整句。
    pub fn cancel_prediction(&mut self) {
        self.engine.cancel_prediction();
        self.monitor.stop();
        self.sentence = None;
    }

    /// 定时器回调：结果到了就画上去。云端词从第一页第二格开始补入，整句挂在 preedit 右侧。
    pub fn poll_prediction(&mut self) {
        let Some(prediction) = self.engine.poll_prediction() else {
            if self.monitor.expired() {
                self.monitor.stop();
            }
            return;
        };
        self.monitor.stop();
        self.apply_prediction(prediction);
    }

    pub(super) fn apply_prediction(&mut self, prediction: Prediction) {
        // 翻译选中文字：译文作为唯一候选摆进窗口，等用户回车替换或 Esc 放弃
        if let Some(job) = self.translation.as_mut() {
            match prediction.sentence {
                Some(text) => {
                    job.result = Some(text.clone());
                    self.reset_session(None, vec![cloud_candidate(text)]);
                    self.render();
                }
                None => {
                    tracing::info!("云端没有给出译文");
                    self.end_translation();
                }
            }
            return;
        }
        if self.engine.composition().is_empty() {
            return;
        }
        // 本地首选保留；高亮已移动到插入点之后或已翻页时，不改变用户正在选的词。
        let layout = &self.session.layout;
        let untouched = self.session.page == 0 && layout.can_insert_cloud(self.session.highlighted);
        if untouched && !prediction.words.is_empty() {
            let mut words = qingjian_core::CandidateList {
                items: prediction
                    .words
                    .into_iter()
                    .map(CloudWord::into_candidate)
                    .collect(),
            };
            self.engine.annotate(&mut words);
            let filled = self.session.layout.set_cloud(words.items);
            tracing::debug!(filled, "云端词已补进候选");
        }
        self.sentence = prediction.sentence;
        self.render();
    }
}

/// 连通性测试的错误换成给用户看的中文；接口返回的原话保留，方便对着服务商文档查。
pub(super) fn describe_predict_error(error: &PredictError) -> String {
    match error {
        PredictError::Shared(error) => describe_predict_error(error),
        PredictError::MissingApiKey(env) => {
            format!("没有 API 密钥。在上面填一个，或设置环境变量 {env}")
        }
        PredictError::Timeout(ms) => {
            format!("{ms} ms 内没有回复。检查网络或代理：输入法进程不读终端里的代理变量")
        }
        PredictError::EmptyReply => "接口通了但没有返回正文，检查模型名是否正确".to_owned(),
        PredictError::BudgetExhausted => {
            "接口通了但回复被截断：模型思考或正文用完了输出额度".to_owned()
        }
        PredictError::InvalidReply(_) => {
            "接口通了但回复格式不正确，没有得到可用的 JSON 正文".to_owned()
        }
        PredictError::Encode(error) => format!("请求编码失败：{error}"),
        PredictError::Runtime(error) => format!("起不了后台线程：{error}"),
        PredictError::Api(error) => format!("请求失败：{error}"),
    }
}

/// 翻译窗口里的一行：译文（或占位文字）当作云端来源的候选画出来。
pub(super) fn cloud_candidate(text: String) -> Candidate {
    Candidate {
        text,
        kind: CandidateKind::Cloud,
        syllables: Vec::new(),
        reading: None,
        translation: None,
        aux_code: None,
    }
}

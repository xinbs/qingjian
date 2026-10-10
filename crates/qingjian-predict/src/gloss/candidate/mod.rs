//! 即时候选的完整译文，与已上屏短词的批次释义分开调度。

mod prompt;
mod reply;
mod request;
mod worker;

pub(super) use request::CandidateRequest;
pub(super) use worker::CandidateWorker;

#[cfg(test)]
mod worker_tests;

#[cfg(test)]
mod tests;

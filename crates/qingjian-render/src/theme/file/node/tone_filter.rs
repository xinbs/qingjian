//! `annotation` 节点 `tones` 里的片段种类名，对应 [`Tone`]：`gloss` / `cloud_gloss` / `fresh` / `faint` / `pos` / `separator` / `code`。

use serde::Deserialize;

use crate::frame::Tone;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ToneFilter {
    Gloss,

    #[serde(rename = "cloud_gloss")]
    CloudGloss,

    Fresh,

    Faint,

    Pos,

    Separator,

    Code,
}

impl ToneFilter {
    pub(crate) fn matches(self, tone: Tone) -> bool {
        matches!(
            (self, tone),
            (Self::Gloss, Tone::Gloss)
                // 旧主题通常只筛 gloss / fresh，云译文仍属于译文，避免升级后被隐藏。
                | (Self::Gloss | Self::Fresh | Self::CloudGloss, Tone::CloudGloss)
                | (Self::Fresh, Tone::Fresh)
                | (Self::Faint, Tone::Faint)
                | (Self::Pos, Tone::Pos)
                | (Self::Separator, Tone::Separator)
                | (Self::Code, Tone::Code)
        )
    }
}

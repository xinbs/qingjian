//! 译文与辅助注释的颜色种类。

/// annotation 片段的种类，决定颜色；青简渲染器的主题还按它挑着画。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// 译文。
    Gloss,

    /// 云端译文，包括个人释义缓存；颜色优先于生词强调色。
    CloudGloss,

    /// 生词的译文（用户还没在候选里见过几轮，`Sense::fresh`），用强调色。
    Fresh,

    /// 其余的淡色片段（假名注音），最浅。
    Faint,

    /// 词性，与 `Faint` 同色。
    Pos,

    /// 义项之间的分隔，与 `Faint` 同色。
    Separator,
}

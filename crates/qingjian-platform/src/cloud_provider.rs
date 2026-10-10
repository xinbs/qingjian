//! 云服务预设：只提供设置页的地址、模型与密钥分组，不在同步界面时修改用户配置。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudProvider {
    DeepSeek,

    MiniMax,

    OpenAi,

    Zhipu,

    Kimi,

    Qwen,

    Custom,
}

impl CloudProvider {
    pub const ALL: [Self; 7] = [
        Self::DeepSeek,
        Self::MiniMax,
        Self::OpenAi,
        Self::Zhipu,
        Self::Kimi,
        Self::Qwen,
        Self::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::DeepSeek => "DeepSeek",
            Self::MiniMax => "MiniMax",
            Self::OpenAi => "OpenAI",
            Self::Zhipu => "智谱 GLM",
            Self::Kimi => "Kimi",
            Self::Qwen => "通义千问",
            Self::Custom => "自定义（OpenAI 兼容）",
        }
    }

    pub fn base_url(self) -> Option<&'static str> {
        Some(match self {
            Self::DeepSeek => "https://api.deepseek.com",
            Self::MiniMax => "https://api.minimaxi.com/v1",
            Self::OpenAi => "https://api.openai.com/v1",
            Self::Zhipu => "https://open.bigmodel.cn/api/paas/v4",
            Self::Kimi => "https://api.moonshot.cn/v1",
            Self::Qwen => "https://dashscope.aliyuncs.com/compatible-mode/v1",
            Self::Custom => return None,
        })
    }

    pub fn model(self) -> Option<&'static str> {
        Some(match self {
            Self::DeepSeek => "deepseek-flash",
            Self::MiniMax => "MiniMax-M3",
            Self::OpenAi => "gpt-4.1-mini",
            Self::Zhipu => "glm-4.7-flash",
            Self::Kimi => "kimi-k2.6",
            Self::Qwen => "qwen-plus",
            Self::Custom => return None,
        })
    }

    /// 只匹配明确的官方地址；模型名可以自定义，不决定服务商。
    pub fn for_url(base_url: &str) -> Self {
        match base_url.trim().trim_end_matches('/') {
            "https://api.deepseek.com" | "https://api.deepseek.com/v1" => Self::DeepSeek,
            "https://api.minimaxi.com" | "https://api.minimaxi.com/v1" => Self::MiniMax,
            "https://api.openai.com/v1" => Self::OpenAi,
            "https://open.bigmodel.cn/api/paas/v4" => Self::Zhipu,
            "https://api.moonshot.cn/v1" => Self::Kimi,
            "https://dashscope.aliyuncs.com/compatible-mode/v1" => Self::Qwen,
            _ => Self::Custom,
        }
    }

    /// 自定义选择由密钥分组保留，打开窗口不会把它误改回官方预设。
    pub fn selected(base_url: &str, key_env: &str) -> Self {
        if key_env.starts_with("QINGJIAN_API_KEY_CUSTOM_") {
            Self::Custom
        } else {
            Self::for_url(base_url)
        }
    }

    /// 密钥按服务商隔离；自定义地址逐字节编码，避免不同地址共享密钥。
    pub fn key_env(self, base_url: &str) -> String {
        let suffix = match self {
            Self::DeepSeek => "DEEPSEEK",
            Self::MiniMax => "MINIMAX",
            Self::OpenAi => "OPENAI",
            Self::Zhipu => "ZHIPU",
            Self::Kimi => "KIMI",
            Self::Qwen => "QWEN",
            Self::Custom => {
                use std::fmt::Write;
                let mut name = "QINGJIAN_API_KEY_CUSTOM_".to_owned();
                for byte in base_url.trim().trim_end_matches('/').bytes() {
                    write!(&mut name, "{byte:02X}").expect("写入字符串不会失败");
                }
                return name;
            }
        };
        format!("QINGJIAN_API_KEY_{suffix}")
    }
}

#[cfg(test)]
mod tests {
    use super::CloudProvider;

    #[test]
    fn matches_official_urls_without_rewriting_custom_values() {
        for provider in CloudProvider::ALL {
            if let Some(url) = provider.base_url() {
                assert_eq!(CloudProvider::for_url(url), provider);
                assert_eq!(CloudProvider::for_url(&format!(" {url}/ ")), provider);
                assert!(provider.model().is_some());
            }
        }
        assert_eq!(
            CloudProvider::for_url("https://api.minimaxi.com"),
            CloudProvider::MiniMax
        );
        for url in [
            "https://api.minimaxi.com.attacker.example/v1",
            "http://api.minimaxi.com/v1",
            "https://api.openai.com/v1/proxy",
            "https://proxy.example/v1",
        ] {
            assert_eq!(CloudProvider::for_url(url), CloudProvider::Custom);
        }
    }

    #[test]
    fn provider_and_custom_credentials_are_isolated() {
        let mut names = std::collections::BTreeSet::new();
        for provider in CloudProvider::ALL {
            let url = provider.base_url().unwrap_or("https://custom.example/v1");
            assert!(names.insert(provider.key_env(url)));
        }
        let first = CloudProvider::Custom.key_env("https://proxy.example/v1");
        let second = CloudProvider::Custom.key_env("https://other.example/v1");
        assert_ne!(first, second);
        assert_eq!(
            first,
            CloudProvider::Custom.key_env(" https://proxy.example/v1/ ")
        );
        assert_eq!(
            CloudProvider::selected("https://api.openai.com/v1", &first),
            CloudProvider::Custom
        );
        assert_eq!(
            CloudProvider::selected("https://api.minimaxi.com/v1", "QINGJIAN_API_KEY"),
            CloudProvider::MiniMax
        );
    }
}

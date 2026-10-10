//! 配置文件的运行时状态：路径、当前生效的值、上次读取时的修改时间。
//!
//! 启动、菜单开关、切回输入法时的热加载都从这里拿 `Config`，壳只认这一份。
//! 解析失败不让输入法退出（输入优先于一切附加功能）：保留上一份能用的配置，把错误留给菜单显示。

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use qingjian_platform::{CloudProvider, Config};

use super::paths;

pub struct Settings {
    /// 配置文件路径；拿不到用户目录时为 `None`，此时一切按默认值、菜单开关不落盘。
    path: Option<PathBuf>,

    /// 当前生效的配置。
    config: Config,

    /// 上次成功读取时文件的修改时间，用来判断用户有没有手改过。
    modified: Option<SystemTime>,

    /// 最近一次读取失败的原因；成功后清掉。
    error: Option<String>,
}

impl Settings {
    /// 首次加载：先读同目录的 `.env`，文件不存在就写模板，再读配置。
    pub fn load() -> Self {
        let mut settings = Self {
            path: paths::config_file(),
            config: Config::default(),
            modified: None,
            error: None,
        };
        if let Some(path) = settings.path.clone() {
            load_dotenv(&path);
            match Config::write_template_if_missing(&path) {
                Ok(true) => tracing::info!(path = %path.display(), "已写出配置模板"),
                Ok(false) => {}
                Err(error) => tracing::warn!(%error, "写配置模板失败"),
            }
            settings.read(&path);
        }
        settings
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// 最近一次读取的错误文案，给菜单显示。
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// 文件的修改时间和上次读的不一样就重读；返回是否重读了（重读失败也算，配置没变但错误状态变了）。
    pub fn reload_if_changed(&mut self) -> bool {
        let Some(path) = self.path.clone() else {
            return false;
        };
        if modified_time(&path) == self.modified {
            return false;
        }
        self.reload();
        true
    }

    /// 强制重读，`.env` 也再读一遍（不覆盖已有的环境变量，只补新加的）。
    pub fn reload(&mut self) {
        if let Some(path) = self.path.clone() {
            load_dotenv(&path);
            self.read(&path);
        }
    }

    /// 原地改一个布尔键并重读，见 [`Self::set_value`]。
    pub fn set_bool(&mut self, section: &str, key: &str, value: bool) -> bool {
        self.set_value(section, key, value)
    }

    /// 原地改一个键并重读。写失败只记日志，返回 `false`。
    pub fn set_value(
        &mut self,
        section: &str,
        key: &str,
        value: impl Into<toml_edit::Value>,
    ) -> bool {
        let Some(path) = self.path.clone() else {
            tracing::warn!("没有配置文件路径，设置不落盘");
            return false;
        };
        let value = value.into();
        if let Err(error) = Config::set_value(&path, section, key, value.clone()) {
            tracing::warn!(%error, section, key, "写配置失败");
            return false;
        }
        tracing::info!(section, key, value = %value.to_string().trim(), "配置已改");
        self.read(&path);
        true
    }

    /// 把一个环境变量（密钥）写进配置同目录的 `.env` 并立即注入当前进程。
    /// 文件仅本用户可读；值不进日志。返回是否成功。
    pub fn set_env_var(&self, name: &str, value: &str) -> bool {
        let Some(path) = &self.path else {
            tracing::warn!("没有配置目录，密钥无处可存");
            return false;
        };
        // 先登记再动文件：后面哪一步失败，日志里都不会出现这个值
        qingjian_platform::logs::secrets::register(value);
        let env_file = path.with_file_name(".env");
        let existing = std::fs::read_to_string(&env_file).unwrap_or_default();
        let prefix = format!("{name}=");
        let mut lines: Vec<&str> = existing
            .lines()
            .filter(|line| !line.trim_start().starts_with(&prefix))
            .collect();
        let entry = format!("{prefix}{value}");
        lines.push(&entry);
        let content = format!("{}\n", lines.join("\n"));
        // 原子写且仅本用户可读（0600）
        let written = qingjian_core::storage::write_atomic_private(&env_file, |file| {
            file.write_all(content.as_bytes())
        });
        if let Err(error) = written {
            tracing::warn!(%error, path = %env_file.display(), "写 .env 失败");
            return false;
        }
        // 解析错误的原文里带着出错的那一行（含密钥），不进日志
        if dotenvy::from_path_override(&env_file).is_err() {
            tracing::warn!(path = %env_file.display(), ".env 写入后重新加载失败");
            return false;
        }
        tracing::info!(name, path = %env_file.display(), "密钥已写入 .env");
        true
    }

    /// 换服务之前保存原来的密钥，新服务只读取自己的分组，不回退到全局密钥。
    pub fn set_predict_connection(&mut self, base_url: &str, model: &str, key_env: &str) -> bool {
        let Some(path) = self.path.clone() else {
            tracing::warn!("没有配置文件路径，云服务设置不落盘");
            return false;
        };
        let previous = &self.config.predict;
        let previous_env = CloudProvider::selected(&previous.base_url, &previous.api_key_env)
            .key_env(&previous.base_url);
        if let Some(key) = previous.resolve_api_key() {
            // 原配置中的明文密钥也要保存到旧分组，才能安全清空当前连接的明文覆盖项。
            if (previous
                .api_key
                .as_deref()
                .is_some_and(|key| !key.trim().is_empty())
                || previous.api_key_env != previous_env)
                && !self.set_env_var(&previous_env, &key)
            {
                return false;
            }
            let same_address = previous.base_url.trim().trim_end_matches('/')
                == base_url.trim().trim_end_matches('/');
            if same_address && key_env != previous_env && !self.set_env_var(key_env, &key) {
                return false;
            }
        }
        let values = [
            ("base_url", toml_edit::Value::from(base_url)),
            ("model", toml_edit::Value::from(model)),
            ("api_key_env", toml_edit::Value::from(key_env)),
            ("api_key", toml_edit::Value::from("")),
        ];
        if let Err(error) = Config::set_values(&path, "predict", &values) {
            tracing::warn!(%error, "云服务设置没有保存");
            return false;
        }
        tracing::info!(base_url, model, "云服务设置已改");
        self.read(&path);
        true
    }

    fn read(&mut self, path: &Path) {
        match Config::load(path) {
            Ok(config) => {
                tracing::info!(
                    path = %path.display(),
                    predict = config.predict.enabled,
                    fuzzy = config.fuzzy.any(),
                    "配置已加载"
                );
                self.config = config;
                self.error = None;
            }
            Err(error) => {
                tracing::warn!(%error, "配置读取失败，沿用上一份");
                self.error = Some(error.to_string());
            }
        }
        // 失败也记时间：同一份坏文件不用每次激活都重读一遍
        self.modified = modified_time(path);
    }
}

/// 输入法进程由 launchd 拉起，看不到 shell 的环境变量：配置同目录的 `.env`（如 `QINGJIAN_API_KEY=...`）先读进环境。
fn load_dotenv(config_path: &Path) {
    let env_file = config_path.with_file_name(".env");
    match dotenvy::from_path(&env_file) {
        Ok(()) => tracing::info!(path = %env_file.display(), "已加载 .env"),
        Err(dotenvy::Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {}
        // 解析错误的原文里带着出错的那一行（含密钥），只记是哪一类错
        Err(dotenvy::Error::LineParse(_, position)) => {
            tracing::warn!(path = %env_file.display(), position, ".env 有一行格式不对，整个文件没读进来");
        }
        Err(dotenvy::Error::Io(error)) => {
            tracing::warn!(path = %env_file.display(), %error, ".env 读取失败");
        }
        Err(_) => tracing::warn!(path = %env_file.display(), ".env 读取失败"),
    }
}

fn modified_time(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
}

#[cfg(test)]
mod tests {
    use super::Settings;
    use qingjian_platform::{CloudProvider, Config};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST: AtomicU64 = AtomicU64::new(0);

    fn write_test_config(path: &std::path::Path, config: &Config) {
        Config::set_values(
            path,
            "predict",
            &[
                ("base_url", config.predict.base_url.as_str().into()),
                (
                    "api_key",
                    config.predict.api_key.as_deref().unwrap_or("").into(),
                ),
                ("api_key_env", config.predict.api_key_env.as_str().into()),
                ("slots", (config.predict.slots as i64).into()),
            ],
        )
        .unwrap();
    }

    #[test]
    fn switching_connections_preserves_keys_without_transferring_them() {
        let id = NEXT_TEST.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "qingjian-cloud-settings-{}-{id}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        let first_url = format!("https://first-{}-{id}.example/v1", std::process::id());
        let second_url = format!("https://second-{}-{id}.example/v1", std::process::id());
        let mut config = Config::default();
        config.predict.base_url = first_url.clone();
        let legacy_env = format!("QINGJIAN_TEST_LEGACY_KEY_{}_{id}", std::process::id());
        config.predict.api_key_env = legacy_env.clone();
        config.predict.slots = 3;
        write_test_config(&path, &config);
        let first_env = CloudProvider::Custom.key_env(&first_url);
        let second_env = CloudProvider::Custom.key_env(&second_url);
        let mut settings = Settings {
            path: Some(path.clone()),
            config,
            modified: None,
            error: None,
        };
        assert!(settings.set_env_var(&legacy_env, "old-test-token"));

        assert!(settings.set_predict_connection(&second_url, "new-model", &second_env));
        assert!(settings.config().predict.resolve_api_key().is_none());
        assert!(!settings.config().predict.enabled);
        assert_eq!(settings.config().predict.slots, 3);
        assert!(
            std::fs::read_to_string(dir.join(".env"))
                .unwrap()
                .contains(&first_env)
        );
        assert!(
            std::fs::read_to_string(dir.join(".env"))
                .unwrap()
                .contains(&legacy_env)
        );
        assert!(settings.set_env_var(&second_env, "new-test-token"));

        assert!(settings.set_predict_connection(&first_url, "first-model", &first_env));
        assert_eq!(
            settings.config().predict.resolve_api_key().as_deref(),
            Some("old-test-token")
        );
        assert_eq!(std::env::var(&second_env).unwrap(), "new-test-token");
        let saved = Config::load(&path).unwrap();
        assert_eq!(saved.predict.api_key.as_deref(), Some(""));
        assert_eq!(saved.predict.api_key_env, first_env);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn saving_same_connection_migrates_legacy_inline_key() {
        let id = NEXT_TEST.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("qingjian-cloud-legacy-{}-{id}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        let url = format!("https://legacy-{}-{id}.example/v1", std::process::id());
        let mut config = Config::default();
        config.predict.base_url = url.clone();
        config.predict.api_key = Some("legacy-test-token".to_owned());
        write_test_config(&path, &config);
        let key_env = CloudProvider::Custom.key_env(&url);
        let mut settings = Settings {
            path: Some(path),
            config,
            modified: None,
            error: None,
        };

        assert!(settings.set_predict_connection(&url, "chosen-model", &key_env));
        assert_eq!(
            settings.config().predict.resolve_api_key().as_deref(),
            Some("legacy-test-token")
        );
        assert_eq!(settings.config().predict.model, "chosen-model");
        std::fs::remove_dir_all(dir).unwrap();
    }
}

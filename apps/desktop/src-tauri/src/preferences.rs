use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppLocale {
    #[default]
    #[serde(rename = "en")]
    English,
    #[serde(rename = "zh-CN")]
    SimplifiedChinese,
}

impl AppLocale {
    pub const fn settings_title(self) -> &'static str {
        match self {
            Self::English => "Link Helm Settings",
            Self::SimplifiedChinese => "Link Helm 设置",
        }
    }

    pub const fn selector_title(self) -> &'static str {
        match self {
            Self::English => "Choose a browser profile",
            Self::SimplifiedChinese => "选择浏览器身份",
        }
    }

    pub const fn tray_labels(self) -> TrayLabels {
        match self {
            Self::English => TrayLabels {
                open_settings: "Open Settings...",
                ask_next: "Ask Next Time",
                pause: "Pause Routing",
                rescan: "Rescan Profiles",
                quit: "Quit Link Helm",
            },
            Self::SimplifiedChinese => TrayLabels {
                open_settings: "打开设置...",
                ask_next: "下次询问",
                pause: "暂停路由",
                rescan: "重新扫描身份",
                quit: "退出 Link Helm",
            },
        }
    }
}

pub struct TrayLabels {
    pub open_settings: &'static str,
    pub ask_next: &'static str,
    pub pause: &'static str,
    pub rescan: &'static str,
    pub quit: &'static str,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct AppPreferences {
    locale: AppLocale,
}

pub struct PreferencesStore {
    path: PathBuf,
    preferences: AppPreferences,
}

impl PreferencesStore {
    pub fn load(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let preferences = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self { path, preferences }
    }

    pub fn locale(&self) -> AppLocale {
        self.preferences.locale
    }

    pub fn save_locale(&mut self, locale: AppLocale) -> Result<(), String> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| "preferences file has no parent directory".to_string())?;
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let preferences = AppPreferences { locale };
        let json = serde_json::to_vec_pretty(&preferences).map_err(|error| error.to_string())?;
        let temporary_path = temporary_path(&self.path);
        std::fs::write(&temporary_path, json).map_err(|error| error.to_string())?;
        std::fs::rename(&temporary_path, &self.path).map_err(|error| error.to_string())?;
        self.preferences = preferences;
        Ok(())
    }
}

fn temporary_path(path: &Path) -> PathBuf {
    path.with_extension("tmp")
}

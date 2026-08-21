//! Linux integration using XDG paths, desktop handlers, and best-effort X11 process discovery.

#![cfg(target_os = "linux")]

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use gio::prelude::AppInfoExt;
use platform_api::{
    BrowserOpenIntent, ExecutionReceipt, PlatformAdapter, PlatformError, PlatformEventSink,
    PlatformQuery,
};
use router_model::browser::BrowserSession;
use router_model::ids::{BrowserId, ProfileId};
use serde::Serialize;

const DESKTOP_FILE_NAME: &str = "link-helm.desktop";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SystemIntegrationStatus {
    pub http_handler: Option<String>,
    pub https_handler: Option<String>,
    pub is_default_browser: bool,
    pub accessibility_trusted: bool,
    pub accessibility_required: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LinuxPlatformAdapter;

impl LinuxPlatformAdapter {
    pub fn new() -> Self {
        Self
    }

    pub fn browser_executable(&self, browser_id: &BrowserId) -> Option<PathBuf> {
        let names: &[&str] = match browser_id.as_str() {
            "com.google.Chrome" => &["google-chrome", "google-chrome-stable"],
            "com.microsoft.edgemac" => &["microsoft-edge", "microsoft-edge-stable"],
            "com.brave.Browser" => &["brave-browser"],
            "org.mozilla.firefox" => &["firefox"],
            _ => return None,
        };
        names.iter().find_map(|name| find_executable(name))
    }

    pub fn system_status(&self, _application_id: &str) -> SystemIntegrationStatus {
        let http_handler = xdg_handler("x-scheme-handler/http");
        let https_handler = xdg_handler("x-scheme-handler/https");
        SystemIntegrationStatus {
            is_default_browser: http_handler.as_deref() == Some(DESKTOP_FILE_NAME)
                && https_handler.as_deref() == Some(DESKTOP_FILE_NAME),
            http_handler,
            https_handler,
            accessibility_trusted: true,
            accessibility_required: false,
        }
    }

    pub fn set_default_browser(&self, _application_id: &str) -> Result<(), PlatformError> {
        let desktop_dir = data_home().join("applications");
        std::fs::create_dir_all(&desktop_dir).map_err(|error| {
            PlatformError::Failed(format!("cannot create applications directory: {error}"))
        })?;
        let executable = std::env::current_exe().map_err(|error| {
            PlatformError::Failed(format!("cannot resolve Link Helm executable: {error}"))
        })?;
        let desktop_file = desktop_dir.join(DESKTOP_FILE_NAME);
        let contents = format!(
            "[Desktop Entry]\nType=Application\nName=Link Helm\nExec=\"{}\" %u\nTerminal=false\nNoDisplay=true\nMimeType=x-scheme-handler/http;x-scheme-handler/https;text/html;\n",
            executable.display()
        );
        std::fs::write(&desktop_file, contents).map_err(|error| {
            PlatformError::Failed(format!("cannot write desktop entry: {error}"))
        })?;
        for scheme in ["x-scheme-handler/http", "x-scheme-handler/https"] {
            run_xdg_mime(DESKTOP_FILE_NAME, scheme)?;
        }
        Ok(())
    }

    pub fn open_default_browser_settings(&self) -> Result<(), PlatformError> {
        let settings_commands: &[(&str, &[&str])] = &[
            ("gnome-control-center", &["default-apps"]),
            ("systemsettings6", &["kcm_componentchooser"]),
            ("systemsettings5", &["kcm_componentchooser"]),
            ("systemsettings", &["kcm_componentchooser"]),
            ("exo-preferred-applications", &[]),
            ("mate-default-applications-properties", &[]),
        ];
        for &(program, args) in settings_commands {
            if find_executable(program).is_some() {
                Command::new(program).args(args).spawn().map_err(|error| {
                    PlatformError::Failed(format!(
                        "cannot open default applications settings with {program}: {error}"
                    ))
                })?;
                return Ok(());
            }
        }
        Err(PlatformError::Unsupported)
    }

    pub fn open_accessibility_settings(&self) -> Result<(), PlatformError> {
        Err(PlatformError::Unsupported)
    }

    pub fn frontmost_application_bundle_id(&self) -> Option<String> {
        let window = command_stdout("xprop", &["-root", "_NET_ACTIVE_WINDOW"])?;
        let window_id = window.split_whitespace().last()?.trim();
        let pid = command_stdout("xprop", &["-id", window_id, "_NET_WM_PID"])?
            .split_whitespace()
            .last()?
            .parse::<u32>()
            .ok()?;
        process_browser_id(pid).map(|browser_id| browser_id.as_str().to_string())
    }

    pub fn running_supported_browser_ids(&self) -> Vec<BrowserId> {
        let mut seen = HashSet::new();
        let mut result = Vec::new();
        let Ok(entries) = std::fs::read_dir("/proc") else {
            return result;
        };
        for entry in entries.flatten() {
            let Some(pid) = entry
                .file_name()
                .to_str()
                .and_then(|name| name.parse::<u32>().ok())
            else {
                continue;
            };
            if let Some(id) = process_browser_id(pid) {
                if seen.insert(id.clone()) {
                    result.push(id);
                }
            }
        }
        result
    }

    pub fn last_used_profile_id(&self, browser_id: &BrowserId) -> Option<ProfileId> {
        if browser_id.as_str() == "org.mozilla.firefox" {
            return firefox_default_profile(&self.browser_data_dir(browser_id)?)
                .map(ProfileId::new);
        }
        let bytes = std::fs::read(self.browser_data_dir(browser_id)?.join("Local State")).ok()?;
        serde_json::from_slice::<serde_json::Value>(&bytes)
            .ok()?
            .pointer("/profile/last_used")?
            .as_str()
            .map(ProfileId::new)
    }

    pub fn active_profile_ids(&self, browser_id: &BrowserId) -> Vec<ProfileId> {
        self.last_used_profile_id(browser_id).into_iter().collect()
    }

    pub fn choose_source_application(&self) -> Result<Option<String>, PlatformError> {
        Err(PlatformError::Unsupported)
    }
}

impl PlatformQuery for LinuxPlatformAdapter {
    fn browser_data_dir(&self, browser_id: &BrowserId) -> Option<PathBuf> {
        let config = config_home();
        match browser_id.as_str() {
            "com.google.Chrome" => Some(config.join("google-chrome")),
            "com.microsoft.edgemac" => Some(config.join("microsoft-edge")),
            "com.brave.Browser" => Some(config.join("BraveSoftware/Brave-Browser")),
            "org.mozilla.firefox" => Some(firefox_data_dir()),
            _ => None,
        }
    }

    fn list_profile_dirs(&self, data_dir: &Path) -> Vec<PathBuf> {
        std::fs::read_dir(data_dir)
            .map(|entries| {
                entries
                    .flatten()
                    .map(|entry| entry.path())
                    .filter(|path| path.is_dir())
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl PlatformAdapter for LinuxPlatformAdapter {
    fn observe(&self, _sink: Box<dyn PlatformEventSink>) -> Result<(), PlatformError> {
        Err(PlatformError::NotImplemented(
            "Linux foreground observation is polled from X11 when available".to_string(),
        ))
    }
    fn query_sessions(&self) -> Result<Vec<BrowserSession>, PlatformError> {
        Ok(Vec::new())
    }
    fn execute(&self, intent: BrowserOpenIntent) -> Result<ExecutionReceipt, PlatformError> {
        let executable = self
            .browser_executable(&intent.browser_id)
            .ok_or_else(|| PlatformError::Failed("browser executable is not installed".into()))?;
        Command::new(&executable)
            .args(&intent.args)
            .spawn()
            .map_err(|error| {
                PlatformError::Failed(format!(
                    "browser launch failed for {}: {error}",
                    executable.display()
                ))
            })?;
        Ok(ExecutionReceipt::confirmed())
    }
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}
fn config_home() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".config"))
}
fn data_home() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".local/share"))
}

fn find_executable(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH")?
        .to_string_lossy()
        .split(':')
        .map(PathBuf::from)
        .map(|dir| dir.join(name))
        .find(|path| path.is_file())
}
fn command_stdout(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}
fn run_command(program: &str, args: &[&str]) -> Result<std::process::Output, PlatformError> {
    Command::new(program)
        .args(args)
        .output()
        .map_err(|error| PlatformError::Failed(format!("{program} failed to start: {error}")))
        .and_then(|output| {
            if output.status.success() {
                Ok(output)
            } else {
                Err(PlatformError::Failed(format!(
                    "{program} failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                )))
            }
        })
}
fn run_xdg_mime(desktop_file: &str, mime: &str) -> Result<(), PlatformError> {
    run_command("xdg-mime", &["default", desktop_file, mime]).map(|_| ())
}
fn xdg_handler(scheme: &str) -> Option<String> {
    let uri_scheme = scheme.strip_prefix("x-scheme-handler/").unwrap_or(scheme);
    gio::AppInfo::default_for_uri_scheme(uri_scheme)
        .and_then(|app| app.id())
        .map(|id| id.to_string())
        .or_else(|| xdg_mime_handler(scheme))
}
fn xdg_mime_handler(scheme: &str) -> Option<String> {
    command_stdout("xdg-mime", &["query", "default", scheme])
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
fn process_browser_id(pid: u32) -> Option<BrowserId> {
    let executable = std::fs::read_link(format!("/proc/{pid}/exe")).ok()?;
    let name = executable
        .file_name()?
        .to_string_lossy()
        .to_ascii_lowercase();
    let id = match name.as_str() {
        "google-chrome" | "google-chrome-stable" => "com.google.Chrome",
        "microsoft-edge" | "microsoft-edge-stable" => "com.microsoft.edgemac",
        "brave" | "brave-browser" => "com.brave.Browser",
        "firefox" => "org.mozilla.firefox",
        _ => return None,
    };
    Some(BrowserId::new(id))
}
fn firefox_data_dir() -> PathBuf {
    let home = home_dir();
    let candidates = [
        home.join("snap/firefox/common/.mozilla/firefox"),
        home.join(".var/app/org.mozilla.firefox/.mozilla/firefox"),
        home.join(".mozilla/firefox"),
    ];
    candidates
        .iter()
        .find(|path| path.join("profiles.ini").is_file())
        .cloned()
        .unwrap_or_else(|| home.join(".mozilla/firefox"))
}
fn firefox_default_profile(data_dir: &Path) -> Option<String> {
    let content = std::fs::read_to_string(data_dir.join("profiles.ini")).ok()?;
    let mut name = None;
    let mut path = None;
    let mut is_default = false;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            if is_default {
                if let Some(path) = path.take() {
                    return Some(path);
                }
            }
            name = None;
            path = None;
            is_default = false;
        }
        if let Some(value) = line.strip_prefix("Name=") {
            name = Some(value.to_string());
        }
        if let Some(value) = line.strip_prefix("Path=") {
            path = Some(value.to_string());
        }
        if line == "Default=1" {
            is_default = true;
        }
    }
    if is_default {
        path.or(name)
    } else {
        path.or(name)
    }
}

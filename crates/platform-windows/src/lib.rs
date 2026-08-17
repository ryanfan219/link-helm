//! Windows integration for browser discovery, direct launch, URL-handler
//! registration, and foreground process observation.

#![cfg(target_os = "windows")]

use std::collections::HashSet;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Output;

use platform_api::{
    BrowserOpenIntent, ExecutionReceipt, PlatformAdapter, PlatformError, PlatformEventSink,
    PlatformQuery,
};
use router_model::browser::BrowserSession;
use router_model::ids::{BrowserId, ProfileId};
use serde::Serialize;
use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::Shell::{
    SHChangeNotify, ShellExecuteW, SHCNE_ASSOCCHANGED, SHCNF_IDLIST,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowThreadProcessId, SW_SHOWNORMAL,
};
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};
use winreg::RegKey;

const URL_PROG_ID: &str = "LinkHelm.Url";
const REGISTERED_APP_NAME: &str = "Link Helm";
const DEFAULT_APPS_URI: &str = "ms-settings:defaultapps?registeredAppUser=Link%20Helm";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SystemIntegrationStatus {
    pub http_handler: Option<String>,
    pub https_handler: Option<String>,
    pub is_default_browser: bool,
    pub accessibility_trusted: bool,
    pub accessibility_required: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WindowsPlatformAdapter;

impl WindowsPlatformAdapter {
    pub fn new() -> Self {
        Self
    }

    pub fn browser_executable(&self, browser_id: &BrowserId) -> Option<PathBuf> {
        browser_executable_candidates(browser_id)
            .into_iter()
            .find(|path| path.is_file())
    }

    pub fn system_status(&self, _application_id: &str) -> SystemIntegrationStatus {
        let http_handler = user_choice_handler("http");
        let https_handler = user_choice_handler("https");
        SystemIntegrationStatus {
            is_default_browser: http_handler.as_deref() == Some(URL_PROG_ID)
                && https_handler.as_deref() == Some(URL_PROG_ID),
            http_handler,
            https_handler,
            accessibility_trusted: true,
            accessibility_required: false,
        }
    }

    pub fn set_default_browser(&self, _application_id: &str) -> Result<(), PlatformError> {
        register_url_handler()?;
        self.open_default_browser_settings()
    }

    pub fn open_default_browser_settings(&self) -> Result<(), PlatformError> {
        shell_open(
            DEFAULT_APPS_URI,
            "cannot open Windows Default Apps settings",
        )
    }

    pub fn open_accessibility_settings(&self) -> Result<(), PlatformError> {
        Err(PlatformError::Unsupported)
    }

    pub fn frontmost_application_bundle_id(&self) -> Option<String> {
        foreground_process_name()
    }

    pub fn running_supported_browser_ids(&self) -> Vec<BrowserId> {
        let mut seen = HashSet::new();
        running_process_names()
            .into_iter()
            .filter_map(|name| browser_id_for_process(&name))
            .filter(|browser_id| seen.insert(browser_id.clone()))
            .collect()
    }

    pub fn last_used_profile_id(&self, browser_id: &BrowserId) -> Option<ProfileId> {
        if browser_id.as_str() == "org.mozilla.firefox" {
            return self
                .browser_data_dir(browser_id)
                .and_then(|dir| firefox_default_profile(&dir))
                .map(ProfileId::new);
        }
        let data_dir = self.browser_data_dir(browser_id)?;
        let bytes = std::fs::read(data_dir.join("Local State")).ok()?;
        parse_last_used_profile_id(&bytes).map(ProfileId::new)
    }

    pub fn active_profile_ids(&self, browser_id: &BrowserId) -> Vec<ProfileId> {
        if browser_id.as_str() == "org.mozilla.firefox" {
            return self.last_used_profile_id(browser_id).into_iter().collect();
        }
        let Some(data_dir) = self.browser_data_dir(browser_id) else {
            return Vec::new();
        };
        let Ok(bytes) = std::fs::read(data_dir.join("Local State")) else {
            return Vec::new();
        };
        parse_active_profile_ids(&bytes)
            .into_iter()
            .map(ProfileId::new)
            .collect()
    }

    pub fn choose_source_application(&self) -> Result<Option<String>, PlatformError> {
        const SCRIPT: &str = r#"Add-Type -AssemblyName System.Windows.Forms; $dialog = New-Object System.Windows.Forms.OpenFileDialog; $dialog.Title = 'Choose source application'; $dialog.Filter = 'Applications (*.exe)|*.exe'; if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) { [System.IO.Path]::GetFileName($dialog.FileName).ToLowerInvariant() }"#;
        let Output {
            status,
            stdout,
            stderr,
        } = std::process::Command::new("powershell.exe")
            .args(["-NoLogo", "-NoProfile", "-STA", "-Command", SCRIPT])
            .output()
            .map_err(|error| {
                PlatformError::Failed(format!("application chooser failed to start: {error}"))
            })?;

        if !status.success() {
            return Err(PlatformError::Failed(format!(
                "application chooser failed: {}",
                String::from_utf8_lossy(&stderr).trim()
            )));
        }
        let application_id = String::from_utf8_lossy(&stdout).trim().to_string();
        Ok((!application_id.is_empty()).then_some(application_id))
    }
}

impl PlatformQuery for WindowsPlatformAdapter {
    fn browser_data_dir(&self, browser_id: &BrowserId) -> Option<PathBuf> {
        let (root, relative) = match browser_id.as_str() {
            "com.google.Chrome" => ("LOCALAPPDATA", "Google/Chrome/User Data"),
            "com.microsoft.edgemac" => ("LOCALAPPDATA", "Microsoft/Edge/User Data"),
            "com.brave.Browser" => ("LOCALAPPDATA", "BraveSoftware/Brave-Browser/User Data"),
            "org.mozilla.firefox" => ("APPDATA", "Mozilla/Firefox"),
            _ => return None,
        };
        std::env::var_os(root)
            .map(PathBuf::from)
            .map(|path| path.join(relative))
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

impl PlatformAdapter for WindowsPlatformAdapter {
    fn observe(&self, _sink: Box<dyn PlatformEventSink>) -> Result<(), PlatformError> {
        Err(PlatformError::NotImplemented(
            "Win32 foreground observation is polled by the desktop shell".to_string(),
        ))
    }

    fn query_sessions(&self) -> Result<Vec<BrowserSession>, PlatformError> {
        Ok(Vec::new())
    }

    fn execute(&self, intent: BrowserOpenIntent) -> Result<ExecutionReceipt, PlatformError> {
        let executable = self
            .browser_executable(&intent.browser_id)
            .ok_or_else(|| PlatformError::Failed("browser executable is not installed".into()))?;
        std::process::Command::new(&executable)
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

fn browser_executable_candidates(browser_id: &BrowserId) -> Vec<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let program_files = std::env::var_os("ProgramFiles").map(PathBuf::from);
    let program_files_x86 = std::env::var_os("ProgramFiles(x86)").map(PathBuf::from);
    let roots: Vec<(Option<PathBuf>, &str)> = match browser_id.as_str() {
        "com.google.Chrome" => vec![
            (local.clone(), "Google/Chrome/Application/chrome.exe"),
            (
                program_files.clone(),
                "Google/Chrome/Application/chrome.exe",
            ),
            (
                program_files_x86.clone(),
                "Google/Chrome/Application/chrome.exe",
            ),
        ],
        "com.microsoft.edgemac" => vec![
            (
                program_files_x86.clone(),
                "Microsoft/Edge/Application/msedge.exe",
            ),
            (
                program_files.clone(),
                "Microsoft/Edge/Application/msedge.exe",
            ),
            (local.clone(), "Microsoft/Edge/Application/msedge.exe"),
        ],
        "com.brave.Browser" => vec![
            (
                local.clone(),
                "BraveSoftware/Brave-Browser/Application/brave.exe",
            ),
            (
                program_files.clone(),
                "BraveSoftware/Brave-Browser/Application/brave.exe",
            ),
            (
                program_files_x86,
                "BraveSoftware/Brave-Browser/Application/brave.exe",
            ),
        ],
        "org.mozilla.firefox" => vec![
            (program_files.clone(), "Mozilla Firefox/firefox.exe"),
            (program_files_x86, "Mozilla Firefox/firefox.exe"),
            (local, "Mozilla Firefox/firefox.exe"),
        ],
        _ => Vec::new(),
    };
    roots
        .into_iter()
        .filter_map(|(root, relative)| root.map(|path| path.join(relative)))
        .collect()
}

fn browser_id_for_process(process_name: &str) -> Option<BrowserId> {
    let id = match process_name {
        "chrome.exe" => "com.google.Chrome",
        "msedge.exe" => "com.microsoft.edgemac",
        "brave.exe" => "com.brave.Browser",
        "firefox.exe" => "org.mozilla.firefox",
        _ => return None,
    };
    Some(BrowserId::new(id))
}

fn parse_last_used_profile_id(bytes: &[u8]) -> Option<String> {
    serde_json::from_slice::<serde_json::Value>(bytes)
        .ok()?
        .pointer("/profile/last_used")?
        .as_str()
        .filter(|profile_id| !profile_id.trim().is_empty())
        .map(str::to_string)
}

fn parse_active_profile_ids(bytes: &[u8]) -> Vec<String> {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return Vec::new();
    };
    let Some(profile_ids) = value
        .pointer("/profile/last_active_profiles")
        .and_then(serde_json::Value::as_array)
    else {
        return Vec::new();
    };
    let mut seen = HashSet::new();
    profile_ids
        .iter()
        .filter_map(serde_json::Value::as_str)
        .filter(|profile_id| !profile_id.trim().is_empty())
        .filter(|profile_id| seen.insert((*profile_id).to_string()))
        .map(str::to_string)
        .collect()
}

fn firefox_default_profile(data_dir: &Path) -> Option<String> {
    let content = std::fs::read_to_string(data_dir.join("installs.ini")).ok()?;
    content.lines().find_map(|line| {
        line.trim()
            .strip_prefix("Default=")
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
    })
}

fn user_choice_handler(scheme: &str) -> Option<String> {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(
            format!(
                r"Software\Microsoft\Windows\Shell\Associations\UrlAssociations\{scheme}\UserChoice"
            ),
            KEY_READ,
        )
        .ok()?
        .get_value("ProgId")
        .ok()
}

fn register_url_handler() -> Result<(), PlatformError> {
    let executable = std::env::current_exe().map_err(|error| {
        PlatformError::Failed(format!("cannot resolve Link Helm executable: {error}"))
    })?;
    let command = format!(r#""{}" "%1""#, executable.display());
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    let (handler, _) = hkcu
        .create_subkey(format!(r"Software\Classes\{URL_PROG_ID}"))
        .map_err(registry_error)?;
    handler
        .set_value("", &"Link Helm URL")
        .map_err(registry_error)?;
    handler
        .set_value("URL Protocol", &"")
        .map_err(registry_error)?;
    let (icon, _) = handler
        .create_subkey("DefaultIcon")
        .map_err(registry_error)?;
    icon.set_value("", &format!(r#""{}",0"#, executable.display()))
        .map_err(registry_error)?;
    let (open_command, _) = handler
        .create_subkey(r"shell\open\command")
        .map_err(registry_error)?;
    open_command
        .set_value("", &command)
        .map_err(registry_error)?;

    let (capabilities, _) = hkcu
        .create_subkey(r"Software\LinkHelm\Capabilities")
        .map_err(registry_error)?;
    capabilities
        .set_value("ApplicationName", &REGISTERED_APP_NAME)
        .map_err(registry_error)?;
    capabilities
        .set_value(
            "ApplicationIcon",
            &format!(r#""{}",0"#, executable.display()),
        )
        .map_err(registry_error)?;
    capabilities
        .set_value(
            "ApplicationDescription",
            &"Routes web links to browser profiles",
        )
        .map_err(registry_error)?;
    let (associations, _) = capabilities
        .create_subkey("UrlAssociations")
        .map_err(registry_error)?;
    associations
        .set_value("http", &URL_PROG_ID)
        .map_err(registry_error)?;
    associations
        .set_value("https", &URL_PROG_ID)
        .map_err(registry_error)?;
    let (registered, _) = hkcu
        .create_subkey(r"Software\RegisteredApplications")
        .map_err(registry_error)?;
    registered
        .set_value(REGISTERED_APP_NAME, &r"Software\LinkHelm\Capabilities")
        .map_err(registry_error)?;

    unsafe {
        SHChangeNotify(
            SHCNE_ASSOCCHANGED as i32,
            SHCNF_IDLIST,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    Ok(())
}

fn registry_error(error: std::io::Error) -> PlatformError {
    PlatformError::Failed(format!("Windows registry update failed: {error}"))
}

fn foreground_process_name() -> Option<String> {
    let window = unsafe { GetForegroundWindow() };
    if window.is_null() {
        return None;
    }
    let mut process_id = 0;
    unsafe { GetWindowThreadProcessId(window, &mut process_id) };
    process_name(process_id)
}

fn process_name(process_id: u32) -> Option<String> {
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
    if process.is_null() {
        return None;
    }
    let mut buffer = vec![0_u16; 32768];
    let mut length = buffer.len() as u32;
    let succeeded =
        unsafe { QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut length) };
    unsafe { CloseHandle(process) };
    if succeeded == 0 {
        return None;
    }
    PathBuf::from(String::from_utf16_lossy(&buffer[..length as usize]))
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
}

fn running_process_names() -> Vec<String> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Vec::new();
    }
    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
    let mut names = Vec::new();
    let mut has_entry = unsafe { Process32FirstW(snapshot, &mut entry) } != 0;
    while has_entry {
        let length = entry
            .szExeFile
            .iter()
            .position(|character| *character == 0)
            .unwrap_or(entry.szExeFile.len());
        names.push(String::from_utf16_lossy(&entry.szExeFile[..length]).to_lowercase());
        has_entry = unsafe { Process32NextW(snapshot, &mut entry) } != 0;
    }
    unsafe { CloseHandle(snapshot) };
    names
}

fn shell_open(target: &str, context: &str) -> Result<(), PlatformError> {
    let operation = wide("open");
    let target = wide(target);
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            target.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if result as isize <= 32 {
        Err(PlatformError::Failed(format!(
            "{context}: ShellExecuteW returned {}",
            result as isize
        )))
    } else {
        Ok(())
    }
}

fn wide(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(Some(0)).collect()
}

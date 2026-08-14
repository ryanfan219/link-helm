//! macOS integration for browser discovery, direct launch, default-handler
//! status, and Accessibility permission state.

mod accessibility;
mod launch_services;

use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Output};

#[cfg(unix)]
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::net::UnixStream;

use platform_api::{
    BrowserOpenIntent, ExecutionReceipt, PlatformAdapter, PlatformError, PlatformEventSink,
    PlatformQuery,
};
use router_model::browser::BrowserSession;
use router_model::browser::OpenDisposition;
use router_model::ids::{BrowserId, ProfileId};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SystemIntegrationStatus {
    pub http_handler: Option<String>,
    pub https_handler: Option<String>,
    pub is_default_browser: bool,
    pub accessibility_trusted: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MacOsPlatformAdapter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrowserLaunchStrategy {
    ActiveWindowAutomation,
    DirectExecutable,
}

fn browser_launch_strategy(disposition: OpenDisposition) -> BrowserLaunchStrategy {
    match disposition {
        OpenDisposition::ActiveWindow => BrowserLaunchStrategy::ActiveWindowAutomation,
        OpenDisposition::ExistingWindow | OpenDisposition::NewWindow => {
            BrowserLaunchStrategy::DirectExecutable
        }
    }
}

fn uses_chromium_singleton(browser_id: &BrowserId) -> bool {
    matches!(
        browser_id.as_str(),
        "com.google.Chrome" | "com.microsoft.edgemac" | "com.brave.Browser"
    )
}

fn spawn_and_reap(command: &mut std::process::Command) -> std::io::Result<u32> {
    let mut child = command.spawn()?;
    let pid = child.id();
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(pid)
}

fn encode_singleton_message(current_dir: &str, executable: &str, args: &[String]) -> Vec<u8> {
    let mut message = b"START\0".to_vec();
    message.extend_from_slice(current_dir.as_bytes());
    for argument in std::iter::once(executable).chain(args.iter().map(String::as_str)) {
        message.push(0);
        message.extend_from_slice(argument.as_bytes());
    }
    message
}

#[cfg(unix)]
fn notify_running_chromium(
    data_dir: &Path,
    executable: &Path,
    args: &[String],
) -> std::io::Result<bool> {
    let socket_link = data_dir.join("SingletonSocket");
    let socket_path = match std::fs::read_link(&socket_link) {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    let local_cookie = match std::fs::read_link(data_dir.join("SingletonCookie")) {
        Ok(cookie) => cookie,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    let remote_cookie_path = socket_path
        .parent()
        .ok_or_else(|| std::io::Error::other("Chrome singleton socket has no parent directory"))?
        .join("SingletonCookie");
    let remote_cookie = std::fs::read_link(remote_cookie_path)?;
    if local_cookie != remote_cookie {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "Chrome singleton cookie verification failed",
        ));
    }

    let mut stream = match UnixStream::connect(socket_path) {
        Ok(stream) => stream,
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
            ) =>
        {
            return Ok(false);
        }
        Err(error) => return Err(error),
    };
    let timeout = Some(std::time::Duration::from_secs(5));
    stream.set_read_timeout(timeout)?;
    stream.set_write_timeout(timeout)?;
    let current_dir = std::env::current_dir()?;
    let message = encode_singleton_message(
        current_dir.to_string_lossy().as_ref(),
        executable.to_string_lossy().as_ref(),
        args,
    );
    stream.write_all(&message)?;
    stream.shutdown(std::net::Shutdown::Write)?;

    let mut response = [0_u8; 8];
    let length = stream.read(&mut response)?;
    match &response[..length] {
        b"ACK" => Ok(true),
        b"SHUTDOWN" => Ok(false),
        response => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "unexpected Chrome singleton response: {}",
                String::from_utf8_lossy(response)
            ),
        )),
    }
}

const CHROME_OPEN_FRONT_WINDOW_SCRIPT: &str = r#"
on run argv
    set targetUrl to item 1 of argv
    tell application "Google Chrome"
        if (count windows) is 0 then
            open location targetUrl
        else
            tell window 1 to make new tab with properties {URL:targetUrl}
        end if
        activate
    end tell
end run
"#;

fn parse_application_chooser_output(
    status: ExitStatus,
    stdout: &[u8],
    stderr: &[u8],
) -> Result<Option<String>, PlatformError> {
    if status.success() {
        let bundle_id = String::from_utf8_lossy(stdout).trim().to_string();
        return if bundle_id.is_empty() {
            Err(PlatformError::Failed(
                "application chooser returned an empty bundle ID".to_string(),
            ))
        } else {
            Ok(Some(bundle_id))
        };
    }

    let message = String::from_utf8_lossy(stderr).trim().to_string();
    if message.contains("(-128)") {
        Ok(None)
    } else {
        Err(PlatformError::Failed(format!(
            "application chooser failed: {message}"
        )))
    }
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

    let mut seen = std::collections::HashSet::new();
    profile_ids
        .iter()
        .filter_map(serde_json::Value::as_str)
        .filter(|profile_id| !profile_id.trim().is_empty())
        .filter(|profile_id| seen.insert((*profile_id).to_string()))
        .map(str::to_string)
        .collect()
}

impl MacOsPlatformAdapter {
    pub fn new() -> Self {
        Self
    }

    pub fn browser_executable(&self, browser_id: &BrowserId) -> Option<PathBuf> {
        let path = match browser_id.as_str() {
            "com.google.Chrome" => "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "com.microsoft.edgemac" => {
                "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge"
            }
            "com.brave.Browser" => "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
            "org.mozilla.firefox" => "/Applications/Firefox.app/Contents/MacOS/firefox",
            _ => return None,
        };
        Some(PathBuf::from(path))
    }

    pub fn system_status(&self, bundle_id: &str) -> SystemIntegrationStatus {
        let http_handler = launch_services::default_handler("http");
        let https_handler = launch_services::default_handler("https");
        SystemIntegrationStatus {
            is_default_browser: launch_services::is_default_handler(
                http_handler.as_deref(),
                https_handler.as_deref(),
                bundle_id,
            ),
            http_handler,
            https_handler,
            accessibility_trusted: accessibility::is_trusted(),
        }
    }

    pub fn open_default_browser_settings(&self) -> Result<(), PlatformError> {
        launch_services::open_default_browser_settings().map_err(PlatformError::Failed)
    }

    pub fn set_default_browser(&self, bundle_id: &str) -> Result<(), PlatformError> {
        launch_services::set_default_browser(bundle_id).map_err(PlatformError::Failed)
    }

    pub fn open_accessibility_settings(&self) -> Result<(), PlatformError> {
        accessibility::open_settings().map_err(PlatformError::Failed)
    }

    pub fn frontmost_application_bundle_id(&self) -> Option<String> {
        #[cfg(target_os = "macos")]
        {
            use objc2_app_kit::NSWorkspace;

            NSWorkspace::sharedWorkspace()
                .frontmostApplication()
                .and_then(|application| application.bundleIdentifier())
                .map(|bundle_id| bundle_id.to_string())
        }

        #[cfg(not(target_os = "macos"))]
        None
    }

    pub fn running_supported_browser_ids(&self) -> Vec<BrowserId> {
        #[cfg(target_os = "macos")]
        {
            use objc2_app_kit::NSWorkspace;

            let applications = NSWorkspace::sharedWorkspace().runningApplications();
            (0..applications.count())
                .filter_map(|index| applications.objectAtIndex(index).bundleIdentifier())
                .map(|bundle_id| bundle_id.to_string())
                .filter(|bundle_id| {
                    self.browser_executable(&BrowserId::new(bundle_id))
                        .is_some()
                })
                .map(BrowserId::new)
                .collect()
        }

        #[cfg(not(target_os = "macos"))]
        Vec::new()
    }

    pub fn last_used_profile_id(&self, browser_id: &BrowserId) -> Option<ProfileId> {
        let data_dir = self.browser_data_dir(browser_id)?;
        let bytes = std::fs::read(data_dir.join("Local State")).ok()?;
        parse_last_used_profile_id(&bytes).map(ProfileId::new)
    }

    pub fn active_profile_ids(&self, browser_id: &BrowserId) -> Vec<ProfileId> {
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
        let Output {
            status,
            stdout,
            stderr,
        } = std::process::Command::new("/usr/bin/osascript")
            .args([
                "-e",
                "id of (choose application with prompt \"Choose source application\")",
            ])
            .output()
            .map_err(|error| {
                PlatformError::Failed(format!("application chooser failed to start: {error}"))
            })?;

        parse_application_chooser_output(status, &stdout, &stderr)
    }
}

impl PlatformQuery for MacOsPlatformAdapter {
    fn browser_data_dir(&self, browser_id: &BrowserId) -> Option<PathBuf> {
        let relative = match browser_id.as_str() {
            "com.google.Chrome" => "Google/Chrome",
            "com.microsoft.edgemac" => "Microsoft Edge",
            "com.brave.Browser" => "BraveSoftware/Brave-Browser",
            "org.mozilla.firefox" => "Firefox",
            _ => return None,
        };
        let home = std::env::var_os("HOME").map(PathBuf::from)?;
        Some(home.join("Library/Application Support").join(relative))
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

impl PlatformAdapter for MacOsPlatformAdapter {
    fn observe(&self, _sink: Box<dyn PlatformEventSink>) -> Result<(), PlatformError> {
        // Hook: NSWorkspace notifications + AXUIElement window/focus observation
        // to freeze a pre-routing RuntimeSnapshot.
        Err(PlatformError::NotImplemented(
            "NSWorkspace/AX observation".to_string(),
        ))
    }

    fn query_sessions(&self) -> Result<Vec<BrowserSession>, PlatformError> {
        // Hook: enumerate browser processes and windows, then map each window to
        // a stable profile ID.
        Ok(Vec::new())
    }

    fn execute(&self, intent: BrowserOpenIntent) -> Result<ExecutionReceipt, PlatformError> {
        match browser_launch_strategy(intent.disposition) {
            BrowserLaunchStrategy::ActiveWindowAutomation => {
                return self.open_active_browser_window(&intent);
            }
            BrowserLaunchStrategy::DirectExecutable => {}
        }

        let executable = self
            .browser_executable(&intent.browser_id)
            .ok_or_else(|| PlatformError::Failed("unknown browser executable".to_string()))?;
        if !executable.is_file() {
            return Err(PlatformError::Failed(format!(
                "browser executable is not installed: {}",
                executable.display()
            )));
        }

        #[cfg(unix)]
        if uses_chromium_singleton(&intent.browser_id) {
            let data_dir = self
                .browser_data_dir(&intent.browser_id)
                .ok_or_else(|| PlatformError::Failed("browser data dir unknown".to_string()))?;
            match notify_running_chromium(&data_dir, &executable, &intent.args) {
                Ok(true) => return Ok(ExecutionReceipt::confirmed()),
                Ok(false) => {}
                Err(error) => {
                    return Err(PlatformError::Failed(format!(
                        "running browser notification failed: {error}"
                    )));
                }
            }
        }

        spawn_and_reap(std::process::Command::new(executable).args(&intent.args))
            .map_err(|error| PlatformError::Failed(format!("browser launch failed: {error}")))?;
        Ok(ExecutionReceipt::confirmed())
    }
}

impl MacOsPlatformAdapter {
    fn open_active_browser_window(
        &self,
        intent: &BrowserOpenIntent,
    ) -> Result<ExecutionReceipt, PlatformError> {
        if intent.browser_id.as_str() != "com.google.Chrome" {
            std::process::Command::new("/usr/bin/open")
                .args(["-b", intent.browser_id.as_str(), intent.url.as_str()])
                .spawn()
                .map_err(|error| {
                    PlatformError::Failed(format!("browser launch failed: {error}"))
                })?;
            return Ok(ExecutionReceipt::confirmed());
        }

        let output = std::process::Command::new("/usr/bin/osascript")
            .args(["-e", CHROME_OPEN_FRONT_WINDOW_SCRIPT, intent.url.as_str()])
            .output()
            .map_err(|error| {
                PlatformError::Failed(format!("Chrome automation failed to start: {error}"))
            })?;
        if output.status.success() {
            Ok(ExecutionReceipt::confirmed())
        } else {
            Err(PlatformError::Failed(format!(
                "Chrome automation failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::process::ExitStatus;

    use platform_api::PlatformQuery;
    use router_model::browser::OpenDisposition;
    use router_model::ids::BrowserId;

    use super::{
        browser_launch_strategy, encode_singleton_message, notify_running_chromium,
        parse_active_profile_ids, parse_application_chooser_output, parse_last_used_profile_id,
        spawn_and_reap, uses_chromium_singleton, BrowserLaunchStrategy, MacOsPlatformAdapter,
    };

    #[test]
    fn singleton_protocol_is_limited_to_chromium_browsers() {
        assert!(uses_chromium_singleton(&BrowserId::new(
            "com.google.Chrome"
        )));
        assert!(uses_chromium_singleton(&BrowserId::new(
            "com.microsoft.edgemac"
        )));
        assert!(uses_chromium_singleton(&BrowserId::new(
            "com.brave.Browser"
        )));
        assert!(!uses_chromium_singleton(&BrowserId::new(
            "org.mozilla.firefox"
        )));
    }

    #[test]
    fn chromium_singleton_message_preserves_profile_and_url_arguments() {
        let message = encode_singleton_message(
            "/tmp/lynko",
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            &[
                "--profile-directory=Default".to_string(),
                "https://example.com/path".to_string(),
            ],
        );

        assert_eq!(
            message,
            b"START\0/tmp/lynko\0/Applications/Google Chrome.app/Contents/MacOS/Google Chrome\0--profile-directory=Default\0https://example.com/path"
        );
    }

    #[cfg(unix)]
    #[test]
    fn running_chromium_is_notified_through_its_singleton_socket() {
        use std::io::{Read, Write};
        use std::os::unix::fs::symlink;
        use std::os::unix::net::UnixListener;

        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!(".tmp-s-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let data_dir = root.join("data");
        let socket_dir = root.join("socket");
        std::fs::create_dir_all(&data_dir).unwrap();
        std::fs::create_dir_all(&socket_dir).unwrap();
        let socket_path = socket_dir.join("SingletonSocket");
        let listener = match UnixListener::bind(&socket_path) {
            Ok(listener) => listener,
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                std::fs::remove_dir_all(root).unwrap();
                eprintln!("socket test skipped because the sandbox denied bind()");
                return;
            }
            Err(error) => panic!("singleton test socket should bind: {error}"),
        };
        symlink(&socket_path, data_dir.join("SingletonSocket")).unwrap();
        symlink("test-cookie", data_dir.join("SingletonCookie")).unwrap();
        symlink("test-cookie", socket_dir.join("SingletonCookie")).unwrap();

        let (sender, receiver) = std::sync::mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut message = Vec::new();
            stream.read_to_end(&mut message).unwrap();
            stream.write_all(b"ACK").unwrap();
            sender.send(message).unwrap();
        });

        let notified = notify_running_chromium(
            &data_dir,
            std::path::Path::new("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
            &[
                "--profile-directory=Default".to_string(),
                "https://example.com".to_string(),
            ],
        )
        .unwrap();

        assert!(notified);
        let message = receiver.recv().unwrap();
        assert!(message
            .windows(b"--profile-directory=Default".len())
            .any(|window| window == b"--profile-directory=Default"));
        server.join().unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn launched_child_is_reaped_after_exit() {
        let mut command = std::process::Command::new("/usr/bin/true");
        let pid = spawn_and_reap(&mut command).expect("child should start");

        for _ in 0..50 {
            let status = std::process::Command::new("/bin/kill")
                .args(["-0", &pid.to_string()])
                .stderr(std::process::Stdio::null())
                .status()
                .expect("process existence should be testable");
            if !status.success() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        panic!("child process {pid} was not reaped");
    }

    #[test]
    fn maps_supported_browser_ids_to_application_support_directories() {
        let platform = MacOsPlatformAdapter::new();

        let chrome = platform
            .browser_data_dir(&BrowserId::new("com.google.Chrome"))
            .expect("Chrome data directory");
        assert!(chrome.ends_with("Library/Application Support/Google/Chrome"));

        let firefox = platform
            .browser_data_dir(&BrowserId::new("org.mozilla.firefox"))
            .expect("Firefox data directory");
        assert!(firefox.ends_with("Library/Application Support/Firefox"));

        assert!(platform
            .browser_data_dir(&BrowserId::new("com.example.unknown"))
            .is_none());
    }

    #[test]
    fn resolves_known_browser_executables_without_shell_commands() {
        let platform = MacOsPlatformAdapter::new();

        assert_eq!(
            platform
                .browser_executable(&BrowserId::new("com.google.Chrome"))
                .unwrap(),
            std::path::PathBuf::from(
                "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
            )
        );
        assert!(platform
            .browser_executable(&BrowserId::new("com.example.unknown"))
            .is_none());
    }

    #[test]
    fn specified_profile_uses_direct_profile_routing() {
        assert_eq!(
            browser_launch_strategy(OpenDisposition::ExistingWindow),
            BrowserLaunchStrategy::DirectExecutable
        );
    }

    #[test]
    fn active_window_uses_existing_browser_automation() {
        assert_eq!(
            browser_launch_strategy(OpenDisposition::ActiveWindow),
            BrowserLaunchStrategy::ActiveWindowAutomation
        );
    }

    #[test]
    fn explicit_new_window_uses_browser_arguments() {
        assert_eq!(
            browser_launch_strategy(OpenDisposition::NewWindow),
            BrowserLaunchStrategy::DirectExecutable
        );
    }

    #[cfg(unix)]
    #[test]
    fn chooser_output_returns_a_trimmed_bundle_id() {
        use std::os::unix::process::ExitStatusExt;

        let result = parse_application_chooser_output(
            ExitStatus::from_raw(0),
            b"com.alibaba.DingTalkMac\n",
            b"",
        )
        .unwrap();

        assert_eq!(result.as_deref(), Some("com.alibaba.DingTalkMac"));
    }

    #[cfg(unix)]
    #[test]
    fn chooser_cancellation_returns_no_selection() {
        use std::os::unix::process::ExitStatusExt;

        let result = parse_application_chooser_output(
            ExitStatus::from_raw(256),
            b"",
            b"execution error: User canceled. (-128)\n",
        )
        .unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn parses_stable_last_used_chromium_profile_id() {
        let json = br#"{"profile":{"last_used":"Profile 6"}}"#;

        assert_eq!(
            parse_last_used_profile_id(json).as_deref(),
            Some("Profile 6")
        );
        assert_eq!(parse_last_used_profile_id(b"{}"), None);
    }

    #[test]
    fn parses_all_profiles_with_open_chromium_windows() {
        let json = br#"{"profile":{"last_active_profiles":["Default","Profile 6","Default"]}}"#;

        assert_eq!(
            parse_active_profile_ids(json),
            vec!["Default".to_string(), "Profile 6".to_string()]
        );
    }
}

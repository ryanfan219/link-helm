use std::sync::Mutex;

use platform_api::PlatformAdapter;
use router_model::browser::OpenDisposition;
use router_model::config::RouterConfig;
use router_model::routing::RouteDecision;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::diagnostics::DiagnosticEvent;
use crate::preferences::{AppLocale, PreferencesStore};
use crate::state::{BrowserInstallation, ConfigImportPreview, DesktopService};

const SYSTEM_SETTINGS_BUNDLE_ID: &str = "com.apple.systempreferences";

pub struct AppState {
    pub service: Mutex<DesktopService>,
    pub preferences: Mutex<PreferencesStore>,
}

#[derive(Default)]
struct FocusReturnTracker {
    system_settings_seen: bool,
}

impl FocusReturnTracker {
    fn observe(&mut self, frontmost_bundle_id: Option<&str>) -> bool {
        if frontmost_bundle_id == Some(SYSTEM_SETTINGS_BUNDLE_ID) {
            self.system_settings_seen = true;
            false
        } else {
            self.system_settings_seen
        }
    }
}

fn restore_settings_after_system_settings(app: AppHandle) {
    std::thread::spawn(move || {
        let platform = platform_macos::MacOsPlatformAdapter::new();
        let mut tracker = FocusReturnTracker::default();

        loop {
            let frontmost_bundle_id = platform.frontmost_application_bundle_id();
            if tracker.observe(frontmost_bundle_id.as_deref()) {
                std::thread::sleep(std::time::Duration::from_millis(150));
                crate::tray::show_settings(&app);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    });
}

#[derive(Debug, Clone, Serialize)]
pub struct AppSnapshot {
    pub locale: AppLocale,
    pub config: RouterConfig,
    pub config_error: Option<String>,
    pub browsers: Vec<BrowserInstallation>,
    pub paused: bool,
    pub ask_next: bool,
    pub diagnostics: Vec<DiagnosticEvent>,
    pub diagnostics_limit: usize,
    pub diagnostics_error: Option<String>,
    pub system: platform_macos::SystemIntegrationStatus,
}

#[derive(Debug, Clone, Serialize)]
pub struct SelectorState {
    pub locale: AppLocale,
    pub pending: Vec<crate::state::PendingRoute>,
    pub browsers: Vec<BrowserInstallation>,
}

#[tauri::command]
pub fn get_state(app: AppHandle, state: State<'_, AppState>) -> Result<AppSnapshot, String> {
    let service = state.service.lock().map_err(|error| error.to_string())?;
    let locale = state
        .preferences
        .lock()
        .map_err(|error| error.to_string())?
        .locale();
    Ok(AppSnapshot {
        locale,
        config: service.config.clone(),
        config_error: service.config_error.clone(),
        browsers: service.browsers.clone(),
        paused: service.paused,
        ask_next: service.ask_next,
        diagnostics: service.diagnostics.events().to_vec(),
        diagnostics_limit: service.diagnostics.limit(),
        diagnostics_error: service.diagnostics.persistence_error().map(str::to_string),
        system: service.platform().system_status(&app.config().identifier),
    })
}

#[tauri::command]
pub fn set_locale(
    app: AppHandle,
    state: State<'_, AppState>,
    locale: AppLocale,
) -> Result<AppLocale, String> {
    let previous_locale = {
        let mut preferences = state
            .preferences
            .lock()
            .map_err(|error| error.to_string())?;
        let previous_locale = preferences.locale();
        preferences.save_locale(locale)?;
        previous_locale
    };
    if let Err(error) = crate::tray::set_locale(&app, locale) {
        if let Ok(mut preferences) = state.preferences.lock() {
            let _ = preferences.save_locale(previous_locale);
        }
        let _ = crate::tray::set_locale(&app, previous_locale);
        return Err(error.to_string());
    }
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.set_title(locale.settings_title());
    }
    if let Some(window) = app.get_webview_window("selector") {
        let _ = window.set_title(locale.selector_title());
    }
    Ok(locale)
}

#[tauri::command]
pub async fn set_default_browser(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<platform_macos::SystemIntegrationStatus, String> {
    let platform = {
        let service = state.service.lock().map_err(|error| error.to_string())?;
        *service.platform()
    };
    let bundle_id = app.config().identifier.clone();
    let registration_bundle_id = bundle_id.clone();

    tauri::async_runtime::spawn_blocking(move || {
        platform.set_default_browser(&registration_bundle_id)
    })
    .await
    .map_err(|error| format!("default browser registration task failed: {error}"))?
    .map_err(|error| error.to_string())?;

    Ok(platform.system_status(&bundle_id))
}

#[tauri::command]
pub fn open_default_browser_settings(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let platform = {
        let service = state.service.lock().map_err(|error| error.to_string())?;
        *service.platform()
    };
    platform
        .open_default_browser_settings()
        .map_err(|error| error.to_string())?;
    restore_settings_after_system_settings(app);
    Ok(())
}

#[tauri::command]
pub fn open_accessibility_settings(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let platform = {
        let service = state.service.lock().map_err(|error| error.to_string())?;
        *service.platform()
    };
    platform
        .open_accessibility_settings()
        .map_err(|error| error.to_string())?;
    restore_settings_after_system_settings(app);
    Ok(())
}

#[tauri::command]
pub async fn choose_source_application(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let platform = {
        let service = state.service.lock().map_err(|error| error.to_string())?;
        *service.platform()
    };
    let result = tauri::async_runtime::spawn_blocking(move || platform.choose_source_application())
        .await
        .map_err(|error| format!("application chooser task failed: {error}"))?
        .map_err(|error| error.to_string());
    crate::tray::show_settings(&app);
    result
}

#[tauri::command]
pub fn scan_browsers(state: State<'_, AppState>) -> Result<Vec<BrowserInstallation>, String> {
    let mut service = state.service.lock().map_err(|error| error.to_string())?;
    Ok(service.scan_browsers())
}

#[tauri::command]
pub fn save_config(state: State<'_, AppState>, config: RouterConfig) -> Result<(), String> {
    state
        .service
        .lock()
        .map_err(|error| error.to_string())?
        .save_config(config)
}

#[tauri::command]
pub fn export_config(state: State<'_, AppState>) -> Result<String, String> {
    state
        .service
        .lock()
        .map_err(|error| error.to_string())?
        .export_config()
}

#[tauri::command]
pub fn preview_import_config(
    state: State<'_, AppState>,
    json: String,
) -> Result<ConfigImportPreview, String> {
    state
        .service
        .lock()
        .map_err(|error| error.to_string())?
        .preview_import_config(&json)
}

#[tauri::command]
pub fn import_config(state: State<'_, AppState>, json: String) -> Result<(), String> {
    state
        .service
        .lock()
        .map_err(|error| error.to_string())?
        .import_config(&json)
}

#[tauri::command]
pub fn preview_route(
    state: State<'_, AppState>,
    source_app: String,
    url: String,
) -> Result<RouteDecision, String> {
    state
        .service
        .lock()
        .map_err(|error| error.to_string())?
        .preview(source_app, url)
}

#[tauri::command]
pub fn test_open(
    state: State<'_, AppState>,
    browser_id: String,
    profile_id: String,
    url: String,
) -> Result<(), String> {
    let mut service = state.service.lock().map_err(|error| error.to_string())?;
    let parsed_url = url::Url::parse(&url).map_err(|error| error.to_string())?;
    let installation = service
        .browsers
        .iter()
        .find(|browser| browser.descriptor.id.as_str() == browser_id)
        .ok_or_else(|| "browser has not been scanned".to_string())?;
    let profile = installation
        .profiles
        .iter()
        .find(|profile| profile.profile_id.as_str() == profile_id)
        .ok_or_else(|| "profile was not found".to_string())?;
    let intent = service.build_open_intent(profile, &parsed_url, test_open_disposition())?;
    service
        .platform()
        .execute(intent)
        .map_err(|error| error.to_string())?;
    let identity_id = format!("{browser_id}/{profile_id}");
    service.diagnostics.record_route(
        "com.example.lynko.settings",
        &parsed_url,
        "opened",
        None,
        Some(&identity_id),
        None,
    );
    Ok(())
}

#[tauri::command]
pub fn set_paused(state: State<'_, AppState>, paused: bool) -> Result<(), String> {
    state
        .service
        .lock()
        .map_err(|error| error.to_string())?
        .paused = paused;
    Ok(())
}

#[tauri::command]
pub fn set_ask_next(state: State<'_, AppState>, ask_next: bool) -> Result<(), String> {
    state
        .service
        .lock()
        .map_err(|error| error.to_string())?
        .ask_next = ask_next;
    Ok(())
}

#[tauri::command]
pub fn clear_diagnostics(state: State<'_, AppState>) -> Result<(), String> {
    state
        .service
        .lock()
        .map_err(|error| error.to_string())?
        .diagnostics
        .clear();
    Ok(())
}

#[tauri::command]
pub fn set_diagnostics_limit(
    state: State<'_, AppState>,
    limit: usize,
) -> Result<(), String> {
    state
        .service
        .lock()
        .map_err(|error| error.to_string())?
        .diagnostics
        .set_limit(limit)
}

#[tauri::command]
pub fn get_selector_state(state: State<'_, AppState>) -> Result<SelectorState, String> {
    let service = state.service.lock().map_err(|error| error.to_string())?;
    let locale = state
        .preferences
        .lock()
        .map_err(|error| error.to_string())?
        .locale();
    Ok(SelectorState {
        locale,
        pending: service.pending_routes().to_vec(),
        browsers: service.browsers.clone(),
    })
}

#[tauri::command]
pub fn choose_pending(
    state: State<'_, AppState>,
    id: u64,
    browser_id: String,
    profile_id: String,
) -> Result<(), String> {
    state
        .service
        .lock()
        .map_err(|error| error.to_string())?
        .choose_pending(id, &browser_id, &profile_id)
}

#[tauri::command]
pub fn cancel_pending(state: State<'_, AppState>, id: u64) -> Result<(), String> {
    state
        .service
        .lock()
        .map_err(|error| error.to_string())?
        .cancel_pending(id)
}

fn test_open_disposition() -> OpenDisposition {
    OpenDisposition::ExistingWindow
}

#[cfg(test)]
mod tests {
    use router_model::browser::OpenDisposition;

    use super::{test_open_disposition, FocusReturnTracker};

    #[test]
    fn profile_test_prefers_an_existing_browser_managed_window() {
        assert_eq!(test_open_disposition(), OpenDisposition::ExistingWindow);
    }

    #[test]
    fn focus_is_not_restored_before_system_settings_has_been_seen() {
        let mut tracker = FocusReturnTracker::default();

        assert!(!tracker.observe(Some("com.apple.Safari")));
        assert!(!tracker.observe(None));
    }

    #[test]
    fn focus_is_restored_after_leaving_system_settings() {
        let mut tracker = FocusReturnTracker::default();

        assert!(!tracker.observe(Some("com.apple.systempreferences")));
        assert!(!tracker.observe(Some("com.apple.systempreferences")));
        assert!(tracker.observe(Some("com.apple.Safari")));
    }
}

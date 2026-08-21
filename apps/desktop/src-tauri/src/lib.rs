mod commands;
mod diagnostics;
mod platform;
mod preferences;
mod routing;
mod state;
mod tray;

use config_store::ConfigStore;
use tauri::Manager;

use commands::AppState;
use state::DesktopService;

fn should_hide_settings(label: &str, focused: bool) -> bool {
    label == "settings" && focused
}

fn start_foreground_browser_observer(app: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        if let Some(state) = app.try_state::<AppState>() {
            if let Ok(mut service) = state.service.lock() {
                service.observe_foreground_browser();
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(400));
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
        routing::handle_url_args(app, args);
    }));

    let app = builder
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::set_default_browser,
            commands::open_default_browser_settings,
            commands::open_accessibility_settings,
            commands::choose_source_application,
            commands::scan_browsers,
            commands::save_config,
            commands::export_config,
            commands::preview_import_config,
            commands::import_config,
            commands::preview_route,
            commands::test_open,
            commands::set_paused,
            commands::set_ask_next,
            commands::clear_diagnostics,
            commands::set_diagnostics_limit,
            commands::set_locale,
            commands::get_selector_state,
            commands::choose_pending,
            commands::cancel_pending,
        ])
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let preferences =
                preferences::PreferencesStore::load(data_dir.join("preferences.json"));
            let locale = preferences.locale();
            let mut service = DesktopService::new(ConfigStore::new(data_dir.join("config.json")));
            service.scan_browsers();
            app.manage(AppState {
                service: std::sync::Mutex::new(service),
                preferences: std::sync::Mutex::new(preferences),
            });
            tray::setup(app, locale)?;
            if let Some(window) = app.get_webview_window("settings") {
                window.set_title(locale.settings_title())?;
            }
            start_foreground_browser_observer(app.handle().clone());
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            routing::handle_url_args(app.handle(), std::env::args());
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "settings" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    if should_hide_settings(window.label(), window.is_focused().unwrap_or(false)) {
                        let _ = window.hide();
                    }
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building Link Helm");

    #[cfg(target_os = "macos")]
    let mut app = app;

    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);

    app.run(|app, event| {
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Opened { urls } = event {
            for url in urls {
                routing::handle_opened_url(app, url);
            }
        }

        #[cfg(not(target_os = "macos"))]
        let _ = (app, event);
    });
}

#[cfg(test)]
mod tests {
    use super::should_hide_settings;

    #[test]
    fn only_a_focused_settings_close_request_hides_settings() {
        assert!(should_hide_settings("settings", true));
        assert!(!should_hide_settings("settings", false));
        assert!(!should_hide_settings("selector", true));
    }
}

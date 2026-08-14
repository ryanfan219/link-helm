use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{App, AppHandle, Manager};

use crate::commands::AppState;

const OPEN_SETTINGS: &str = "open-settings";
const ASK_NEXT: &str = "ask-next";
const PAUSE: &str = "pause-routing";
const RESCAN: &str = "rescan-profiles";
const QUIT: &str = "quit";

pub fn show_settings(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn setup(app: &App) -> tauri::Result<()> {
    let open = MenuItem::with_id(
        app,
        OPEN_SETTINGS,
        "Open Settings...",
        true,
        Some("CmdOrCtrl+,"),
    )?;
    let ask = CheckMenuItem::with_id(app, ASK_NEXT, "Ask Next Time", true, false, None::<&str>)?;
    let pause = CheckMenuItem::with_id(app, PAUSE, "Pause Routing", true, false, None::<&str>)?;
    let rescan = MenuItem::with_id(app, RESCAN, "Rescan Profiles", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, QUIT, "Quit Lynko", true, Some("CmdOrCtrl+Q"))?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &ask, &pause, &rescan, &separator, &quit])?;

    TrayIconBuilder::new()
        .icon(tauri::image::Image::from_bytes(include_bytes!(
            "../icons/tray-icon.png"
        ))?)
        .icon_as_template(true)
        .tooltip("Lynko")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            OPEN_SETTINGS => show_settings(app),
            ASK_NEXT => {
                if let Some(state) = app.try_state::<AppState>() {
                    if let Ok(mut service) = state.service.lock() {
                        service.ask_next = !service.ask_next;
                    }
                }
            }
            PAUSE => {
                if let Some(state) = app.try_state::<AppState>() {
                    if let Ok(mut service) = state.service.lock() {
                        service.paused = !service.paused;
                    }
                }
            }
            RESCAN => {
                if let Some(state) = app.try_state::<AppState>() {
                    if let Ok(mut service) = state.service.lock() {
                        service.scan_browsers();
                    }
                }
            }
            QUIT => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

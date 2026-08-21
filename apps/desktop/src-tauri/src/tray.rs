use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{App, AppHandle, Manager, Runtime};

use crate::commands::AppState;
use crate::preferences::AppLocale;

const OPEN_SETTINGS: &str = "open-settings";
const ASK_NEXT: &str = "ask-next";
const PAUSE: &str = "pause-routing";
const RESCAN: &str = "rescan-profiles";
const QUIT: &str = "quit";
const TRAY_ID: &str = "main";

#[cfg(target_os = "linux")]
const TRAY_ICON: &[u8] = include_bytes!("../icons/icon.png");
#[cfg(not(target_os = "linux"))]
const TRAY_ICON: &[u8] = include_bytes!("../icons/tray-icon.png");

pub fn show_settings(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn menu<R: Runtime, M: Manager<R>>(
    manager: &M,
    locale: AppLocale,
    ask_next: bool,
    paused: bool,
) -> tauri::Result<Menu<R>> {
    let labels = locale.tray_labels();
    let open = MenuItem::with_id(
        manager,
        OPEN_SETTINGS,
        labels.open_settings,
        true,
        Some("CmdOrCtrl+,"),
    )?;
    let ask = CheckMenuItem::with_id(
        manager,
        ASK_NEXT,
        labels.ask_next,
        true,
        ask_next,
        None::<&str>,
    )?;
    let pause = CheckMenuItem::with_id(manager, PAUSE, labels.pause, true, paused, None::<&str>)?;
    let rescan = MenuItem::with_id(manager, RESCAN, labels.rescan, true, None::<&str>)?;
    let quit = MenuItem::with_id(manager, QUIT, labels.quit, true, Some("CmdOrCtrl+Q"))?;
    let separator = PredefinedMenuItem::separator(manager)?;
    Menu::with_items(manager, &[&open, &ask, &pause, &rescan, &separator, &quit])
}

pub fn set_locale(app: &AppHandle, locale: AppLocale) -> tauri::Result<()> {
    let (ask_next, paused) = app
        .try_state::<AppState>()
        .and_then(|state| {
            state
                .service
                .lock()
                .ok()
                .map(|service| (service.ask_next, service.paused))
        })
        .unwrap_or((false, false));
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        tray.set_menu(Some(menu(app, locale, ask_next, paused)?))?;
    }
    Ok(())
}

pub fn setup(app: &App, locale: AppLocale) -> tauri::Result<()> {
    let menu = menu(app, locale, false, false)?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(tauri::image::Image::from_bytes(TRAY_ICON)?)
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("Link Helm")
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

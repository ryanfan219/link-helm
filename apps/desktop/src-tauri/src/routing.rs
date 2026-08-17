use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::commands::AppState;
use crate::preferences::AppLocale;
use crate::state::RouteDisposition;

pub fn show_selector(app: &AppHandle) -> tauri::Result<()> {
    let locale = app
        .try_state::<AppState>()
        .and_then(|state| {
            state
                .preferences
                .lock()
                .ok()
                .map(|preferences| preferences.locale())
        })
        .unwrap_or(AppLocale::English);
    if let Some(window) = app.get_webview_window("selector") {
        window.set_title(locale.selector_title())?;
        window.show()?;
        window.set_focus()?;
        window.eval("window.location.reload()")?;
        return Ok(());
    }
    WebviewWindowBuilder::new(app, "selector", WebviewUrl::App("selector.html".into()))
        .title(locale.selector_title())
        .inner_size(520.0, 430.0)
        .min_inner_size(420.0, 320.0)
        .resizable(true)
        .center()
        .build()?;
    Ok(())
}

pub fn handle_opened_url(app: &AppHandle, url: url::Url) {
    let own_application_id = own_application_id(app);
    let result = app
        .try_state::<AppState>()
        .ok_or_else(|| "application state is unavailable".to_string())
        .and_then(|state| {
            let mut service = state.service.lock().map_err(|error| error.to_string())?;
            let source_app = source_application_or_unknown(
                service.platform().frontmost_application_bundle_id(),
                &own_application_id,
            );
            service.route_url(url, source_app)
        });
    match result {
        Ok(RouteDisposition::Ask) => {
            let _ = show_selector(app);
        }
        Ok(RouteDisposition::Opened | RouteDisposition::Failed) => {}
        Err(error) => eprintln!("Link Helm could not route URL: {error}"),
    }
}

pub fn handle_url_args(app: &AppHandle, args: impl IntoIterator<Item = String>) {
    for url in args.into_iter().filter_map(|argument| {
        url::Url::parse(&argument)
            .ok()
            .filter(|url| matches!(url.scheme(), "http" | "https"))
    }) {
        handle_opened_url(app, url);
    }
}

#[cfg(target_os = "macos")]
fn own_application_id(app: &AppHandle) -> String {
    app.config().identifier.clone()
}

#[cfg(target_os = "windows")]
fn own_application_id(_app: &AppHandle) -> String {
    std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().to_lowercase())
        })
        .unwrap_or_default()
}

fn source_application_or_unknown(candidate: Option<String>, own_bundle_id: &str) -> String {
    candidate
        .filter(|bundle_id| !bundle_id.is_empty() && bundle_id != own_bundle_id)
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::source_application_or_unknown;

    #[test]
    fn source_application_filters_the_router_itself() {
        assert_eq!(
            source_application_or_unknown(Some("com.apple.mail".into()), "com.example.linkhelm"),
            "com.apple.mail"
        );
        assert_eq!(
            source_application_or_unknown(
                Some("com.example.linkhelm".into()),
                "com.example.linkhelm"
            ),
            "unknown"
        );
        assert_eq!(
            source_application_or_unknown(None, "com.example.linkhelm"),
            "unknown"
        );
    }
}

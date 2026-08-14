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
    let own_bundle_id = app.config().identifier.clone();
    let result = app
        .try_state::<AppState>()
        .ok_or_else(|| "application state is unavailable".to_string())
        .and_then(|state| {
            let mut service = state.service.lock().map_err(|error| error.to_string())?;
            let source_app = source_application_or_unknown(
                service.platform().frontmost_application_bundle_id(),
                &own_bundle_id,
            );
            service.route_url(url, source_app)
        });
    match result {
        Ok(RouteDisposition::Ask) => {
            let _ = show_selector(app);
        }
        Ok(RouteDisposition::Opened | RouteDisposition::Failed) => {}
        Err(error) => eprintln!("Lynko could not route URL: {error}"),
    }
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
            source_application_or_unknown(Some("com.apple.mail".into()), "com.example.lynko"),
            "com.apple.mail"
        );
        assert_eq!(
            source_application_or_unknown(Some("com.example.lynko".into()), "com.example.lynko"),
            "unknown"
        );
        assert_eq!(
            source_application_or_unknown(None, "com.example.lynko"),
            "unknown"
        );
    }
}

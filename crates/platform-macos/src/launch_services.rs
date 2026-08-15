use core_foundation::base::TCFType;
use core_foundation::string::{CFString, CFStringRef};
use std::sync::mpsc;
use std::time::Duration;

use block2::RcBlock;
use objc2_app_kit::NSWorkspace;
use objc2_foundation::{NSBundle, NSError, NSString};

#[link(name = "CoreServices", kind = "framework")]
unsafe extern "C" {
    fn LSCopyDefaultHandlerForURLScheme(scheme: CFStringRef) -> CFStringRef;
}

const DEFAULT_BROWSER_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.Desktop-Settings.extension";

pub fn default_handler(scheme: &str) -> Option<String> {
    let scheme = CFString::new(scheme);
    let handler = unsafe { LSCopyDefaultHandlerForURLScheme(scheme.as_concrete_TypeRef()) };
    if handler.is_null() {
        None
    } else {
        Some(unsafe { CFString::wrap_under_create_rule(handler) }.to_string())
    }
}

pub const fn default_browser_settings_url() -> &'static str {
    DEFAULT_BROWSER_SETTINGS_URL
}

pub fn open_default_browser_settings() -> Result<(), String> {
    std::process::Command::new("/usr/bin/open")
        .arg(default_browser_settings_url())
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("cannot open default browser settings: {error}"))
}

pub fn set_default_browser(bundle_id: &str) -> Result<(), String> {
    register_default_handlers(
        bundle_id,
        |scheme, _| request_default_handler(scheme),
        default_handler,
    )
}

fn request_default_handler(scheme: &str) -> Result<(), String> {
    let workspace = NSWorkspace::sharedWorkspace();
    let application_url = NSBundle::mainBundle().bundleURL();
    let scheme = NSString::from_str(scheme);
    let (sender, receiver) = mpsc::sync_channel(1);
    let completion = RcBlock::new(move |error: *mut NSError| {
        let result = if error.is_null() {
            Ok(())
        } else {
            let error = unsafe { &*error };
            Err(format!(
                "{} ({} {})",
                error.localizedDescription(),
                error.domain(),
                error.code()
            ))
        };
        let _ = sender.send(result);
    });

    workspace.setDefaultApplicationAtURL_toOpenURLsWithScheme_completionHandler(
        &application_url,
        &scheme,
        Some(&completion),
    );

    match receiver.recv_timeout(Duration::from_secs(120)) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            Err("timed out waiting for macOS authorization".to_string())
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err("macOS registration callback was disconnected".to_string())
        }
    }
}

fn register_default_handlers(
    bundle_id: &str,
    mut set_handler: impl FnMut(&str, &str) -> Result<(), String>,
    mut get_handler: impl FnMut(&str) -> Option<String>,
) -> Result<(), String> {
    for scheme in ["http", "https"] {
        if let Err(error) = set_handler(scheme, bundle_id) {
            let actual = get_handler(scheme);
            if actual.as_deref() != Some(bundle_id) {
                return Err(format!("cannot register {scheme} handler: {error}"));
            }
        }
    }
    for scheme in ["http", "https"] {
        let actual = get_handler(scheme);
        if actual.as_deref() != Some(bundle_id) {
            return Err(format!(
                "{scheme} handler verification failed: expected {bundle_id}, found {}",
                actual.as_deref().unwrap_or("none")
            ));
        }
    }
    Ok(())
}

pub fn is_default_handler(http: Option<&str>, https: Option<&str>, bundle_id: &str) -> bool {
    http == Some(bundle_id) && https == Some(bundle_id)
}

#[cfg(test)]
mod tests {
    use super::{default_browser_settings_url, is_default_handler, register_default_handlers};

    #[test]
    fn direct_registration_sets_and_verifies_both_web_schemes() {
        let mut registrations = Vec::new();

        register_default_handlers(
            "com.example.linkhelm",
            |scheme, bundle_id| {
                registrations.push((scheme.to_string(), bundle_id.to_string()));
                Ok(())
            },
            |_| Some("com.example.linkhelm".to_string()),
        )
        .unwrap();

        assert_eq!(
            registrations,
            vec![
                ("http".to_string(), "com.example.linkhelm".to_string()),
                ("https".to_string(), "com.example.linkhelm".to_string())
            ]
        );
    }

    #[test]
    fn direct_registration_rejects_unverified_handler_state() {
        let error = register_default_handlers(
            "com.example.linkhelm",
            |_, _| Ok(()),
            |scheme| {
                (scheme == "http")
                    .then(|| "com.example.linkhelm".to_string())
                    .or_else(|| Some("com.google.Chrome".to_string()))
            },
        )
        .unwrap_err();

        assert!(error.contains("https"));
        assert!(error.contains("com.google.Chrome"));
    }

    #[test]
    fn direct_registration_identifies_the_scheme_that_failed() {
        let error = register_default_handlers(
            "com.example.linkhelm",
            |scheme, _| {
                if scheme == "https" {
                    Err("permission denied".to_string())
                } else {
                    Ok(())
                }
            },
            |_| Some("com.google.Chrome".to_string()),
        )
        .unwrap_err();

        assert_eq!(
            error,
            "cannot register https handler: permission denied".to_string()
        );
    }

    #[test]
    fn requires_both_web_schemes_to_be_registered() {
        assert!(is_default_handler(
            Some("com.example.linkhelm"),
            Some("com.example.linkhelm"),
            "com.example.linkhelm"
        ));
        assert!(!is_default_handler(
            Some("com.example.linkhelm"),
            Some("com.google.Chrome"),
            "com.example.linkhelm"
        ));
    }

    #[test]
    fn default_browser_action_opens_the_desktop_settings_pane() {
        assert_eq!(
            default_browser_settings_url(),
            "x-apple.systempreferences:com.apple.Desktop-Settings.extension"
        );
    }
}

use serde_json::Value;
use url::Url;

use platform_api::{BrowserOpenIntent, PlatformQuery};
use router_model::browser::{
    BrowserCapabilities, BrowserDescriptor, BrowserProfile, OpenDisposition,
};
use router_model::ids::{BrowserId, ProfileId};

use super::{BrowserAdapter, BrowserError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChromiumAdapter {
    browser_id: BrowserId,
    display_name: String,
    executable_hint: Option<String>,
    data_dir_name: String,
}

impl ChromiumAdapter {
    pub fn new(
        browser_id: impl Into<String>,
        display_name: impl Into<String>,
        executable_hint: Option<String>,
        data_dir_name: impl Into<String>,
    ) -> Self {
        Self {
            browser_id: BrowserId::new(browser_id),
            display_name: display_name.into(),
            executable_hint,
            data_dir_name: data_dir_name.into(),
        }
    }
}

impl BrowserAdapter for ChromiumAdapter {
    fn descriptor(&self) -> BrowserDescriptor {
        BrowserDescriptor {
            id: self.browser_id.clone(),
            display_name: self.display_name.clone(),
            executable_hint: self.executable_hint.clone(),
        }
    }

    fn capabilities(&self) -> BrowserCapabilities {
        BrowserCapabilities {
            discover_profiles: true,
            map_windows_to_profiles: false,
            open_in_existing_profile: true,
            create_profile_window: true,
            distinguish_incognito: false,
        }
    }

    fn discover_profiles(
        &self,
        platform: &dyn PlatformQuery,
    ) -> Result<Vec<BrowserProfile>, BrowserError> {
        let data_dir = platform
            .browser_data_dir(&self.browser_id)
            .ok_or_else(|| BrowserError::Other("browser data dir unknown".to_string()))?;
        let profile_dirs = platform.list_profile_dirs(&data_dir);
        let display_names = read_profile_display_names(&data_dir);

        let mut profiles = Vec::new();
        for dir in profile_dirs {
            let name = dir
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if name != "Default" && !name.starts_with("Profile ") {
                continue;
            }
            profiles.push(BrowserProfile {
                browser_id: self.browser_id.clone(),
                profile_id: ProfileId::new(name.clone()),
                display_name: display_names
                    .get(&name)
                    .cloned()
                    .unwrap_or_else(|| name.clone()),
                launch_args: vec![format!("--profile-directory={name}")],
                is_incognito: false,
            });
        }
        profiles.sort_by(|left, right| left.profile_id.as_str().cmp(right.profile_id.as_str()));
        Ok(profiles)
    }

    fn build_open_intent(
        &self,
        profile: &BrowserProfile,
        url: &Url,
        disposition: OpenDisposition,
    ) -> Result<BrowserOpenIntent, BrowserError> {
        let mut args = profile.launch_args.clone();
        if disposition == OpenDisposition::NewWindow {
            args.push("--new-window".to_string());
        }
        args.push(url.as_str().to_string());

        Ok(BrowserOpenIntent {
            browser_id: self.browser_id.clone(),
            profile_id: Some(profile.profile_id.clone()),
            url: url.clone(),
            args,
            disposition,
        })
    }
}

fn read_profile_display_names(
    data_dir: &std::path::Path,
) -> std::collections::HashMap<String, String> {
    let Ok(bytes) = std::fs::read(data_dir.join("Local State")) else {
        return std::collections::HashMap::new();
    };
    let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
        return std::collections::HashMap::new();
    };

    value
        .pointer("/profile/info_cache")
        .and_then(Value::as_object)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|(directory, profile)| {
                    profile
                        .get("name")
                        .and_then(Value::as_str)
                        .filter(|name| !name.trim().is_empty())
                        .map(|name| (directory.clone(), name.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

pub struct ChromeAdapter;
pub struct EdgeAdapter;
pub struct BraveAdapter;

macro_rules! chromium_variant {
    ($name:ident, $make:expr) => {
        impl $name {
            pub fn adapter() -> ChromiumAdapter {
                $make
            }
        }
    };
}

chromium_variant!(
    ChromeAdapter,
    ChromiumAdapter::new(
        "com.google.Chrome",
        "Google Chrome",
        Some("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".to_string()),
        "Google/Chrome",
    )
);

chromium_variant!(
    EdgeAdapter,
    ChromiumAdapter::new(
        "com.microsoft.edgemac",
        "Microsoft Edge",
        Some("/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge".to_string()),
        "Microsoft Edge",
    )
);

chromium_variant!(
    BraveAdapter,
    ChromiumAdapter::new(
        "com.brave.Browser",
        "Brave Browser",
        Some("/Applications/Brave Browser.app/Contents/MacOS/Brave Browser".to_string()),
        "BraveSoftware/Brave-Browser",
    )
);

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    struct FakePlatform {
        dir: PathBuf,
    }

    impl PlatformQuery for FakePlatform {
        fn browser_data_dir(&self, _browser_id: &BrowserId) -> Option<PathBuf> {
            Some(self.dir.clone())
        }

        fn list_profile_dirs(&self, data_dir: &std::path::Path) -> Vec<PathBuf> {
            std::fs::read_dir(data_dir)
                .map(|entries| {
                    entries
                        .flatten()
                        .map(|e| e.path())
                        .filter(|p| p.is_dir())
                        .collect()
                })
                .unwrap_or_default()
        }
    }

    #[test]
    fn discovers_chromium_profile_directories() {
        let dir = std::env::temp_dir().join("lynko-chromium-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Default")).unwrap();
        std::fs::create_dir_all(dir.join("Profile 1")).unwrap();
        std::fs::create_dir_all(dir.join("System Profile")).unwrap();
        std::fs::create_dir_all(dir.join("ShaderCache")).unwrap();

        let adapter = ChromeAdapter::adapter();
        let profiles = adapter
            .discover_profiles(&FakePlatform { dir: dir.clone() })
            .unwrap();
        let ids: Vec<_> = profiles.iter().map(|p| p.profile_id.as_str()).collect();
        assert!(ids.contains(&"Default"));
        assert!(ids.contains(&"Profile 1"));
        assert!(!ids.contains(&"System Profile"));
        assert!(!ids.contains(&"ShaderCache"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn uses_local_state_profile_names_and_stable_directory_ids() {
        let dir = std::env::temp_dir().join("lynko-chromium-local-state-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Default")).unwrap();
        std::fs::create_dir_all(dir.join("Profile 2")).unwrap();
        std::fs::write(
            dir.join("Local State"),
            r#"{
              "profile": {
                "info_cache": {
                  "Default": { "name": "Personal" },
                  "Profile 2": { "name": "Work" },
                  "Profile 9": { "name": "Deleted" }
                }
              }
            }"#,
        )
        .unwrap();

        let profiles = ChromeAdapter::adapter()
            .discover_profiles(&FakePlatform { dir: dir.clone() })
            .unwrap();

        assert_eq!(profiles.len(), 2);
        assert_eq!(profiles[0].profile_id.as_str(), "Default");
        assert_eq!(profiles[0].display_name, "Personal");
        assert_eq!(profiles[1].profile_id.as_str(), "Profile 2");
        assert_eq!(profiles[1].display_name, "Work");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn builds_open_intent_with_profile_arg_and_url() {
        let adapter = ChromeAdapter::adapter();
        let profile = BrowserProfile {
            browser_id: BrowserId::new("com.google.Chrome"),
            profile_id: ProfileId::new("Work"),
            display_name: "Work".to_string(),
            launch_args: vec!["--profile-directory=Work".to_string()],
            is_incognito: false,
        };
        let intent = adapter
            .build_open_intent(
                &profile,
                &Url::parse("https://example.com/x?y=1").unwrap(),
                OpenDisposition::NewWindow,
            )
            .unwrap();
        assert_eq!(intent.args.len(), 3);
        assert_eq!(intent.args[1], "--new-window");
        assert_eq!(intent.args[2], "https://example.com/x?y=1");
    }

    #[test]
    fn capabilities_only_advertise_implemented_chromium_behaviour() {
        let capabilities = ChromeAdapter::adapter().capabilities();

        assert!(capabilities.discover_profiles);
        assert!(capabilities.create_profile_window);
        assert!(!capabilities.map_windows_to_profiles);
        assert!(capabilities.open_in_existing_profile);
        assert!(!capabilities.distinguish_incognito);
    }
}

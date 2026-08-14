use url::Url;

use platform_api::{BrowserOpenIntent, PlatformQuery};
use router_model::browser::{
    BrowserCapabilities, BrowserDescriptor, BrowserProfile, OpenDisposition,
};
use router_model::ids::{BrowserId, ProfileId};

use super::{BrowserAdapter, BrowserError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FirefoxAdapter;

impl BrowserAdapter for FirefoxAdapter {
    fn descriptor(&self) -> BrowserDescriptor {
        BrowserDescriptor {
            id: BrowserId::new("org.mozilla.firefox"),
            display_name: "Firefox".to_string(),
            executable_hint: Some("/Applications/Firefox.app/Contents/MacOS/firefox".to_string()),
        }
    }

    fn capabilities(&self) -> BrowserCapabilities {
        BrowserCapabilities {
            discover_profiles: true,
            map_windows_to_profiles: false,
            open_in_existing_profile: false,
            create_profile_window: true,
            distinguish_incognito: false,
        }
    }

    fn discover_profiles(
        &self,
        platform: &dyn PlatformQuery,
    ) -> Result<Vec<BrowserProfile>, BrowserError> {
        let data_dir = platform
            .browser_data_dir(&self.descriptor().id)
            .ok_or_else(|| BrowserError::Other("firefox data dir unknown".to_string()))?;
        let profiles_ini = data_dir.join("profiles.ini");
        let content = std::fs::read_to_string(&profiles_ini)
            .map_err(|e| BrowserError::Other(format!("cannot read profiles.ini: {e}")))?;

        Ok(parse_profiles_ini(&content))
    }

    fn build_open_intent(
        &self,
        profile: &BrowserProfile,
        url: &Url,
        disposition: OpenDisposition,
    ) -> Result<BrowserOpenIntent, BrowserError> {
        let mut args = profile.launch_args.clone();
        if disposition == OpenDisposition::NewWindow {
            args.push("-new-window".to_string());
        }
        args.push(url.as_str().to_string());

        Ok(BrowserOpenIntent {
            browser_id: self.descriptor().id,
            profile_id: Some(profile.profile_id.clone()),
            url: url.clone(),
            args,
            disposition,
        })
    }
}

fn parse_profiles_ini(content: &str) -> Vec<BrowserProfile> {
    let mut profiles = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_path: Option<String> = None;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            if let (Some(name), Some(path)) = (current_name.take(), current_path.take()) {
                profiles.push(BrowserProfile {
                    browser_id: BrowserId::new("org.mozilla.firefox"),
                    profile_id: ProfileId::new(path),
                    display_name: name.clone(),
                    launch_args: vec!["-P".to_string(), name, "-no-remote".to_string()],
                    is_incognito: false,
                });
            }
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            match key.trim() {
                "Name" => current_name = Some(value.trim().to_string()),
                "Path" => current_path = Some(value.trim().to_string()),
                _ => {}
            }
        }
    }

    if let (Some(name), Some(path)) = (current_name, current_path) {
        profiles.push(BrowserProfile {
            browser_id: BrowserId::new("org.mozilla.firefox"),
            profile_id: ProfileId::new(path),
            display_name: name.clone(),
            launch_args: vec!["-P".to_string(), name, "-no-remote".to_string()],
            is_incognito: false,
        });
    }

    profiles
}

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

        fn list_profile_dirs(&self, _data_dir: &std::path::Path) -> Vec<PathBuf> {
            Vec::new()
        }
    }

    #[test]
    fn parses_firefox_profiles_ini() {
        let dir = std::env::temp_dir().join("lynko-firefox-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("profiles.ini"),
            "[Profile0]\nName=default\nIsRelative=1\nPath=Profiles/abc.default\n\n[Profile1]\nName=work\nIsRelative=1\nPath=Profiles/xyz.work\n",
        )
        .unwrap();

        let adapter = FirefoxAdapter;
        let profiles = adapter
            .discover_profiles(&FakePlatform { dir: dir.clone() })
            .unwrap();
        assert_eq!(profiles.len(), 2);
        assert_eq!(profiles[0].profile_id.as_str(), "Profiles/abc.default");
        assert_eq!(profiles[1].display_name, "work");
        assert_eq!(profiles[1].launch_args[1], "work");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn builds_firefox_open_intent() {
        let profile = BrowserProfile {
            browser_id: BrowserId::new("org.mozilla.firefox"),
            profile_id: ProfileId::new("Profiles/xyz.work"),
            display_name: "work".to_string(),
            launch_args: vec![
                "-P".to_string(),
                "work".to_string(),
                "-no-remote".to_string(),
            ],
            is_incognito: false,
        };
        let intent = FirefoxAdapter
            .build_open_intent(
                &profile,
                &Url::parse("https://example.com").unwrap(),
                OpenDisposition::ExistingWindow,
            )
            .unwrap();
        assert_eq!(intent.args.last().unwrap(), "https://example.com/");
    }
}

use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::ids::{BrowserId, ProfileId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserDescriptor {
    pub id: BrowserId,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executable_hint: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BrowserCapabilities {
    pub discover_profiles: bool,
    pub map_windows_to_profiles: bool,
    pub open_in_existing_profile: bool,
    pub create_profile_window: bool,
    pub distinguish_incognito: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserProfile {
    pub browser_id: BrowserId,
    pub profile_id: ProfileId,
    pub display_name: String,
    #[serde(default)]
    pub launch_args: Vec<String>,
    #[serde(default)]
    pub is_incognito: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WindowRef(pub String);

#[derive(Debug, Clone, PartialEq)]
pub struct BrowserSession {
    pub browser_id: BrowserId,
    pub profile_id: ProfileId,
    pub windows: Vec<WindowRef>,
    pub is_incognito: bool,
    pub last_user_activation: Option<Instant>,
}

impl BrowserSession {
    pub fn is_available(&self) -> bool {
        !self.is_incognito && !self.windows.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdentityRef {
    pub browser_id: BrowserId,
    pub profile_id: ProfileId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenDisposition {
    ActiveWindow,
    ExistingWindow,
    NewWindow,
}

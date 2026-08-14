pub mod chromium;
pub mod firefox;

use url::Url;

use platform_api::{BrowserOpenIntent, PlatformQuery};
use router_model::browser::{
    BrowserCapabilities, BrowserDescriptor, BrowserProfile, OpenDisposition,
};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BrowserError {
    #[error("profile not found: {0}")]
    ProfileNotFound(String),
    #[error("browser does not support this capability")]
    Unsupported,
    #[error("browser error: {0}")]
    Other(String),
}

pub trait BrowserAdapter {
    fn descriptor(&self) -> BrowserDescriptor;
    fn capabilities(&self) -> BrowserCapabilities;
    fn discover_profiles(
        &self,
        platform: &dyn PlatformQuery,
    ) -> Result<Vec<BrowserProfile>, BrowserError>;
    fn build_open_intent(
        &self,
        profile: &BrowserProfile,
        url: &Url,
        disposition: OpenDisposition,
    ) -> Result<BrowserOpenIntent, BrowserError>;
}

pub use chromium::{BraveAdapter, ChromeAdapter, ChromiumAdapter, EdgeAdapter};
pub use firefox::FirefoxAdapter;

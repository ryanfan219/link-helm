use std::path::PathBuf;

use url::Url;

use router_model::browser::{BrowserSession, OpenDisposition};
use router_model::ids::{BrowserId, ProfileId};
use router_model::routing::RuntimeSnapshot;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserOpenIntent {
    pub browser_id: BrowserId,
    pub profile_id: Option<ProfileId>,
    pub url: Url,
    pub args: Vec<String>,
    pub disposition: OpenDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionError {
    UnsupportedCapability,
    ProfileUnavailable,
    WindowUnavailable,
    ExecutionUnconfirmed,
    LaunchFailed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionReceipt {
    pub confirmed: bool,
    pub error: Option<ExecutionError>,
}

impl ExecutionReceipt {
    pub fn confirmed() -> Self {
        Self {
            confirmed: true,
            error: None,
        }
    }

    pub fn unconfirmed(error: ExecutionError) -> Self {
        Self {
            confirmed: false,
            error: Some(error),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlatformError {
    #[error("platform operation failed: {0}")]
    Failed(String),
    #[error("capability is not supported on this platform")]
    Unsupported,
    #[error("not implemented yet: {0}")]
    NotImplemented(String),
}

pub trait PlatformQuery {
    fn browser_data_dir(&self, browser_id: &BrowserId) -> Option<PathBuf>;
    fn list_profile_dirs(&self, data_dir: &std::path::Path) -> Vec<PathBuf>;
}

pub trait PlatformEventSink {
    fn on_activation(&self, snapshot: RuntimeSnapshot);
    fn on_route_executed(&self, receipt: ExecutionReceipt);
}

pub trait PlatformAdapter {
    fn observe(&self, sink: Box<dyn PlatformEventSink>) -> Result<(), PlatformError>;
    fn query_sessions(&self) -> Result<Vec<BrowserSession>, PlatformError>;
    fn execute(&self, intent: BrowserOpenIntent) -> Result<ExecutionReceipt, PlatformError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receipts_carry_confirmation_semantics() {
        assert!(ExecutionReceipt::confirmed().confirmed);
        assert!(!ExecutionReceipt::unconfirmed(ExecutionError::ExecutionUnconfirmed).confirmed);
    }
}

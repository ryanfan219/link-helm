pub mod browser;
pub mod config;
pub mod domain;
pub mod ids;
pub mod routing;

pub use browser::{
    BrowserCapabilities, BrowserDescriptor, BrowserProfile, BrowserSession, IdentityRef,
    OpenDisposition, WindowRef,
};
pub use config::{
    Enforcement, FallbackScope, RouteRule, RouterConfig, RuleMatcher, RuleTarget, TargetMode,
    UnavailableAction, SCHEMA_VERSION,
};
pub use domain::{Domain, DomainError, DomainPattern};
pub use ids::{AppId, BrowserId, ProfileId, RuleId};
pub use routing::{
    CandidateKind, DecisionReason, FinalAction, OpenCandidate, RouteContext, RouteDecision,
    RuntimeSnapshot,
};

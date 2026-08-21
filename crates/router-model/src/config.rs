use serde::{Deserialize, Serialize};

use crate::domain::DomainPattern;
use crate::ids::{AppId, BrowserId, ProfileId, RuleId};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetMode {
    SpecifiedProfile,
    BrowserDefault,
    ActiveInBrowser,
    GloballyActive,
    Ask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Enforcement {
    #[default]
    Prefer,
    Force,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FallbackScope {
    #[default]
    None,
    SameBrowser,
    AnyActiveBrowser,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnavailableAction {
    #[default]
    Ask,
    OpenTargetProfile,
    CreateTargetWindow,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
}

impl<T> OneOrMany<T> {
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        match self {
            Self::One(value) => std::slice::from_ref(value).iter(),
            Self::Many(values) => values.iter(),
        }
    }

    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Many(values) if values.is_empty())
    }
}

impl<T> From<T> for OneOrMany<T> {
    fn from(value: T) -> Self {
        Self::One(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleMatcher {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_app: Option<OneOrMany<AppId>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<OneOrMany<DomainPattern>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleTarget {
    pub mode: TargetMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser_id: Option<BrowserId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<ProfileId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteRule {
    pub id: RuleId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub order: u32,
    pub matcher: RuleMatcher,
    pub target: RuleTarget,
    #[serde(default)]
    pub enforcement: Enforcement,
    #[serde(default)]
    pub fallback_scope: FallbackScope,
    #[serde(default)]
    pub unavailable_action: UnavailableAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouterConfig {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub rules: Vec<RouteRule>,
}

fn default_true() -> bool {
    true
}

fn default_schema_version() -> u32 {
    SCHEMA_VERSION
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            rules: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    #[error("rule {0}: name must not be blank")]
    BlankName(String),
    #[error("rule {0}: Ask must not set enforcement, fallback_scope or unavailable_action")]
    AskHasExtraTargeting(String),
    #[error("rule {0}: Force cannot use a fallback to another identity")]
    ForceCannotFallback(String),
    #[error("rule {0}: SpecifiedProfile requires both browser_id and profile_id")]
    SpecifiedProfileRequiresIdentity(String),
    #[error("rule {0}: BrowserDefault requires browser_id")]
    BrowserDefaultRequiresBrowser(String),
    #[error("rule {0}: ActiveInBrowser requires browser_id")]
    ActiveInBrowserRequiresBrowser(String),
}

impl RouteRule {
    pub fn validate(&self) -> Result<(), ConfigError> {
        let id = self.id.as_str().to_string();
        if self
            .name
            .as_ref()
            .is_some_and(|name| name.trim().is_empty())
        {
            return Err(ConfigError::BlankName(id));
        }
        match self.target.mode {
            TargetMode::Ask => {
                if self.enforcement != Enforcement::Prefer
                    || self.fallback_scope != FallbackScope::None
                    || self.unavailable_action != UnavailableAction::Ask
                {
                    return Err(ConfigError::AskHasExtraTargeting(id));
                }
            }
            TargetMode::SpecifiedProfile => {
                if self.target.browser_id.is_none() || self.target.profile_id.is_none() {
                    return Err(ConfigError::SpecifiedProfileRequiresIdentity(id));
                }
                if self.enforcement == Enforcement::Force
                    && self.fallback_scope != FallbackScope::None
                {
                    return Err(ConfigError::ForceCannotFallback(id));
                }
            }
            TargetMode::BrowserDefault => {
                if self.target.browser_id.is_none() {
                    return Err(ConfigError::BrowserDefaultRequiresBrowser(id));
                }
            }
            TargetMode::ActiveInBrowser => {
                if self.target.browser_id.is_none() {
                    return Err(ConfigError::ActiveInBrowserRequiresBrowser(id));
                }
            }
            TargetMode::GloballyActive => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(mode: TargetMode, enforcement: Enforcement) -> RouteRule {
        RouteRule {
            id: RuleId::new("r1"),
            name: None,
            enabled: true,
            order: 0,
            matcher: RuleMatcher {
                source_app: None,
                domain: None,
            },
            target: RuleTarget {
                mode,
                browser_id: Some(BrowserId::new("browser")),
                profile_id: Some(ProfileId::new("profile")),
            },
            enforcement,
            fallback_scope: FallbackScope::None,
            unavailable_action: UnavailableAction::Ask,
        }
    }

    #[test]
    fn ask_must_not_carry_targeting_policy() {
        let mut r = rule(TargetMode::Ask, Enforcement::Prefer);
        r.enforcement = Enforcement::Force;
        assert!(r.validate().is_err());
    }

    #[test]
    fn force_cannot_fallback_to_other_identity() {
        let mut r = rule(TargetMode::SpecifiedProfile, Enforcement::Force);
        r.fallback_scope = FallbackScope::SameBrowser;
        assert!(matches!(
            r.validate(),
            Err(ConfigError::ForceCannotFallback(_))
        ));
    }

    #[test]
    fn specified_profile_requires_both_ids() {
        let mut r = rule(TargetMode::SpecifiedProfile, Enforcement::Prefer);
        r.target.profile_id = None;
        assert!(matches!(
            r.validate(),
            Err(ConfigError::SpecifiedProfileRequiresIdentity(_))
        ));
    }

    #[test]
    fn global_matcher_example_round_trips() {
        let json = r#"{
            "id": "r-default",
            "matcher": {},
            "target": { "mode": "globally_active" },
            "enforcement": "prefer",
            "fallback_scope": "any_active_browser",
            "unavailable_action": "ask"
        }"#;
        let parsed: RouteRule = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.target.mode, TargetMode::GloballyActive);
        assert!(parsed.enabled);
        assert_eq!(parsed.order, 0);
    }

    #[test]
    fn legacy_rule_without_name_remains_valid() {
        let json = r#"{
            "id": "legacy-rule",
            "matcher": {},
            "target": { "mode": "ask" }
        }"#;

        let parsed: RouteRule = serde_json::from_str(json).unwrap();

        assert_eq!(parsed.name, None);
        assert!(parsed.validate().is_ok());
    }

    #[test]
    fn matcher_accepts_multiple_source_apps_and_domains() {
        let json = r#"{
            "id": "multi-match",
            "matcher": {
                "source_app": ["com.apple.mail", "com.alibaba.DingTalkMac"],
                "domain": ["example.com", "*.internal.example"]
            },
            "target": { "mode": "ask" }
        }"#;

        let parsed: RouteRule = serde_json::from_str(json).unwrap();
        let serialized = serde_json::to_value(parsed).unwrap();

        assert_eq!(
            serialized["matcher"]["source_app"],
            serde_json::json!(["com.apple.mail", "com.alibaba.DingTalkMac"])
        );
        assert_eq!(
            serialized["matcher"]["domain"],
            serde_json::json!(["example.com", "*.internal.example"])
        );
    }

    #[test]
    fn named_rule_round_trips() {
        let mut named = rule(TargetMode::Ask, Enforcement::Prefer);
        named.name = Some("Open work links".to_string());

        let json = serde_json::to_string(&named).unwrap();
        let parsed: RouteRule = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.name.as_deref(), Some("Open work links"));
    }

    #[test]
    fn explicitly_blank_rule_name_is_rejected() {
        let mut named = rule(TargetMode::Ask, Enforcement::Prefer);
        named.name = Some("   ".to_string());

        assert!(matches!(named.validate(), Err(ConfigError::BlankName(_))));
    }
}

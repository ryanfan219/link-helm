use url::Url;

use router_model::config::{
    Enforcement, FallbackScope, RouteRule, RouterConfig, TargetMode, UnavailableAction,
};
use router_model::ids::{BrowserId, ProfileId, RuleId};
use router_model::routing::{
    CandidateKind, DecisionReason, FinalAction, OpenCandidate, RouteContext, RouteDecision,
    RuntimeSnapshot,
};
use router_model::IdentityRef;
use router_model::{BrowserSession, Domain, RuleMatcher};

pub trait RouteEngine {
    fn decide(
        &self,
        context: &RouteContext,
        config: &RouterConfig,
        runtime: &RuntimeSnapshot,
    ) -> RouteDecision;
}

#[derive(Default)]
pub struct DefaultRouteEngine;

impl DefaultRouteEngine {
    pub fn new() -> Self {
        Self
    }
}

impl RouteEngine for DefaultRouteEngine {
    fn decide(
        &self,
        context: &RouteContext,
        config: &RouterConfig,
        runtime: &RuntimeSnapshot,
    ) -> RouteDecision {
        if context.paused {
            return RouteDecision::ask(DecisionReason::Paused);
        }
        if context.ask_next {
            return RouteDecision::ask(DecisionReason::AskNext);
        }

        let scheme = context.url.scheme().to_ascii_lowercase();
        if scheme != "http" && scheme != "https" {
            return RouteDecision::fail(DecisionReason::UnsupportedScheme);
        }

        let Some(host) = normalize_host(&context.url) else {
            return RouteDecision::fail(DecisionReason::InvalidUrl);
        };

        if let Some(rule) = find_matching_rule(config, context.source_app.as_str(), &host) {
            return decide_from_rule(rule, runtime);
        }

        if let Some(identity) = &runtime.source_identity {
            return decide_builtin_same_browser(identity, runtime);
        }

        RouteDecision::ask(DecisionReason::NoMatchingRule)
    }
}

fn normalize_host(url: &Url) -> Option<String> {
    let host = url.host_str()?;
    Domain::normalize(host).ok().map(|d| d.into_inner())
}

fn find_matching_rule<'a>(
    config: &'a RouterConfig,
    source_app: &str,
    host: &str,
) -> Option<&'a RouteRule> {
    let mut candidates: Vec<&'a RouteRule> = config
        .rules
        .iter()
        .filter(|rule| rule.enabled && rule_matches(rule, source_app, host))
        .collect();

    candidates.sort_by_key(|rule| (matcher_tier(&rule.matcher), rule.order, rule.id.as_str()));
    candidates.into_iter().next()
}

fn matcher_tier(matcher: &RuleMatcher) -> u8 {
    let has_source_app = matcher
        .source_app
        .as_ref()
        .is_some_and(|values| !values.is_empty());
    let has_domain = matcher
        .domain
        .as_ref()
        .is_some_and(|values| !values.is_empty());
    match (has_source_app, has_domain) {
        (true, true) => 0,
        (false, true) => 1,
        (true, false) => 2,
        (false, false) => 3,
    }
}

fn rule_matches(rule: &RouteRule, source_app: &str, host: &str) -> bool {
    if let Some(apps) = &rule.matcher.source_app {
        if !apps.is_empty() && !apps.iter().any(|app| app.as_str() == source_app) {
            return false;
        }
    }
    if let Some(patterns) = &rule.matcher.domain {
        if !patterns.is_empty() && !patterns.iter().any(|pattern| pattern.matches(host)) {
            return false;
        }
    }
    true
}

fn decide_from_rule(rule: &RouteRule, runtime: &RuntimeSnapshot) -> RouteDecision {
    let matched = Some(rule.id.clone());

    match rule.target.mode {
        TargetMode::Ask => RouteDecision {
            matched_rule_id: matched,
            candidates: Vec::new(),
            unavailable_action: UnavailableAction::Ask,
            final_action: FinalAction::Ask,
            reason: DecisionReason::MatchedRule(rule.id.clone()),
        },
        TargetMode::SpecifiedProfile => {
            let browser_id = rule.target.browser_id.clone().expect("validated rule");
            let profile_id = rule.target.profile_id.clone().expect("validated rule");

            if find_session(runtime, Some(&browser_id), Some(&profile_id)).is_some() {
                let candidate = specified_candidate(browser_id, Some(profile_id));
                return open(matched, vec![candidate], rule);
            }

            match rule.enforcement {
                Enforcement::Force => unavailable(
                    rule,
                    matched,
                    Some(new_window_candidate(browser_id, profile_id)),
                ),
                Enforcement::Prefer => {
                    if let Some(fallback) = resolve_fallback(rule, runtime) {
                        return open(matched, vec![fallback], rule);
                    }
                    unavailable(
                        rule,
                        matched,
                        Some(new_window_candidate(browser_id, profile_id)),
                    )
                }
            }
        }
        TargetMode::BrowserDefault => open(
            matched,
            vec![OpenCandidate {
                browser_id: rule.target.browser_id.clone().expect("validated rule"),
                profile_id: None,
                kind: CandidateKind::BrowserDefault,
            }],
            rule,
        ),
        TargetMode::ActiveInBrowser => {
            let browser_id = rule.target.browser_id.clone().expect("validated rule");
            if let Some(session) = most_recent_available_session(runtime, Some(&browser_id)) {
                let candidate = OpenCandidate {
                    browser_id: session.browser_id.clone(),
                    profile_id: Some(session.profile_id.clone()),
                    kind: CandidateKind::ActiveInBrowser,
                };
                return open(matched, vec![candidate], rule);
            }
            unavailable(rule, matched, None)
        }
        TargetMode::GloballyActive => {
            if let Some(session) = most_recent_available_session(runtime, None) {
                let candidate = OpenCandidate {
                    browser_id: session.browser_id.clone(),
                    profile_id: Some(session.profile_id.clone()),
                    kind: CandidateKind::GloballyActive,
                };
                return open(matched, vec![candidate], rule);
            }
            unavailable(rule, matched, None)
        }
    }
}

fn decide_builtin_same_browser(identity: &IdentityRef, runtime: &RuntimeSnapshot) -> RouteDecision {
    if let Some(session) = find_session(
        runtime,
        Some(&identity.browser_id),
        Some(&identity.profile_id),
    ) {
        let candidate =
            specified_candidate(session.browser_id.clone(), Some(session.profile_id.clone()));
        RouteDecision {
            matched_rule_id: None,
            candidates: vec![candidate],
            unavailable_action: UnavailableAction::Ask,
            final_action: FinalAction::Open,
            reason: DecisionReason::BuiltInSameBrowser,
        }
    } else {
        RouteDecision::ask(DecisionReason::BuiltInSameBrowser)
    }
}

fn resolve_fallback(rule: &RouteRule, runtime: &RuntimeSnapshot) -> Option<OpenCandidate> {
    match rule.fallback_scope {
        FallbackScope::None => None,
        FallbackScope::SameBrowser => {
            let browser_id = rule.target.browser_id.as_ref()?;
            most_recent_available_session(runtime, Some(browser_id)).map(|session| OpenCandidate {
                browser_id: session.browser_id.clone(),
                profile_id: Some(session.profile_id.clone()),
                kind: CandidateKind::ActiveInBrowser,
            })
        }
        FallbackScope::AnyActiveBrowser => {
            most_recent_available_session(runtime, None).map(|session| OpenCandidate {
                browser_id: session.browser_id.clone(),
                profile_id: Some(session.profile_id.clone()),
                kind: CandidateKind::GloballyActive,
            })
        }
    }
}

fn find_session<'a>(
    runtime: &'a RuntimeSnapshot,
    browser_id: Option<&BrowserId>,
    profile_id: Option<&ProfileId>,
) -> Option<&'a BrowserSession> {
    runtime.sessions.iter().find(|session| {
        session.is_available()
            && browser_id.is_none_or(|b| session.browser_id == *b)
            && profile_id.is_none_or(|p| session.profile_id == *p)
    })
}

fn most_recent_available_session<'a>(
    runtime: &'a RuntimeSnapshot,
    browser_id: Option<&BrowserId>,
) -> Option<&'a BrowserSession> {
    runtime
        .sessions
        .iter()
        .filter(|session| session.is_available())
        .filter(|session| browser_id.is_none_or(|b| session.browser_id == *b))
        .filter(|session| session.last_user_activation.is_some())
        .max_by_key(|session| session.last_user_activation.expect("filtered"))
}

fn specified_candidate(browser_id: BrowserId, profile_id: Option<ProfileId>) -> OpenCandidate {
    OpenCandidate {
        browser_id,
        profile_id,
        kind: CandidateKind::SpecifiedProfile,
    }
}

fn new_window_candidate(browser_id: BrowserId, profile_id: ProfileId) -> OpenCandidate {
    OpenCandidate {
        browser_id,
        profile_id: Some(profile_id),
        kind: CandidateKind::NewTargetWindow,
    }
}

fn open(
    matched: Option<RuleId>,
    candidates: Vec<OpenCandidate>,
    rule: &RouteRule,
) -> RouteDecision {
    RouteDecision {
        matched_rule_id: matched,
        candidates,
        unavailable_action: rule.unavailable_action,
        final_action: FinalAction::Open,
        reason: DecisionReason::MatchedRule(rule.id.clone()),
    }
}

fn unavailable(
    rule: &RouteRule,
    matched: Option<RuleId>,
    create: Option<OpenCandidate>,
) -> RouteDecision {
    match rule.unavailable_action {
        UnavailableAction::OpenTargetProfile => {
            if let Some(mut candidate) = create {
                candidate.kind = CandidateKind::SpecifiedProfile;
                RouteDecision {
                    matched_rule_id: matched,
                    candidates: vec![candidate],
                    unavailable_action: rule.unavailable_action,
                    final_action: FinalAction::Open,
                    reason: DecisionReason::MatchedRule(rule.id.clone()),
                }
            } else {
                RouteDecision {
                    matched_rule_id: matched,
                    candidates: Vec::new(),
                    unavailable_action: rule.unavailable_action,
                    final_action: FinalAction::Ask,
                    reason: DecisionReason::MatchedRule(rule.id.clone()),
                }
            }
        }
        UnavailableAction::CreateTargetWindow => {
            if let Some(candidate) = create {
                RouteDecision {
                    matched_rule_id: matched,
                    candidates: vec![candidate],
                    unavailable_action: rule.unavailable_action,
                    final_action: FinalAction::Open,
                    reason: DecisionReason::MatchedRule(rule.id.clone()),
                }
            } else {
                RouteDecision {
                    matched_rule_id: matched,
                    candidates: Vec::new(),
                    unavailable_action: rule.unavailable_action,
                    final_action: FinalAction::Ask,
                    reason: DecisionReason::MatchedRule(rule.id.clone()),
                }
            }
        }
        UnavailableAction::Ask => RouteDecision {
            matched_rule_id: matched,
            candidates: Vec::new(),
            unavailable_action: rule.unavailable_action,
            final_action: FinalAction::Ask,
            reason: DecisionReason::MatchedRule(rule.id.clone()),
        },
        UnavailableAction::Fail => RouteDecision {
            matched_rule_id: matched,
            candidates: Vec::new(),
            unavailable_action: rule.unavailable_action,
            final_action: FinalAction::Fail,
            reason: DecisionReason::MatchedRule(rule.id.clone()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    use router_model::browser::{BrowserSession, WindowRef};
    use router_model::config::{
        Enforcement, FallbackScope, OneOrMany, RouteRule, RuleMatcher, RuleTarget, TargetMode,
        UnavailableAction,
    };
    use router_model::ids::{AppId, BrowserId, ProfileId, RuleId};
    use router_model::routing::{RouteContext, RuntimeSnapshot};
    use router_model::{DomainPattern, IdentityRef};

    fn ctx(url: &str) -> RouteContext {
        RouteContext {
            url: Url::parse(url).unwrap(),
            source_app: AppId::new("com.example.SourceApp"),
            event_id: "evt-1".to_string(),
            ask_next: false,
            paused: false,
        }
    }

    fn session(
        browser: &str,
        profile: &str,
        windows: usize,
        incognito: bool,
        last_active: Option<Instant>,
    ) -> BrowserSession {
        BrowserSession {
            browser_id: BrowserId::new(browser),
            profile_id: ProfileId::new(profile),
            windows: (0..windows).map(|i| WindowRef(format!("w{i}"))).collect(),
            is_incognito: incognito,
            last_user_activation: last_active,
        }
    }

    fn rule(
        id: &str,
        matcher: RuleMatcher,
        target: RuleTarget,
        enforcement: Enforcement,
        fallback: FallbackScope,
        unavailable: UnavailableAction,
    ) -> RouteRule {
        RouteRule {
            id: RuleId::new(id),
            name: None,
            enabled: true,
            order: 0,
            matcher,
            target,
            enforcement,
            fallback_scope: fallback,
            unavailable_action: unavailable,
        }
    }

    fn globals(config: RouterConfig, runtime: RuntimeSnapshot) -> RouteDecision {
        DefaultRouteEngine::new().decide(&ctx("https://example.com/a"), &config, &runtime)
    }

    #[test]
    fn multiple_match_values_use_or_within_each_field() {
        let multi_rule = rule(
            "multi",
            RuleMatcher {
                source_app: Some(OneOrMany::Many(vec![
                    AppId::new("com.example.Other"),
                    AppId::new("com.example.SourceApp"),
                ])),
                domain: Some(OneOrMany::Many(vec![
                    DomainPattern::parse("other.example").unwrap(),
                    DomainPattern::parse("example.com").unwrap(),
                ])),
            },
            RuleTarget {
                mode: TargetMode::Ask,
                browser_id: None,
                profile_id: None,
            },
            Enforcement::Prefer,
            FallbackScope::None,
            UnavailableAction::Ask,
        );
        assert!(rule_matches(
            &multi_rule,
            "com.example.SourceApp",
            "example.com"
        ));
        assert!(!rule_matches(
            &multi_rule,
            "com.example.Unknown",
            "example.com"
        ));
        assert!(!rule_matches(
            &multi_rule,
            "com.example.SourceApp",
            "unknown.example"
        ));

        let config = RouterConfig {
            schema_version: router_model::SCHEMA_VERSION,
            rules: vec![multi_rule],
        };

        let decision = globals(
            config,
            RuntimeSnapshot {
                sessions: Vec::new(),
                source_identity: None,
            },
        );

        assert_eq!(decision.matched_rule_id, Some(RuleId::new("multi")));
    }

    #[test]
    fn picks_highest_priority_tier() {
        let config = RouterConfig {
            schema_version: router_model::SCHEMA_VERSION,
            rules: vec![
                rule(
                    "global",
                    RuleMatcher {
                        source_app: None,
                        domain: None,
                    },
                    RuleTarget {
                        mode: TargetMode::GloballyActive,
                        browser_id: None,
                        profile_id: None,
                    },
                    Enforcement::Prefer,
                    FallbackScope::None,
                    UnavailableAction::Ask,
                ),
                rule(
                    "domain",
                    RuleMatcher {
                        source_app: None,
                        domain: Some(DomainPattern::parse("example.com").unwrap().into()),
                    },
                    RuleTarget {
                        mode: TargetMode::Ask,
                        browser_id: None,
                        profile_id: None,
                    },
                    Enforcement::Prefer,
                    FallbackScope::None,
                    UnavailableAction::Ask,
                ),
            ],
        };
        let d = globals(
            config,
            RuntimeSnapshot {
                sessions: vec![],
                source_identity: None,
            },
        );
        assert_eq!(d.reason, DecisionReason::MatchedRule(RuleId::new("domain")));
    }

    #[test]
    fn same_tier_honours_order() {
        let config = RouterConfig {
            schema_version: router_model::SCHEMA_VERSION,
            rules: vec![
                rule(
                    "second",
                    RuleMatcher {
                        source_app: None,
                        domain: Some(DomainPattern::parse("example.com").unwrap().into()),
                    },
                    RuleTarget {
                        mode: TargetMode::Ask,
                        browser_id: None,
                        profile_id: None,
                    },
                    Enforcement::Prefer,
                    FallbackScope::None,
                    UnavailableAction::Ask,
                ),
                rule(
                    "first",
                    RuleMatcher {
                        source_app: None,
                        domain: Some(DomainPattern::parse("example.com").unwrap().into()),
                    },
                    RuleTarget {
                        mode: TargetMode::Ask,
                        browser_id: None,
                        profile_id: None,
                    },
                    Enforcement::Prefer,
                    FallbackScope::None,
                    UnavailableAction::Ask,
                ),
            ],
        };
        let mut c = config.clone();
        c.rules[0].order = 2;
        c.rules[1].order = 1;
        let d = globals(
            c,
            RuntimeSnapshot {
                sessions: vec![],
                source_identity: None,
            },
        );
        assert_eq!(d.reason, DecisionReason::MatchedRule(RuleId::new("first")));
    }

    #[test]
    fn globally_active_uses_most_recent_available_session() {
        let early = Instant::now();
        let late = early + Duration::from_secs(10);
        let runtime = RuntimeSnapshot {
            sessions: vec![
                session("chrome", "work", 1, false, Some(early)),
                session("firefox", "home", 1, false, Some(late)),
            ],
            source_identity: None,
        };
        let config = RouterConfig {
            schema_version: router_model::SCHEMA_VERSION,
            rules: vec![rule(
                "g",
                RuleMatcher {
                    source_app: None,
                    domain: None,
                },
                RuleTarget {
                    mode: TargetMode::GloballyActive,
                    browser_id: None,
                    profile_id: None,
                },
                Enforcement::Prefer,
                FallbackScope::None,
                UnavailableAction::Ask,
            )],
        };
        let d = globals(config, runtime);
        assert_eq!(d.final_action, FinalAction::Open);
        let c = d.primary().unwrap();
        assert_eq!(c.browser_id.as_str(), "firefox");
        assert_eq!(c.profile_id.as_ref().unwrap().as_str(), "home");
    }

    #[test]
    fn incognito_and_windowless_sessions_are_not_available() {
        let now = Instant::now();
        let runtime = RuntimeSnapshot {
            sessions: vec![
                session("chrome", "incog", 1, true, Some(now)),
                session("chrome", "empty", 0, false, Some(now)),
            ],
            source_identity: None,
        };
        let config = RouterConfig {
            schema_version: router_model::SCHEMA_VERSION,
            rules: vec![rule(
                "g",
                RuleMatcher {
                    source_app: None,
                    domain: None,
                },
                RuleTarget {
                    mode: TargetMode::GloballyActive,
                    browser_id: None,
                    profile_id: None,
                },
                Enforcement::Prefer,
                FallbackScope::None,
                UnavailableAction::Ask,
            )],
        };
        let d = globals(config, runtime);
        assert_eq!(d.final_action, FinalAction::Ask);
    }

    #[test]
    fn unavailable_ask_preserves_the_matched_rule_id() {
        let config = RouterConfig {
            schema_version: router_model::SCHEMA_VERSION,
            rules: vec![rule(
                "ask-when-unavailable",
                RuleMatcher {
                    source_app: None,
                    domain: None,
                },
                RuleTarget {
                    mode: TargetMode::GloballyActive,
                    browser_id: None,
                    profile_id: None,
                },
                Enforcement::Prefer,
                FallbackScope::None,
                UnavailableAction::Ask,
            )],
        };

        let decision = globals(
            config,
            RuntimeSnapshot {
                sessions: vec![],
                source_identity: None,
            },
        );

        assert_eq!(
            decision.matched_rule_id,
            Some(RuleId::new("ask-when-unavailable"))
        );
    }

    #[test]
    fn unavailable_fail_preserves_the_matched_rule_id() {
        let config = RouterConfig {
            schema_version: router_model::SCHEMA_VERSION,
            rules: vec![rule(
                "fail-when-unavailable",
                RuleMatcher {
                    source_app: None,
                    domain: None,
                },
                RuleTarget {
                    mode: TargetMode::GloballyActive,
                    browser_id: None,
                    profile_id: None,
                },
                Enforcement::Prefer,
                FallbackScope::None,
                UnavailableAction::Fail,
            )],
        };

        let decision = globals(
            config,
            RuntimeSnapshot {
                sessions: vec![],
                source_identity: None,
            },
        );

        assert_eq!(decision.final_action, FinalAction::Fail);
        assert_eq!(
            decision.matched_rule_id,
            Some(RuleId::new("fail-when-unavailable"))
        );
    }

    #[test]
    fn force_specified_profile_does_not_fall_back() {
        let config = RouterConfig {
            schema_version: router_model::SCHEMA_VERSION,
            rules: vec![rule(
                "force",
                RuleMatcher {
                    source_app: None,
                    domain: None,
                },
                RuleTarget {
                    mode: TargetMode::SpecifiedProfile,
                    browser_id: Some(BrowserId::new("chrome")),
                    profile_id: Some(ProfileId::new("work")),
                },
                Enforcement::Force,
                FallbackScope::None,
                UnavailableAction::CreateTargetWindow,
            )],
        };
        // another chrome profile is active, but Force must not switch to it
        let runtime = RuntimeSnapshot {
            sessions: vec![session("chrome", "other", 1, false, Some(Instant::now()))],
            source_identity: None,
        };
        let d = globals(config, runtime);
        let c = d.primary().unwrap();
        assert_eq!(c.kind, CandidateKind::NewTargetWindow);
        assert_eq!(c.profile_id.as_ref().unwrap().as_str(), "work");
    }

    #[test]
    fn unavailable_target_profile_can_be_opened_without_forcing_a_new_window() {
        let config = RouterConfig {
            schema_version: router_model::SCHEMA_VERSION,
            rules: vec![rule(
                "reuse-profile",
                RuleMatcher {
                    source_app: None,
                    domain: None,
                },
                RuleTarget {
                    mode: TargetMode::SpecifiedProfile,
                    browser_id: Some(BrowserId::new("chrome")),
                    profile_id: Some(ProfileId::new("work")),
                },
                Enforcement::Prefer,
                FallbackScope::None,
                UnavailableAction::OpenTargetProfile,
            )],
        };

        let decision = globals(
            config,
            RuntimeSnapshot {
                sessions: vec![],
                source_identity: None,
            },
        );

        assert_eq!(decision.final_action, FinalAction::Open);
        assert_eq!(
            decision.primary().unwrap().kind,
            CandidateKind::SpecifiedProfile
        );
    }

    #[test]
    fn prefer_falls_back_same_browser_then_any_active() {
        let now = Instant::now();
        let runtime = RuntimeSnapshot {
            sessions: vec![
                session("chrome", "other", 1, false, Some(now)),
                session(
                    "firefox",
                    "home",
                    1,
                    false,
                    Some(now - Duration::from_secs(5)),
                ),
            ],
            source_identity: None,
        };
        let config = RouterConfig {
            schema_version: router_model::SCHEMA_VERSION,
            rules: vec![rule(
                "prefer",
                RuleMatcher {
                    source_app: None,
                    domain: None,
                },
                RuleTarget {
                    mode: TargetMode::SpecifiedProfile,
                    browser_id: Some(BrowserId::new("chrome")),
                    profile_id: Some(ProfileId::new("work")),
                },
                Enforcement::Prefer,
                FallbackScope::SameBrowser,
                UnavailableAction::Ask,
            )],
        };
        let d = globals(config, runtime);
        let c = d.primary().unwrap();
        assert_eq!(c.browser_id.as_str(), "chrome");
        assert_eq!(c.profile_id.as_ref().unwrap().as_str(), "other");
    }

    #[test]
    fn builtin_same_browser_behaviour_uses_source_identity() {
        let config = RouterConfig::default();
        let runtime = RuntimeSnapshot {
            sessions: vec![session("chrome", "work", 1, false, Some(Instant::now()))],
            source_identity: Some(IdentityRef {
                browser_id: BrowserId::new("chrome"),
                profile_id: ProfileId::new("work"),
            }),
        };
        let d = globals(config, runtime);
        assert_eq!(d.reason, DecisionReason::BuiltInSameBrowser);
        assert_eq!(
            d.primary().unwrap().profile_id.as_ref().unwrap().as_str(),
            "work"
        );
    }

    #[test]
    fn global_default_rule_overrides_builtin() {
        let config = RouterConfig {
            schema_version: router_model::SCHEMA_VERSION,
            rules: vec![rule(
                "default",
                RuleMatcher {
                    source_app: None,
                    domain: None,
                },
                RuleTarget {
                    mode: TargetMode::Ask,
                    browser_id: None,
                    profile_id: None,
                },
                Enforcement::Prefer,
                FallbackScope::None,
                UnavailableAction::Ask,
            )],
        };
        let runtime = RuntimeSnapshot {
            sessions: vec![session("chrome", "work", 1, false, Some(Instant::now()))],
            source_identity: Some(IdentityRef {
                browser_id: BrowserId::new("chrome"),
                profile_id: ProfileId::new("work"),
            }),
        };
        let d = globals(config, runtime);
        assert_eq!(
            d.reason,
            DecisionReason::MatchedRule(RuleId::new("default"))
        );
        assert_eq!(d.final_action, FinalAction::Ask);
    }

    #[test]
    fn non_http_scheme_fails() {
        let config = RouterConfig::default();
        let runtime = RuntimeSnapshot {
            sessions: vec![],
            source_identity: None,
        };
        let d =
            DefaultRouteEngine::new().decide(&ctx("mailto:user@example.com"), &config, &runtime);
        assert_eq!(d.final_action, FinalAction::Fail);
        assert_eq!(d.reason, DecisionReason::UnsupportedScheme);
    }

    #[test]
    fn paused_and_ask_next_return_ask() {
        let config = RouterConfig::default();
        let runtime = RuntimeSnapshot {
            sessions: vec![],
            source_identity: None,
        };
        let mut c = ctx("https://example.com");
        c.paused = true;
        assert_eq!(
            DefaultRouteEngine::new()
                .decide(&c, &config, &runtime)
                .reason,
            DecisionReason::Paused
        );
        c.paused = false;
        c.ask_next = true;
        assert_eq!(
            DefaultRouteEngine::new()
                .decide(&c, &config, &runtime)
                .reason,
            DecisionReason::AskNext
        );
    }
}

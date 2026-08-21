use std::collections::{HashMap, HashSet};
use std::time::Instant;

use browser_adapters::{BraveAdapter, BrowserAdapter, ChromeAdapter, EdgeAdapter, FirefoxAdapter};
use config_store::{ConfigStore, ConfigStoreError};
use platform_api::PlatformAdapter;
use router_core::{DefaultRouteEngine, RouteEngine};
use router_model::browser::OpenDisposition;
use router_model::browser::{
    BrowserCapabilities, BrowserDescriptor, BrowserProfile, BrowserSession, WindowRef,
};
use router_model::config::{RouterConfig, SCHEMA_VERSION};
use router_model::ids::{AppId, BrowserId, ProfileId};
use router_model::routing::{
    CandidateKind, FinalAction, RouteContext, RouteDecision, RuntimeSnapshot,
};
use serde::Serialize;

use crate::diagnostics::DiagnosticLog;
use crate::platform::DesktopPlatformAdapter;

#[derive(Debug, Clone, Serialize)]
pub struct BrowserInstallation {
    pub descriptor: BrowserDescriptor,
    pub capabilities: BrowserCapabilities,
    pub installed: bool,
    pub profiles: Vec<BrowserProfile>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PendingRoute {
    pub id: u64,
    pub domain: String,
    pub source_app: String,
    pub rule_id: Option<String>,
    #[serde(skip)]
    url: url::Url,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteDisposition {
    Opened,
    Ask,
    Failed,
}

fn candidate_disposition(candidate: &router_model::routing::OpenCandidate) -> OpenDisposition {
    match candidate.kind {
        CandidateKind::ActiveInBrowser | CandidateKind::GloballyActive => {
            OpenDisposition::ActiveWindow
        }
        CandidateKind::NewTargetWindow => OpenDisposition::NewWindow,
        CandidateKind::SpecifiedProfile | CandidateKind::BrowserDefault => {
            OpenDisposition::ExistingWindow
        }
    }
}

fn candidate_identity_id(candidate: &router_model::routing::OpenCandidate) -> Option<String> {
    candidate
        .profile_id
        .as_ref()
        .map(|profile_id| format!("{}/{}", candidate.browser_id, profile_id))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConfigImportPreview {
    pub schema_version: u32,
    pub rule_count: usize,
    pub enabled_rule_count: usize,
}

pub struct DesktopService {
    pub config: RouterConfig,
    pub config_error: Option<String>,
    pub browsers: Vec<BrowserInstallation>,
    pub paused: bool,
    pub ask_next: bool,
    pub diagnostics: DiagnosticLog,
    pending_routes: Vec<PendingRoute>,
    next_event_id: u64,
    store: ConfigStore,
    engine: DefaultRouteEngine,
    platform: DesktopPlatformAdapter,
    recent_identities: HashMap<BrowserId, (ProfileId, Instant)>,
    available_profiles: HashMap<BrowserId, HashSet<ProfileId>>,
}

impl DesktopService {
    pub fn new(store: ConfigStore) -> Self {
        let (config, config_error) = match store.load() {
            Ok(config) => (config, None),
            Err(ConfigStoreError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                (RouterConfig::default(), None)
            }
            Err(error) => (RouterConfig::default(), Some(error.to_string())),
        };
        let diagnostics = DiagnosticLog::open(store.path().with_file_name("diagnostics.json"));
        Self {
            config,
            config_error,
            browsers: Vec::new(),
            paused: false,
            ask_next: false,
            diagnostics,
            pending_routes: Vec::new(),
            next_event_id: 1,
            store,
            engine: DefaultRouteEngine::new(),
            platform: DesktopPlatformAdapter::new(),
            recent_identities: HashMap::new(),
            available_profiles: HashMap::new(),
        }
    }

    pub fn save_config(&mut self, mut config: RouterConfig) -> Result<(), String> {
        if config.schema_version > SCHEMA_VERSION {
            return Err(format!(
                "unsupported schema version: {}",
                config.schema_version
            ));
        }
        for rule in &mut config.rules {
            if let Some(name) = &mut rule.name {
                *name = name.trim().to_string();
            }
            rule.validate().map_err(|error| error.to_string())?;
        }
        self.store
            .save(&config)
            .map_err(|error| error.to_string())?;
        self.config = config;
        self.config_error = None;
        Ok(())
    }

    pub fn export_config(&self) -> Result<String, String> {
        serde_json::to_string_pretty(&self.config).map_err(|error| error.to_string())
    }

    pub fn preview_import_config(&self, json: &str) -> Result<ConfigImportPreview, String> {
        let config = self.parse_import_config(json)?;
        Ok(ConfigImportPreview {
            schema_version: config.schema_version,
            rule_count: config.rules.len(),
            enabled_rule_count: config.rules.iter().filter(|rule| rule.enabled).count(),
        })
    }

    pub fn import_config(&mut self, json: &str) -> Result<(), String> {
        let config = self.parse_import_config(json)?;
        self.save_config(config)
    }

    fn parse_import_config(&self, json: &str) -> Result<RouterConfig, String> {
        let config: RouterConfig =
            serde_json::from_str(json).map_err(|error| format!("invalid config JSON: {error}"))?;
        if config.schema_version > SCHEMA_VERSION {
            return Err(format!(
                "unsupported schema version: {}",
                config.schema_version
            ));
        }
        for rule in &config.rules {
            rule.validate().map_err(|error| error.to_string())?;
        }
        Ok(config)
    }

    pub fn scan_browsers(&mut self) -> Vec<BrowserInstallation> {
        let adapters: Vec<Box<dyn BrowserAdapter>> = vec![
            Box::new(ChromeAdapter::adapter()),
            Box::new(EdgeAdapter::adapter()),
            Box::new(BraveAdapter::adapter()),
            Box::new(FirefoxAdapter),
        ];

        self.browsers = adapters
            .into_iter()
            .map(|adapter| {
                let descriptor = adapter.descriptor();
                let capabilities = adapter.capabilities();
                let installed = self
                    .platform
                    .browser_executable(&descriptor.id)
                    .is_some_and(|path| path.is_file());
                let (profiles, error) = if installed {
                    match adapter.discover_profiles(&self.platform) {
                        Ok(profiles) => (profiles, None),
                        Err(error) => (Vec::new(), Some(error.to_string())),
                    }
                } else {
                    (Vec::new(), None)
                };
                BrowserInstallation {
                    descriptor,
                    capabilities,
                    installed,
                    profiles,
                    error,
                }
            })
            .collect();
        self.seed_running_browser_identities();
        self.browsers.clone()
    }

    fn seed_running_browser_identities(&mut self) {
        self.refresh_running_browser_sessions();
        self.seed_frontmost_browser_identity(self.platform.frontmost_application_bundle_id());
    }

    fn seed_frontmost_browser_identity(&mut self, bundle_id: Option<String>) {
        let Some(bundle_id) = bundle_id else {
            return;
        };
        let browser_id = BrowserId::new(bundle_id);
        if self.platform.browser_executable(&browser_id).is_some() {
            self.record_current_browser_identity(browser_id);
        }
    }

    pub fn observe_foreground_browser(&mut self) {
        let Some(bundle_id) = self.platform.frontmost_application_bundle_id() else {
            return;
        };
        let browser_id = BrowserId::new(bundle_id);
        if self.platform.browser_executable(&browser_id).is_some() {
            self.refresh_running_browser_sessions();
            self.record_current_browser_identity(browser_id);
        }
    }

    fn refresh_running_browser_sessions(&mut self) {
        let running_browser_ids = self.platform.running_supported_browser_ids();
        self.available_profiles.clear();
        for browser_id in running_browser_ids {
            for profile_id in self.platform.active_profile_ids(&browser_id) {
                let discovered = self.browsers.iter().any(|browser| {
                    browser.profiles.iter().any(|profile| {
                        profile.browser_id == browser_id && profile.profile_id == profile_id
                    })
                });
                if discovered {
                    self.record_available_profile(browser_id.clone(), profile_id);
                }
            }
        }
    }

    fn record_current_browser_identity(&mut self, browser_id: BrowserId) {
        let Some(profile_id) = self.platform.last_used_profile_id(&browser_id) else {
            return;
        };
        let discovered = self
            .browsers
            .iter()
            .flat_map(|browser| browser.profiles.iter())
            .any(|profile| profile.browser_id == browser_id && profile.profile_id == profile_id);
        if discovered {
            self.record_active_identity(browser_id, profile_id, Instant::now());
        }
    }

    fn record_active_identity(
        &mut self,
        browser_id: BrowserId,
        profile_id: ProfileId,
        activation: Instant,
    ) {
        self.record_available_profile(browser_id.clone(), profile_id.clone());
        self.recent_identities
            .insert(browser_id, (profile_id, activation));
    }

    fn record_available_profile(&mut self, browser_id: BrowserId, profile_id: ProfileId) {
        self.available_profiles
            .entry(browser_id)
            .or_default()
            .insert(profile_id);
    }

    fn runtime_snapshot(&self) -> RuntimeSnapshot {
        RuntimeSnapshot {
            sessions: self
                .available_profiles
                .iter()
                .flat_map(|(browser_id, profile_ids)| {
                    profile_ids.iter().map(|profile_id| BrowserSession {
                        browser_id: browser_id.clone(),
                        profile_id: profile_id.clone(),
                        windows: vec![WindowRef("observed-browser-window".to_string())],
                        is_incognito: false,
                        last_user_activation: self.recent_identities.get(browser_id).and_then(
                            |(recent_profile_id, activation)| {
                                (recent_profile_id == profile_id).then_some(*activation)
                            },
                        ),
                    })
                })
                .collect(),
            source_identity: None,
        }
    }

    pub fn preview(&self, source_app: String, url: String) -> Result<RouteDecision, String> {
        let url = url::Url::parse(&url).map_err(|error| error.to_string())?;
        let runtime = self.runtime_snapshot();
        Ok(self.engine.decide(
            &RouteContext {
                url,
                source_app: AppId::new(source_app),
                event_id: "preview".to_string(),
                ask_next: self.ask_next,
                paused: self.paused,
            },
            &self.config,
            &runtime,
        ))
    }

    pub fn platform(&self) -> &DesktopPlatformAdapter {
        &self.platform
    }

    pub fn build_open_intent(
        &self,
        profile: &BrowserProfile,
        url: &url::Url,
        disposition: OpenDisposition,
    ) -> Result<platform_api::BrowserOpenIntent, String> {
        let adapter: Box<dyn BrowserAdapter> = match profile.browser_id.as_str() {
            "com.google.Chrome" => Box::new(ChromeAdapter::adapter()),
            "com.microsoft.edgemac" => Box::new(EdgeAdapter::adapter()),
            "com.brave.Browser" => Box::new(BraveAdapter::adapter()),
            "org.mozilla.firefox" => Box::new(FirefoxAdapter),
            _ => return Err("unsupported browser".to_string()),
        };
        adapter
            .build_open_intent(profile, url, disposition)
            .map_err(|error| error.to_string())
    }

    pub fn pending_routes(&self) -> &[PendingRoute] {
        &self.pending_routes
    }

    pub fn route_url(
        &mut self,
        url: url::Url,
        source_app: String,
    ) -> Result<RouteDisposition, String> {
        let event_id = self.next_event_id;
        self.next_event_id += 1;
        let runtime = self.runtime_snapshot();
        let decision = self.engine.decide(
            &RouteContext {
                url: url.clone(),
                source_app: AppId::new(source_app.clone()),
                event_id: event_id.to_string(),
                ask_next: self.ask_next,
                paused: self.paused,
            },
            &self.config,
            &runtime,
        );
        self.ask_next = false;
        let matched_rule_id = decision.matched_rule_id.as_ref().map(ToString::to_string);

        match decision.final_action {
            FinalAction::Ask => {
                self.pending_routes.push(PendingRoute {
                    id: event_id,
                    domain: url.host_str().unwrap_or_default().to_ascii_lowercase(),
                    source_app: source_app.clone(),
                    rule_id: matched_rule_id.clone(),
                    url: url.clone(),
                });
                self.diagnostics.record_route(
                    &source_app,
                    &url,
                    "ask",
                    matched_rule_id.as_deref(),
                    None,
                    None,
                );
                Ok(RouteDisposition::Ask)
            }
            FinalAction::Fail => {
                self.diagnostics.record_route(
                    &source_app,
                    &url,
                    "failed",
                    matched_rule_id.as_deref(),
                    None,
                    Some(format!("{:?}", decision.reason)),
                );
                Ok(RouteDisposition::Failed)
            }
            FinalAction::Open => {
                let mut last_error = None;
                let mut last_identity_id = None;
                for candidate in decision.candidates {
                    let disposition = candidate_disposition(&candidate);
                    last_identity_id = candidate_identity_id(&candidate);
                    let intent = if candidate.kind == CandidateKind::BrowserDefault {
                        Ok(platform_api::BrowserOpenIntent {
                            browser_id: candidate.browser_id.clone(),
                            profile_id: None,
                            url: url.clone(),
                            args: vec![url.as_str().to_string()],
                            disposition,
                        })
                    } else {
                        let Some(profile_id) = candidate.profile_id.as_ref() else {
                            continue;
                        };
                        let profile = self
                            .browsers
                            .iter()
                            .flat_map(|browser| browser.profiles.iter())
                            .find(|profile| {
                                profile.browser_id == candidate.browser_id
                                    && profile.profile_id == *profile_id
                            })
                            .cloned();
                        let Some(profile) = profile else {
                            last_error = Some("profile_unavailable".to_string());
                            continue;
                        };
                        self.build_open_intent(&profile, &url, disposition)
                    };
                    match intent.and_then(|intent| {
                        self.platform
                            .execute(intent)
                            .map_err(|error| error.to_string())
                    }) {
                        Ok(_) => {
                            self.diagnostics.record_route(
                                &source_app,
                                &url,
                                "opened",
                                matched_rule_id.as_deref(),
                                last_identity_id.as_deref(),
                                None,
                            );
                            return Ok(RouteDisposition::Opened);
                        }
                        Err(error) => last_error = Some(error),
                    }
                }
                self.diagnostics.record_route(
                    &source_app,
                    &url,
                    "failed",
                    matched_rule_id.as_deref(),
                    last_identity_id.as_deref(),
                    last_error.clone(),
                );
                Err(last_error.unwrap_or_else(|| "route had no executable candidate".to_string()))
            }
        }
    }

    pub fn choose_pending(
        &mut self,
        id: u64,
        browser_id: &str,
        profile_id: &str,
    ) -> Result<(), String> {
        let index = self
            .pending_routes
            .iter()
            .position(|route| route.id == id)
            .ok_or_else(|| "pending route was not found".to_string())?;
        let route = self.pending_routes[index].clone();
        let profile = self
            .browsers
            .iter()
            .flat_map(|browser| browser.profiles.iter())
            .find(|profile| {
                profile.browser_id.as_str() == browser_id
                    && profile.profile_id.as_str() == profile_id
            })
            .cloned()
            .ok_or_else(|| "profile was not found".to_string())?;
        let intent =
            self.build_open_intent(&profile, &route.url, OpenDisposition::ExistingWindow)?;
        self.platform
            .execute(intent)
            .map_err(|error| error.to_string())?;
        self.pending_routes.remove(index);
        let identity_id = format!("{browser_id}/{profile_id}");
        self.diagnostics.record_route(
            &route.source_app,
            &route.url,
            "opened",
            route.rule_id.as_deref(),
            Some(&identity_id),
            None,
        );
        Ok(())
    }

    pub fn cancel_pending(&mut self, id: u64) -> Result<(), String> {
        let index = self
            .pending_routes
            .iter()
            .position(|route| route.id == id)
            .ok_or_else(|| "pending route was not found".to_string())?;
        let route = self.pending_routes.remove(index);
        self.diagnostics.record_route(
            &route.source_app,
            &route.url,
            "cancelled",
            route.rule_id.as_deref(),
            None,
            None,
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use router_model::config::{
        Enforcement, FallbackScope, RouteRule, RuleMatcher, RuleTarget, TargetMode,
        UnavailableAction,
    };
    use router_model::ids::{BrowserId, RuleId};
    use router_model::DomainPattern;

    use super::*;

    fn temporary_service(name: &str) -> DesktopService {
        let path = std::env::temp_dir().join(name).join("config.json");
        let _ = std::fs::remove_file(&path);
        DesktopService::new(ConfigStore::new(path))
    }

    #[test]
    fn invalid_rules_are_not_saved() {
        let mut service = temporary_service("link-helm-invalid-rule-service");
        let config = RouterConfig {
            schema_version: 1,
            rules: vec![RouteRule {
                id: RuleId::new("invalid"),
                name: None,
                enabled: true,
                order: 0,
                matcher: RuleMatcher {
                    source_app: None,
                    domain: None,
                },
                target: RuleTarget {
                    mode: TargetMode::SpecifiedProfile,
                    browser_id: Some(BrowserId::new("com.google.Chrome")),
                    profile_id: None,
                },
                enforcement: Enforcement::Force,
                fallback_scope: FallbackScope::None,
                unavailable_action: UnavailableAction::Ask,
            }],
        };

        assert!(service.save_config(config).is_err());
        assert!(service.config.rules.is_empty());
    }

    #[test]
    fn recent_browser_identities_become_available_runtime_sessions() {
        let mut service = temporary_service("link-helm-recent-identity-snapshot-service");
        let older = Instant::now() - Duration::from_secs(5);
        let newer = Instant::now();
        service.record_active_identity(
            BrowserId::new("com.google.Chrome"),
            router_model::ids::ProfileId::new("Default"),
            older,
        );
        service.record_active_identity(
            BrowserId::new("com.brave.Browser"),
            router_model::ids::ProfileId::new("Profile 1"),
            newer,
        );

        let runtime = service.runtime_snapshot();

        assert_eq!(runtime.sessions.len(), 2);
        assert!(runtime
            .sessions
            .iter()
            .all(|session| session.is_available()));
        assert_eq!(
            runtime
                .sessions
                .iter()
                .max_by_key(|session| session.last_user_activation)
                .unwrap()
                .browser_id
                .as_str(),
            "com.brave.Browser"
        );
    }

    #[test]
    fn a_non_browser_frontmost_app_does_not_invent_recent_browser_activation() {
        let mut service = temporary_service("link-helm-no-invented-browser-activation-service");
        service.record_available_profile(
            BrowserId::new("com.google.Chrome"),
            ProfileId::new("Default"),
        );
        service.record_available_profile(
            BrowserId::new("com.brave.Browser"),
            ProfileId::new("Profile 1"),
        );

        service.seed_frontmost_browser_identity(Some("com.alibaba.DingTalkMac".to_string()));

        assert!(service
            .runtime_snapshot()
            .sessions
            .iter()
            .all(|session| session.last_user_activation.is_none()));
    }

    #[test]
    fn active_candidates_request_active_window_execution() {
        use router_model::routing::{CandidateKind, OpenCandidate};

        assert_eq!(
            candidate_disposition(&OpenCandidate {
                browser_id: BrowserId::new("com.google.Chrome"),
                profile_id: Some(router_model::ids::ProfileId::new("Default")),
                kind: CandidateKind::GloballyActive,
            }),
            OpenDisposition::ActiveWindow
        );
    }

    #[test]
    fn diagnostic_identity_uses_the_candidate_browser_and_profile() {
        use router_model::routing::{CandidateKind, OpenCandidate};

        let identity = candidate_identity_id(&OpenCandidate {
            browser_id: BrowserId::new("com.google.Chrome"),
            profile_id: Some(ProfileId::new("Default")),
            kind: CandidateKind::SpecifiedProfile,
        });

        assert_eq!(identity.as_deref(), Some("com.google.Chrome/Default"));
    }

    #[test]
    fn active_browser_rule_uses_the_recorded_identity_instead_of_asking() {
        let mut service = temporary_service("link-helm-active-browser-preview-service");
        service.record_active_identity(
            BrowserId::new("com.google.Chrome"),
            router_model::ids::ProfileId::new("Profile 6"),
            Instant::now(),
        );
        service.config.rules.push(RouteRule {
            id: RuleId::new("active-chrome"),
            name: Some("Active Chrome".to_string()),
            enabled: true,
            order: 0,
            matcher: RuleMatcher {
                source_app: None,
                domain: None,
            },
            target: RuleTarget {
                mode: TargetMode::ActiveInBrowser,
                browser_id: Some(BrowserId::new("com.google.Chrome")),
                profile_id: None,
            },
            enforcement: Enforcement::Prefer,
            fallback_scope: FallbackScope::None,
            unavailable_action: UnavailableAction::Ask,
        });

        let decision = service
            .preview(
                "com.alibaba.DingTalkMac".into(),
                "https://example.com".into(),
            )
            .unwrap();

        assert_eq!(decision.final_action, FinalAction::Open);
        assert_eq!(
            decision
                .primary()
                .unwrap()
                .profile_id
                .as_ref()
                .unwrap()
                .as_str(),
            "Profile 6"
        );
    }

    #[test]
    fn an_open_specified_profile_wins_before_any_active_fallback() {
        let mut service = temporary_service("link-helm-open-specified-profile-service");
        service.record_active_identity(
            BrowserId::new("com.google.Chrome"),
            router_model::ids::ProfileId::new("Profile 6"),
            Instant::now(),
        );
        service.record_available_profile(
            BrowserId::new("com.google.Chrome"),
            router_model::ids::ProfileId::new("Default"),
        );
        service.config.rules.push(RouteRule {
            id: RuleId::new("prefer-default"),
            name: Some("Default".to_string()),
            enabled: true,
            order: 0,
            matcher: RuleMatcher {
                source_app: None,
                domain: None,
            },
            target: RuleTarget {
                mode: TargetMode::SpecifiedProfile,
                browser_id: Some(BrowserId::new("com.google.Chrome")),
                profile_id: Some(router_model::ids::ProfileId::new("Default")),
            },
            enforcement: Enforcement::Prefer,
            fallback_scope: FallbackScope::AnyActiveBrowser,
            unavailable_action: UnavailableAction::OpenTargetProfile,
        });

        let decision = service
            .preview(
                "com.alibaba.DingTalkMac".into(),
                "https://example.com".into(),
            )
            .unwrap();

        let candidate = decision.primary().unwrap();
        assert_eq!(candidate.kind, CandidateKind::SpecifiedProfile);
        assert_eq!(candidate.profile_id.as_ref().unwrap().as_str(), "Default");
    }

    #[test]
    fn saved_rule_names_are_trimmed() {
        let mut service = temporary_service("link-helm-trimmed-rule-name-service");
        let config = RouterConfig {
            schema_version: 1,
            rules: vec![RouteRule {
                id: RuleId::new("named"),
                name: Some("  DingTalk links  ".to_string()),
                enabled: true,
                order: 0,
                matcher: RuleMatcher {
                    source_app: None,
                    domain: None,
                },
                target: RuleTarget {
                    mode: TargetMode::Ask,
                    browser_id: None,
                    profile_id: None,
                },
                enforcement: Enforcement::Prefer,
                fallback_scope: FallbackScope::None,
                unavailable_action: UnavailableAction::Ask,
            }],
        };

        service.save_config(config).unwrap();

        assert_eq!(
            service.config.rules[0].name.as_deref(),
            Some("DingTalk links")
        );
    }

    #[test]
    fn browser_scan_reports_installed_and_missing_browsers() {
        let mut service = temporary_service("link-helm-browser-scan-service");

        let browsers = service.scan_browsers();

        assert_eq!(browsers.len(), 4);
        assert!(browsers
            .iter()
            .any(|browser| browser.descriptor.id.as_str() == "com.google.Chrome"));
        assert!(browsers
            .iter()
            .all(|browser| browser.installed || browser.profiles.is_empty()));
    }

    #[test]
    fn preview_uses_saved_rules() {
        let mut service = temporary_service("link-helm-preview-service");
        service.config.rules.push(RouteRule {
            id: RuleId::new("ask-example"),
            name: None,
            enabled: true,
            order: 0,
            matcher: RuleMatcher {
                source_app: None,
                domain: Some(DomainPattern::parse("example.com").unwrap().into()),
            },
            target: RuleTarget {
                mode: TargetMode::Ask,
                browser_id: None,
                profile_id: None,
            },
            enforcement: Enforcement::Prefer,
            fallback_scope: FallbackScope::None,
            unavailable_action: UnavailableAction::Ask,
        });

        let decision = service
            .preview("com.apple.mail".into(), "https://example.com/a".into())
            .unwrap();
        assert_eq!(decision.matched_rule_id.unwrap().as_str(), "ask-example");
    }

    #[test]
    fn unmatched_external_url_is_queued_for_selection() {
        let mut service = temporary_service("link-helm-pending-route-service");

        let disposition = service
            .route_url(
                url::Url::parse("https://example.com/private?token=secret").unwrap(),
                "unknown".to_string(),
            )
            .unwrap();

        assert_eq!(disposition, RouteDisposition::Ask);
        assert_eq!(service.pending_routes().len(), 1);
        assert_eq!(service.pending_routes()[0].domain, "example.com");
    }

    #[test]
    fn failed_pending_selection_keeps_the_route_available() {
        let mut service = temporary_service("link-helm-failed-pending-selection-service");
        service
            .route_url(
                url::Url::parse("https://example.com/private").unwrap(),
                "com.apple.mail".to_string(),
            )
            .unwrap();
        let id = service.pending_routes()[0].id;

        let result = service.choose_pending(id, "com.google.Chrome", "missing-profile");

        assert!(result.is_err());
        assert_eq!(service.pending_routes().len(), 1);
        assert_eq!(service.pending_routes()[0].id, id);
    }

    #[test]
    fn cancelling_a_pending_route_removes_it_without_opening() {
        let mut service = temporary_service("link-helm-cancel-pending-route-service");
        service
            .route_url(
                url::Url::parse("https://example.com/private").unwrap(),
                "com.apple.mail".to_string(),
            )
            .unwrap();
        let id = service.pending_routes()[0].id;

        service.cancel_pending(id).unwrap();

        assert!(service.pending_routes().is_empty());
        assert_eq!(
            service.diagnostics.events().last().unwrap().outcome,
            "cancelled"
        );
    }

    #[test]
    fn ask_next_is_consumed_by_exactly_one_route() {
        let mut service = temporary_service("link-helm-ask-next-service");
        service.config.rules.push(RouteRule {
            id: RuleId::new("fail-all"),
            name: None,
            enabled: true,
            order: 0,
            matcher: RuleMatcher {
                source_app: None,
                domain: None,
            },
            target: RuleTarget {
                mode: TargetMode::GloballyActive,
                browser_id: None,
                profile_id: None,
            },
            enforcement: Enforcement::Prefer,
            fallback_scope: FallbackScope::None,
            unavailable_action: UnavailableAction::Fail,
        });
        service.ask_next = true;

        let first = service
            .route_url(
                url::Url::parse("https://first.example").unwrap(),
                "com.apple.mail".to_string(),
            )
            .unwrap();
        let second = service
            .route_url(
                url::Url::parse("https://second.example").unwrap(),
                "com.apple.mail".to_string(),
            )
            .unwrap();

        assert_eq!(first, RouteDisposition::Ask);
        assert_eq!(second, RouteDisposition::Failed);
        assert!(!service.ask_next);
        assert_eq!(service.pending_routes().len(), 1);
    }

    #[test]
    fn unsupported_scheme_fails_without_entering_the_selector_queue() {
        let mut service = temporary_service("link-helm-unsupported-scheme-service");

        let disposition = service
            .route_url(
                url::Url::parse("file:///Users/example/private.txt").unwrap(),
                "com.apple.finder".to_string(),
            )
            .unwrap();

        assert_eq!(disposition, RouteDisposition::Failed);
        assert!(service.pending_routes().is_empty());
    }

    #[test]
    fn invalid_import_does_not_replace_the_current_config() {
        let mut service = temporary_service("link-helm-invalid-import-service");
        service.config.rules.push(RouteRule {
            id: RuleId::new("existing"),
            name: None,
            enabled: true,
            order: 0,
            matcher: RuleMatcher {
                source_app: None,
                domain: None,
            },
            target: RuleTarget {
                mode: TargetMode::Ask,
                browser_id: None,
                profile_id: None,
            },
            enforcement: Enforcement::Prefer,
            fallback_scope: FallbackScope::None,
            unavailable_action: UnavailableAction::Ask,
        });

        let result = service.import_config(
            r#"{"schema_version":1,"rules":[{"id":"broken","matcher":{},"target":{"mode":"specified_profile"}}]}"#,
        );

        assert!(result.is_err());
        assert_eq!(service.config.rules.len(), 1);
        assert_eq!(service.config.rules[0].id.as_str(), "existing");
    }

    #[test]
    fn exported_config_can_be_previewed_and_imported() {
        let mut source = temporary_service("link-helm-export-source-service");
        source.config.rules.push(RouteRule {
            id: RuleId::new("exported"),
            name: None,
            enabled: true,
            order: 0,
            matcher: RuleMatcher {
                source_app: None,
                domain: Some(DomainPattern::parse("example.com").unwrap().into()),
            },
            target: RuleTarget {
                mode: TargetMode::Ask,
                browser_id: None,
                profile_id: None,
            },
            enforcement: Enforcement::Prefer,
            fallback_scope: FallbackScope::None,
            unavailable_action: UnavailableAction::Ask,
        });
        let json = source.export_config().unwrap();
        let mut target = temporary_service("link-helm-import-target-service");

        let preview = target.preview_import_config(&json).unwrap();
        assert_eq!(preview.rule_count, 1);
        assert!(target.config.rules.is_empty());

        target.import_config(&json).unwrap();
        assert_eq!(target.config.rules[0].id.as_str(), "exported");
    }

    #[test]
    fn damaged_config_enters_safe_mode_without_overwriting_the_file() {
        let dir = std::env::temp_dir().join("link-helm-damaged-config-service");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        let damaged = "{not-json";
        std::fs::write(&path, damaged).unwrap();

        let service = DesktopService::new(ConfigStore::new(&path));

        assert!(service.config.rules.is_empty());
        assert!(service.config_error.is_some());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), damaged);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

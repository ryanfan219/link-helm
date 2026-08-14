use serde::Serialize;
use url::Url;

use crate::browser::BrowserSession;
use crate::config::UnavailableAction;
use crate::ids::{AppId, BrowserId, ProfileId, RuleId};
use crate::IdentityRef;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteContext {
    pub url: Url,
    pub source_app: AppId,
    pub event_id: String,
    pub ask_next: bool,
    pub paused: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeSnapshot {
    pub sessions: Vec<BrowserSession>,
    pub source_identity: Option<IdentityRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CandidateKind {
    SpecifiedProfile,
    ActiveInBrowser,
    GloballyActive,
    NewTargetWindow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OpenCandidate {
    pub browser_id: BrowserId,
    pub profile_id: Option<ProfileId>,
    pub kind: CandidateKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum FinalAction {
    Open,
    Ask,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum DecisionReason {
    MatchedRule(RuleId),
    BuiltInSameBrowser,
    NoMatchingRule,
    UnsupportedScheme,
    InvalidUrl,
    Paused,
    AskNext,
}

#[derive(Debug, Clone, Serialize)]
pub struct RouteDecision {
    pub matched_rule_id: Option<RuleId>,
    pub candidates: Vec<OpenCandidate>,
    pub unavailable_action: UnavailableAction,
    pub final_action: FinalAction,
    pub reason: DecisionReason,
}

impl RouteDecision {
    pub fn primary(&self) -> Option<&OpenCandidate> {
        self.candidates.first()
    }

    pub fn ask(reason: DecisionReason) -> Self {
        Self {
            matched_rule_id: None,
            candidates: Vec::new(),
            unavailable_action: UnavailableAction::Ask,
            final_action: FinalAction::Ask,
            reason,
        }
    }

    pub fn fail(reason: DecisionReason) -> Self {
        Self {
            matched_rule_id: None,
            candidates: Vec::new(),
            unavailable_action: UnavailableAction::Fail,
            final_action: FinalAction::Fail,
            reason,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DecisionReason, RouteDecision};

    #[test]
    fn route_decision_can_be_serialized_for_ipc() {
        let decision = RouteDecision::ask(DecisionReason::NoMatchingRule);

        let value = serde_json::to_value(decision).expect("route decision should serialize");

        assert_eq!(value["final_action"], "Ask");
        assert_eq!(value["reason"], "NoMatchingRule");
    }
}

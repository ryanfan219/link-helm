use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const DEFAULT_DIAGNOSTIC_LIMIT: usize = 1000;
pub const MAX_DIAGNOSTIC_LIMIT: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticEvent {
    pub timestamp_ms: u64,
    pub source_app: String,
    pub domain: String,
    pub rule_id: Option<String>,
    pub identity_id: Option<String>,
    pub outcome: String,
    pub error: Option<String>,
}

#[derive(Debug)]
pub struct DiagnosticLog {
    events: Vec<DiagnosticEvent>,
    limit: usize,
    path: Option<PathBuf>,
    persistence_error: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum StoredDiagnostics {
    Current {
        limit: usize,
        events: Vec<DiagnosticEvent>,
    },
    Legacy(Vec<DiagnosticEvent>),
}

#[derive(Serialize)]
struct StoredDiagnosticsRef<'a> {
    limit: usize,
    events: &'a [DiagnosticEvent],
}

impl Default for DiagnosticLog {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            limit: DEFAULT_DIAGNOSTIC_LIMIT,
            path: None,
            persistence_error: None,
        }
    }
}

impl DiagnosticLog {
    pub fn open(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        match std::fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<StoredDiagnostics>(&bytes) {
                Ok(stored) => {
                    let (limit, mut events) = match stored {
                        StoredDiagnostics::Current { limit, events } => {
                            (limit.clamp(1, MAX_DIAGNOSTIC_LIMIT), events)
                        }
                        StoredDiagnostics::Legacy(events) => {
                            (DEFAULT_DIAGNOSTIC_LIMIT, events)
                        }
                    };
                    trim_events(&mut events, limit);
                    Self {
                        events,
                        limit,
                        path: Some(path),
                        persistence_error: None,
                    }
                }
                Err(error) => Self {
                    events: Vec::new(),
                    limit: DEFAULT_DIAGNOSTIC_LIMIT,
                    path: Some(path),
                    persistence_error: Some(format!("diagnostics file is invalid: {error}")),
                },
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self {
                events: Vec::new(),
                limit: DEFAULT_DIAGNOSTIC_LIMIT,
                path: Some(path),
                persistence_error: None,
            },
            Err(error) => Self {
                events: Vec::new(),
                limit: DEFAULT_DIAGNOSTIC_LIMIT,
                path: Some(path),
                persistence_error: Some(format!("diagnostics could not be loaded: {error}")),
            },
        }
    }

    pub fn events(&self) -> &[DiagnosticEvent] {
        &self.events
    }

    pub fn limit(&self) -> usize {
        self.limit
    }

    pub fn set_limit(&mut self, limit: usize) -> Result<(), String> {
        if !(1..=MAX_DIAGNOSTIC_LIMIT).contains(&limit) {
            return Err(format!(
                "diagnostics limit must be between 1 and {MAX_DIAGNOSTIC_LIMIT}"
            ));
        }
        self.limit = limit;
        trim_events(&mut self.events, self.limit);
        self.persist();
        self.persistence_error.clone().map_or(Ok(()), Err)
    }

    pub fn persistence_error(&self) -> Option<&str> {
        self.persistence_error.as_deref()
    }

    pub fn record_route(
        &mut self,
        source_app: &str,
        url: &url::Url,
        outcome: &str,
        rule_id: Option<&str>,
        identity_id: Option<&str>,
        error: Option<String>,
    ) {
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.events.push(DiagnosticEvent {
            timestamp_ms,
            source_app: source_app.to_string(),
            domain: url.host_str().unwrap_or_default().to_ascii_lowercase(),
            rule_id: rule_id.map(str::to_string),
            identity_id: identity_id.map(str::to_string),
            outcome: outcome.to_string(),
            error,
        });
        trim_events(&mut self.events, self.limit);
        self.persist();
    }

    pub fn clear(&mut self) {
        self.events.clear();
        self.persist();
    }

    fn persist(&mut self) {
        let Some(path) = self.path.as_deref() else {
            return;
        };
        self.persistence_error = persist_events(path, self.limit, &self.events).err();
    }
}

fn trim_events(events: &mut Vec<DiagnosticEvent>, limit: usize) {
    if events.len() > limit {
        events.drain(..events.len() - limit);
    }
}

fn persist_events(
    path: &Path,
    limit: usize,
    events: &[DiagnosticEvent],
) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "diagnostics file has no parent directory".to_string())?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let json = serde_json::to_vec_pretty(&StoredDiagnosticsRef { limit, events })
        .map_err(|error| error.to_string())?;
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, json).map_err(|error| error.to_string())?;
    std::fs::rename(&temporary, path).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::DiagnosticLog;

    #[test]
    fn diagnostics_keep_domain_but_not_sensitive_url_parts() {
        let mut log = DiagnosticLog::default();
        let url = url::Url::parse("https://Example.com/private/path?token=secret#account").unwrap();

        log.record_route(
            "com.example.source",
            &url,
            "failed",
            None,
            None,
            Some("launch_failed".into()),
        );

        let json = serde_json::to_string(log.events()).unwrap();
        assert!(json.contains("example.com"));
        assert!(!json.contains("private"));
        assert!(!json.contains("secret"));
        assert!(!json.contains("account"));
    }

    #[test]
    fn route_diagnostics_preserve_rule_and_target_identity() {
        let mut log = DiagnosticLog::default();
        let url = url::Url::parse("https://example.com/private?token=secret").unwrap();

        log.record_route(
            "com.example.source",
            &url,
            "opened",
            Some("dingtalk-chrome-default"),
            Some("com.google.Chrome/Default"),
            None,
        );

        let event = log.events().last().unwrap();
        assert_eq!(event.rule_id.as_deref(), Some("dingtalk-chrome-default"));
        assert_eq!(
            event.identity_id.as_deref(),
            Some("com.google.Chrome/Default")
        );
        let json = serde_json::to_string(event).unwrap();
        assert!(!json.contains("private"));
        assert!(!json.contains("secret"));
    }

    #[test]
    fn diagnostics_are_bounded() {
        let mut log = DiagnosticLog::default();
        let url = url::Url::parse("https://example.com").unwrap();
        for _ in 0..1005 {
            log.record_route("source", &url, "ok", None, None, None);
        }
        assert_eq!(log.events().len(), 1000);
    }

    #[test]
    fn persisted_diagnostics_survive_restart_and_remain_bounded() {
        let dir = std::env::temp_dir().join("link-helm-persisted-diagnostics-test");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("diagnostics.json");
        let url = url::Url::parse("https://example.com/private?token=secret").unwrap();
        let mut log = DiagnosticLog::open(&path);
        for _ in 0..1005 {
            log.record_route("source", &url, "opened", None, None, None);
        }

        let restored = DiagnosticLog::open(&path);

        assert_eq!(restored.events().len(), 1000);
        let json = std::fs::read_to_string(&path).unwrap();
        assert!(json.contains("example.com"));
        assert!(!json.contains("private"));
        assert!(!json.contains("secret"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

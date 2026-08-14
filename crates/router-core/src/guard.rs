use std::collections::HashSet;

#[derive(Debug, Default)]
pub struct RouterGuard {
    seen_events: HashSet<String>,
    in_flight_urls: HashSet<String>,
}

impl RouterGuard {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_duplicate(&self, event_id: &str) -> bool {
        self.seen_events.contains(event_id)
    }

    pub fn mark_seen(&mut self, event_id: impl Into<String>) {
        self.seen_events.insert(event_id.into());
    }

    pub fn begin_route(&mut self, url_fingerprint: impl Into<String>) -> bool {
        self.in_flight_urls.insert(url_fingerprint.into())
    }

    pub fn end_route(&mut self, url_fingerprint: &str) {
        self.in_flight_urls.remove(url_fingerprint);
    }

    pub fn is_in_flight(&self, url_fingerprint: &str) -> bool {
        self.in_flight_urls.contains(url_fingerprint)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_apple_event_delivery_is_rejected() {
        let mut guard = RouterGuard::new();
        guard.mark_seen("apple-event-1");
        assert!(guard.is_duplicate("apple-event-1"));
        assert!(!guard.is_duplicate("apple-event-2"));
    }

    #[test]
    fn in_flight_url_guards_against_loop() {
        let mut guard = RouterGuard::new();
        assert!(guard.begin_route("https://example.com/path"));
        assert!(guard.is_in_flight("https://example.com/path"));
        guard.end_route("https://example.com/path");
        assert!(!guard.is_in_flight("https://example.com/path"));
    }
}

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DomainError {
    #[error("domain is empty")]
    Empty,
    #[error("domain is not a valid IDNA hostname")]
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Domain(String);

impl Domain {
    pub fn normalize(input: &str) -> Result<Self, DomainError> {
        let trimmed = input.trim().trim_end_matches('.');
        if trimmed.is_empty() {
            return Err(DomainError::Empty);
        }
        let ascii = idna::domain_to_ascii(trimmed).map_err(|_| DomainError::Invalid)?;
        Ok(Self(ascii.to_ascii_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl std::fmt::Display for Domain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DomainPattern {
    Exact(Domain),
    Wildcard(Domain),
}

impl DomainPattern {
    pub fn parse(input: &str) -> Result<Self, DomainError> {
        if let Some(rest) = input.strip_prefix("*.") {
            Ok(Self::Wildcard(Domain::normalize(rest)?))
        } else {
            Ok(Self::Exact(Domain::normalize(input)?))
        }
    }

    pub fn matches(&self, host: &str) -> bool {
        let Ok(normalized) = Domain::normalize(host) else {
            return false;
        };
        match self {
            Self::Exact(pattern) => pattern == &normalized,
            Self::Wildcard(pattern) => normalized
                .as_str()
                .ends_with(&format!(".{}", pattern.as_str())),
        }
    }

    pub fn as_str(&self) -> String {
        match self {
            Self::Exact(d) => d.to_string(),
            Self::Wildcard(d) => format!("*.{}", d),
        }
    }
}

impl Serialize for DomainPattern {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.as_str())
    }
}

impl<'de> Deserialize<'de> for DomainPattern {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_case_and_idna() {
        assert_eq!(
            Domain::normalize("EXAMPLE.COM").unwrap().as_str(),
            "example.com"
        );
        assert_eq!(
            Domain::normalize("bücher.example").unwrap().as_str(),
            "xn--bcher-kva.example"
        );
    }

    #[test]
    fn rejects_empty_domain() {
        assert!(matches!(Domain::normalize(""), Err(DomainError::Empty)));
    }

    #[test]
    fn wildcard_matches_subdomains_but_not_root() {
        let pattern = DomainPattern::parse("*.example.com").unwrap();
        assert!(pattern.matches("a.example.com"));
        assert!(pattern.matches("a.b.example.com"));
        assert!(!pattern.matches("example.com"));
        assert!(!pattern.matches("notexample.com"));
    }

    #[test]
    fn exact_matches_only_same_domain_case_insensitively() {
        let pattern = DomainPattern::parse("Example.COM").unwrap();
        assert!(pattern.matches("example.com"));
        assert!(!pattern.matches("a.example.com"));
    }

    #[test]
    fn serializes_as_plain_string() {
        let exact = DomainPattern::parse("example.com").unwrap();
        let wildcard = DomainPattern::parse("*.example.com").unwrap();
        assert_eq!(serde_json::to_string(&exact).unwrap(), "\"example.com\"");
        assert_eq!(
            serde_json::to_string(&wildcard).unwrap(),
            "\"*.example.com\""
        );
        let back: DomainPattern = serde_json::from_str("\"*.Example.COM\"").unwrap();
        assert_eq!(back, wildcard);
    }
}

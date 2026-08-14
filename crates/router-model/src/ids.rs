use serde::{Deserialize, Serialize};

macro_rules! id_newtype {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn into_inner(self) -> String {
                self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

id_newtype!(BrowserId);
id_newtype!(ProfileId);
id_newtype!(AppId);
id_newtype!(RuleId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_transparent_strings() {
        let id = BrowserId::new("com.example.Browser");
        assert_eq!(id.as_str(), "com.example.Browser");
        assert_eq!(id.to_string(), "com.example.Browser");
        assert_eq!(id.into_inner(), "com.example.Browser");
    }
}

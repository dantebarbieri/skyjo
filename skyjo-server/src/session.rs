use std::fmt;

/// Opaque session token for player identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionToken(String);

impl SessionToken {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for SessionToken {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SessionToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for SessionToken {
    fn from(s: String) -> Self {
        Self(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_token_is_valid_uuid() {
        let token = SessionToken::new();
        uuid::Uuid::parse_str(token.as_str()).expect("token should be a valid UUID");
    }

    #[test]
    fn as_str_returns_inner_string() {
        let token = SessionToken::from("test-value".to_string());
        assert_eq!(token.as_str(), "test-value");
    }

    #[test]
    fn display_matches_as_str() {
        let token = SessionToken::from("abc-123".to_string());
        assert_eq!(token.to_string(), "abc-123");
        assert_eq!(token.to_string(), token.as_str());
    }

    #[test]
    fn from_string_round_trip() {
        let original = "my-session-id".to_string();
        let token = SessionToken::from(original.clone());
        assert_eq!(token.as_str(), original);
    }

    #[test]
    fn equality() {
        let a = SessionToken::from("same".to_string());
        let b = SessionToken::from("same".to_string());
        let c = SessionToken::from("different".to_string());
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn two_new_tokens_are_unique() {
        let a = SessionToken::new();
        let b = SessionToken::new();
        assert_ne!(a, b);
    }

    #[test]
    fn hash_consistency() {
        use std::collections::HashSet;
        let token = SessionToken::from("hashable".to_string());
        let mut set = HashSet::new();
        set.insert(token.clone());
        assert!(set.contains(&token));
    }
}

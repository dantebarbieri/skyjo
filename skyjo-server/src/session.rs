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

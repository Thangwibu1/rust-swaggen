//! Toolkit/database-independent identities and safe boundary errors.
pub mod dynamic_vars;
pub mod extensions;
pub mod governance;
pub mod graphql;
pub mod security;
pub mod server_profiles;
pub mod ssrf;
pub mod workspace;
use serde::{Deserialize, Serialize};
use std::fmt;
pub const DRAFT_LIMIT: usize = 128 * 1024;
pub const SECRET_LIMIT: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    InvalidId,
    Limit,
    Storage,
    Recovery,
    Version,
    Workspace,
    Protection,
    Missing,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidId => "Invalid identifier",
            Self::Limit => "Size or count limit exceeded",
            Self::Storage => "Storage unavailable; keep unsaved changes and retry after recovery",
            Self::Recovery => "Local data needs recovery; no data was overwritten",
            Self::Version => "Unsupported local format; upgrade required",
            Self::Workspace => "Workspace identity does not match local data",
            Self::Protection => "Windows secret protection failed",
            Self::Missing => "No saved secret with this name",
        })
    }
}
impl std::error::Error for Error {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct LocalId(String);
impl LocalId {
    pub fn parse(value: impl Into<String>) -> Result<Self, Error> {
        let value = value.into();
        let reserved = [
            "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7",
            "com8", "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
        ];
        if value.is_empty()
            || value.len() > 64
            || reserved.contains(&value.as_str())
            || !value
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
            || !value.as_bytes()[0].is_ascii_alphanumeric()
        {
            return Err(Error::InvalidId);
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for LocalId {
    type Error = Error;
    fn try_from(v: String) -> Result<Self, Error> {
        Self::parse(v)
    }
}
impl From<LocalId> for String {
    fn from(v: LocalId) -> Self {
        v.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretRef {
    pub workspace: LocalId,
    pub name: LocalId,
}
impl SecretRef {
    pub fn binding(&self) -> Vec<u8> {
        format!(
            "Swaggen:secret:v1:{}:{}",
            self.workspace.as_str(),
            self.name.as_str()
        )
        .into_bytes()
    }
}

/// Deliberately has no Serialize or Clone. Debug never includes content.
pub struct Secret(Vec<u8>);
impl Secret {
    pub fn new(value: Vec<u8>) -> Result<Self, Error> {
        let value = Self(value);
        if value.0.is_empty() || value.0.len() > SECRET_LIMIT {
            Err(Error::Limit)
        } else {
            Ok(value)
        }
    }
    pub fn expose(&self) -> &[u8] {
        &self.0
    }
}
impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret([REDACTED])")
    }
}
impl Drop for Secret {
    fn drop(&mut self) {
        for byte in &mut self.0 {
            // SAFETY: each byte is exclusively borrowed, live and aligned.
            unsafe {
                std::ptr::write_volatile(byte, 0);
            }
        }
        std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ids_reject_paths_devices_and_case_aliases() {
        for value in [
            "", "../other", "a/b", "a\\b", "a:b", "CON", "con", "lpt1", "a.", "Other", "_root",
        ] {
            assert!(LocalId::parse(value).is_err());
        }
        assert!(LocalId::parse("personal-01").is_ok());
    }
    #[test]
    fn secret_debug_and_errors_do_not_disclose_payload() {
        let secret = Secret::new(b"private-test-canary".to_vec()).unwrap();
        assert_eq!(format!("{secret:?}"), "Secret([REDACTED])");
        assert!(
            !Error::Protection
                .to_string()
                .contains("private-test-canary")
        );
    }
    #[test]
    fn binding_separates_workspaces_and_names() {
        let a = SecretRef {
            workspace: LocalId::parse("alpha").unwrap(),
            name: LocalId::parse("token").unwrap(),
        };
        let b = SecretRef {
            workspace: LocalId::parse("beta").unwrap(),
            name: a.name.clone(),
        };
        assert_ne!(a.binding(), b.binding());
    }
}

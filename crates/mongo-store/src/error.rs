//! Safe boundary errors returned by server persistence; never exposes DB URIs or stack traces.
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    InvalidInput(String),
    Unauthorized(String),
    Forbidden(String),
    NotFound(String),
    Conflict(String),
    RateLimited(String),
    BootstrapCompleted,
    InvitationExpired,
    InvitationAlreadyUsed,
    InvitationRevoked,
    InvitationAccountMismatch,
    OwnerRequired(String),
    CursorExpired(i64),
    DatabaseUnavailable,
    InternalSafe,
}

impl StoreError {
    /// Returns `(http_status_code, machine_code, safe_message, retryable)`.
    pub fn to_http_parts(&self) -> (u16, &'static str, &str, bool) {
        match self {
            Self::InvalidInput(msg) => (400, "invalid_input", msg, false),
            Self::Unauthorized(msg) => (401, "unauthorized", msg, false),
            Self::Forbidden(msg) => (403, "forbidden", msg, false),
            Self::NotFound(msg) => (404, "not_found", msg, false),
            Self::Conflict(msg) => (409, "conflict", msg, false),
            Self::RateLimited(msg) => (429, "rate_limited", msg, true),
            Self::BootstrapCompleted => (
                409,
                "bootstrap_already_completed",
                "Server owner has already been initialized",
                false,
            ),
            Self::InvitationExpired => (
                410,
                "invitation_expired",
                "This invitation link has expired",
                false,
            ),
            Self::InvitationAlreadyUsed => (
                409,
                "invitation_already_used",
                "This invitation has already been used",
                false,
            ),
            Self::InvitationRevoked => (
                410,
                "invitation_revoked",
                "This invitation was revoked by a workspace admin",
                false,
            ),
            Self::InvitationAccountMismatch => (
                403,
                "invitation_account_mismatch",
                "Signed-in or supplied account email does not match this invitation",
                false,
            ),
            Self::OwnerRequired(msg) => (409, "owner_required", msg, false),
            Self::CursorExpired(_) => (
                410,
                "cursor_expired",
                "Sync cursor expired beyond server event retention; fetch a consistent snapshot",
                false,
            ),
            Self::DatabaseUnavailable => (
                503,
                "database_unavailable",
                "Database topology unavailable; no changes committed",
                true,
            ),
            Self::InternalSafe => (
                500,
                "internal_error",
                "Internal server error; check server correlation ID",
                false,
            ),
        }
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (_, code, msg, _) = self.to_http_parts();
        write!(f, "{code}: {msg}")
    }
}

impl std::error::Error for StoreError {}

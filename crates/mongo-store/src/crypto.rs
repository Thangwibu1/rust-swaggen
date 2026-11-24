//! Password hashing (Argon2id), opaque token SHA-256 hashing, and login rate limiting.
use crate::error::StoreError;
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::{collections::HashMap, sync::Mutex};

pub const SESSION_TTL_SECS: i64 = 14 * 24 * 3600;
const LOGIN_WINDOW_SECS: i64 = 15 * 60;
const LOGIN_MAX_FAILURES: usize = 5;

pub fn normalize_email(email: &str) -> Result<String, StoreError> {
    let trimmed = email.trim().to_ascii_lowercase();
    let Some((local, domain)) = trimmed.split_once('@') else {
        return Err(StoreError::InvalidInput("Valid email address is required".into()));
    };
    if local.is_empty()
        || domain.is_empty()
        || !domain.contains('.')
        || trimmed.len() > 254
        || trimmed.chars().any(char::is_whitespace)
    {
        return Err(StoreError::InvalidInput("Valid email address is required".into()));
    }
    Ok(trimmed)
}

pub fn normalize_slug(slug: &str) -> Result<String, StoreError> {
    let trimmed = slug.trim().to_ascii_lowercase();
    if trimmed.len() < 2
        || trimmed.len() > 64
        || !trimmed
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
    {
        return Err(StoreError::InvalidInput(
            "Workspace slug must be 2-64 chars of [a-z0-9-_]".into(),
        ));
    }
    Ok(trimmed)
}

pub fn validate_password(password: &str) -> Result<(), StoreError> {
    if password.len() < 10 {
        return Err(StoreError::InvalidInput(
            "Password must be at least 10 characters".into(),
        ));
    }
    if password.len() > 256 {
        return Err(StoreError::InvalidInput(
            "Password exceeds 256 characters".into(),
        ));
    }
    Ok(())
}

pub fn hash_password(password: &str) -> Result<String, StoreError> {
    validate_password(password)?;
    let mut salt_bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut salt_bytes);
    let salt = SaltString::encode_b64(&salt_bytes).map_err(|_| StoreError::InternalSafe)?;
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| StoreError::InternalSafe)?;
    Ok(hash.to_string())
}

pub fn verify_password(password: &str, phc_hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(phc_hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

pub fn generate_opaque_token(prefix: &str) -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    format!("{prefix}_{}", URL_SAFE_NO_PAD.encode(bytes))
}

pub fn sha256_hex(secret: &str) -> String {
    let digest = Sha256::digest(secret.as_bytes());
    hex::encode(digest)
}

#[derive(Debug, Default)]
pub struct LoginRateLimiter {
    failures: Mutex<HashMap<String, Vec<i64>>>,
}

impl LoginRateLimiter {
    pub fn check_allowed(&self, key: &str, now: i64) -> Result<(), StoreError> {
        let mut guard = self.failures.lock().map_err(|_| StoreError::InternalSafe)?;
        let entries = guard.entry(key.to_owned()).or_default();
        entries.retain(|ts| now - *ts < LOGIN_WINDOW_SECS);
        if entries.len() >= LOGIN_MAX_FAILURES {
            return Err(StoreError::RateLimited(
                "Too many failed login attempts; wait 15 minutes before retrying".into(),
            ));
        }
        Ok(())
    }

    pub fn record_failure(&self, key: &str, now: i64) {
        if let Ok(mut guard) = self.failures.lock() {
            let entries = guard.entry(key.to_owned()).or_default();
            entries.retain(|ts| now - *ts < LOGIN_WINDOW_SECS);
            entries.push(now);
        }
    }

    pub fn clear(&self, key: &str) {
        if let Ok(mut guard) = self.failures.lock() {
            guard.remove(key);
        }
    }
}

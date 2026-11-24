//! Consistent backup bundle creation and verified clean-target restore (OP05 / OP06).
use crate::error::StoreError;
use mongodb::{Database, bson::Document};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const BACKUP_FORMAT_VERSION: u32 = 1;
pub const BACKUP_COLLECTIONS: &[&str] = &[
    "schema_migrations",
    "users",
    "workspaces",
    "memberships",
    "workspace_permission_fences",
    "workspace_counters",
    "entities",
    "revisions",
    "applied_operations",
    "change_events",
    "comments",
    "notifications",
    "audit_events",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BackupManifest {
    pub format_version: u32,
    pub schema_migration_version: i32,
    pub created_at_unix: i64,
    pub collection_counts: BTreeMap<String, usize>,
    pub payload_sha256: String,
    pub key_policy_note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BackupBundle {
    pub manifest: BackupManifest,
    pub collections: BTreeMap<String, Vec<serde_json::Value>>,
}

impl BackupBundle {
    pub fn build(
        schema_migration_version: i32,
        created_at_unix: i64,
        collections: BTreeMap<String, Vec<serde_json::Value>>,
    ) -> Result<Self, StoreError> {
        let mut counts = BTreeMap::new();
        for (name, docs) in &collections {
            counts.insert(name.clone(), docs.len());
        }
        let payload_bytes = serde_json::to_vec(&collections).map_err(|_| StoreError::InternalSafe)?;
        let payload_sha256 = hex::encode(Sha256::digest(&payload_bytes));
        Ok(Self {
            manifest: BackupManifest {
                format_version: BACKUP_FORMAT_VERSION,
                schema_migration_version,
                created_at_unix,
                collection_counts: counts,
                payload_sha256,
                key_policy_note: "Encryption keys/DPAPI secrets are stored strictly outside this backup bundle.".into(),
            },
            collections,
        })
    }

    pub fn verify_integrity(&self) -> Result<(), String> {
        if self.manifest.format_version != BACKUP_FORMAT_VERSION {
            return Err(format!(
                "Unsupported backup format version {}",
                self.manifest.format_version
            ));
        }
        for (k, expected_cnt) in &self.manifest.collection_counts {
            let actual = self.collections.get(k).map(Vec::len).unwrap_or(0);
            if actual != *expected_cnt {
                return Err(format!(
                    "Count mismatch for collection '{k}': manifest={expected_cnt}, actual={actual}"
                ));
            }
        }
        let payload_bytes =
            serde_json::to_vec(&self.collections).map_err(|e| format!("Serialize error: {e}"))?;
        let computed = hex::encode(Sha256::digest(&payload_bytes));
        if computed != self.manifest.payload_sha256 {
            return Err("Backup checksum mismatch: bundle corrupted or tampered".into());
        }
        Ok(())
    }
}

pub async fn export_mongo_backup(
    db: &Database,
    schema_version: i32,
    now_unix: i64,
) -> Result<BackupBundle, StoreError> {
    let mut collections = BTreeMap::new();
    for &col_name in BACKUP_COLLECTIONS {
        let col = db.collection::<Document>(col_name);
        let mut cursor = col
            .find(mongodb::bson::doc! {})
            .await
            .map_err(|_| StoreError::DatabaseUnavailable)?;
        let mut docs = Vec::new();
        while cursor
            .advance()
            .await
            .map_err(|_| StoreError::DatabaseUnavailable)?
        {
            let doc = cursor
                .deserialize_current()
                .map_err(|_| StoreError::InternalSafe)?;
            let val = serde_json::to_value(&doc).map_err(|_| StoreError::InternalSafe)?;
            docs.push(val);
        }
        collections.insert(col_name.to_owned(), docs);
    }
    BackupBundle::build(schema_version, now_unix, collections)
}

pub async fn restore_mongo_backup(db: &Database, bundle: &BackupBundle) -> Result<(), String> {
    bundle.verify_integrity()?;
    // Refuse restoring onto a non-empty database to preserve clean-target isolation (OP06)
    for &col_name in BACKUP_COLLECTIONS {
        let count = db
            .collection::<Document>(col_name)
            .count_documents(mongodb::bson::doc! {})
            .await
            .map_err(|_| "Failed checking target MongoDB cleanliness".to_owned())?;
        if count > 0 {
            return Err(format!(
                "Target database collection '{col_name}' is not empty ({count} docs); restore requires a clean DB"
            ));
        }
    }
    for (col_name, rows) in &bundle.collections {
        if rows.is_empty() {
            continue;
        }
        let col = db.collection::<Document>(col_name);
        let mut bson_docs = Vec::with_capacity(rows.len());
        for row in rows {
            let doc: Document = serde_json::from_value(row.clone())
                .map_err(|e| format!("Invalid BSON doc in '{col_name}': {e}"))?;
            bson_docs.push(doc);
        }
        col.insert_many(bson_docs)
            .await
            .map_err(|e| format!("Failed restoring '{col_name}': {e}"))?;
        let actual = col
            .count_documents(mongodb::bson::doc! {})
            .await
            .map_err(|_| "Verify count failed".to_owned())? as usize;
        let expected = bundle
            .manifest
            .collection_counts
            .get(col_name)
            .copied()
            .unwrap_or(0);
        if actual != expected {
            return Err(format!(
                "Post-restore verification mismatch for '{col_name}': expected {expected}, got {actual}"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn backup_bundle_verifies_integrity_and_rejects_tampering() {
        let mut cols = BTreeMap::new();
        cols.insert(
            "entities".into(),
            vec![json!({"entity_id":"req-1","revision":3,"name":"List"})],
        );
        cols.insert("memberships".into(), vec![json!({"user_id":"u1","role":"owner"})]);
        let mut bundle = BackupBundle::build(1, 12345, cols).unwrap();
        assert!(bundle.verify_integrity().is_ok());

        // Tamper payload -> SHA-256 verification fails cleanly
        bundle
            .collections
            .get_mut("entities")
            .unwrap()
            .push(json!({"entity_id":"injected"}));
        assert!(bundle.verify_integrity().is_err());
    }
}

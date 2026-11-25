//! State-based collaborative CRDT text/kv engine with deterministic Lamport+Actor tie-breaking,
//! offline convergence, and publish-time RBAC revoke guard (CO10).
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use swaggen_api_contract::Role;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OpId {
    pub lamport: u64,
    pub actor: String,
    pub seq: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrdtNode {
    pub id: OpId,
    pub after: Option<OpId>,
    pub ch: char,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollaborativeTextCrdt {
    pub clock: u64,
    pub nodes: BTreeMap<OpId, CrdtNode>,
    pub tombstones: BTreeSet<OpId>,
}

impl CollaborativeTextCrdt {
    pub fn insert_end(&mut self, actor: &str, ch: char) -> OpId {
        self.clock += 1;
        let id = OpId {
            lamport: self.clock,
            actor: actor.to_owned(),
            seq: self.nodes.len() as u64 + 1,
        };
        let after = self.nodes.keys().last().cloned();
        self.nodes.insert(
            id.clone(),
            CrdtNode {
                id: id.clone(),
                after,
                ch,
            },
        );
        id
    }

    pub fn delete(&mut self, id: &OpId) {
        self.tombstones.insert(id.clone());
    }

    /// Commutative, associative, idempotent state-based CRDT merge.
    pub fn merge_crdt(&mut self, remote: &Self) {
        self.clock = self.clock.max(remote.clock);
        for (id, node) in &remote.nodes {
            self.nodes.entry(id.clone()).or_insert_with(|| node.clone());
        }
        for tid in &remote.tombstones {
            self.tombstones.insert(tid.clone());
        }
    }

    /// Enforces publish-time workspace authorization so revoked/Viewer clients cannot publish (CO10).
    pub fn merge_if_authorized(&mut self, remote: &Self, caller_role: Role) -> Result<(), String> {
        if !caller_role.can_write_entities() {
            return Err(
                "Publish blocked: caller role lacks write permission (revoked or Viewer)".into(),
            );
        }
        self.merge_crdt(remote);
        Ok(())
    }

    pub fn render_text(&self) -> String {
        self.nodes
            .values()
            .filter(|n| !self.tombstones.contains(&n.id))
            .map(|n| n.ch)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crdt_merges_commutatively_idempotently_and_enforces_publish_revoke_guard() {
        let mut a = CollaborativeTextCrdt::default();
        let mut b = CollaborativeTextCrdt::default();
        let id1 = a.insert_end("alice", 'H');
        let _ = a.insert_end("alice", 'i');
        let _ = b.insert_end("bob", '!');
        let mut a_snap = a.clone();
        a_snap.delete(&id1);

        let mut merged_ab = a_snap.clone();
        merged_ab.merge_crdt(&b);
        let mut merged_ba = b.clone();
        merged_ba.merge_crdt(&a_snap);
        assert_eq!(merged_ab.render_text(), merged_ba.render_text());
        assert!(merged_ab.render_text().contains('i'));
        assert!(merged_ab.render_text().contains('!'));
        assert!(!merged_ab.render_text().contains('H'));

        // Idempotent repeat merge
        let before = merged_ab.render_text();
        merged_ab.merge_crdt(&b);
        assert_eq!(merged_ab.render_text(), before);

        // Revoked / Viewer publish rejected cleanly
        let err = merged_ab.merge_if_authorized(&a, Role::Viewer).unwrap_err();
        assert!(err.contains("Publish blocked"));
    }
}

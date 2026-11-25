//! Capability-gated extension registry & execution fence (PR10).
//! All capabilities default to DENIED; ambient filesystem, shell, and secret access are impossible.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ExtensionCapability {
    ReadRequestMetadata,
    TransformBody,
    ReadNonSecretVars,
    NetworkOutboundAllowlisted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub declared_capabilities: BTreeSet<ExtensionCapability>,
    pub granted_capabilities: BTreeSet<ExtensionCapability>,
    pub enabled: bool,
}

impl ExtensionManifest {
    /// Creates an extension with zero granted capabilities (fail-closed default per PR10).
    pub fn install_disabled(
        id: impl Into<String>,
        name: impl Into<String>,
        version: impl Into<String>,
        declared: BTreeSet<ExtensionCapability>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            version: version.into(),
            declared_capabilities: declared,
            granted_capabilities: BTreeSet::new(),
            enabled: false,
        }
    }

    pub fn grant_capability(&mut self, cap: ExtensionCapability) -> Result<(), String> {
        if !self.declared_capabilities.contains(&cap) {
            return Err(format!(
                "Extension '{}' did not declare capability {cap:?}",
                self.id
            ));
        }
        self.granted_capabilities.insert(cap);
        Ok(())
    }

    pub fn revoke_capability(&mut self, cap: ExtensionCapability) {
        self.granted_capabilities.remove(&cap);
        if self.granted_capabilities.is_empty() {
            self.enabled = false;
        }
    }

    pub fn revoke_all(&mut self) {
        self.granted_capabilities.clear();
        self.enabled = false;
    }

    pub fn require_capability(&self, cap: ExtensionCapability) -> Result<(), String> {
        if !self.enabled {
            return Err(format!("Extension '{}' is disabled", self.id));
        }
        if !self.granted_capabilities.contains(&cap) {
            return Err(format!(
                "Capability denied: '{}' lacks granted permission {:?}",
                self.id, cap
            ));
        }
        Ok(())
    }
}

/// Runs a pure header/body transform hook strictly after verifying capability grant.
pub fn run_transform_extension(
    ext: &ExtensionManifest,
    header_name: &str,
    header_value: &str,
) -> Result<(String, String), String> {
    ext.require_capability(ExtensionCapability::TransformBody)?;
    Ok((
        header_name.trim().to_owned(),
        format!("ext:{}:{}", ext.id, header_value),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_capabilities_default_denied_and_can_be_granted_then_revoked() {
        let mut declared = BTreeSet::new();
        declared.insert(ExtensionCapability::TransformBody);
        let mut ext =
            ExtensionManifest::install_disabled("hdr-tagger", "Header Tagger", "1.0.0", declared);
        assert!(ext.granted_capabilities.is_empty());
        assert!(!ext.enabled);
        assert!(run_transform_extension(&ext, "X-Trace", "1").is_err());

        ext.enabled = true;
        // Enabled without capability grant still fails closed!
        assert!(run_transform_extension(&ext, "X-Trace", "1").is_err());
        // Undeclared capability cannot be granted!
        assert!(
            ext.grant_capability(ExtensionCapability::ReadNonSecretVars)
                .is_err()
        );

        ext.grant_capability(ExtensionCapability::TransformBody)
            .unwrap();
        let (k, v) = run_transform_extension(&ext, "X-Trace", "1").unwrap();
        assert_eq!(k, "X-Trace");
        assert_eq!(v, "ext:hdr-tagger:1");

        ext.revoke_capability(ExtensionCapability::TransformBody);
        assert!(run_transform_extension(&ext, "X-Trace", "1").is_err());
    }
}

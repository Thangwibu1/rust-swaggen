//! Allowlisted diagnostics (Z07 / SC07). There is deliberately no input for
//! user content, error strings, environment variables, filesystem paths or secret values.
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use swaggen_domain::{Error, LocalId, security::DesktopPolicy};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticPreview {
    pub include_lines: Vec<(&'static str, String)>,
    pub exclude_lines: Vec<&'static str>,
    pub rendered_text: String,
}

pub fn build_preview(
    workspace: &LocalId,
    loaded: bool,
    pending: bool,
    recovery_needed: bool,
    revision: u64,
    secret_count: usize,
    policy: &DesktopPolicy,
) -> DiagnosticPreview {
    let include_lines = vec![
        ("App version", env!("CARGO_PKG_VERSION").to_owned()),
        ("Platform", std::env::consts::OS.to_owned()),
        ("Workspace id", workspace.as_str().to_owned()),
        ("Local schema version", "1".to_owned()),
        ("Workspace open", loaded.to_string()),
        ("Operation pending", pending.to_string()),
        ("Recovery needed", recovery_needed.to_string()),
        ("Local revision", revision.to_string()),
        (
            "Saved secret count (names hidden)",
            secret_count.to_string(),
        ),
        ("Analytics enabled", policy.analytics_enabled.to_string()),
        (
            "Crash upload enabled",
            policy.crash_upload_enabled.to_string(),
        ),
        (
            "Remote HTML execution",
            policy.remote_html_execution.to_string(),
        ),
        (
            "External ref auto-fetch",
            policy.external_ref_fetch.to_string(),
        ),
        (
            "Upload mode",
            "No automatic upload (manual export only)".to_owned(),
        ),
    ];
    let exclude_lines = vec![
        "Tokens, passwords, DPAPI ciphertexts, and secret names",
        "Draft text, request bodies, headers, and query strings",
        "Filesystem paths, Windows account names, and environment variables",
        "Collaboration URIs, database URIs, and server credentials",
    ];
    let mut rendered = String::from("Swaggen Diagnostic Report Preview (v1)\n\n[INCLUDE]\n");
    for (k, v) in &include_lines {
        rendered.push_str(&format!("- {k}: {v}\n"));
    }
    rendered.push_str("\n[EXCLUDE]\n");
    for item in &exclude_lines {
        rendered.push_str(&format!("- {item}\n"));
    }
    DiagnosticPreview {
        include_lines,
        exclude_lines,
        rendered_text: rendered,
    }
}

pub fn preview(loaded: bool, pending: bool, recovery_needed: bool) -> String {
    let ws = LocalId::parse("personal").expect("static workspace id");
    build_preview(
        &ws,
        loaded,
        pending,
        recovery_needed,
        0,
        0,
        &DesktopPolicy::default(),
    )
    .rendered_text
}

/// Exports the reviewed diagnostic preview to a safe, non-symlink file inside
/// the validated export directory. Never overwrites existing files.
pub fn export_reviewed_bundle(
    export_dir: &Path,
    preview: &DiagnosticPreview,
) -> Result<PathBuf, Error> {
    if !export_dir.is_absolute() {
        return Err(Error::Storage);
    }
    reject_links(export_dir)?;
    fs::create_dir_all(export_dir).map_err(|_| Error::Storage)?;
    reject_links(export_dir)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| Error::Storage)?
        .as_nanos();
    let target = export_dir.join(format!(
        "swaggen-diagnostics-{}-{stamp}.txt",
        std::process::id()
    ));
    reject_links(&target)?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&target)
        .map_err(|_| Error::Storage)?;
    file.write_all(preview.rendered_text.as_bytes())
        .map_err(|_| Error::Storage)?;
    file.sync_all().map_err(|_| Error::Storage)?;
    Ok(target)
}

fn reject_links(path: &Path) -> Result<(), Error> {
    for p in path.ancestors() {
        match fs::symlink_metadata(p) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    return Err(Error::Storage);
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if meta.file_attributes() & 0x400 != 0 {
                        return Err(Error::Storage);
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(Error::Storage),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_preview_contains_no_upload_and_exports_safely() {
        let ws = LocalId::parse("personal").unwrap();
        let preview = build_preview(&ws, true, false, false, 3, 2, &DesktopPolicy::default());
        assert!(preview.rendered_text.contains("Analytics enabled: false"));
        assert!(
            preview
                .rendered_text
                .contains("Crash upload enabled: false")
        );
        assert!(preview.rendered_text.contains("No automatic upload"));
        assert!(preview.rendered_text.contains("[EXCLUDE]"));
        let root = std::env::temp_dir().join(format!(
            "swaggen-diag-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = export_reviewed_bundle(&root, &preview).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), preview.rendered_text);
        fs::remove_dir_all(root).unwrap();
    }
}

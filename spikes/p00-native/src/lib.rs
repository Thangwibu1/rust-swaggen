//! P00-only framework and bounded HTTP experiments, not product code.
pub use swaggen_p00_transport as transport;

/// Keep spike-generated data inside the project, regardless of launcher directory.
pub fn fixture_directory() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.local/p00-native")
}

pub const EDITOR_LIMIT: usize = 128 * 1024;
const HISTORY_LIMIT: usize = 16;

#[derive(Default)]
pub struct History {
    undo: std::collections::VecDeque<String>,
    redo: Vec<String>,
}

impl History {
    pub fn remember(&mut self, before: String) {
        if self.undo.len() == HISTORY_LIMIT {
            self.undo.pop_front();
        }
        self.undo.push_back(before);
        self.redo.clear();
    }
    pub fn undo(&mut self, current: String) -> Option<String> {
        let old = self.undo.pop_back()?;
        self.redo.push(current);
        Some(old)
    }
    pub fn redo(&mut self, current: String) -> Option<String> {
        let next = self.redo.pop()?;
        self.undo.push_back(current);
        Some(next)
    }
}

pub fn visible_range(offset: f32, height: f32, count: usize) -> std::ops::Range<usize> {
    let start = (offset.max(0.0) / 32.0).floor() as usize;
    let start = start.min(count);
    start..(start + (height.max(0.0) / 32.0).ceil() as usize + 3).min(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_preserves_unicode_and_clears_redo_on_branch() {
        let mut h = History::default();
        h.remember("Tiếng Việt".into());
        assert_eq!(h.undo("API".into()).as_deref(), Some("Tiếng Việt"));
        assert_eq!(h.redo("Tiếng Việt".into()).as_deref(), Some("API"));
        assert_eq!(h.undo("API".into()).as_deref(), Some("Tiếng Việt"));
        h.remember("changed".into());
        assert!(h.redo("branch".into()).is_none());
    }
    #[test]
    fn history_has_fixed_capacity() {
        let mut h = History::default();
        for i in 0..100 {
            h.remember(i.to_string());
        }
        assert_eq!(h.undo.len(), HISTORY_LIMIT);
    }
    #[test]
    fn virtualization_is_bounded_and_reaches_last_row() {
        assert_eq!(visible_range(0.0, 640.0, 1000), 0..23);
        assert_eq!(visible_range(31936.0, 640.0, 1000), 998..1000);
        assert_eq!(visible_range(-1.0, 640.0, 0), 0..0);
    }
}

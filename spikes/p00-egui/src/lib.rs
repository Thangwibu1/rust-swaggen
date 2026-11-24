//! Bounded, fixture-only editor helpers; not an OpenAPI/JSON/YAML validator.
use eframe::egui::{self, Color32, FontId, TextFormat, text::LayoutJob};
use std::ops::Range;

pub struct Splitter {
    pub width: f32,
    active: bool,
}

impl Default for Splitter {
    fn default() -> Self {
        Self {
            width: 250.0,
            active: false,
        }
    }
}

impl Splitter {
    /// Process ordered native pointer events, including a complete drag batched
    /// into one frame. Never depend on a previous-frame widget response.
    pub fn apply(&mut self, events: &[egui::Event], top: f32, maximum: f32) {
        for event in events {
            match event {
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    ..
                } if (pos.x - self.width).abs() <= 8.0 && pos.y >= top => self.active = true,
                egui::Event::PointerMoved(pos) if self.active => {
                    self.width = pos.x.clamp(170.0, maximum.max(170.0))
                }
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    ..
                } if self.active => {
                    self.width = pos.x.clamp(170.0, maximum.max(170.0));
                    self.active = false;
                }
                egui::Event::PointerGone => self.active = false,
                _ => {}
            }
        }
    }
}

pub fn matches_in_chars(text: &str, query: &str) -> Vec<Range<usize>> {
    if query.is_empty() {
        return Vec::new();
    }
    let mut previous_byte = 0;
    let mut previous_chars = 0;
    text.match_indices(query)
        .map(|(start, found)| {
            previous_chars += text[previous_byte..start].chars().count();
            previous_byte = start;
            previous_chars..previous_chars + found.chars().count()
        })
        .collect()
}

/// Small JSON/YAML lexical display only. Never alters text or asserts validity.
pub fn highlight(text: &str) -> LayoutJob {
    let mut job = LayoutJob::default();
    let mut at = 0;
    while at < text.len() {
        let begin = at;
        let first = text[at..].chars().next().unwrap();
        at += first.len_utf8();
        let color = if first == '"' || first == '\'' {
            let mut escaped = false;
            for next in text[at..].chars() {
                at += next.len_utf8();
                if next == first && !escaped {
                    break;
                }
                escaped = next == '\\' && !escaped;
            }
            Color32::from_rgb(0, 105, 65)
        } else if first == '#' {
            while at < text.len() && text.as_bytes()[at] != b'\n' {
                at += text[at..].chars().next().unwrap().len_utf8();
            }
            Color32::from_gray(100)
        } else if first.is_ascii_digit() || first == '-' {
            while at < text.len()
                && (text.as_bytes()[at].is_ascii_alphanumeric()
                    || b".+-".contains(&text.as_bytes()[at]))
            {
                at += 1;
            }
            Color32::from_rgb(125, 55, 145)
        } else if "{}[]:,".contains(first) {
            Color32::from_rgb(30, 75, 165)
        } else {
            Color32::from_gray(30)
        };
        job.append(
            &text[begin..at],
            0.0,
            TextFormat {
                font_id: FontId::monospace(15.0),
                color,
                ..Default::default()
            },
        );
    }
    job
}

#[derive(Default)]
pub struct HighlightCache {
    source: String,
    job: LayoutJob,
}

impl HighlightCache {
    pub fn layout(
        &mut self,
        ui: &egui::Ui,
        text: &str,
        width: f32,
    ) -> std::sync::Arc<egui::Galley> {
        if self.source != text {
            self.source = text.into();
            self.job = highlight(text);
        }
        let mut job = self.job.clone();
        job.wrap.max_width = width;
        ui.fonts_mut(|fonts| fonts.layout_job(job))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::epaint::text::ByteRangeExt as _;

    fn pointer(x: f32, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos: egui::pos2(x, 300.0),
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    #[test]
    fn complete_drag_in_one_frame_retains_released_width() {
        let mut splitter = Splitter::default();
        splitter.apply(
            &[
                pointer(250.0, true),
                egui::Event::PointerMoved(egui::pos2(420.0, 300.0)),
                pointer(420.0, false),
            ],
            130.0,
            700.0,
        );
        assert_eq!(splitter.width, 420.0);
        splitter.apply(
            &[egui::Event::PointerMoved(egui::pos2(500.0, 300.0))],
            130.0,
            700.0,
        );
        assert_eq!(splitter.width, 420.0);
    }

    #[test]
    fn unrelated_click_and_capture_loss_do_not_resize() {
        let mut splitter = Splitter::default();
        splitter.apply(&[pointer(100.0, true), pointer(500.0, false)], 130.0, 700.0);
        assert_eq!(splitter.width, 250.0);
        splitter.apply(
            &[
                pointer(250.0, true),
                egui::Event::PointerGone,
                egui::Event::PointerMoved(egui::pos2(500.0, 300.0)),
            ],
            130.0,
            700.0,
        );
        assert_eq!(splitter.width, 250.0);
    }

    #[test]
    fn unicode_search_returns_character_not_byte_offsets() {
        assert_eq!(
            matches_in_chars("đ🙂 Việt, Việt", "Việt"),
            vec![3..7, 9..13]
        );
        assert!(matches_in_chars("Việt", "").is_empty());
        assert!(matches_in_chars("Việt", "viet").is_empty());
    }

    #[test]
    fn highlighting_preserves_every_unicode_byte_and_complete_coverage() {
        for fixture in [
            "{\"Việt\":\"escaped \\\" #\",\"n\":1.3e-2}",
            "title: 'Tiếng Việt 🙂' # comment\npaths: {}\n",
            "\"unfinished đ🙂\\",
            "",
        ] {
            let job = highlight(fixture);
            let recovered: String = job
                .sections
                .iter()
                .map(|s| &job.text[s.byte_range.as_usize()])
                .collect();
            assert_eq!(recovered, fixture);
            assert_eq!(job.text, fixture);
        }
    }

    #[test]
    fn quoted_hash_does_not_consume_following_yaml_tokens() {
        let job = highlight("'a#b': 42 # tail");
        assert_eq!(&job.text[job.sections[0].byte_range.as_usize()], "'a#b'");
        assert!(
            job.sections
                .iter()
                .any(|s| &job.text[s.byte_range.as_usize()] == "42")
        );
        assert_eq!(
            &job.text[job.sections.last().unwrap().byte_range.as_usize()],
            "# tail"
        );
    }
}

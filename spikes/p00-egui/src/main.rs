#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
use eframe::egui;
use std::{path::PathBuf, sync::mpsc};
use swaggen_p00::journal::{Batch, Journal};
use swaggen_p00_egui::{HighlightCache, Splitter, matches_in_chars};
use swaggen_p00_transport as transport;

const JSON: &str =
    "{\n  \"name\": \"Swaggen — Tiếng Việt\",\n  \"fixture\": true,\n  \"requests\": 1000\n}";
const YAML: &str =
    "openapi: 3.1.0\ninfo:\n  title: Swaggen — Tiếng Việt\n  version: 0.0.0\npaths: {}\n";
enum Job {
    Save(String),
    Restore,
    Download,
}
enum Completed {
    Saved,
    Restored(String),
    Downloaded(transport::Transfer),
}
fn directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.local/p00-egui")
}
fn work(job: Job) -> Result<Completed, String> {
    let directory = directory();
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    match job {
        Job::Save(draft) => {
            let (mut journal, _) =
                Journal::open(&directory.join("fixture.journal")).map_err(|e| e.to_string())?;
            journal
                .append(&Batch {
                    draft: draft.into_bytes(),
                    outbox: Vec::new(),
                })
                .map_err(|e| e.to_string())?;
            Ok(Completed::Saved)
        }
        Job::Restore => {
            if !directory.join("fixture.journal").exists() {
                return Err("No saved fixture exists".into());
            }
            let (_, batches) =
                Journal::open(&directory.join("fixture.journal")).map_err(|e| e.to_string())?;
            let last = batches.last().ok_or("No saved fixture exists")?;
            let draft =
                String::from_utf8(last.draft.clone()).map_err(|_| "Fixture is not UTF-8")?;
            if draft.chars().count() > 65_536 {
                return Err("Saved fixture exceeds editor limit".into());
            }
            Ok(Completed::Restored(draft))
        }
        Job::Download => {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .map_err(|e| e.to_string())?;
            runtime
                .block_on(transport::run(directory))
                .map(Completed::Downloaded)
        }
    }
}
struct Probe {
    draft: String,
    search: String,
    cached_draft: String,
    cached_search: String,
    matches: Vec<std::ops::Range<usize>>,
    match_index: usize,
    choose_match: bool,
    selected: usize,
    scroll_to: Option<usize>,
    rendered_rows: usize,
    modal: bool,
    focus_dialog: bool,
    focus_editor: bool,
    focus_search: bool,
    highlight: HighlightCache,
    worker: Option<mpsc::Receiver<Result<Completed, String>>>,
    status: String,
    transfer: Option<transport::Transfer>,
    splitter: Splitter,
    splitter_top: f32,
}
impl Default for Probe {
    fn default() -> Self {
        Self {
            draft: JSON.into(),
            search: String::new(),
            cached_draft: String::new(),
            cached_search: String::new(),
            matches: Vec::new(),
            match_index: 0,
            choose_match: false,
            selected: 1,
            scroll_to: None,
            rendered_rows: 0,
            modal: false,
            focus_dialog: false,
            focus_editor: false,
            focus_search: false,
            highlight: HighlightCache::default(),
            worker: None,
            status: "Isolated P00 fixture; no production data or sync".into(),
            transfer: None,
            splitter: Splitter::default(),
            splitter_top: 130.0,
        }
    }
}
impl Probe {
    fn start(&mut self, ctx: &egui::Context, job: Job) {
        if self.worker.is_some() {
            return;
        }
        let (send, receive) = mpsc::sync_channel(1);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = work(job);
            let _ = send.send(result);
            ctx.request_repaint();
        });
        self.worker = Some(receive);
        self.status = "Working in background; editor and dialog remain available".into();
    }
    fn poll(&mut self) {
        let result = match self.worker.as_ref().map(|r| r.try_recv()) {
            Some(Ok(result)) => result,
            Some(Err(mpsc::TryRecvError::Disconnected)) => {
                Err("Fixture worker disconnected".into())
            }
            _ => return,
        };
        self.worker = None;
        match result {
            Ok(Completed::Saved) => {
                self.status = "Fixture draft saved after sync_all; empty outbox".into()
            }
            Ok(Completed::Restored(draft)) => {
                self.draft = draft;
                self.status = "Restored actual saved fixture from journal".into();
            }
            Ok(Completed::Downloaded(transfer)) => {
                self.status = format!(
                    "Transferred {} bytes; preview {} bytes; {} ms (fixture, not benchmark)",
                    transfer.bytes,
                    transfer.preview.len(),
                    transfer.milliseconds
                );
                self.transfer = Some(transfer);
            }
            Err(error) => self.status = format!("Fixture error: {error}; draft retained"),
        }
    }
    fn refresh_search(&mut self) {
        if self.cached_draft != self.draft || self.cached_search != self.search {
            self.matches = matches_in_chars(&self.draft, &self.search);
            self.cached_draft.clone_from(&self.draft);
            self.cached_search.clone_from(&self.search);
            self.match_index = 0;
        }
    }
    fn dialog(&mut self, ctx: &egui::Context) {
        let dialog = egui::Modal::new(egui::Id::unique("fixture-dialog")).show(ctx, |ui| {
            ui.heading("Native dialog fixture");
            ui.label("Background controls are excluded. Tab stays in this dialog.");
            let close = ui.button("Return to editor");
            if self.focus_dialog {
                close.request_focus();
                self.focus_dialog = false;
            }
            if close.clicked() {
                ui.close();
            }
        });
        if dialog.should_close() {
            self.modal = false;
            self.focus_editor = true;
            ctx.request_repaint();
        }
    }
}
impl eframe::App for Probe {
    fn ui(&mut self, root: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.poll();
        let ctx = root.ctx().clone();
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::P)) {
            self.modal = true;
            self.focus_dialog = true;
        }
        if self.modal {
            self.dialog(&ctx);
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::F)) {
            self.focus_search = true;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::S)) {
            self.start(&ctx, Job::Save(self.draft.clone()));
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::ALT, egui::Key::ArrowDown)) {
            self.selected = (self.selected + 1).min(1000);
            self.scroll_to = Some(self.selected);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::ALT, egui::Key::ArrowUp)) {
            self.selected = self.selected.saturating_sub(1).max(1);
            self.scroll_to = Some(self.selected);
        }
        let previous = ctx.input_mut(|i| i.consume_key(egui::Modifiers::SHIFT, egui::Key::F3));
        let next = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F3));
        self.refresh_search();
        let events = ctx.input(|input| input.events.clone());
        self.splitter.apply(
            &events,
            self.splitter_top,
            (ctx.content_rect().width() * 0.5).min(700.0),
        );

        let header = egui::Panel::top("header").show(root, |ui| {
            ui.heading("Swaggen · native framework gate");
            ui.label("P00 fixtures only · no remote services, accounts or production data");
            ui.horizontal_wrapped(|ui| {
                if ui.button("Load JSON").clicked() {
                    self.draft = JSON.into();
                }
                if ui.button("Load YAML").clicked() {
                    self.draft = YAML.into();
                }
                if ui.button("Dialog (Ctrl+P)").clicked() {
                    self.modal = true;
                    self.focus_dialog = true;
                    ctx.request_repaint();
                }
                ui.add_enabled_ui(self.worker.is_none(), |ui| {
                    if ui.button("Save fixture (Ctrl+S)").clicked() {
                        self.start(&ctx, Job::Save(self.draft.clone()));
                    }
                    if ui.button("Restore fixture").clicked() {
                        self.start(&ctx, Job::Restore);
                    }
                    if ui.button("Stream 20 MiB fixture").clicked() {
                        self.start(&ctx, Job::Download);
                    }
                });
            });
            ui.label(&self.status);
            ui.label(format!(
                "Viewport {:.0}×{:.0} points · {:.2} pixels/point · previous frame CPU {:.2} ms",
                ctx.content_rect().width(),
                ctx.content_rect().height(),
                ctx.pixels_per_point(),
                frame.info().cpu_usage.unwrap_or(0.0) * 1000.0
            ));
        });
        self.splitter_top = header.response.rect.bottom();
        let panel = egui::Panel::left("requests")
            .exact_size(self.splitter.width)
            .resizable(false)
            .show(root, |ui| {
                // Keep the allocated width. Otherwise Frame shrinks back to the
                // text's natural width after every resize gesture.
                ui.set_min_width(ui.available_width());
                ui.heading("1,000 fixture requests");
                ui.horizontal(|ui| {
                    let label = ui.label("Request number");
                    if ui
                        .add(egui::DragValue::new(&mut self.selected).range(1..=1000))
                        .labelled_by(label.id)
                        .changed()
                    {
                        self.scroll_to = Some(self.selected);
                    }
                });
                ui.label("Alt+Up / Alt+Down select");
                ui.label(format!(
                    "Selected: {} · drawn: {}",
                    self.selected, self.rendered_rows
                ));
                let height = ui.text_style_height(&egui::TextStyle::Button);
                let mut scroll = egui::ScrollArea::vertical();
                if let Some(row) = self.scroll_to.take() {
                    scroll = scroll.vertical_scroll_offset(
                        (row - 1) as f32 * (height + ui.spacing().item_spacing.y),
                    );
                }
                scroll.show_rows(ui, height, 1_000, |ui, rows| {
                    self.rendered_rows = rows.len();
                    for row in rows {
                        if ui
                            .selectable_label(
                                self.selected == row + 1,
                                format!("GET /fixture/{}", row + 1),
                            )
                            .clicked()
                        {
                            self.selected = row + 1;
                        }
                    }
                });
            });
        egui::CentralPanel::default().show(root, |ui| {
            ui.label(format!(
                "Request pane width: {:.0} points; drag separator to resize",
                panel.response.rect.width()
            ));
            ui.horizontal_wrapped(|ui| {
                let label = ui.label("Find text");
                let search = ui
                    .add(
                        egui::TextEdit::singleline(&mut self.search)
                            .id_salt("search")
                            .char_limit(256)
                            .desired_width(220.0),
                    )
                    .labelled_by(label.id);
                if self.focus_search {
                    search.request_focus();
                    self.focus_search = false;
                }
                if (ui.button("Previous (Shift+F3)").clicked() || previous)
                    && !self.matches.is_empty()
                {
                    self.match_index =
                        (self.match_index + self.matches.len() - 1) % self.matches.len();
                    self.choose_match = true;
                }
                if (ui.button("Next (F3)").clicked() || next) && !self.matches.is_empty() {
                    self.match_index = (self.match_index + 1) % self.matches.len();
                    self.choose_match = true;
                }
                ui.label(format!("{} matches", self.matches.len()));
            });
            let label = ui.label("Request body — JSON/YAML lexical highlighting; Tab moves focus");
            // Horizontal scrolling supplies infinite available width. A finite editor
            // width is required for valid native accessibility bounding rectangles.
            let width = ui.available_width().max(32.0);
            let highlight = &mut self.highlight;
            let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, width| {
                highlight.layout(ui, text.as_str(), width)
            };
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let mut output = egui::TextEdit::multiline(&mut self.draft)
                        .id_salt("body")
                        .code_editor()
                        .lock_focus(false)
                        .char_limit(65_536)
                        .desired_width(width)
                        .desired_rows(18)
                        .layouter(&mut layouter)
                        .show(ui);
                    let response = output.response.response.clone().labelled_by(label.id);
                    if self.focus_editor {
                        response.request_focus();
                        self.focus_editor = false;
                    }
                    if self.choose_match {
                        if let Some(range) = self.matches.get(self.match_index) {
                            output.state.cursor.set_char_range(Some(
                                egui::text::CCursorRange::two(
                                    egui::text::CCursor::new(range.start),
                                    egui::text::CCursor::new(range.end),
                                ),
                            ));
                            output.state.store(&ctx, response.id);
                            response.request_focus();
                            ctx.request_repaint();
                        }
                        self.choose_match = false;
                    }
                    if let Some(transfer) = &self.transfer {
                        ui.label(format!("Raw fixture file: {}", transfer.path.display()));
                        ui.label(format!(
                            "Preview buffer capped at {} bytes",
                            transport::PREVIEW_CAP
                        ));
                        ui.collapsing("First 1 KiB preview", |ui| {
                            ui.monospace(&transfer.preview[..transfer.preview.len().min(1024)]);
                        });
                    }
                });
        });
        let edge = egui::Rect::from_min_max(
            egui::pos2(self.splitter.width - 5.0, self.splitter_top),
            egui::pos2(self.splitter.width + 5.0, ctx.content_rect().bottom()),
        );
        let handle = root.interact(
            edge,
            egui::Id::unique("native-splitter"),
            egui::Sense::click_and_drag(),
        );
        handle.widget_info(|| {
            egui::WidgetInfo::labeled(egui::Role::Splitter, true, "Resize request pane")
        });
        if handle.hovered() {
            ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
        self.refresh_search();
    }
}
fn main() -> eframe::Result {
    eframe::run_native(
        "Swaggen P00 - egui Accessibility Comparison",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1440.0, 900.0])
                .with_min_inner_size([720.0, 480.0]),
            renderer: eframe::Renderer::Wgpu,
            ..Default::default()
        },
        Box::new(|cc| {
            cc.egui_ctx.enable_accesskit();
            let mut visuals = egui::Visuals::light();
            visuals.panel_fill = egui::Color32::WHITE;
            visuals.window_fill = egui::Color32::WHITE;
            visuals.extreme_bg_color = egui::Color32::WHITE;
            visuals.widgets.inactive.weak_bg_fill = egui::Color32::WHITE;
            cc.egui_ctx.set_visuals(visuals);
            Ok(Box::new(Probe::default()))
        }),
    )
}

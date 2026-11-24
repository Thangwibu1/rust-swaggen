#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
use iced::{
    Element, Length, Subscription, Task, Theme,
    widget::{button, column, container, row, scrollable, space, text, text_editor, text_input},
};
use swaggen_p00_native::{EDITOR_LIMIT, History, transport, visible_range};

const JSON: &str = "{\n  \"name\": \"Swaggen P00 — Tiếng Việt\",\n  \"fixture\": true,\n  \"requests\": 1000,\n  \"note\": \"Native compatibility experiment, not product UI\"\n}";
const YAML: &str =
    "openapi: 3.1.0\ninfo:\n  title: Swaggen — Tiếng Việt\n  version: 0.0.0\npaths: {}\n";

fn gate_button<'a>(content: impl Into<Element<'a, Message>>) -> iced::widget::Button<'a, Message> {
    use iced::widget::button as make_button;
    make_button(content).style(|theme, status| {
        let mut style = button::text(theme, status);
        style.background = Some(iced::Color::WHITE.into());
        style.border = iced::Border {
            color: iced::Color::from_rgb8(205, 210, 218),
            width: 1.0,
            radius: 3.0.into(),
        };
        style
    })
}

struct State {
    editor: text_editor::Content,
    history: History,
    syntax: &'static str,
    search: String,
    matches: usize,
    offset: f32,
    viewport: f32,
    selected: usize,
    status: String,
    response: Option<transport::Transfer>,
    busy: bool,
    saving: bool,
    scale: f32,
    modal: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            editor: text_editor::Content::with_text(JSON),
            history: History::default(),
            syntax: "json",
            search: String::new(),
            matches: 0,
            offset: 0.0,
            viewport: 640.0,
            selected: 0,
            status: "P00 fixture only — no production services or sync claims".into(),
            response: None,
            busy: false,
            saving: false,
            scale: 1.0,
            modal: false,
        }
    }
}

#[derive(Clone, Debug)]
enum Message {
    Edit(text_editor::Action),
    Search(String),
    Find,
    Undo,
    Redo,
    Save,
    Saved(Result<(), String>),
    Select(usize),
    Scroll(f32, f32),
    Load(&'static str),
    Send,
    Transferred(Result<transport::Transfer, String>),
    Scale(f32),
    Modal(bool),
}

fn main() -> iced::Result {
    iced::application(State::default, update, view)
        .title("Swaggen P00 — Native Framework Gate")
        .theme(Theme::Light)
        .window_size((1440.0, 900.0))
        .centered()
        .scale_factor(|s: &State| s.scale)
        .subscription(subscription)
        .run()
}

fn shortcut(key: &iced::keyboard::Key, modifiers: iced::keyboard::Modifiers) -> Option<Message> {
    use iced::keyboard::{Key, key::Named};
    if !modifiers.control() {
        return None;
    }
    match key.as_ref() {
        Key::Character("z") if modifiers.shift() => Some(Message::Redo),
        Key::Character("z") => Some(Message::Undo),
        Key::Character("y") => Some(Message::Redo),
        Key::Character("s") => Some(Message::Save),
        Key::Character("p") => Some(Message::Modal(true)),
        Key::Character("f") => Some(Message::Find),
        Key::Named(Named::Enter) => Some(Message::Send),
        _ => None,
    }
}

fn subscription(_: &State) -> Subscription<Message> {
    iced::event::listen_with(|event, status, _| {
        if status == iced::event::Status::Captured {
            return None;
        }
        match event {
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                if key == iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) {
                    Some(Message::Modal(false))
                } else {
                    shortcut(&key, modifiers)
                }
            }
            _ => None,
        }
    })
}

fn update(state: &mut State, message: Message) -> Task<Message> {
    if state.modal
        && !matches!(
            message,
            Message::Modal(_) | Message::Saved(_) | Message::Transferred(_)
        )
    {
        return Task::none();
    }
    match message {
        Message::Edit(action) => {
            if action.is_edit() {
                let before = state.editor.text();
                state.editor.perform(action);
                if state.editor.text().len() > EDITOR_LIMIT {
                    state.editor = text_editor::Content::with_text(&before);
                    state.status =
                        "Editor spike limit 128KiB; edit rejected without losing draft".into();
                } else {
                    state.history.remember(before);
                }
            } else {
                state.editor.perform(action);
            }
        }
        Message::Search(query) => state.search = query,
        Message::Find => {
            state.matches = if state.search.is_empty() {
                0
            } else {
                state.editor.text().matches(&state.search).count()
            };
            state.status = format!(
                "Search: {} exact matches (spike count only; no folding)",
                state.matches
            );
        }
        Message::Undo => {
            if let Some(value) = state.history.undo(state.editor.text()) {
                state.editor = text_editor::Content::with_text(&value);
            }
        }
        Message::Redo => {
            if let Some(value) = state.history.redo(state.editor.text()) {
                state.editor = text_editor::Content::with_text(&value);
            }
        }
        Message::Load(syntax) => {
            state.history.remember(state.editor.text());
            state.syntax = syntax;
            state.editor =
                text_editor::Content::with_text(if syntax == "yaml" { YAML } else { JSON });
        }
        Message::Scroll(offset, height) => {
            state.offset = offset;
            state.viewport = height;
        }
        Message::Select(id) => {
            state.selected = id;
            state.status = format!("Selected fixture request {:04}", id + 1);
        }
        Message::Scale(scale) => state.scale = scale,
        Message::Modal(open) => state.modal = open,
        Message::Save if !state.saving => {
            state.saving = true;
            let draft = state.editor.text().into_bytes();
            state.status = "Saving fixture draft on worker...".into();
            return Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        let folder = swaggen_p00_native::fixture_directory();
                        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
                        let (mut journal, _) =
                            swaggen_p00::journal::Journal::open(&folder.join("editor.journal"))
                                .map_err(|e| e.to_string())?;
                        journal
                            .append(&swaggen_p00::journal::Batch {
                                draft,
                                outbox: Vec::new(),
                            })
                            .map_err(|e| e.to_string())?;
                        Ok(())
                    })
                    .await
                    .map_err(|e| e.to_string())?
                },
                Message::Saved,
            );
        }
        Message::Save => {}
        Message::Saved(result) => {
            state.saving = false;
            state.status = match result {
                Ok(()) => "Fixture draft synced to journal (not published)".into(),
                Err(e) => format!("Save failed; draft retained: {e}"),
            }
        }
        Message::Send if !state.busy => {
            state.busy = true;
            state.status = "Streaming real 20MiB loopback fixture on worker...".into();
            return Task::perform(
                async { transport::run(swaggen_p00_native::fixture_directory()).await },
                Message::Transferred,
            );
        }
        Message::Send => {}
        Message::Transferred(result) => {
            state.busy = false;
            match result {
                Ok(result) => {
                    state.status = format!(
                        "{} bytes verified to file; preview capped at {} bytes; one transfer {}ms",
                        result.bytes,
                        result.preview.len(),
                        result.milliseconds
                    );
                    state.response = Some(result);
                }
                Err(e) => state.status = format!("Fixture transfer failed: {e}"),
            }
        }
    }
    Task::none()
}

fn view(state: &State) -> Element<'_, Message> {
    if state.modal {
        // Replaces the underlying widget tree: background controls cannot receive focus.
        return container(
            column![
                text("P00 focus isolation dialog").size(24),
                text("Only this close button is in the active widget tree. Escape closes."),
                gate_button("Close dialog").on_press(Message::Modal(false))
            ]
            .spacing(20),
        )
        .center(Length::Fill)
        .into();
    }
    let compact = state.scale >= 1.5;
    let range = visible_range(state.offset, state.viewport, 1000);
    let mut rows = column![space().height(range.start as f32 * 32.0)];
    for id in range.clone() {
        rows = rows.push(
            gate_button(
                text(format!(
                    "{} GET  /fixture/{:04}",
                    if id == state.selected { ">" } else { " " },
                    id + 1
                ))
                .size(14),
            )
            .height(32)
            .width(Length::Fill)
            .on_press(Message::Select(id))
            .style(button::text),
        );
    }
    rows = rows.push(space().height((1000 - range.end) as f32 * 32.0));
    let sidebar = column![
        text("1,000 fixture requests").size(if compact { 16 } else { 18 }),
        text(format!("{} rows rendered", range.len())).size(12),
        scrollable(rows)
            .height(Length::Fill)
            .on_scroll(|v| Message::Scroll(v.absolute_offset().y, v.bounds().height))
    ]
    .spacing(10)
    .width(if compact { 200 } else { 290 });
    let editor = text_editor(&state.editor)
        .height(Length::Fill)
        .font(iced::Font::MONOSPACE)
        .size(15)
        .highlight(state.syntax, iced::highlighter::Theme::InspiredGitHub)
        .key_binding(|press| {
            if let Some(message) = shortcut(&press.key, press.modifiers) {
                Some(text_editor::Binding::Custom(message))
            } else {
                text_editor::Binding::from_key_press(press)
            }
        })
        .on_action(Message::Edit);
    let preview = if compact {
        state.response.as_ref().map_or_else(
            || "No response yet — controlled HTTP fixture".into(),
            |result| {
                format!(
                    "{} bytes to file\nPreview: {} bytes",
                    result.bytes,
                    result.preview.len()
                )
            },
        )
    } else if let Some(result) = &state.response {
        format!(
            "Raw bytes: {}\nIn-memory preview: {} bytes\nFirst 160 bytes:\n{}",
            result.bytes,
            result.preview.len(),
            &result.preview[..160.min(result.preview.len())]
        )
    } else {
        "No response yet. Send runs a real local Axum HTTP fixture.\nRaw body goes to file; in-memory preview is bounded.\nNo remote service, account or synchronization is simulated.".into()
    };
    let main = column![
        row![
            gate_button("JSON").on_press(Message::Load("json")),
            gate_button("YAML").on_press(Message::Load("yaml")),
            gate_button("Undo").on_press(Message::Undo),
            gate_button("Redo").on_press(Message::Redo),
            gate_button(if compact { "Save" } else { "Save / Ctrl+S" }).on_press(Message::Save),
            gate_button(if state.busy {
                "Streaming..."
            } else if compact {
                "Send"
            } else {
                "Send 20MiB / Ctrl+Enter"
            })
            .on_press(Message::Send)
        ]
        .spacing(8)
        .wrap(),
        row![
            text_input("Find exact text", &state.search)
                .on_input(Message::Search)
                .on_submit(Message::Find),
            gate_button("Find").on_press(Message::Find),
            text(format!("{} matches", state.matches))
        ]
        .spacing(8),
        editor,
        container(text(preview).font(iced::Font::MONOSPACE).size(13))
            .padding(14)
            .height(if compact { 70 } else { 165 }),
    ]
    .spacing(if compact { 8 } else { 12 })
    .width(Length::Fill);
    let description: Element<'_, Message> = if compact {
        space().height(0).into()
    } else {
        text("Compatibility fixture — 128KiB editor, bounded undo; not a product screen")
            .size(13)
            .into()
    };
    container(
        column![
            row![
                text("Swaggen / P00 native gate").size(if compact { 20 } else { 25 }),
                space().width(Length::Fill),
                gate_button(if compact {
                    "Dialog / Ctrl+P"
                } else {
                    "Focus dialog / Ctrl+P"
                })
                .on_press(Message::Modal(true))
            ]
            .spacing(10),
            description,
            row![
                gate_button("100%").on_press(Message::Scale(1.0)),
                gate_button("125%").on_press(Message::Scale(1.25)),
                gate_button("150%").on_press(Message::Scale(1.5)),
                gate_button("200%").on_press(Message::Scale(2.0)),
                text(if compact {
                    "App scale only"
                } else {
                    "Application scale only; does not certify OS per-monitor DPI"
                })
            ]
            .spacing(8),
            row![sidebar, main].spacing(20).height(Length::Fill),
            text(&state.status).size(13),
        ]
        .spacing(if compact { 8 } else { 14 }),
    )
    .padding(if compact { 12 } else { 20 })
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

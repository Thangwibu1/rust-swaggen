//! Shared white-surface design tokens and reusable native components (51:40541, 4:139, 4:120).
//! No IO, telemetry, or remote rendering.
use eframe::egui;

pub struct Tokens;

impl Tokens {
    pub const BG_PAGE: egui::Color32 = egui::Color32::from_rgb(0xFB, 0xFC, 0xFA);
    pub const BG_SURFACE: egui::Color32 = egui::Color32::WHITE;
    pub const TEXT_PRIMARY: egui::Color32 = egui::Color32::from_rgb(0x20, 0x37, 0x35);
    pub const TEXT_SECONDARY: egui::Color32 = egui::Color32::from_rgb(0x60, 0x75, 0x72);
    pub const TEXT_BRAND: egui::Color32 = egui::Color32::from_rgb(0x24, 0x5C, 0x59);
    pub const TEXT_DANGER: egui::Color32 = egui::Color32::from_rgb(0xA2, 0x3F, 0x3F);
    pub const TEXT_WARNING: egui::Color32 = egui::Color32::from_rgb(0x7B, 0x5B, 0x1C);
    pub const STATUS_SUCCESS: egui::Color32 = egui::Color32::from_rgb(0x28, 0x66, 0x49);
    pub const STATUS_INFO: egui::Color32 = egui::Color32::from_rgb(0x34, 0x5D, 0x8E);
    pub const BORDER_DEFAULT: egui::Color32 = egui::Color32::from_rgb(0xDC, 0xE6, 0xE1);
    pub const BORDER_FOCUS: egui::Color32 = egui::Color32::from_rgb(0x24, 0x5C, 0x59);

    pub const METHOD_GET: egui::Color32 = egui::Color32::from_rgb(0x28, 0x66, 0x49);
    pub const METHOD_POST: egui::Color32 = egui::Color32::from_rgb(0xA8, 0x55, 0x0A);
    pub const METHOD_PUT: egui::Color32 = egui::Color32::from_rgb(0x26, 0x59, 0xA8);
    pub const METHOD_PATCH: egui::Color32 = egui::Color32::from_rgb(0x6D, 0x3B, 0xB8);
    pub const METHOD_DELETE: egui::Color32 = egui::Color32::from_rgb(0xA2, 0x3F, 0x3F);
    pub const METHOD_HEAD: egui::Color32 = egui::Color32::from_rgb(0x24, 0x5C, 0x59);
    pub const METHOD_OPTIONS: egui::Color32 = egui::Color32::from_rgb(0x9E, 0x2A, 0x6B);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeKind {
    Info,
    Success,
    Warning,
    Danger,
}

impl NoticeKind {
    pub fn color(self) -> egui::Color32 {
        match self {
            Self::Info => Tokens::STATUS_INFO,
            Self::Success => Tokens::STATUS_SUCCESS,
            Self::Warning => Tokens::TEXT_WARNING,
            Self::Danger => Tokens::TEXT_DANGER,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Success => "OK",
            Self::Warning => "WARN",
            Self::Danger => "ERROR",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiTheme {
    #[default]
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiLocale {
    #[default]
    En,
    Vi,
}

pub fn tr(locale: UiLocale, en: &'static str, vi: &'static str) -> &'static str {
    match locale {
        UiLocale::En => en,
        UiLocale::Vi => vi,
    }
}

pub fn install(context: &egui::Context) {
    install_with_theme(context, UiTheme::Light);
}

pub fn install_with_theme(context: &egui::Context, theme: UiTheme) {
    if theme == UiTheme::Dark {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = egui::Color32::from_rgb(0x16, 0x1B, 0x1A);
        visuals.window_fill = egui::Color32::from_rgb(0x1E, 0x24, 0x23);
        visuals.selection.bg_fill = egui::Color32::from_rgb(0x24, 0x5C, 0x59);
        context.set_visuals(visuals);
        context.enable_accesskit();
        return;
    }
    let mut visuals = egui::Visuals::light();
    visuals.panel_fill = Tokens::BG_SURFACE;
    visuals.window_fill = Tokens::BG_SURFACE;
    visuals.extreme_bg_color = Tokens::BG_SURFACE;
    visuals.faint_bg_color = Tokens::BG_PAGE;
    visuals.override_text_color = Some(Tokens::TEXT_PRIMARY);
    visuals.widgets.noninteractive.bg_fill = Tokens::BG_SURFACE;
    visuals.widgets.noninteractive.weak_bg_fill = Tokens::BG_SURFACE;
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, Tokens::BORDER_DEFAULT);
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, Tokens::TEXT_PRIMARY);
    visuals.widgets.inactive.bg_fill = Tokens::BG_SURFACE;
    visuals.widgets.inactive.weak_bg_fill = Tokens::BG_SURFACE;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, Tokens::BORDER_DEFAULT);
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, Tokens::TEXT_PRIMARY);
    visuals.widgets.hovered.bg_fill = Tokens::BG_SURFACE;
    visuals.widgets.hovered.weak_bg_fill = Tokens::BG_SURFACE;
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.5, Tokens::BORDER_FOCUS);
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.5, Tokens::TEXT_BRAND);
    visuals.widgets.active.bg_fill = Tokens::BG_SURFACE;
    visuals.widgets.active.weak_bg_fill = Tokens::BG_SURFACE;
    visuals.widgets.active.bg_stroke = egui::Stroke::new(2.0, Tokens::BORDER_FOCUS);
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.5, Tokens::TEXT_BRAND);
    visuals.selection.bg_fill = egui::Color32::from_rgb(0xE8, 0xF4, 0xF0);
    visuals.selection.stroke = egui::Stroke::new(2.0, Tokens::BORDER_FOCUS);
    visuals.window_stroke = egui::Stroke::new(1.0, Tokens::BORDER_DEFAULT);
    context.set_visuals(visuals);
    context.enable_accesskit();
}

pub fn section(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.heading(egui::RichText::new(title).color(Tokens::TEXT_PRIMARY));
    ui.label(egui::RichText::new(subtitle).color(Tokens::TEXT_SECONDARY));
    ui.add_space(4.0);
    ui.separator();
    ui.add_space(4.0);
}

pub fn surface_card(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(Tokens::BG_SURFACE)
        .stroke(egui::Stroke::new(1.0, Tokens::BORDER_DEFAULT))
        .corner_radius(8)
        .inner_margin(12)
        .show(ui, add_contents);
}

pub fn primary_button(ui: &mut egui::Ui, enabled: bool, label: &str) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(
            egui::RichText::new(label)
                .color(Tokens::TEXT_BRAND)
                .strong(),
        )
        .fill(Tokens::BG_SURFACE)
        .stroke(egui::Stroke::new(1.5, Tokens::BORDER_FOCUS))
        .corner_radius(6),
    )
}

pub fn secondary_button(ui: &mut egui::Ui, enabled: bool, label: &str) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(egui::RichText::new(label).color(Tokens::TEXT_PRIMARY))
            .fill(Tokens::BG_SURFACE)
            .stroke(egui::Stroke::new(1.0, Tokens::BORDER_DEFAULT))
            .corner_radius(6),
    )
}

pub fn danger_button(ui: &mut egui::Ui, enabled: bool, label: &str) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(
            egui::RichText::new(label)
                .color(Tokens::TEXT_DANGER)
                .strong(),
        )
        .fill(Tokens::BG_SURFACE)
        .stroke(egui::Stroke::new(1.5, Tokens::TEXT_DANGER))
        .corner_radius(6),
    )
}

pub fn status_badge(ui: &mut egui::Ui, kind: NoticeKind, text: &str) {
    egui::Frame::new()
        .fill(Tokens::BG_SURFACE)
        .stroke(egui::Stroke::new(1.0, Tokens::BORDER_DEFAULT))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(format!("[{}] {}", kind.label(), text))
                    .color(kind.color())
                    .small(),
            );
        });
}

pub fn method_color(method: &str) -> egui::Color32 {
    match method.to_ascii_uppercase().as_str() {
        "GET" => Tokens::METHOD_GET,
        "POST" => Tokens::METHOD_POST,
        "PUT" => Tokens::METHOD_PUT,
        "PATCH" => Tokens::METHOD_PATCH,
        "DELETE" => Tokens::METHOD_DELETE,
        "HEAD" => Tokens::METHOD_HEAD,
        "OPTIONS" => Tokens::METHOD_OPTIONS,
        _ => Tokens::TEXT_PRIMARY,
    }
}

pub fn method_badge(ui: &mut egui::Ui, method: &str) {
    let upper = method.to_ascii_uppercase();
    egui::Frame::new()
        .fill(Tokens::BG_SURFACE)
        .stroke(egui::Stroke::new(1.0, Tokens::BORDER_DEFAULT))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(upper.clone())
                    .color(method_color(&upper))
                    .monospace()
                    .strong()
                    .small(),
            );
        });
}

pub fn notice_banner(ui: &mut egui::Ui, kind: NoticeKind, title: &str, detail: &str) {
    egui::Frame::new()
        .fill(Tokens::BG_SURFACE)
        .stroke(egui::Stroke::new(1.0, kind.color()))
        .corner_radius(8)
        .inner_margin(10)
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    egui::RichText::new(format!("[{}] {title}:", kind.label()))
                        .color(kind.color())
                        .strong(),
                );
                ui.label(egui::RichText::new(detail).color(Tokens::TEXT_PRIMARY));
            });
        });
}

pub fn code_surface(ui: &mut egui::Ui, text: &str) {
    egui::Frame::new()
        .fill(Tokens::BG_SURFACE)
        .stroke(egui::Stroke::new(1.0, Tokens::BORDER_DEFAULT))
        .corner_radius(6)
        .inner_margin(8)
        .show(ui, |ui| {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(text)
                        .monospace()
                        .color(Tokens::TEXT_PRIMARY),
                )
                .selectable(true),
            );
        });
}

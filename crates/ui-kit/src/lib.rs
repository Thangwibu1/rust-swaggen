//! Shared Figma-aligned white-surface design system (Tokens, JetBrains Mono Regular + Bold,
//! theme-aware light/dark helpers, single-edge dividers, custom vector BrandMark/icons,
//! Google Sign-In button, and reusable desktop components).
//! No network IO, telemetry, or remote rendering.
use eframe::egui;
use std::path::PathBuf;

pub struct Tokens;

impl Tokens {
    // Core surfaces (exact Figma variable definitions from Swaggen file)
    pub const BG_PAGE: egui::Color32 = egui::Color32::WHITE;
    pub const BG_SURFACE: egui::Color32 = egui::Color32::WHITE;
    pub const BG_SUBTLE: egui::Color32 = egui::Color32::from_rgb(0xF8, 0xFA, 0xFC);
    pub const BG_SELECTED: egui::Color32 = egui::Color32::from_rgb(0xEF, 0xF6, 0xFF);
    pub const BG_DISABLED: egui::Color32 = egui::Color32::from_rgb(0xED, 0xF0, 0xED);

    // Dark theme surfaces
    pub const DARK_BG_PAGE: egui::Color32 = egui::Color32::from_rgb(0x11, 0x13, 0x16);
    pub const DARK_BG_SURFACE: egui::Color32 = egui::Color32::from_rgb(0x16, 0x19, 0x1D);
    pub const DARK_BG_SUBTLE: egui::Color32 = egui::Color32::from_rgb(0x1E, 0x22, 0x28);
    pub const DARK_BG_SELECTED: egui::Color32 = egui::Color32::from_rgb(0x1A, 0x2C, 0x47);
    pub const DARK_BORDER: egui::Color32 = egui::Color32::from_rgb(0x2A, 0x2F, 0x37);
    pub const DARK_TEXT_PRIMARY: egui::Color32 = egui::Color32::from_rgb(0xF1, 0xF3, 0xF5);
    pub const DARK_TEXT_SECONDARY: egui::Color32 = egui::Color32::from_rgb(0x9A, 0xA0, 0xA6);

    // Typography & borders
    pub const TEXT_PRIMARY: egui::Color32 = egui::Color32::from_rgb(0x20, 0x21, 0x24);
    pub const TEXT_SECONDARY: egui::Color32 = egui::Color32::from_rgb(0x65, 0x68, 0x6F);
    pub const TEXT_BRAND: egui::Color32 = egui::Color32::from_rgb(0x20, 0x21, 0x24);
    pub const TEXT_DANGER: egui::Color32 = egui::Color32::from_rgb(0xB9, 0x1C, 0x1C);
    pub const TEXT_WARNING: egui::Color32 = egui::Color32::from_rgb(0xB4, 0x53, 0x09);

    // Semantic accents
    pub const BRAND_TEAL: egui::Color32 = egui::Color32::from_rgb(0x24, 0x5C, 0x59);
    pub const BRAND_SALMON: egui::Color32 = egui::Color32::from_rgb(0xF5, 0xA8, 0x9A);
    pub const BRAND_CREAM: egui::Color32 = egui::Color32::from_rgb(0xFF, 0xEF, 0xD2);
    pub const STATUS_SUCCESS: egui::Color32 = egui::Color32::from_rgb(0x15, 0x80, 0x3D);
    pub const STATUS_INFO: egui::Color32 = egui::Color32::from_rgb(0x1D, 0x4E, 0xD8);
    pub const BORDER_DEFAULT: egui::Color32 = egui::Color32::from_rgb(0xE5, 0xE7, 0xEB);
    pub const BORDER_FOCUS: egui::Color32 = egui::Color32::from_rgb(0x20, 0x21, 0x24);

    // HTTP method foreground tokens
    pub const METHOD_GET: egui::Color32 = egui::Color32::from_rgb(0x15, 0x80, 0x3D);
    pub const METHOD_POST: egui::Color32 = egui::Color32::from_rgb(0xB4, 0x53, 0x09);
    pub const METHOD_PUT: egui::Color32 = egui::Color32::from_rgb(0x1D, 0x4E, 0xD8);
    pub const METHOD_PATCH: egui::Color32 = egui::Color32::from_rgb(0x7C, 0x3A, 0xED);
    pub const METHOD_DELETE: egui::Color32 = egui::Color32::from_rgb(0xB9, 0x1C, 0x1C);
    pub const METHOD_HEAD: egui::Color32 = egui::Color32::from_rgb(0x0F, 0x76, 0x6E);
    pub const METHOD_OPTIONS: egui::Color32 = egui::Color32::from_rgb(0x9E, 0x2A, 0x6B);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
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
            Self::Info => "Info",
            Self::Success => "OK",
            Self::Warning => "Warning",
            Self::Danger => "Error",
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconKind {
    Blocks,
    BookOpen,
    Play,
    GitBranch,
    Users,
    Globe,
    Terminal,
    Settings,
    Folder,
    FileCode,
    History,
    Search,
    Bell,
    ShieldCheck,
    KeyRound,
    Plus,
    ChevronDown,
    ChevronRight,
    AlertCircle,
    Check,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvatarTone {
    Teal,
    Salmon,
    Cream,
}

pub fn tr(locale: UiLocale, en: &'static str, vi: &'static str) -> &'static str {
    match locale {
        UiLocale::En => en,
        UiLocale::Vi => vi,
    }
}

pub fn is_dark(ui: &egui::Ui) -> bool {
    ui.visuals().dark_mode
}

pub fn page_bg(ui: &egui::Ui) -> egui::Color32 {
    if is_dark(ui) {
        Tokens::DARK_BG_PAGE
    } else {
        Tokens::BG_PAGE
    }
}

pub fn surface_bg(ui: &egui::Ui) -> egui::Color32 {
    if is_dark(ui) {
        Tokens::DARK_BG_SURFACE
    } else {
        Tokens::BG_SURFACE
    }
}

pub fn subtle_bg(ui: &egui::Ui) -> egui::Color32 {
    if is_dark(ui) {
        Tokens::DARK_BG_SUBTLE
    } else {
        Tokens::BG_SUBTLE
    }
}

pub fn selected_bg(ui: &egui::Ui) -> egui::Color32 {
    if is_dark(ui) {
        Tokens::DARK_BG_SELECTED
    } else {
        Tokens::BG_SELECTED
    }
}

pub fn border_color(ui: &egui::Ui) -> egui::Color32 {
    if is_dark(ui) {
        Tokens::DARK_BORDER
    } else {
        Tokens::BORDER_DEFAULT
    }
}

pub fn text_primary(ui: &egui::Ui) -> egui::Color32 {
    if is_dark(ui) {
        Tokens::DARK_TEXT_PRIMARY
    } else {
        Tokens::TEXT_PRIMARY
    }
}

pub fn text_secondary(ui: &egui::Ui) -> egui::Color32 {
    if is_dark(ui) {
        Tokens::DARK_TEXT_SECONDARY
    } else {
        Tokens::TEXT_SECONDARY
    }
}

/// Returns RichText styled with the authentic JetBrains Mono Bold font family.
pub fn bold_text(text: &str, size: f32, color: egui::Color32) -> egui::RichText {
    egui::RichText::new(text)
        .font(egui::FontId::new(size, egui::FontFamily::Name("bold".into())))
        .color(color)
}

/// Paints a single crisp 1px edge divider (eliminates double borders between adjacent panels).
pub fn paint_edge_divider(ui: &egui::Ui, rect: egui::Rect, edge: Edge) {
    let st = egui::Stroke::new(1.0, border_color(ui));
    let seg = match edge {
        Edge::Top => [rect.left_top(), rect.right_top()],
        Edge::Bottom => [rect.left_bottom(), rect.right_bottom()],
        Edge::Left => [rect.left_top(), rect.left_bottom()],
        Edge::Right => [rect.right_top(), rect.right_bottom()],
    };
    ui.painter().line_segment(seg, st);
}

fn load_first_existing(paths: &[PathBuf]) -> Option<Vec<u8>> {
    for p in paths {
        if let Ok(bytes) = std::fs::read(p) {
            return Some(bytes);
        }
    }
    None
}

/// Loads JetBrains Mono Regular + Bold (falling back to Cascadia Mono / Consolas + Segoe UI
/// for full Vietnamese diacritic support) without embedding external proprietary font binaries.
fn configure_fonts(context: &egui::Context) {
    let mut defs = egui::FontDefinitions::default();
    let local_fonts = std::env::var_os("LOCALAPPDATA")
        .map(|p| PathBuf::from(p).join("Microsoft\\Windows\\Fonts"));
    let win_fonts = std::env::var_os("WINDIR").map(|p| PathBuf::from(p).join("Fonts"));

    let mut reg_candidates = Vec::new();
    let mut bold_candidates = Vec::new();
    let mut fallback_candidates = Vec::new();

    if let Some(ref lf) = local_fonts {
        reg_candidates.push(lf.join("JetBrainsMono-Regular.ttf"));
        reg_candidates.push(lf.join("JetBrainsMonoNL-Regular.ttf"));
        bold_candidates.push(lf.join("JetBrainsMono-Bold.ttf"));
        bold_candidates.push(lf.join("JetBrainsMonoNL-Bold.ttf"));
        bold_candidates.push(lf.join("JetBrainsMono-SemiBold.ttf"));
    }
    if let Some(ref wf) = win_fonts {
        reg_candidates.push(wf.join("JetBrainsMono-Regular.ttf"));
        reg_candidates.push(wf.join("CascadiaMono.ttf"));
        reg_candidates.push(wf.join("consola.ttf"));
        bold_candidates.push(wf.join("JetBrainsMono-Bold.ttf"));
        bold_candidates.push(wf.join("consolab.ttf"));
        bold_candidates.push(wf.join("segoeuib.ttf"));
        fallback_candidates.push(wf.join("segoeui.ttf"));
    }

    let mut reg_stack = Vec::new();
    let mut bold_stack = Vec::new();

    if let Some(bytes) = load_first_existing(&reg_candidates) {
        defs.font_data.insert(
            "jb_regular".into(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
        reg_stack.push("jb_regular".to_owned());
    }
    if let Some(bytes) = load_first_existing(&bold_candidates) {
        defs.font_data.insert(
            "jb_bold".into(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
        bold_stack.push("jb_bold".to_owned());
    } else if !reg_stack.is_empty() {
        bold_stack.push("jb_regular".to_owned());
    }
    if let Some(bytes) = load_first_existing(&fallback_candidates) {
        defs.font_data.insert(
            "vn_fallback".into(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
        reg_stack.push("vn_fallback".to_owned());
        bold_stack.push("vn_fallback".to_owned());
    }

    if !reg_stack.is_empty() {
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            let entry = defs.families.entry(family).or_default();
            for (pos, name) in reg_stack.iter().enumerate() {
                entry.insert(pos, name.clone());
            }
        }
    }
    let mut bold_family = bold_stack;
    if let Some(existing) = defs.families.get(&egui::FontFamily::Proportional) {
        for name in existing {
            if !bold_family.contains(name) {
                bold_family.push(name.clone());
            }
        }
    }
    defs.families
        .insert(egui::FontFamily::Name("bold".into()), bold_family);

    context.set_fonts(defs);
}

pub fn install(context: &egui::Context) {
    install_with_theme(context, UiTheme::Light);
}

pub fn install_with_theme(context: &egui::Context, theme: UiTheme) {
    configure_fonts(context);

    let egui_theme = if theme == UiTheme::Dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    };
    let mut style = (*context.style_of(egui_theme)).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 8.0);
    style.spacing.interact_size = egui::vec2(36.0, 36.0);
    style.spacing.window_margin = egui::Margin::same(20);
    style.spacing.indent = 16.0;
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::new(22.0, egui::FontFamily::Name("bold".into())),
    );
    style.text_styles.insert(
        egui::TextStyle::Body,
        egui::FontId::new(13.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        egui::FontId::new(13.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Monospace,
        egui::FontId::new(13.0, egui::FontFamily::Monospace),
    );
    style.text_styles.insert(
        egui::TextStyle::Small,
        egui::FontId::new(11.5, egui::FontFamily::Proportional),
    );

    if theme == UiTheme::Dark {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Tokens::DARK_BG_SURFACE;
        visuals.window_fill = Tokens::DARK_BG_SURFACE;
        visuals.extreme_bg_color = Tokens::DARK_BG_PAGE;
        visuals.faint_bg_color = Tokens::DARK_BG_SUBTLE;
        visuals.selection.bg_fill = Tokens::DARK_BG_SELECTED;
        visuals.widgets.inactive.bg_fill = Tokens::DARK_BG_SURFACE;
        visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, Tokens::DARK_BORDER);
        visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(6);
        visuals.widgets.hovered.bg_fill = Tokens::DARK_BG_SUBTLE;
        visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(6);
        visuals.window_stroke = egui::Stroke::new(1.0, Tokens::DARK_BORDER);
        visuals.window_corner_radius = egui::CornerRadius::same(10);
        style.visuals = visuals;
        context.set_style_of(egui_theme, style);
        context.set_theme(egui_theme);
        context.enable_accesskit();
        return;
    }

    let mut visuals = egui::Visuals::light();
    visuals.panel_fill = Tokens::BG_SURFACE;
    visuals.window_fill = Tokens::BG_SURFACE;
    visuals.extreme_bg_color = Tokens::BG_SURFACE;
    visuals.faint_bg_color = Tokens::BG_SUBTLE;
    visuals.override_text_color = Some(Tokens::TEXT_PRIMARY);
    visuals.widgets.noninteractive.bg_fill = Tokens::BG_SURFACE;
    visuals.widgets.noninteractive.weak_bg_fill = Tokens::BG_SURFACE;
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, Tokens::BORDER_DEFAULT);
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, Tokens::TEXT_PRIMARY);
    visuals.widgets.inactive.bg_fill = Tokens::BG_SURFACE;
    visuals.widgets.inactive.weak_bg_fill = Tokens::BG_SURFACE;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, Tokens::BORDER_DEFAULT);
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, Tokens::TEXT_PRIMARY);
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(6);
    visuals.widgets.hovered.bg_fill = Tokens::BG_SUBTLE;
    visuals.widgets.hovered.weak_bg_fill = Tokens::BG_SUBTLE;
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, Tokens::BORDER_FOCUS);
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, Tokens::TEXT_PRIMARY);
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(6);
    visuals.widgets.active.bg_fill = Tokens::BG_SELECTED;
    visuals.widgets.active.weak_bg_fill = Tokens::BG_SELECTED;
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.5, Tokens::BORDER_FOCUS);
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.5, Tokens::TEXT_PRIMARY);
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(6);
    visuals.selection.bg_fill = Tokens::BG_SELECTED;
    visuals.selection.stroke = egui::Stroke::new(1.5, Tokens::STATUS_INFO);
    visuals.window_stroke = egui::Stroke::new(1.0, Tokens::BORDER_DEFAULT);
    visuals.window_corner_radius = egui::CornerRadius::same(10);
    style.visuals = visuals;
    context.set_style_of(egui_theme, style);
    context.set_theme(egui_theme);
    context.enable_accesskit();
}

/// Figma Input component (5:200): 40px tall bordered white input with 12px horizontal padding.
pub fn text_input(
    ui: &mut egui::Ui,
    value: &mut String,
    hint: &str,
    width: f32,
    password: bool,
) -> egui::Response {
    let inner_w = (width - 24.0).max(80.0);
    let mut inner_resp: Option<egui::Response> = None;
    egui::Frame::new()
        .fill(surface_bg(ui))
        .stroke(egui::Stroke::new(1.0, border_color(ui)))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(12, 9))
        .show(ui, |ui| {
            let resp = ui.add(
                egui::TextEdit::singleline(value)
                    .hint_text(hint)
                    .password(password)
                    .frame(egui::Frame::NONE)
                    .desired_width(inner_w),
            );
            inner_resp = Some(resp);
        });
    inner_resp.expect("text_input frame executed")
}

/// Paints a crisp 4-color official Google 'G' mark using smooth circular arcs + crossbar.
fn paint_google_g(painter: &egui::Painter, center: egui::Pos2, radius: f32) {
    let sw = (radius * 0.36).max(2.6);
    let draw_arc = |start_deg: f32, end_deg: f32, color: egui::Color32| {
        let steps = 16;
        let mut pts = Vec::with_capacity(steps + 1);
        for i in 0..=steps {
            let deg = start_deg + (end_deg - start_deg) * (i as f32 / steps as f32);
            let rad = deg.to_radians();
            pts.push(egui::pos2(
                center.x + radius * rad.cos(),
                center.y + radius * rad.sin(),
            ));
        }
        painter.add(egui::Shape::line(pts, egui::Stroke::new(sw, color)));
    };

    // Official Google 4-color brand palette (y grows downward in egui)
    // Red: top-left to top-right (-142° -> -42°)
    draw_arc(-142.0, -42.0, egui::Color32::from_rgb(0xEA, 0x43, 0x35));
    // Yellow: left arc (142° -> 218°)
    draw_arc(142.0, 218.0, egui::Color32::from_rgb(0xFB, 0xBC, 0x05));
    // Green: bottom arc (40° -> 144°)
    draw_arc(40.0, 144.0, egui::Color32::from_rgb(0x34, 0xA8, 0x53));
    // Blue: right-lower arc (-4° -> 44°) + crisp horizontal crossbar
    draw_arc(-4.0, 44.0, egui::Color32::from_rgb(0x42, 0x85, 0xF4));
    painter.line_segment(
        [
            egui::pos2(center.x + 0.5, center.y),
            egui::pos2(center.x + radius + sw * 0.35, center.y),
        ],
        egui::Stroke::new(sw, egui::Color32::from_rgb(0x42, 0x85, 0xF4)),
    );
}

/// Figma A01 Google Sign-In button (44px tall, full width, crisp 4-color G mark + centered label).
pub fn google_button(ui: &mut egui::Ui, enabled: bool, label: &str) -> egui::Response {
    let w = ui.available_width().max(220.0);
    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(w, 44.0),
        if enabled { egui::Sense::click() } else { egui::Sense::hover() },
    );
    let fill = if !enabled || resp.hovered() {
        subtle_bg(ui)
    } else {
        surface_bg(ui)
    };
    let stroke_col = if enabled && resp.hovered() {
        Tokens::BORDER_FOCUS
    } else {
        border_color(ui)
    };
    ui.painter()
        .rect(rect, 8.0, fill, egui::Stroke::new(1.0, stroke_col), egui::StrokeKind::Middle);

    let font_id = egui::FontId::new(13.5, egui::FontFamily::Name("bold".into()));
    let text_color = if enabled { text_primary(ui) } else { text_secondary(ui) };
    let galley = ui
        .fonts_mut(|f| f.layout_no_wrap(label.to_owned(), font_id, text_color));
    let icon_d = 17.0;
    let gap = 10.0;
    let total_w = icon_d + gap + galley.size().x;
    let start_x = rect.center().x - total_w * 0.5;
    let g_center = egui::pos2(start_x + icon_d * 0.5, rect.center().y);

    paint_google_g(ui.painter(), g_center, icon_d * 0.44);
    ui.painter().galley(
        egui::pos2(start_x + icon_d + gap, rect.center().y - galley.size().y * 0.5),
        galley,
        text_color,
    );
    resp
}

/// Renders the Swaggen Brand/Mark (Figma node 4:81): paired code brackets + salmon accent dot.
pub fn brand_mark(ui: &mut egui::Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let p = ui.painter();
    let s = size / 32.0;
    let stroke = egui::Stroke::new((3.0 * s).max(2.0), Tokens::BRAND_TEAL);
    p.line_segment(
        [rect.min + egui::vec2(20.0 * s, 7.5 * s), rect.min + egui::vec2(10.0 * s, 7.5 * s)],
        stroke,
    );
    p.line_segment(
        [rect.min + egui::vec2(10.0 * s, 7.5 * s), rect.min + egui::vec2(7.0 * s, 11.5 * s)],
        stroke,
    );
    p.line_segment(
        [rect.min + egui::vec2(7.0 * s, 11.5 * s), rect.min + egui::vec2(11.0 * s, 15.0 * s)],
        stroke,
    );
    p.line_segment(
        [rect.min + egui::vec2(11.0 * s, 15.0 * s), rect.min + egui::vec2(21.0 * s, 17.0 * s)],
        stroke,
    );
    p.line_segment(
        [rect.min + egui::vec2(21.0 * s, 17.0 * s), rect.min + egui::vec2(25.0 * s, 21.0 * s)],
        stroke,
    );
    p.line_segment(
        [rect.min + egui::vec2(25.0 * s, 21.0 * s), rect.min + egui::vec2(22.0 * s, 24.5 * s)],
        stroke,
    );
    p.line_segment(
        [rect.min + egui::vec2(22.0 * s, 24.5 * s), rect.min + egui::vec2(9.0 * s, 24.5 * s)],
        stroke,
    );
    p.circle_filled(
        rect.min + egui::vec2(24.5 * s, 7.5 * s),
        (2.6 * s).max(2.0),
        Tokens::BRAND_SALMON,
    );
}

/// Paints minimal crisp 1.5px stroke Lucide-style vector icons matching 01-component-library.
pub fn paint_icon(painter: &egui::Painter, rect: egui::Rect, kind: IconKind, color: egui::Color32) {
    let c = rect.center();
    let r = rect.width().min(rect.height()) * 0.38;
    let st = egui::Stroke::new(1.5, color);
    match kind {
        IconKind::Blocks => {
            let s = r * 0.85;
            painter.rect_stroke(
                egui::Rect::from_min_max(c - egui::vec2(s, s), c - egui::vec2(1.5, 1.5)),
                2.0,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                egui::Rect::from_min_max(c + egui::vec2(1.5, -s), c + egui::vec2(s, -1.5)),
                2.0,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                egui::Rect::from_min_max(c + egui::vec2(-s, 1.5), c + egui::vec2(-1.5, s)),
                2.0,
                st,
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                egui::Rect::from_min_max(c + egui::vec2(1.5, 1.5), c + egui::vec2(s, s)),
                2.0,
                st,
                egui::StrokeKind::Middle,
            );
        }
        IconKind::BookOpen => {
            painter.rect_stroke(
                egui::Rect::from_center_size(c, egui::vec2(r * 1.9, r * 1.5)),
                2.0,
                st,
                egui::StrokeKind::Middle,
            );
            painter.line_segment([c + egui::vec2(0.0, -r * 0.75), c + egui::vec2(0.0, r * 0.75)], st);
        }
        IconKind::Play => {
            painter.add(egui::Shape::closed_line(
                vec![
                    c + egui::vec2(-r * 0.55, -r * 0.85),
                    c + egui::vec2(r * 0.85, 0.0),
                    c + egui::vec2(-r * 0.55, r * 0.85),
                ],
                st,
            ));
        }
        IconKind::GitBranch => {
            painter.circle_stroke(c + egui::vec2(-r * 0.5, -r * 0.55), 2.5, st);
            painter.circle_stroke(c + egui::vec2(-r * 0.5, r * 0.55), 2.5, st);
            painter.circle_stroke(c + egui::vec2(r * 0.55, -r * 0.25), 2.5, st);
            painter.line_segment(
                [c + egui::vec2(-r * 0.5, -r * 0.3), c + egui::vec2(-r * 0.5, r * 0.3)],
                st,
            );
            painter.line_segment(
                [c + egui::vec2(-r * 0.5, r * 0.1), c + egui::vec2(r * 0.35, -r * 0.2)],
                st,
            );
        }
        IconKind::Users => {
            painter.circle_stroke(c + egui::vec2(-r * 0.25, -r * 0.4), r * 0.38, st);
            painter.line_segment(
                [c + egui::vec2(-r * 0.9, r * 0.75), c + egui::vec2(r * 0.4, r * 0.75)],
                st,
            );
            painter.circle_stroke(c + egui::vec2(r * 0.55, -r * 0.3), r * 0.28, st);
        }
        IconKind::Globe => {
            painter.circle_stroke(c, r * 0.95, st);
            painter.line_segment([c + egui::vec2(-r * 0.9, 0.0), c + egui::vec2(r * 0.9, 0.0)], st);
            painter.line_segment([c + egui::vec2(0.0, -r * 0.9), c + egui::vec2(0.0, r * 0.9)], st);
        }
        IconKind::Terminal => {
            painter.line_segment(
                [c + egui::vec2(-r * 0.7, -r * 0.45), c + egui::vec2(-r * 0.15, 0.0)],
                st,
            );
            painter.line_segment(
                [c + egui::vec2(-r * 0.15, 0.0), c + egui::vec2(-r * 0.7, r * 0.45)],
                st,
            );
            painter.line_segment(
                [c + egui::vec2(r * 0.05, r * 0.45), c + egui::vec2(r * 0.75, r * 0.45)],
                st,
            );
        }
        IconKind::Settings => {
            painter.circle_stroke(c, r * 0.4, st);
            painter.circle_stroke(c, r * 0.85, st);
        }
        IconKind::Folder => {
            painter.rect_stroke(
                egui::Rect::from_center_size(c + egui::vec2(0.0, 1.0), egui::vec2(r * 1.9, r * 1.4)),
                2.0,
                st,
                egui::StrokeKind::Middle,
            );
            painter.line_segment(
                [c + egui::vec2(-r * 0.95, -r * 0.65), c + egui::vec2(-r * 0.2, -r * 0.65)],
                st,
            );
        }
        IconKind::FileCode => {
            painter.rect_stroke(
                egui::Rect::from_center_size(c, egui::vec2(r * 1.5, r * 1.9)),
                2.0,
                st,
                egui::StrokeKind::Middle,
            );
            painter.line_segment([c + egui::vec2(-r * 0.3, 0.0), c + egui::vec2(r * 0.3, 0.0)], st);
        }
        IconKind::History => {
            painter.circle_stroke(c, r * 0.9, st);
            painter.line_segment([c, c + egui::vec2(0.0, -r * 0.5)], st);
            painter.line_segment([c, c + egui::vec2(r * 0.4, r * 0.2)], st);
        }
        IconKind::Search => {
            painter.circle_stroke(c + egui::vec2(-r * 0.18, -r * 0.18), r * 0.6, st);
            painter.line_segment(
                [c + egui::vec2(r * 0.25, r * 0.25), c + egui::vec2(r * 0.85, r * 0.85)],
                st,
            );
        }
        IconKind::Bell => {
            painter.circle_stroke(c + egui::vec2(0.0, -r * 0.1), r * 0.65, st);
            painter.line_segment(
                [c + egui::vec2(-r * 0.8, r * 0.55), c + egui::vec2(r * 0.8, r * 0.55)],
                st,
            );
        }
        IconKind::ShieldCheck => {
            painter.rect_stroke(
                egui::Rect::from_center_size(c, egui::vec2(r * 1.5, r * 1.7)),
                4.0,
                st,
                egui::StrokeKind::Middle,
            );
            painter.line_segment(
                [c + egui::vec2(-r * 0.35, 0.0), c + egui::vec2(-r * 0.05, r * 0.35)],
                st,
            );
            painter.line_segment(
                [c + egui::vec2(-r * 0.05, r * 0.35), c + egui::vec2(r * 0.45, -r * 0.3)],
                st,
            );
        }
        IconKind::KeyRound => {
            painter.circle_stroke(c + egui::vec2(-r * 0.4, 0.0), r * 0.4, st);
            painter.line_segment([c, c + egui::vec2(r * 0.85, 0.0)], st);
            painter.line_segment([c + egui::vec2(r * 0.6, 0.0), c + egui::vec2(r * 0.6, r * 0.35)], st);
        }
        IconKind::Plus => {
            painter.line_segment([c + egui::vec2(-r * 0.7, 0.0), c + egui::vec2(r * 0.7, 0.0)], st);
            painter.line_segment([c + egui::vec2(0.0, -r * 0.7), c + egui::vec2(0.0, r * 0.7)], st);
        }
        IconKind::ChevronDown => {
            painter.line_segment([c + egui::vec2(-r * 0.6, -r * 0.25), c + egui::vec2(0.0, r * 0.35)], st);
            painter.line_segment([c + egui::vec2(0.0, r * 0.35), c + egui::vec2(r * 0.6, -r * 0.25)], st);
        }
        IconKind::ChevronRight => {
            painter.line_segment([c + egui::vec2(-r * 0.25, -r * 0.6), c + egui::vec2(r * 0.35, 0.0)], st);
            painter.line_segment([c + egui::vec2(r * 0.35, 0.0), c + egui::vec2(-r * 0.25, r * 0.6)], st);
        }
        IconKind::AlertCircle => {
            painter.circle_stroke(c, r * 0.9, st);
            painter.line_segment([c + egui::vec2(0.0, -r * 0.4), c + egui::vec2(0.0, r * 0.1)], st);
            painter.circle_filled(c + egui::vec2(0.0, r * 0.45), 1.2, color);
        }
        IconKind::Check => {
            painter.line_segment([c + egui::vec2(-r * 0.55, 0.0), c + egui::vec2(-r * 0.1, r * 0.45)], st);
            painter.line_segment([c + egui::vec2(-r * 0.1, r * 0.45), c + egui::vec2(r * 0.65, -r * 0.4)], st);
        }
    }
}

/// Activity rail icon button (40x40 rounded button inside the 64px Figma rail).
pub fn rail_icon_button(
    ui: &mut egui::Ui,
    selected: bool,
    kind: IconKind,
    tooltip: &str,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::click());
    let fill = if selected {
        selected_bg(ui)
    } else if resp.hovered() {
        subtle_bg(ui)
    } else {
        surface_bg(ui)
    };
    let stroke = if selected {
        egui::Stroke::new(1.0, Tokens::STATUS_INFO)
    } else if resp.hovered() {
        egui::Stroke::new(1.0, border_color(ui))
    } else {
        egui::Stroke::NONE
    };
    let icon_color = if selected {
        Tokens::STATUS_INFO
    } else {
        text_primary(ui)
    };
    ui.painter().rect(rect, 8.0, fill, stroke, egui::StrokeKind::Middle);
    paint_icon(ui.painter(), rect.shrink(10.0), kind, icon_color);
    resp.on_hover_text(tooltip)
}

/// Sidebar navigation row matching Figma Shell/Project sidebar Navigation item (5:223).
pub fn nav_item(
    ui: &mut egui::Ui,
    kind: IconKind,
    label: &str,
    count: Option<usize>,
    selected: bool,
) -> egui::Response {
    let w = ui.available_width().max(120.0);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 36.0), egui::Sense::click());
    let bg = if selected {
        selected_bg(ui)
    } else if resp.hovered() {
        subtle_bg(ui)
    } else {
        surface_bg(ui)
    };
    ui.painter().rect_filled(rect, 6.0, bg);
    let color = if selected {
        Tokens::STATUS_INFO
    } else {
        text_primary(ui)
    };
    let icon_rect = egui::Rect::from_min_size(rect.min + egui::vec2(10.0, 8.0), egui::vec2(20.0, 20.0));
    paint_icon(ui.painter(), icon_rect, kind, color);
    ui.painter().text(
        rect.min + egui::vec2(38.0, rect.height() * 0.5),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::new(13.0, egui::FontFamily::Proportional),
        color,
    );
    if let Some(n) = count {
        ui.painter().text(
            egui::pos2(rect.right() - 10.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            n.to_string(),
            egui::FontId::new(11.5, egui::FontFamily::Monospace),
            if selected { Tokens::STATUS_INFO } else { text_secondary(ui) },
        );
    }
    resp
}

/// Sidebar Endpoint row matching Figma Endpoint component (5:296).
pub fn endpoint_row(
    ui: &mut egui::Ui,
    method: &str,
    name: &str,
    selected: bool,
) -> egui::Response {
    let w = ui.available_width().max(120.0);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 34.0), egui::Sense::click());
    let bg = if selected {
        selected_bg(ui)
    } else if resp.hovered() {
        subtle_bg(ui)
    } else {
        surface_bg(ui)
    };
    let stroke = if selected {
        egui::Stroke::new(1.0, border_color(ui))
    } else {
        egui::Stroke::NONE
    };
    ui.painter()
        .rect(rect, 6.0, bg, stroke, egui::StrokeKind::Middle);

    let m = method.to_ascii_uppercase();
    ui.painter().text(
        rect.min + egui::vec2(10.0, rect.height() * 0.5),
        egui::Align2::LEFT_CENTER,
        &m,
        egui::FontId::new(11.0, egui::FontFamily::Monospace),
        method_color(&m),
    );
    let max_chars = ((w - 72.0) / 7.2).max(10.0) as usize;
    let clipped: String = if name.chars().count() > max_chars {
        let mut s: String = name.chars().take(max_chars.saturating_sub(1)).collect();
        s.push('…');
        s
    } else {
        name.to_owned()
    };
    ui.painter().text(
        rect.min + egui::vec2(60.0, rect.height() * 0.5),
        egui::Align2::LEFT_CENTER,
        clipped,
        egui::FontId::new(12.5, egui::FontFamily::Proportional),
        text_primary(ui),
    );
    resp
}

/// Top-of-screen Page Title + Breadcrumb + Right Action button (Figma 58px header).
pub fn page_header(
    ui: &mut egui::Ui,
    title: &str,
    subtitle: &str,
    action_label: Option<&str>,
    action_enabled: bool,
) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(bold_text(title, 24.0, text_primary(ui)));
            ui.label(
                egui::RichText::new(subtitle)
                    .size(12.0)
                    .color(text_secondary(ui)),
            );
        });
        if let Some(lbl) = action_label {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if primary_button(ui, action_enabled, lbl).clicked() {
                    clicked = true;
                }
            });
        }
    });
    ui.add_space(12.0);
    clicked
}

pub fn section(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    let _ = page_header(ui, title, subtitle, None, false);
}

/// Figma clean bordered card (corner radius 10, padding 20x24).
pub fn surface_card(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(surface_bg(ui))
        .stroke(egui::Stroke::new(1.0, border_color(ui)))
        .corner_radius(10)
        .inner_margin(egui::Margin::symmetric(24, 20))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            add_contents(ui);
        });
}

/// Card section header row (title left + subtle chevron indicator right).
pub fn card_header(ui: &mut egui::Ui, title: &str) {
    ui.horizontal(|ui| {
        ui.label(bold_text(title, 13.5, text_primary(ui)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
            paint_icon(ui.painter(), rect, IconKind::ChevronDown, text_secondary(ui));
        });
    });
    ui.add_space(10.0);
}

/// Two-column configuration row matching Figma A05 / T02 / S01 / I01 cards.
pub fn config_field_row(
    ui: &mut egui::Ui,
    title: &str,
    subtitle: &str,
    add_input: impl FnOnce(&mut egui::Ui),
) {
    let total_w = ui.available_width().max(420.0);
    let left_w = (total_w * 0.36).clamp(180.0, 340.0);
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(left_w, 42.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.label(bold_text(title, 13.0, text_primary(ui)));
                ui.label(
                    egui::RichText::new(subtitle)
                        .size(11.0)
                        .color(text_secondary(ui)),
                );
            },
        );
        ui.add_space(12.0);
        ui.vertical(|ui| {
            add_input(ui);
        });
    });
    ui.add_space(8.0);
}

/// Uppercase table column header matching Figma `NAME / OPERATION | VALUE / DETAILS | STATUS`.
pub fn table_header(ui: &mut egui::Ui, cols: &[(&str, f32)]) {
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        for (title, w) in cols {
            ui.allocate_ui_with_layout(
                egui::vec2(*w, 24.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.label(
                        egui::RichText::new(*title)
                            .size(11.0)
                            .color(text_secondary(ui)),
                    );
                },
            );
        }
    });
    ui.add_space(4.0);
}

/// Figma table row container (height >=46px, white fill, 1px #E5E7EB border, 6px radius).
pub fn table_row_frame(ui: &mut egui::Ui, add_cells: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(surface_bg(ui))
        .stroke(egui::Stroke::new(1.0, border_color(ui)))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(12, 9))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| add_cells(ui));
        });
    ui.add_space(5.0);
}

/// Figma underline Tab component (5:234): 38px high, active underline indicator.
pub fn underline_tab(ui: &mut egui::Ui, active: bool, label: &str) -> egui::Response {
    let text_color = if active {
        text_primary(ui)
    } else {
        text_secondary(ui)
    };
    let resp = ui.add(
        egui::Button::new(if active {
            bold_text(label, 13.0, text_color)
        } else {
            egui::RichText::new(label).size(13.0).color(text_color)
        })
        .fill(egui::Color32::TRANSPARENT)
        .stroke(egui::Stroke::NONE)
        .min_size(egui::vec2(76.0, 36.0))
        .corner_radius(6),
    );
    if active {
        let r = resp.rect;
        ui.painter().line_segment(
            [
                egui::pos2(r.left() + 8.0, r.bottom() - 2.0),
                egui::pos2(r.right() - 8.0, r.bottom() - 2.0),
            ],
            egui::Stroke::new(2.0, Tokens::STATUS_INFO),
        );
    }
    resp
}

/// Figma Primary button (5:155): surface fill, border, #1D4ED8 Medium/Bold label, 40px height.
pub fn primary_button(ui: &mut egui::Ui, enabled: bool, label: &str) -> egui::Response {
    let color = if enabled {
        Tokens::STATUS_INFO
    } else {
        text_secondary(ui)
    };
    ui.add_enabled(
        enabled,
        egui::Button::new(bold_text(label, 13.0, color))
            .fill(if enabled { surface_bg(ui) } else { subtle_bg(ui) })
            .stroke(egui::Stroke::new(1.0, border_color(ui)))
            .min_size(egui::vec2(124.0, 38.0))
            .corner_radius(6),
    )
}

pub fn secondary_button(ui: &mut egui::Ui, enabled: bool, label: &str) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(
            egui::RichText::new(label)
                .size(13.0)
                .color(text_primary(ui)),
        )
        .fill(surface_bg(ui))
        .stroke(egui::Stroke::new(1.0, border_color(ui)))
        .min_size(egui::vec2(72.0, 36.0))
        .corner_radius(6),
    )
}

pub fn danger_button(ui: &mut egui::Ui, enabled: bool, label: &str) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(bold_text(label, 13.0, Tokens::TEXT_DANGER))
            .fill(surface_bg(ui))
            .stroke(egui::Stroke::new(1.0, Tokens::TEXT_DANGER))
            .min_size(egui::vec2(96.0, 36.0))
            .corner_radius(6),
    )
}

/// Clean bordered badge matching Figma Badge (5:210).
pub fn pill_badge(ui: &mut egui::Ui, color: egui::Color32, text: &str) {
    egui::Frame::new()
        .fill(surface_bg(ui))
        .stroke(egui::Stroke::new(1.0, border_color(ui)))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).color(color).size(11.5));
        });
}

pub fn status_badge(ui: &mut egui::Ui, kind: NoticeKind, text: &str) {
    pill_badge(ui, kind.color(), text);
}

pub fn avatar_circle(ui: &mut egui::Ui, initials: &str, _tone: AvatarTone) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(32.0, 32.0), egui::Sense::click());
    ui.painter().circle(
        rect.center(),
        15.0,
        surface_bg(ui),
        egui::Stroke::new(1.0, border_color(ui)),
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        initials,
        egui::FontId::new(11.0, egui::FontFamily::Monospace),
        text_primary(ui),
    );
    resp
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
        .fill(surface_bg(ui))
        .stroke(egui::Stroke::new(1.0, border_color(ui)))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(upper.clone())
                    .color(method_color(&upper))
                    .monospace()
                    .strong()
                    .size(11.5),
            );
        });
}

/// Clean Figma Notice bar (5:312): surface fill, subtle border, colored status icon & copy.
pub fn notice_banner(ui: &mut egui::Ui, kind: NoticeKind, title: &str, detail: &str) {
    egui::Frame::new()
        .fill(surface_bg(ui))
        .stroke(egui::Stroke::new(1.0, border_color(ui)))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(14, 12))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                let (icon_rect, _) =
                    ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                let icon = match kind {
                    NoticeKind::Success => IconKind::ShieldCheck,
                    _ => IconKind::AlertCircle,
                };
                paint_icon(ui.painter(), icon_rect, icon, kind.color());
                ui.add_space(6.0);
                if !title.is_empty() {
                    ui.label(bold_text(title, 12.5, kind.color()));
                }
                if !detail.is_empty() {
                    ui.label(
                        egui::RichText::new(detail)
                            .color(if title.is_empty() {
                                kind.color()
                            } else {
                                text_primary(ui)
                            })
                            .size(12.5),
                    );
                }
            });
        });
}

pub fn code_surface(ui: &mut egui::Ui, text: &str) {
    code_surface_numbered(ui, text);
}

/// Line-numbered monospace code surface matching Figma Code editor (6:2082 / 6:5869).
pub fn code_surface_numbered(ui: &mut egui::Ui, text: &str) {
    egui::Frame::new()
        .fill(surface_bg(ui))
        .stroke(egui::Stroke::new(1.0, border_color(ui)))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(14, 12))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            for (idx, line) in text.lines().take(250).enumerate() {
                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(
                        egui::vec2(32.0, 22.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.label(
                                egui::RichText::new(format!("{:02}", idx + 1))
                                    .monospace()
                                    .size(11.0)
                                    .color(text_secondary(ui)),
                            );
                        },
                    );
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(line)
                                .monospace()
                                .size(12.5)
                                .color(text_primary(ui)),
                        )
                        .selectable(true),
                    );
                });
            }
            if text.is_empty() {
                ui.label(
                    egui::RichText::new("01  (empty)")
                        .monospace()
                        .size(12.0)
                        .color(text_secondary(ui)),
                );
            }
        });
}

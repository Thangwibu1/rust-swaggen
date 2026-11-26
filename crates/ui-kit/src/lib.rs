//! Shared Figma-aligned white-surface design system (Tokens, JetBrains Mono Regular + Bold,
//! single-edge dividers, authentic Figma BrandMark/Google/Icon assets,
//! Google Sign-In button, and reusable desktop components).
//! No network IO, telemetry, or remote rendering.
use eframe::egui;
use std::path::PathBuf;

pub struct Tokens;

impl Tokens {
    // Core surfaces (exact Figma variable definitions from Swaggen file — white-surface only)
    pub const BG_PAGE: egui::Color32 = egui::Color32::WHITE;
    pub const BG_SURFACE: egui::Color32 = egui::Color32::WHITE;
    pub const BG_SUBTLE: egui::Color32 = egui::Color32::from_rgb(0xF8, 0xFA, 0xFC);
    pub const BG_SELECTED: egui::Color32 = egui::Color32::from_rgb(0xEF, 0xF6, 0xFF);
    pub const BG_DISABLED: egui::Color32 = egui::Color32::from_rgb(0xED, 0xF0, 0xED);

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
    Copy,
    Trash,
}

/// Returns the authentic Figma semantic foreground color for each navigation/action icon.
pub fn icon_semantic_color(kind: IconKind) -> egui::Color32 {
    match kind {
        IconKind::BookOpen | IconKind::Globe | IconKind::Terminal => Tokens::METHOD_HEAD,
        IconKind::Play | IconKind::GitBranch | IconKind::Users => Tokens::METHOD_PATCH,
        IconKind::Folder | IconKind::Bell | IconKind::KeyRound | IconKind::AlertCircle => {
            Tokens::METHOD_POST
        }
        IconKind::FileCode | IconKind::History => Tokens::STATUS_INFO,
        IconKind::ShieldCheck | IconKind::Check => Tokens::STATUS_SUCCESS,
        IconKind::Trash => Tokens::TEXT_DANGER,
        IconKind::Blocks
        | IconKind::Settings
        | IconKind::Search
        | IconKind::Plus
        | IconKind::ChevronDown
        | IconKind::ChevronRight
        | IconKind::Copy => Tokens::TEXT_PRIMARY,
    }
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

pub fn page_bg(ui: &egui::Ui) -> egui::Color32 {
    if ui.visuals().dark_mode {
        egui::Color32::from_rgb(0x0F, 0x17, 0x2A)
    } else {
        Tokens::BG_PAGE
    }
}

pub fn surface_bg(ui: &egui::Ui) -> egui::Color32 {
    if ui.visuals().dark_mode {
        egui::Color32::from_rgb(0x11, 0x18, 0x27)
    } else {
        Tokens::BG_SURFACE
    }
}

pub fn subtle_bg(ui: &egui::Ui) -> egui::Color32 {
    if ui.visuals().dark_mode {
        egui::Color32::from_rgb(0x1E, 0x29, 0x3B)
    } else {
        Tokens::BG_SUBTLE
    }
}

pub fn selected_bg(ui: &egui::Ui) -> egui::Color32 {
    if ui.visuals().dark_mode {
        egui::Color32::from_rgb(0x1E, 0x3A, 0x8A)
    } else {
        Tokens::BG_SELECTED
    }
}

pub fn border_color(ui: &egui::Ui) -> egui::Color32 {
    if ui.visuals().dark_mode {
        egui::Color32::from_rgb(0x33, 0x41, 0x55)
    } else {
        Tokens::BORDER_DEFAULT
    }
}

pub fn text_primary(ui: &egui::Ui) -> egui::Color32 {
    if ui.visuals().dark_mode {
        egui::Color32::from_rgb(0xF1, 0xF5, 0xF9)
    } else {
        Tokens::TEXT_PRIMARY
    }
}

pub fn text_secondary(ui: &egui::Ui) -> egui::Color32 {
    if ui.visuals().dark_mode {
        egui::Color32::from_rgb(0x94, 0xA3, 0xB8)
    } else {
        Tokens::TEXT_SECONDARY
    }
}

/// Returns RichText styled with the authentic JetBrains Mono Bold font family.
pub fn bold_text(text: &str, size: f32, color: egui::Color32) -> egui::RichText {
    egui::RichText::new(text)
        .font(egui::FontId::new(
            size,
            egui::FontFamily::Name("bold".into()),
        ))
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

/// Configures typography across the entire desktop app using JetBrains Mono cleanly for
/// both `Proportional` UI prose and `Monospace` code/URLs (with Segoe UI fallback only for rare symbols).
fn configure_fonts(context: &egui::Context) {
    let mut defs = egui::FontDefinitions::default();
    let local_fonts = std::env::var_os("LOCALAPPDATA")
        .map(|p| PathBuf::from(p).join("Microsoft\\Windows\\Fonts"));
    let win_fonts = std::env::var_os("WINDIR").map(|p| PathBuf::from(p).join("Fonts"));

    let mut ui_fallback_candidates = Vec::new();
    let mut mono_candidates = Vec::new();
    let mut mono_bold_candidates = Vec::new();

    if let Some(ref wf) = win_fonts {
        mono_candidates.push(wf.join("JetBrainsMono-Regular.ttf"));
        mono_candidates.push(wf.join("JetBrainsMonoNL-Regular.ttf"));
        mono_candidates.push(wf.join("CascadiaMono.ttf"));
        mono_candidates.push(wf.join("consola.ttf"));
        mono_bold_candidates.push(wf.join("JetBrainsMono-Bold.ttf"));
        mono_bold_candidates.push(wf.join("JetBrainsMonoNL-Bold.ttf"));
        mono_bold_candidates.push(wf.join("consolab.ttf"));
        ui_fallback_candidates.push(wf.join("segoeui.ttf"));
        ui_fallback_candidates.push(wf.join("seguisym.ttf"));
    }
    if let Some(ref lf) = local_fonts {
        mono_candidates.insert(0, lf.join("JetBrainsMono-Regular.ttf"));
        mono_candidates.insert(1, lf.join("JetBrainsMonoNL-Regular.ttf"));
        mono_candidates.insert(2, lf.join("JetBrainsMono-Medium.ttf"));
        mono_bold_candidates.insert(0, lf.join("JetBrainsMono-Bold.ttf"));
        mono_bold_candidates.insert(1, lf.join("JetBrainsMonoNL-Bold.ttf"));
        mono_bold_candidates.insert(2, lf.join("JetBrainsMono-SemiBold.ttf"));
    }

    let mut primary_stack = Vec::new();
    let mut bold_stack = Vec::new();

    if let Some(bytes) = load_first_existing(&mono_candidates) {
        defs.font_data.insert(
            "jb_regular".into(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
        primary_stack.push("jb_regular".to_owned());
    }
    if let Some(bytes) = load_first_existing(&mono_bold_candidates) {
        defs.font_data.insert(
            "jb_bold".into(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
        bold_stack.push("jb_bold".to_owned());
    }
    if let Some(bytes) = load_first_existing(&ui_fallback_candidates) {
        defs.font_data.insert(
            "ui_fallback".into(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
    }

    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        let entry = defs.families.entry(family).or_default();
        for (pos, name) in primary_stack.iter().enumerate() {
            entry.insert(pos, name.clone());
        }
        if defs.font_data.contains_key("ui_fallback") {
            entry.push("ui_fallback".to_owned());
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

/// Installs the authentic Figma white-surface Light design system & crisp Windows fonts.
pub fn install(context: &egui::Context) {
    configure_fonts(context);

    let egui_theme = egui::Theme::Light;
    let mut style = (*context.style_of(egui_theme)).clone();
    style.spacing.item_spacing = egui::vec2(7.0, 5.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.spacing.interact_size = egui::vec2(26.0, 26.0);
    style.spacing.window_margin = egui::Margin::same(14);
    style.spacing.indent = 14.0;
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::new(17.0, egui::FontFamily::Name("bold".into())),
    );
    style.text_styles.insert(
        egui::TextStyle::Body,
        egui::FontId::new(12.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        egui::FontId::new(12.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Monospace,
        egui::FontId::new(12.0, egui::FontFamily::Monospace),
    );
    style.text_styles.insert(
        egui::TextStyle::Small,
        egui::FontId::new(10.5, egui::FontFamily::Proportional),
    );

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
    context.set_style_of(egui_theme, style.clone());

    let mut dark_style = style;
    let mut dark_vis = egui::Visuals::dark();
    let d_surface = egui::Color32::from_rgb(0x11, 0x18, 0x27);
    let d_subtle = egui::Color32::from_rgb(0x1E, 0x29, 0x3B);
    let d_sel = egui::Color32::from_rgb(0x1E, 0x3A, 0x8A);
    let d_border = egui::Color32::from_rgb(0x33, 0x41, 0x55);
    let d_text = egui::Color32::from_rgb(0xF1, 0xF5, 0xF9);
    dark_vis.panel_fill = d_surface;
    dark_vis.window_fill = d_surface;
    dark_vis.extreme_bg_color = d_surface;
    dark_vis.faint_bg_color = d_subtle;
    dark_vis.override_text_color = Some(d_text);
    dark_vis.widgets.noninteractive.bg_fill = d_surface;
    dark_vis.widgets.noninteractive.weak_bg_fill = d_surface;
    dark_vis.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, d_border);
    dark_vis.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, d_text);
    dark_vis.widgets.inactive.bg_fill = d_surface;
    dark_vis.widgets.inactive.weak_bg_fill = d_surface;
    dark_vis.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, d_border);
    dark_vis.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, d_text);
    dark_vis.widgets.inactive.corner_radius = egui::CornerRadius::same(6);
    dark_vis.widgets.hovered.bg_fill = d_subtle;
    dark_vis.widgets.hovered.weak_bg_fill = d_subtle;
    dark_vis.widgets.hovered.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(0x60, 0xA5, 0xFA));
    dark_vis.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, d_text);
    dark_vis.widgets.hovered.corner_radius = egui::CornerRadius::same(6);
    dark_vis.widgets.active.bg_fill = d_sel;
    dark_vis.widgets.active.weak_bg_fill = d_sel;
    dark_vis.widgets.active.bg_stroke =
        egui::Stroke::new(1.5, egui::Color32::from_rgb(0x60, 0xA5, 0xFA));
    dark_vis.widgets.active.fg_stroke = egui::Stroke::new(1.5, d_text);
    dark_vis.widgets.active.corner_radius = egui::CornerRadius::same(6);
    dark_vis.selection.bg_fill = d_sel;
    dark_vis.selection.stroke = egui::Stroke::new(1.5, egui::Color32::from_rgb(0x60, 0xA5, 0xFA));
    dark_vis.window_stroke = egui::Stroke::new(1.0, d_border);
    dark_vis.window_corner_radius = egui::CornerRadius::same(10);
    dark_style.visuals = dark_vis;
    context.set_style_of(egui::Theme::Dark, dark_style);

    context.set_theme(egui_theme);
    context.enable_accesskit();
}

pub fn apply_theme(context: &egui::Context, dark_mode: bool) {
    context.set_theme(if dark_mode {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    });
}

/// Compact Figma Input component: 29px tall bordered white input with crisp focus ring.
pub fn text_input(
    ui: &mut egui::Ui,
    value: &mut String,
    hint: &str,
    width: f32,
    password: bool,
) -> egui::Response {
    let inner_w = (width - 16.0).max(56.0);
    let mut inner_resp: Option<egui::Response> = None;
    egui::Frame::new()
        .fill(surface_bg(ui))
        .stroke(egui::Stroke::new(1.0, border_color(ui)))
        .corner_radius(5)
        .inner_margin(egui::Margin::symmetric(8, 5))
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

const FIGMA_BRAND_MARK_PNG: &[u8] = include_bytes!("../../../assets/brand-mark.png");
const FIGMA_GOOGLE_G_PNG: &[u8] = include_bytes!("../../../assets/google-g.png");

const ICON_BLOCKS_PNG: &[u8] = include_bytes!("../../../assets/icons/blocks.png");
const ICON_BOOK_OPEN_PNG: &[u8] = include_bytes!("../../../assets/icons/book-open.png");
const ICON_PLAY_PNG: &[u8] = include_bytes!("../../../assets/icons/play.png");
const ICON_GIT_BRANCH_PNG: &[u8] = include_bytes!("../../../assets/icons/git-branch.png");
const ICON_USERS_PNG: &[u8] = include_bytes!("../../../assets/icons/users.png");
const ICON_BELL_PNG: &[u8] = include_bytes!("../../../assets/icons/bell.png");
const ICON_TERMINAL_PNG: &[u8] = include_bytes!("../../../assets/icons/terminal.png");
const ICON_SETTINGS_PNG: &[u8] = include_bytes!("../../../assets/icons/settings.png");
const ICON_FOLDER_PNG: &[u8] = include_bytes!("../../../assets/icons/folder.png");
const ICON_FILE_CODE_PNG: &[u8] = include_bytes!("../../../assets/icons/file-code.png");
const ICON_HISTORY_PNG: &[u8] = include_bytes!("../../../assets/icons/history.png");
const ICON_GLOBE_PNG: &[u8] = include_bytes!("../../../assets/icons/globe.png");
const ICON_KEY_ROUND_PNG: &[u8] = include_bytes!("../../../assets/icons/key-round.png");
const ICON_SEARCH_PNG: &[u8] = include_bytes!("../../../assets/icons/search.png");
const ICON_PLUS_PNG: &[u8] = include_bytes!("../../../assets/icons/plus.png");
const ICON_CHEVRON_DOWN_PNG: &[u8] = include_bytes!("../../../assets/icons/chevron-down.png");
const ICON_CHEVRON_RIGHT_PNG: &[u8] = include_bytes!("../../../assets/icons/chevron-right.png");
const ICON_SHIELD_CHECK_PNG: &[u8] = include_bytes!("../../../assets/icons/shield-check.png");
const ICON_CIRCLE_ALERT_PNG: &[u8] = include_bytes!("../../../assets/icons/circle-alert.png");
const ICON_CHECK_PNG: &[u8] = include_bytes!("../../../assets/icons/check.png");
const ICON_COPY_PNG: &[u8] = include_bytes!("../../../assets/icons/copy.png");
const ICON_TRASH_PNG: &[u8] = include_bytes!("../../../assets/icons/trash.png");

fn icon_asset_spec(kind: IconKind) -> (&'static str, &'static [u8]) {
    match kind {
        IconKind::Blocks => ("figma_icon_blocks", ICON_BLOCKS_PNG),
        IconKind::BookOpen => ("figma_icon_book_open", ICON_BOOK_OPEN_PNG),
        IconKind::Play => ("figma_icon_play", ICON_PLAY_PNG),
        IconKind::GitBranch => ("figma_icon_git_branch", ICON_GIT_BRANCH_PNG),
        IconKind::Users => ("figma_icon_users", ICON_USERS_PNG),
        IconKind::Bell => ("figma_icon_bell", ICON_BELL_PNG),
        IconKind::Terminal => ("figma_icon_terminal", ICON_TERMINAL_PNG),
        IconKind::Settings => ("figma_icon_settings", ICON_SETTINGS_PNG),
        IconKind::Folder => ("figma_icon_folder", ICON_FOLDER_PNG),
        IconKind::FileCode => ("figma_icon_file_code", ICON_FILE_CODE_PNG),
        IconKind::History => ("figma_icon_history", ICON_HISTORY_PNG),
        IconKind::Globe => ("figma_icon_globe", ICON_GLOBE_PNG),
        IconKind::KeyRound => ("figma_icon_key_round", ICON_KEY_ROUND_PNG),
        IconKind::Search => ("figma_icon_search", ICON_SEARCH_PNG),
        IconKind::Plus => ("figma_icon_plus", ICON_PLUS_PNG),
        IconKind::ChevronDown => ("figma_icon_chevron_down", ICON_CHEVRON_DOWN_PNG),
        IconKind::ChevronRight => ("figma_icon_chevron_right", ICON_CHEVRON_RIGHT_PNG),
        IconKind::ShieldCheck => ("figma_icon_shield_check", ICON_SHIELD_CHECK_PNG),
        IconKind::AlertCircle => ("figma_icon_circle_alert", ICON_CIRCLE_ALERT_PNG),
        IconKind::Check => ("figma_icon_check", ICON_CHECK_PNG),
        IconKind::Copy => ("figma_icon_copy", ICON_COPY_PNG),
        IconKind::Trash => ("figma_icon_trash", ICON_TRASH_PNG),
    }
}

fn load_figma_texture(
    ctx: &egui::Context,
    key: &'static str,
    png_bytes: &'static [u8],
) -> Option<egui::TextureHandle> {
    let id = egui::Id::unique(key);
    if let Some(existing) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return Some(existing);
    }
    let decoded = eframe::icon_data::from_png_bytes(png_bytes).ok()?;
    let image = egui::ColorImage::from_rgba_unmultiplied(
        [decoded.width as usize, decoded.height as usize],
        &decoded.rgba,
    );
    let handle = ctx.load_texture(key, image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, handle.clone()));
    Some(handle)
}

/// Figma A01 Google Sign-In button (44px tall, full width, real Figma Brand/Google 11:12128 asset + centered label).
pub fn google_button(ui: &mut egui::Ui, enabled: bool, label: &str) -> egui::Response {
    let w = ui.available_width().max(220.0);
    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(w, 44.0),
        if enabled {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
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
    ui.painter().rect(
        rect,
        8.0,
        fill,
        egui::Stroke::new(1.0, stroke_col),
        egui::StrokeKind::Middle,
    );

    let font_id = egui::FontId::new(13.5, egui::FontFamily::Name("bold".into()));
    let text_color = if enabled {
        text_primary(ui)
    } else {
        text_secondary(ui)
    };
    let galley = ui.fonts_mut(|f| f.layout_no_wrap(label.to_owned(), font_id, text_color));
    let icon_d = 18.0;
    let gap = 10.0;
    let total_w = icon_d + gap + galley.size().x;
    let start_x = rect.center().x - total_w * 0.5;
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(start_x + icon_d * 0.5, rect.center().y),
        egui::vec2(icon_d, icon_d),
    );
    if let Some(tex) =
        load_figma_texture(ui.ctx(), "figma_brand_google_11_12128", FIGMA_GOOGLE_G_PNG)
    {
        ui.painter().image(
            tex.id(),
            icon_rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }
    ui.painter().galley(
        egui::pos2(
            start_x + icon_d + gap,
            rect.center().y - galley.size().y * 0.5,
        ),
        galley,
        text_color,
    );
    resp
}

/// Renders the exact Figma Swaggen Brand/Mark (node 4:81) as a clickable Home trigger.
pub fn brand_mark(ui: &mut egui::Ui, size: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        ui.painter().rect(
            rect.expand(4.0),
            7.0,
            subtle_bg(ui),
            egui::Stroke::new(1.0, border_color(ui)),
            egui::StrokeKind::Middle,
        );
    }
    if let Some(tex) = load_figma_texture(ui.ctx(), "figma_brand_mark_4_81", FIGMA_BRAND_MARK_PNG) {
        ui.painter().image(
            tex.id(),
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }
    resp.on_hover_text("Home (Workspaces overview)")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MiniIconKind {
    Plus,
    Minus,
    Close,
    More,
    Up,
    Down,
}

/// Crisp rounded 22x22 vector action button for `+`, `-`, `×`, `⋯`, `↑`, `↓`.
pub fn mini_icon_button(
    ui: &mut egui::Ui,
    _id_salt: impl std::hash::Hash,
    kind: MiniIconKind,
    hover_accent: egui::Color32,
    tooltip: &str,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let fill = if resp.hovered() {
        selected_bg(ui)
    } else {
        subtle_bg(ui)
    };
    let stroke_col = if resp.hovered() {
        hover_accent
    } else {
        border_color(ui)
    };
    ui.painter().rect(
        rect,
        5.0,
        fill,
        egui::Stroke::new(1.0, stroke_col),
        egui::StrokeKind::Middle,
    );
    let fg = if resp.hovered() {
        hover_accent
    } else {
        text_secondary(ui)
    };
    paint_mini_icon(ui.painter(), rect, kind, fg);
    if tooltip.is_empty() {
        resp
    } else {
        resp.on_hover_text(tooltip)
    }
}

/// Paints a mini crisp vector icon (`+`, `-`, `×`, `⋯`, `↑`, `↓`) centered inside `rect`.
pub fn paint_mini_icon(
    painter: &egui::Painter,
    rect: egui::Rect,
    kind: MiniIconKind,
    fg: egui::Color32,
) {
    let c = rect.center();
    let r = (rect.width().min(rect.height()) * 0.24).clamp(3.0, 5.5);
    let st = egui::Stroke::new(1.5, fg);
    match kind {
        MiniIconKind::Plus => {
            painter.line_segment([egui::pos2(c.x - r, c.y), egui::pos2(c.x + r, c.y)], st);
            painter.line_segment([egui::pos2(c.x, c.y - r), egui::pos2(c.x, c.y + r)], st);
        }
        MiniIconKind::Minus => {
            painter.line_segment([egui::pos2(c.x - r, c.y), egui::pos2(c.x + r, c.y)], st);
        }
        MiniIconKind::Close => {
            let d = r * 0.85;
            painter.line_segment(
                [egui::pos2(c.x - d, c.y - d), egui::pos2(c.x + d, c.y + d)],
                st,
            );
            painter.line_segment(
                [egui::pos2(c.x - d, c.y + d), egui::pos2(c.x + d, c.y - d)],
                st,
            );
        }
        MiniIconKind::More => {
            for dx in [-4.0_f32, 0.0, 4.0] {
                painter.circle_filled(egui::pos2(c.x + dx, c.y), 1.35, fg);
            }
        }
        MiniIconKind::Up => {
            painter.line_segment(
                [
                    egui::pos2(c.x - r, c.y + 1.5),
                    egui::pos2(c.x, c.y - r + 1.0),
                ],
                st,
            );
            painter.line_segment(
                [
                    egui::pos2(c.x + r, c.y + 1.5),
                    egui::pos2(c.x, c.y - r + 1.0),
                ],
                st,
            );
        }
        MiniIconKind::Down => {
            painter.line_segment(
                [
                    egui::pos2(c.x - r, c.y - 1.5),
                    egui::pos2(c.x, c.y + r - 1.0),
                ],
                st,
            );
            painter.line_segment(
                [
                    egui::pos2(c.x + r, c.y - 1.5),
                    egui::pos2(c.x, c.y + r - 1.0),
                ],
                st,
            );
        }
    }
}

/// Paints authentic Figma `Icon/*` transparent PNG assets tinted cleanly with `color`.
pub fn paint_icon(painter: &egui::Painter, rect: egui::Rect, kind: IconKind, color: egui::Color32) {
    let (key, png_bytes) = icon_asset_spec(kind);
    if let Some(tex) = load_figma_texture(painter.ctx(), key, png_bytes) {
        painter.image(
            tex.id(),
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            color,
        );
    }
}

/// Activity rail icon button matching Figma `Shell/Activity rail` (`6:1913`), compact 34px scale.
pub fn rail_icon_button(
    ui: &mut egui::Ui,
    selected: bool,
    kind: IconKind,
    tooltip: &str,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(34.0, 34.0), egui::Sense::click());
    let fill = if selected {
        Tokens::BG_SELECTED
    } else if resp.hovered() {
        Tokens::BG_SUBTLE
    } else {
        Tokens::BG_SURFACE
    };
    let icon_color = if selected {
        Tokens::STATUS_INFO
    } else {
        icon_semantic_color(kind)
    };
    ui.painter().rect_filled(rect, 7.0, fill);
    if selected {
        ui.painter().rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(rect.left() - 5.0, rect.top() + 7.0),
                egui::vec2(2.5, rect.height() - 14.0),
            ),
            1.5,
            Tokens::STATUS_INFO,
        );
    }
    paint_icon(ui.painter(), rect.shrink(8.5), kind, icon_color);
    resp.on_hover_text(tooltip)
}

/// Compact 24x24 icon-only button for switching Workbench split layout (Top/Bottom vs Left/Right).
pub fn split_layout_icon_button(ui: &mut egui::Ui, side_by_side: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::click());
    let fill = if resp.hovered() {
        Tokens::BG_SELECTED
    } else {
        Tokens::BG_SURFACE
    };
    let border = if resp.hovered() {
        Tokens::STATUS_INFO
    } else {
        Tokens::BORDER_DEFAULT
    };
    let fg = if resp.hovered() {
        Tokens::STATUS_INFO
    } else {
        Tokens::TEXT_SECONDARY
    };
    ui.painter().rect(
        rect,
        5.0,
        fill,
        egui::Stroke::new(1.0, border),
        egui::StrokeKind::Middle,
    );

    let box_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(13.0, 11.0));
    let shade = egui::Color32::from_rgba_unmultiplied(fg.r(), fg.g(), fg.b(), 38);
    if side_by_side {
        let left_half = egui::Rect::from_min_max(
            box_rect.min,
            egui::pos2(box_rect.center().x, box_rect.bottom()),
        );
        ui.painter().rect_filled(left_half, 1.5, shade);
        ui.painter().line_segment(
            [
                egui::pos2(box_rect.center().x, box_rect.top()),
                egui::pos2(box_rect.center().x, box_rect.bottom()),
            ],
            egui::Stroke::new(1.25, fg),
        );
    } else {
        let top_half = egui::Rect::from_min_max(
            box_rect.min,
            egui::pos2(box_rect.right(), box_rect.center().y),
        );
        ui.painter().rect_filled(top_half, 1.5, shade);
        ui.painter().line_segment(
            [
                egui::pos2(box_rect.left(), box_rect.center().y),
                egui::pos2(box_rect.right(), box_rect.center().y),
            ],
            egui::Stroke::new(1.25, fg),
        );
    }
    ui.painter().rect_stroke(
        box_rect,
        2.0,
        egui::Stroke::new(1.25, fg),
        egui::StrokeKind::Middle,
    );

    let tip = if side_by_side {
        "Layout: Side-by-Side (click to switch to Top / Bottom)"
    } else {
        "Layout: Top / Bottom (click to switch to Side-by-Side)"
    };
    resp.on_hover_text(tip)
}

/// Compact 27x27 icon-only Filter button for the Sidebar search bar.
pub fn filter_icon_button(ui: &mut egui::Ui, active: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(27.0, 27.0), egui::Sense::click());
    let fill = if active || resp.hovered() {
        Tokens::BG_SELECTED
    } else {
        Tokens::BG_SURFACE
    };
    let border = if active || resp.hovered() {
        Tokens::STATUS_INFO
    } else {
        Tokens::BORDER_DEFAULT
    };
    let fg = if active || resp.hovered() {
        Tokens::STATUS_INFO
    } else {
        Tokens::TEXT_SECONDARY
    };
    ui.painter().rect(
        rect,
        5.0,
        fill,
        egui::Stroke::new(1.0, border),
        egui::StrokeKind::Middle,
    );
    let c = rect.center();
    for (dy, half_w) in [(-3.5_f32, 5.5_f32), (0.0, 3.6), (3.5, 1.8)] {
        ui.painter().line_segment(
            [
                egui::pos2(c.x - half_w, c.y + dy),
                egui::pos2(c.x + half_w, c.y + dy),
            ],
            egui::Stroke::new(1.4, fg),
        );
    }
    resp.on_hover_text("Filter requests by HTTP method or tag")
}

/// Compact Sidebar navigation row (28px tall).
pub fn nav_item(
    ui: &mut egui::Ui,
    kind: IconKind,
    label: &str,
    count: Option<usize>,
    selected: bool,
) -> egui::Response {
    let w = ui.available_width().max(100.0);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 28.0), egui::Sense::click());
    let bg = if selected {
        Tokens::BG_SELECTED
    } else if resp.hovered() {
        Tokens::BG_SUBTLE
    } else {
        Tokens::BG_SURFACE
    };
    ui.painter().rect_filled(rect, 5.0, bg);
    let icon_color = if selected {
        Tokens::STATUS_INFO
    } else {
        icon_semantic_color(kind)
    };
    let text_color = if selected {
        Tokens::STATUS_INFO
    } else {
        Tokens::TEXT_PRIMARY
    };
    let icon_rect =
        egui::Rect::from_min_size(rect.min + egui::vec2(8.0, 6.0), egui::vec2(16.0, 16.0));
    paint_icon(ui.painter(), icon_rect, kind, icon_color);
    let font = if selected {
        egui::FontId::new(12.0, egui::FontFamily::Name("bold".into()))
    } else {
        egui::FontId::new(12.0, egui::FontFamily::Proportional)
    };
    ui.painter().text(
        rect.min + egui::vec2(30.0, rect.height() * 0.5),
        egui::Align2::LEFT_CENTER,
        label,
        font,
        text_color,
    );
    if let Some(n) = count {
        ui.painter().text(
            egui::pos2(rect.right() - 8.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            n.to_string(),
            egui::FontId::new(10.5, egui::FontFamily::Monospace),
            if selected {
                Tokens::STATUS_INFO
            } else {
                Tokens::TEXT_SECONDARY
            },
        );
    }
    resp
}

/// Compact Sidebar Endpoint row (26px tall).
pub fn endpoint_row(ui: &mut egui::Ui, method: &str, name: &str, selected: bool) -> egui::Response {
    let w = ui.available_width().max(100.0);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 26.0), egui::Sense::click());
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
        .rect(rect, 5.0, bg, stroke, egui::StrokeKind::Middle);

    let m = method.to_ascii_uppercase();
    ui.painter().text(
        rect.min + egui::vec2(8.0, rect.height() * 0.5),
        egui::Align2::LEFT_CENTER,
        &m,
        egui::FontId::new(10.0, egui::FontFamily::Monospace),
        method_color(&m),
    );
    let max_chars = ((w - 58.0) / 6.6).max(8.0) as usize;
    let clipped: String = if name.chars().count() > max_chars {
        let mut s: String = name.chars().take(max_chars.saturating_sub(1)).collect();
        s.push('…');
        s
    } else {
        name.to_owned()
    };
    ui.painter().text(
        rect.min + egui::vec2(48.0, rect.height() * 0.5),
        egui::Align2::LEFT_CENTER,
        clipped,
        egui::FontId::new(11.5, egui::FontFamily::Proportional),
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
    page_header_styled(ui, title, subtitle, action_label, action_enabled, true)
}

pub fn page_header_secondary(
    ui: &mut egui::Ui,
    title: &str,
    subtitle: &str,
    action_label: Option<&str>,
    action_enabled: bool,
) -> bool {
    page_header_styled(ui, title, subtitle, action_label, action_enabled, false)
}

fn page_header_styled(
    ui: &mut egui::Ui,
    title: &str,
    subtitle: &str,
    action_label: Option<&str>,
    action_enabled: bool,
    primary_action: bool,
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
                let btn = if primary_action {
                    primary_button(ui, action_enabled, lbl)
                } else {
                    secondary_button(ui, action_enabled, lbl)
                };
                if btn.clicked() {
                    clicked = true;
                }
            });
        }
    });
    ui.add_space(10.0);
    clicked
}

pub fn section(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    let _ = page_header(ui, title, subtitle, None, false);
}

/// Figma clean bordered card (corner radius 10, padding 24x20).
pub fn surface_card(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(surface_bg(ui))
        .stroke(egui::Stroke::new(1.0, border_color(ui)))
        .corner_radius(10)
        .inner_margin(egui::Margin::symmetric(24, 18))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            add_contents(ui);
        });
}

/// Clean card section title (without fake non-clickable chevron).
pub fn card_header(ui: &mut egui::Ui, title: &str) {
    ui.horizontal(|ui| {
        ui.label(bold_text(title, 13.5, text_primary(ui)));
    });
    ui.add_space(8.0);
}

/// Interactive collapsible card header with real toggle chevron (matches Figma R01 cards).
pub fn collapsible_card_header(ui: &mut egui::Ui, title: &str, collapsed: &mut bool) -> bool {
    let w = ui.available_width().max(120.0);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 24.0), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 6.0, Tokens::BG_SUBTLE);
    }
    ui.painter().text(
        egui::pos2(rect.left() + 4.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::new(13.5, egui::FontFamily::Name("bold".into())),
        Tokens::TEXT_PRIMARY,
    );
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(rect.right() - 12.0, rect.center().y),
        egui::vec2(16.0, 16.0),
    );
    paint_icon(
        ui.painter(),
        icon_rect,
        if *collapsed {
            IconKind::ChevronRight
        } else {
            IconKind::ChevronDown
        },
        Tokens::TEXT_SECONDARY,
    );
    let toggled = resp.clicked();
    if toggled {
        *collapsed = !*collapsed;
    }
    ui.add_space(6.0);
    toggled
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
                    ui.set_min_size(egui::vec2(*w, 24.0));
                    ui.set_max_size(egui::vec2(*w, 24.0));
                    ui.label(
                        egui::RichText::new(*title)
                            .size(11.0)
                            .color(text_secondary(ui))
                            .strong(),
                    );
                },
            );
        }
    });
    ui.add_space(4.0);
}

/// Compact table row container (28px tall, white fill, 1px #E5E7EB border, 5px radius).
pub fn table_row_frame(ui: &mut egui::Ui, add_cells: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(surface_bg(ui))
        .stroke(egui::Stroke::new(1.0, border_color(ui)))
        .corner_radius(5)
        .inner_margin(egui::Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| add_cells(ui));
        });
    ui.add_space(2.0);
}

/// Compact underline Tab component (27px high, active `#1D4ED8` underline indicator).
pub fn underline_tab(ui: &mut egui::Ui, active: bool, label: &str) -> egui::Response {
    let text_color = if active {
        text_primary(ui)
    } else {
        text_secondary(ui)
    };
    let resp = ui.add(
        egui::Button::new(if active {
            bold_text(label, 12.0, text_color)
        } else {
            egui::RichText::new(label).size(12.0).color(text_color)
        })
        .fill(egui::Color32::TRANSPARENT)
        .stroke(egui::Stroke::NONE)
        .min_size(egui::vec2(48.0, 27.0))
        .corner_radius(5),
    );
    if active {
        let r = resp.rect;
        ui.painter().line_segment(
            [
                egui::pos2(r.left() + 6.0, r.bottom() - 1.5),
                egui::pos2(r.right() - 6.0, r.bottom() - 1.5),
            ],
            egui::Stroke::new(2.0, Tokens::STATUS_INFO),
        );
    }
    resp
}

/// Compact Primary button (28px height, crisp `#1D4ED8` label).
pub fn primary_button(ui: &mut egui::Ui, enabled: bool, label: &str) -> egui::Response {
    let color = if enabled {
        Tokens::STATUS_INFO
    } else {
        text_secondary(ui)
    };
    ui.add_enabled(
        enabled,
        egui::Button::new(bold_text(label, 12.0, color))
            .fill(if enabled {
                surface_bg(ui)
            } else {
                subtle_bg(ui)
            })
            .stroke(egui::Stroke::new(
                1.0,
                if enabled {
                    Tokens::STATUS_INFO
                } else {
                    border_color(ui)
                },
            ))
            .min_size(egui::vec2(76.0, 28.0))
            .corner_radius(5),
    )
}

pub fn secondary_button(ui: &mut egui::Ui, enabled: bool, label: &str) -> egui::Response {
    let color = if enabled {
        text_primary(ui)
    } else {
        text_secondary(ui)
    };
    ui.add_enabled(
        enabled,
        egui::Button::new(egui::RichText::new(label).size(12.0).color(color))
            .fill(if enabled {
                surface_bg(ui)
            } else {
                subtle_bg(ui)
            })
            .stroke(egui::Stroke::new(1.0, border_color(ui)))
            .min_size(egui::vec2(58.0, 28.0))
            .corner_radius(5),
    )
}

pub fn danger_button(ui: &mut egui::Ui, enabled: bool, label: &str) -> egui::Response {
    let color = if enabled {
        Tokens::TEXT_DANGER
    } else {
        text_secondary(ui)
    };
    ui.add_enabled(
        enabled,
        egui::Button::new(bold_text(label, 12.0, color))
            .fill(if enabled {
                surface_bg(ui)
            } else {
                subtle_bg(ui)
            })
            .stroke(egui::Stroke::new(1.0, border_color(ui)))
            .min_size(egui::vec2(68.0, 28.0))
            .corner_radius(5),
    )
}

/// Clean compact bordered badge (20px tall).
pub fn pill_badge(ui: &mut egui::Ui, color: egui::Color32, text: &str) {
    egui::Frame::new()
        .fill(surface_bg(ui))
        .stroke(egui::Stroke::new(1.0, border_color(ui)))
        .corner_radius(5)
        .inner_margin(egui::Margin::symmetric(7, 2))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).color(color).size(10.5));
        });
}

pub fn status_badge(ui: &mut egui::Ui, kind: NoticeKind, text: &str) {
    pill_badge(ui, kind.color(), text);
}

pub fn avatar_circle(ui: &mut egui::Ui, initials: &str, tone: AvatarTone) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(26.0, 26.0), egui::Sense::click());
    let fill = match tone {
        AvatarTone::Teal => egui::Color32::from_rgb(0xEC, 0xFD, 0xF5),
        AvatarTone::Salmon => egui::Color32::from_rgb(0xFF, 0xF1, 0xEE),
        AvatarTone::Cream => egui::Color32::from_rgb(0xFF, 0xFA, 0xEB),
    };
    ui.painter().circle(
        rect.center(),
        12.5,
        fill,
        egui::Stroke::new(1.0, Tokens::BORDER_DEFAULT),
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        initials,
        egui::FontId::new(10.0, egui::FontFamily::Monospace),
        Tokens::TEXT_PRIMARY,
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
        "GQL" | "GRAPHQL" => egui::Color32::from_rgb(0x7C, 0x3A, 0xED),
        _ => Tokens::TEXT_PRIMARY,
    }
}

/// Compact 3-4 character sidebar badge so wide HTTP methods (DELETE/OPTIONS) never touch labels.
pub fn compact_method_badge(method: &str) -> &'static str {
    match method.to_ascii_uppercase().as_str() {
        "GET" => "GET",
        "POST" => "POST",
        "PUT" => "PUT",
        "PATCH" => "PAT",
        "DELETE" => "DEL",
        "HEAD" => "HEAD",
        "OPTIONS" => "OPT",
        "GQL" | "GRAPHQL" => "GQL",
        _ => "REQ",
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

/// Razor-thin 1px interactive horizontal splitter bar anchored by a stable `egui::Id`.
pub fn horizontal_splitter(ui: &mut egui::Ui, id_salt: &str) -> (egui::Response, f32) {
    let w = ui.available_width().max(60.0);
    let (_, rect) = ui.allocate_space(egui::vec2(w, 5.0));
    let id = egui::Id::unique(("swaggen_hsplit", id_salt));
    let resp = ui.interact(rect, id, egui::Sense::click_and_drag());
    let active = resp.hovered() || resp.dragged();
    if active {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    if resp.dragged() {
        ui.ctx().request_repaint();
    }
    let line_color = if active {
        Tokens::STATUS_INFO
    } else {
        border_color(ui)
    };
    let cy = rect.center().y;
    ui.painter().line_segment(
        [egui::pos2(rect.left(), cy), egui::pos2(rect.right(), cy)],
        egui::Stroke::new(if active { 1.5 } else { 1.0 }, line_color),
    );
    let delta_y = if resp.dragged() {
        resp.drag_delta().y
    } else {
        0.0
    };
    (resp, delta_y)
}

/// Razor-thin 1px interactive vertical splitter bar anchored by a stable `egui::Id`.
pub fn vertical_splitter(ui: &mut egui::Ui, id_salt: &str, height: f32) -> (egui::Response, f32) {
    let h = height.max(60.0);
    let (_, rect) = ui.allocate_space(egui::vec2(5.0, h));
    let id = egui::Id::unique(("swaggen_vsplit", id_salt));
    let resp = ui.interact(rect, id, egui::Sense::click_and_drag());
    let active = resp.hovered() || resp.dragged();
    if active {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    if resp.dragged() {
        ui.ctx().request_repaint();
    }
    let line_color = if active {
        Tokens::STATUS_INFO
    } else {
        border_color(ui)
    };
    let cx = rect.center().x;
    ui.painter().line_segment(
        [egui::pos2(cx, rect.top()), egui::pos2(cx, rect.bottom())],
        egui::Stroke::new(if active { 1.5 } else { 1.0 }, line_color),
    );
    let delta_x = if resp.dragged() {
        resp.drag_delta().x
    } else {
        0.0
    };
    (resp, delta_x)
}

/// Line-numbered monospace code surface with compact IDE-grade spacing & lightweight JSON syntax highlighting.
pub fn code_surface_numbered(ui: &mut egui::Ui, text: &str) {
    let mono_font = egui::FontId::new(12.0, egui::FontFamily::Monospace);
    let color_key = egui::Color32::from_rgb(0x0F, 0x76, 0x6E);
    let color_str = egui::Color32::from_rgb(0x15, 0x80, 0x3D);
    let color_num = egui::Color32::from_rgb(0xB4, 0x53, 0x09);
    let color_kw = egui::Color32::from_rgb(0x7C, 0x3A, 0xED);
    let color_punct = Tokens::TEXT_SECONDARY;
    let color_plain = text_primary(ui);

    egui::Frame::new()
        .fill(surface_bg(ui))
        .stroke(egui::Stroke::new(1.0, border_color(ui)))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 1.0);
            ui.spacing_mut().interact_size.y = 15.0;
            let mut total_lines = 0usize;
            for (idx, line) in text.lines().enumerate() {
                total_lines = idx + 1;
                if idx >= 2000 {
                    continue;
                }
                ui.horizontal(|ui| {
                    ui.spacing_mut().interact_size.y = 15.0;
                    ui.allocate_ui_with_layout(
                        egui::vec2(30.0, 15.0),
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            ui.label(
                                egui::RichText::new(format!("{:02}", idx + 1))
                                    .monospace()
                                    .size(10.5)
                                    .color(text_secondary(ui)),
                            );
                        },
                    );
                    let job = build_highlighted_line_job(
                        line,
                        &mono_font,
                        color_key,
                        color_str,
                        color_num,
                        color_kw,
                        color_punct,
                        color_plain,
                    );
                    ui.add(egui::Label::new(job).extend().selectable(true));
                });
            }
            if total_lines > 2000 {
                ui.label(
                    egui::RichText::new(format!(
                        "… showing first 2,000 lines ({total_lines} total lines). Use Copy or Save to export full body."
                    ))
                    .monospace()
                    .size(11.5)
                    .color(Tokens::TEXT_WARNING),
                );
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

#[allow(clippy::too_many_arguments)]
fn build_highlighted_line_job(
    line: &str,
    font: &egui::FontId,
    color_key: egui::Color32,
    color_str: egui::Color32,
    color_num: egui::Color32,
    color_kw: egui::Color32,
    color_punct: egui::Color32,
    color_plain: egui::Color32,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let trimmed = line.trim_start();
    let indent_len = line.len() - trimmed.len();
    if indent_len > 0 {
        job.append(
            &line[..indent_len],
            0.0,
            egui::TextFormat::simple(font.clone(), color_plain),
        );
    }

    // Fast path for JSON `"key": value` lines
    if trimmed.starts_with('"')
        && let Some(colon_pos) = find_json_key_colon(trimmed)
    {
        let key_part = &trimmed[..=colon_pos];
        let after_colon = &trimmed[colon_pos + 1..];
        let key_only = &key_part[..key_part.len() - 1];
        job.append(
            key_only,
            0.0,
            egui::TextFormat::simple(font.clone(), color_key),
        );
        job.append(
            ":",
            0.0,
            egui::TextFormat::simple(font.clone(), color_punct),
        );

        let val_trim_start = after_colon.len() - after_colon.trim_start().len();
        if val_trim_start > 0 {
            job.append(
                &after_colon[..val_trim_start],
                0.0,
                egui::TextFormat::simple(font.clone(), color_plain),
            );
        }
        let rest = &after_colon[val_trim_start..];
        let (core, trailing_comma) = if let Some(stripped) = rest.strip_suffix(',') {
            (stripped, ",")
        } else {
            (rest, "")
        };
        let val_col = if core.starts_with('"') && core.ends_with('"') {
            color_str
        } else if matches!(core, "true" | "false" | "null") {
            color_kw
        } else if matches!(core, "{" | "}" | "[" | "]" | "{}" | "[]") {
            color_punct
        } else if !core.is_empty()
            && core
                .bytes()
                .all(|b| b.is_ascii_digit() || matches!(b, b'-' | b'+' | b'.' | b'e' | b'E'))
        {
            color_num
        } else {
            color_plain
        };
        if !core.is_empty() {
            job.append(core, 0.0, egui::TextFormat::simple(font.clone(), val_col));
        }
        if !trailing_comma.is_empty() {
            job.append(
                trailing_comma,
                0.0,
                egui::TextFormat::simple(font.clone(), color_punct),
            );
        }
        return job;
    }

    let plain_col = if matches!(
        trimmed,
        "{" | "}" | "[" | "]" | "{," | "}," | "]," | "[," | "{}" | "[]"
    ) {
        color_punct
    } else if trimmed.starts_with('"') {
        color_str
    } else {
        color_plain
    };
    job.append(
        trimmed,
        0.0,
        egui::TextFormat::simple(font.clone(), plain_col),
    );
    job
}

fn find_json_key_colon(trimmed: &str) -> Option<usize> {
    let bytes = trimmed.as_bytes();
    if bytes.first().copied() != Some(b'"') {
        return None;
    }
    let mut i = 1usize;
    let mut escaped = false;
    while i < bytes.len() {
        if escaped {
            escaped = false;
        } else if bytes[i] == b'\\' {
            escaped = true;
        } else if bytes[i] == b'"' {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b':' {
                return Some(j);
            }
            return None;
        }
        i += 1;
    }
    None
}

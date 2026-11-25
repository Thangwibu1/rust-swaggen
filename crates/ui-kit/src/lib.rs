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

pub fn page_bg(_ui: &egui::Ui) -> egui::Color32 {
    Tokens::BG_PAGE
}

pub fn surface_bg(_ui: &egui::Ui) -> egui::Color32 {
    Tokens::BG_SURFACE
}

pub fn subtle_bg(_ui: &egui::Ui) -> egui::Color32 {
    Tokens::BG_SUBTLE
}

pub fn selected_bg(_ui: &egui::Ui) -> egui::Color32 {
    Tokens::BG_SELECTED
}

pub fn border_color(_ui: &egui::Ui) -> egui::Color32 {
    Tokens::BORDER_DEFAULT
}

pub fn text_primary(_ui: &egui::Ui) -> egui::Color32 {
    Tokens::TEXT_PRIMARY
}

pub fn text_secondary(_ui: &egui::Ui) -> egui::Color32 {
    Tokens::TEXT_SECONDARY
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

/// Installs the authentic Figma white-surface Light design system & JetBrains Mono fonts.
pub fn install(context: &egui::Context) {
    configure_fonts(context);

    let egui_theme = egui::Theme::Light;
    let mut style = (*context.style_of(egui_theme)).clone();
    style.spacing.item_spacing = egui::vec2(6.0, 4.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
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
    context.set_style_of(egui_theme, style);
    context.set_theme(egui_theme);
    context.enable_accesskit();
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

/// Renders the exact Figma Swaggen Brand/Mark (node 4:81) exported directly from Figma.
pub fn brand_mark(ui: &mut egui::Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    if let Some(tex) = load_figma_texture(ui.ctx(), "figma_brand_mark_4_81", FIGMA_BRAND_MARK_PNG) {
        ui.painter().image(
            tex.id(),
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
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
        Tokens::TEXT_PRIMARY
    } else {
        Tokens::TEXT_SECONDARY
    };
    ui.add_enabled(
        enabled,
        egui::Button::new(egui::RichText::new(label).size(12.0).color(color))
            .fill(if enabled {
                Tokens::BG_SURFACE
            } else {
                Tokens::BG_DISABLED
            })
            .stroke(egui::Stroke::new(1.0, Tokens::BORDER_DEFAULT))
            .min_size(egui::vec2(58.0, 28.0))
            .corner_radius(5),
    )
}

pub fn danger_button(ui: &mut egui::Ui, enabled: bool, label: &str) -> egui::Response {
    let color = if enabled {
        Tokens::TEXT_DANGER
    } else {
        Tokens::TEXT_SECONDARY
    };
    ui.add_enabled(
        enabled,
        egui::Button::new(bold_text(label, 12.0, color))
            .fill(if enabled {
                Tokens::BG_SURFACE
            } else {
                Tokens::BG_DISABLED
            })
            .stroke(egui::Stroke::new(1.0, Tokens::BORDER_DEFAULT))
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

/// Interactive horizontal splitter bar anchored by a stable `egui::Id` so drag focus NEVER drops
/// even when sibling scrollbars or dynamic rows appear/disappear.
pub fn horizontal_splitter(ui: &mut egui::Ui, id_salt: &str) -> (egui::Response, f32) {
    let w = ui.available_width().max(60.0);
    let (_, rect) = ui.allocate_space(egui::vec2(w, 8.0));
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
        Tokens::BORDER_DEFAULT
    };
    let pill_color = if active {
        Tokens::STATUS_INFO
    } else {
        egui::Color32::from_rgb(0xCB, 0xD5, 0xE1)
    };
    let cy = rect.center().y;
    ui.painter().line_segment(
        [egui::pos2(rect.left(), cy), egui::pos2(rect.right(), cy)],
        egui::Stroke::new(if active { 1.5 } else { 1.0 }, line_color),
    );
    let pill = egui::Rect::from_center_size(rect.center(), egui::vec2(34.0, 3.5));
    ui.painter().rect_filled(pill, 2.0, pill_color);
    let delta_y = if resp.dragged() {
        resp.drag_delta().y
    } else {
        0.0
    };
    (resp, delta_y)
}

/// Interactive vertical splitter bar anchored by a stable `egui::Id` so drag focus NEVER drops.
pub fn vertical_splitter(ui: &mut egui::Ui, id_salt: &str, height: f32) -> (egui::Response, f32) {
    let h = height.max(60.0);
    let (_, rect) = ui.allocate_space(egui::vec2(8.0, h));
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
        Tokens::BORDER_DEFAULT
    };
    let pill_color = if active {
        Tokens::STATUS_INFO
    } else {
        egui::Color32::from_rgb(0xCB, 0xD5, 0xE1)
    };
    let cx = rect.center().x;
    ui.painter().line_segment(
        [egui::pos2(cx, rect.top()), egui::pos2(cx, rect.bottom())],
        egui::Stroke::new(if active { 1.5 } else { 1.0 }, line_color),
    );
    let pill = egui::Rect::from_center_size(rect.center(), egui::vec2(3.5, 34.0));
    ui.painter().rect_filled(pill, 2.0, pill_color);
    let delta_x = if resp.dragged() {
        resp.drag_delta().x
    } else {
        0.0
    };
    (resp, delta_x)
}

/// Line-numbered monospace code surface matching Figma Code editor (6:2082 / 6:5869),
/// with compact IDE-grade line spacing so large JSON/XML bodies fit cleanly on screen.
pub fn code_surface_numbered(ui: &mut egui::Ui, text: &str) {
    egui::Frame::new()
        .fill(surface_bg(ui))
        .stroke(egui::Stroke::new(1.0, border_color(ui)))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 1.5);
            let mut total_lines = 0usize;
            for (idx, line) in text.lines().enumerate() {
                total_lines = idx + 1;
                if idx >= 2000 {
                    continue;
                }
                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(
                        egui::vec2(34.0, 16.5),
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
                                .size(12.0)
                                .color(text_primary(ui)),
                        )
                        .extend()
                        .selectable(true),
                    );
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

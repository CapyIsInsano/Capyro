//! Colores y tipografía de Capyro (basados en el mockup).

use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, FontId};
use std::sync::Arc;

pub const BG_WINDOW: Color32 = Color32::from_rgb(0x1B, 0x1C, 0x20);
pub const BG_VIDEO: Color32 = Color32::from_rgb(0x0B, 0x0C, 0x0F);
pub const BG_PANEL: Color32 = Color32::from_rgb(0x22, 0x23, 0x28);
pub const TRACK: Color32 = Color32::from_rgb(0x3A, 0x3B, 0x40);
pub const TEXT: Color32 = Color32::from_rgb(0xEE, 0xEE, 0xF0);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0xA8, 0xA9, 0xAE);
pub const TEXT_FAINT: Color32 = Color32::from_rgb(0x6E, 0x6F, 0x75);
pub const ICON: Color32 = Color32::from_rgb(0xD6, 0xD7, 0xDB);
pub const ICON_DIM: Color32 = Color32::from_rgb(0xA0, 0xA1, 0xA7);
pub const HOVER_BG: Color32 = Color32::from_rgba_premultiplied(18, 18, 20, 18);
pub const ERROR: Color32 = Color32::from_rgb(0xF0, 0x5A, 0x4F);

pub const WINDOW_RADIUS: u8 = 14;
pub const PAD: f32 = 12.0;

pub fn regular(size: f32) -> FontId {
    FontId::proportional(size)
}

pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}

/// Ajusta los visuals por defecto de egui al tema oscuro de Capyro.
pub fn apply_visuals(ctx: &egui::Context, accent: Color32) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG_WINDOW;
    v.window_fill = BG_PANEL;
    v.extreme_bg_color = BG_VIDEO;
    v.selection.bg_fill = accent;
    v.override_text_color = Some(TEXT);
    v.widgets.noninteractive.bg_stroke.color = TRACK;
    ctx.set_visuals_of(egui::Theme::Dark, v);
    ctx.set_theme(egui::Theme::Dark);
}

/// Si el sistema tiene DejaVu Sans / Segoe UI la usamos (se parece más al
/// mockup y trae negrita). Si no, quedan las fuentes que trae egui.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    const REGULAR: &[&str] = &[
        "/usr/share/fonts/dejavu/DejaVuSans.ttf",          // Gentoo
        "/usr/share/fonts/TTF/DejaVuSans.ttf",             // Arch / Artix
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", // Debian / Devuan / Ubuntu
        "C:\\Windows\\Fonts\\segoeui.ttf",
    ];
    const BOLD: &[&str] = &[
        "/usr/share/fonts/dejavu/DejaVuSans-Bold.ttf",
        "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
        "C:\\Windows\\Fonts\\segoeuib.ttf",
    ];

    let load = |list: &[&str]| list.iter().find_map(|p| std::fs::read(p).ok());

    let mut proportional = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();

    if let Some(bytes) = load(REGULAR) {
        fonts
            .font_data
            .insert("capyro-regular".into(), Arc::new(FontData::from_owned(bytes)));
        proportional.insert(0, "capyro-regular".into());
    }

    let mut bold_family = proportional.clone();
    if let Some(bytes) = load(BOLD) {
        fonts
            .font_data
            .insert("capyro-bold".into(), Arc::new(FontData::from_owned(bytes)));
        bold_family.insert(0, "capyro-bold".into());
    }

    fonts.families.insert(FontFamily::Proportional, proportional);
    fonts.families.insert(FontFamily::Name("bold".into()), bold_family);
    ctx.set_fonts(fonts);
}

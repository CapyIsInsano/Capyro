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
pub const ERROR: Color32 = Color32::from_rgb(0xF0, 0x5A, 0x4F);

/// En Windows la ventana no es transparente: esquinas rectas.
pub const WINDOW_RADIUS: u8 = if cfg!(windows) { 0 } else { 14 };
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

/// Linux: DejaVu Sans si está (se parece más al mockup y trae negrita).
/// Windows: Segoe UI, la del sistema. Si no, quedan las fuentes de egui.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    #[cfg(not(windows))]
    const REGULAR: &[&str] = &[
        "/usr/share/fonts/dejavu/DejaVuSans.ttf",          // Gentoo
        "/usr/share/fonts/TTF/DejaVuSans.ttf",             // Arch / Artix
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", // Debian / Devuan / Ubuntu
    ];
    #[cfg(not(windows))]
    const BOLD: &[&str] = &[
        "/usr/share/fonts/dejavu/DejaVuSans-Bold.ttf",
        "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    ];
    // relativas a %WINDIR%\Fonts
    #[cfg(windows)]
    const REGULAR: &[&str] = &["segoeui.ttf", "arial.ttf"];
    #[cfg(windows)]
    const BOLD: &[&str] = &["segoeuib.ttf", "arialbd.ttf"];

    let load = |list: &[&str]| {
        list.iter().find_map(|p| {
            #[cfg(windows)]
            let p = std::path::PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into()))
                .join("Fonts")
                .join(p);
            std::fs::read(p).ok()
        })
    };

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

    // Windows: los símbolos (▸, ✓...) que no trae Segoe UI salen de Segoe UI Symbol
    #[cfg(windows)]
    if let Some(bytes) = load(&["seguisym.ttf"]) {
        fonts
            .font_data
            .insert("capyro-symbols".into(), Arc::new(FontData::from_owned(bytes)));
        let at = proportional.len().min(1);
        proportional.insert(at, "capyro-symbols".into());
        let at = bold_family.len().min(1);
        bold_family.insert(at, "capyro-symbols".into());
    }

    fonts.families.insert(FontFamily::Proportional, proportional);
    fonts.families.insert(FontFamily::Name("bold".into()), bold_family);
    ctx.set_fonts(fonts);
}

/// Sombra suave alrededor de `rect` (con esquinas de `radius`): un anillo
/// que va de `alpha` pegado al borde a transparente a `spread` puntos, corrido
/// `dy` hacia abajo. Solo se pinta el anillo: una sombra hecha con un
/// rectángulo oscuro debajo hace que se pinte toda el área dos veces, y con
/// una placa de video por software (llvmpipe) cada píxel cuesta.
pub fn soft_shadow(painter: &egui::Painter, rect: egui::Rect, radius: f32, spread: f32, dy: f32, alpha: u8) {
    use egui::{pos2, vec2, Pos2};
    let r = rect.translate(vec2(0.0, dy));
    let rad = radius.min(r.width() / 2.0).min(r.height() / 2.0).max(0.0);
    // contorno redondeado: 6 puntos por esquina
    let outline = |grow: f32| -> Vec<Pos2> {
        let rr = rad + grow;
        let corners = [
            (pos2(r.right() - rad, r.bottom() - rad), 0.0f32),
            (pos2(r.left() + rad, r.bottom() - rad), 90.0),
            (pos2(r.left() + rad, r.top() + rad), 180.0),
            (pos2(r.right() - rad, r.top() + rad), 270.0),
        ];
        let mut pts = Vec::with_capacity(24);
        for (c, start) in corners {
            for k in 0..6 {
                let a = (start + 90.0 * k as f32 / 5.0).to_radians();
                pts.push(c + vec2(a.cos(), a.sin()) * rr);
            }
        }
        pts
    };
    let inner = outline(0.0);
    let outer = outline(spread);
    let mut mesh = egui::Mesh::default();
    let dark = Color32::from_black_alpha(alpha);
    for (a, b) in inner.iter().zip(outer.iter()) {
        mesh.colored_vertex(*a, dark);
        mesh.colored_vertex(*b, Color32::TRANSPARENT);
    }
    let n = inner.len() as u32;
    for i in 0..n {
        let j = (i + 1) % n;
        let (a0, a1, b0, b1) = (2 * i, 2 * i + 1, 2 * j, 2 * j + 1);
        mesh.add_triangle(a0, a1, b1);
        mesh.add_triangle(a0, b1, b0);
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// Pinta el fondo de la ventana (`full`, con esquinas de `radius`) salvo
/// los `holes` (rectángulos que algo opaco va a tapar enseguida: el área de
/// video, los paneles de la vista de música). Así esa parte no se pinta dos
/// veces: con una placa de video por software (llvmpipe), pintar la ventana
/// entera y encima el video o los paneles era casi el doble de trabajo.
/// Los huecos se achican 2 puntos (el borde suavizado de lo de arriba queda
/// sobre el fondo) y sus esquinas se rellenan, por si lo de arriba es
/// redondeado (`(rect, radio)`).
pub fn fill_window(painter: &egui::Painter, full: egui::Rect, radius: u8, holes: &[(egui::Rect, f32)]) {
    use egui::{pos2, CornerRadius, Rect};
    let wr = radius as f32;
    // huecos útiles: lejos de las esquinas redondeadas de la ventana
    let holes: Vec<(Rect, f32)> = holes
        .iter()
        .map(|(r, rad)| {
            let h = r.shrink(2.0);
            let h = Rect::from_min_max(
                pos2(h.left().max(full.left()), h.top().max(full.top() + wr)),
                pos2(h.right().min(full.right()), h.bottom().min(full.bottom() - wr)),
            );
            (h, *rad)
        })
        .filter(|(h, _)| h.width() > 8.0 && h.height() > 8.0)
        .collect();
    if holes.is_empty() {
        painter.rect_filled(full, radius, BG_WINDOW);
        return;
    }
    let mut ys = vec![full.top(), full.bottom()];
    for (h, _) in &holes {
        ys.push(h.top());
        ys.push(h.bottom());
    }
    ys.sort_by(|a, b| a.total_cmp(b));
    ys.dedup_by(|a, b| (*a - *b).abs() < 0.5);
    for w in ys.windows(2) {
        let (y0, y1) = (w[0], w[1]);
        if y1 - y0 < 0.5 {
            continue;
        }
        // tramos de x libres en esta franja
        let mut cover: Vec<(f32, f32)> = holes
            .iter()
            .filter(|(h, _)| h.top() <= y0 + 0.01 && h.bottom() >= y1 - 0.01)
            .map(|(h, _)| (h.left(), h.right()))
            .collect();
        cover.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut x = full.left();
        let mut segs = Vec::new();
        for (l, r) in cover {
            if l > x {
                segs.push((x, l));
            }
            x = x.max(r);
        }
        if x < full.right() {
            segs.push((x, full.right()));
        }
        let top = (y0 - full.top()).abs() < 0.5;
        let bottom = (y1 - full.bottom()).abs() < 0.5;
        for (l, r) in segs {
            // 1 punto de más arriba y abajo: sin rayitas entre franjas
            let rect = Rect::from_min_max(pos2(l, y0 - if top { 0.0 } else { 1.0 }), pos2(r, y1 + if bottom { 0.0 } else { 1.0 }));
            let cr = CornerRadius {
                nw: if top && (l - full.left()).abs() < 0.5 { radius } else { 0 },
                ne: if top && (r - full.right()).abs() < 0.5 { radius } else { 0 },
                sw: if bottom && (l - full.left()).abs() < 0.5 { radius } else { 0 },
                se: if bottom && (r - full.right()).abs() < 0.5 { radius } else { 0 },
            };
            painter.rect_filled(rect, cr, BG_WINDOW);
        }
    }
    // las esquinas de los huecos (lo de arriba es redondeado)
    for (h, rad) in &holes {
        let s = rad + 3.0;
        if s <= 3.0 {
            continue;
        }
        for c in [h.left_top(), h.right_top() - egui::vec2(s, 0.0), h.left_bottom() - egui::vec2(0.0, s), h.right_bottom() - egui::vec2(s, s)] {
            painter.rect_filled(Rect::from_min_size(c, egui::vec2(s, s)), 0, BG_WINDOW);
        }
    }
}

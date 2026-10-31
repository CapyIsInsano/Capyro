//! Ecualizador avanzado de 15 bandas: perfiles, bandas para arrastrar y la curva.

use super::*;

/// Frecuencias de las 15 bandas, para mostrarlas.
const BAND_LABELS: [&str; EQ15_BANDS] = [
    "25", "40", "63", "100", "160", "250", "400", "630", "1k", "1.6k", "2.5k", "4k", "6.3k", "10k", "16k",
];

/// Ecualizadores listos para usar (ganancia por banda en dB, de graves a agudos).
const EQ_PRESETS: [(&str, [f32; EQ15_BANDS]); 10] = [
    ("Plano", [0.0; EQ15_BANDS]),
    ("Graves", [6.0, 5.0, 4.0, 3.0, 2.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
    ("Voces", [-3.0, -2.0, -1.0, 0.0, 0.0, 1.0, 3.0, 4.0, 4.0, 3.0, 2.0, 1.0, 0.0, -1.0, -2.0]),
    ("Brillante", [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 5.0]),
    ("Rock", [5.0, 4.0, 3.0, 1.0, -1.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 3.0, 4.0, 4.0, 4.0]),
    ("Pop", [-1.0, -1.0, 0.0, 1.0, 2.0, 3.0, 3.0, 2.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0]),
    ("Jazz", [3.0, 2.0, 1.0, 2.0, -1.0, -1.0, 0.0, 1.0, 2.0, 2.0, 2.0, 2.0, 3.0, 3.0, 4.0]),
    ("Clásica", [2.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 3.0]),
    ("Electrónica", [5.0, 4.0, 3.0, 1.0, 0.0, -2.0, -2.0, 0.0, 1.0, 2.0, 3.0, 3.0, 4.0, 5.0, 4.0]),
    ("Cine", [2.0, 2.0, 1.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0, 2.0, 2.0, 3.0, 3.0, 2.0]),
];

/// Línea suave que pasa por todos los puntos (Catmull-Rom).
fn smooth(points: &[Pos2]) -> Vec<Pos2> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let mut out = Vec::with_capacity(points.len() * 8);
    for i in 0..points.len() - 1 {
        let p0 = points[i.saturating_sub(1)];
        let p1 = points[i];
        let p2 = points[i + 1];
        let p3 = points[(i + 2).min(points.len() - 1)];
        for k in 0..8 {
            let t = k as f32 / 8.0;
            let (t2, t3) = (t * t, t * t * t);
            let f = |a: f32, b: f32, c: f32, d: f32| {
                0.5 * (2.0 * b + (-a + c) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2 + (-a + 3.0 * b - 3.0 * c + d) * t3)
            };
            out.push(pos2(f(p0.x, p1.x, p2.x, p3.x), f(p0.y, p1.y, p2.y, p3.y)));
        }
    }
    out.push(*points.last().expect("puntos"));
    out
}

impl Studio {
    /// Aplica el ecualizador de 15 bandas al motor (sin tocar los demás efectos).
    pub(super) fn apply_eq(&self, core: &Core) {
        let mut fx: AudioFx = core.fx();
        fx.eq15_on = self.eq_on;
        fx.eq15 = self.eq;
        core.set_fx(&fx);
    }

    /// Ecualizador avanzado: interruptor, perfiles y las 15 bandas.
    pub(super) fn eq_section(&mut self, ui: &mut Ui, core: &Core, accent: Color32) {
        let w = ui.available_width();
        let anim = crate::system::perf::animations();
        let alto = crate::ui::alto::on();
        let ctx = ui.ctx().clone();
        let painter = ui.painter().clone();

        /* --- encabezado --- */
        let (head, _) = ui.allocate_exact_size(vec2(w, 24.0), Sense::hover());
        let tg = painter.layout_no_wrap("Ecualizador avanzado".to_owned(), bold(13.0), TEXT);
        let tw = tg.size().x;
        painter.galley(pos2(head.left(), head.center().y - tg.size().y / 2.0), tg, TEXT);
        painter.text(pos2(head.left() + tw + 8.0, head.center().y + 1.0), Align2::LEFT_CENTER, "15 bandas", regular(11.0), TEXT_FAINT);
        let mut changed = false;
        let sw_c = pos2(head.right() - 17.0, head.center().y);
        if switch(ui, &painter, "studio-eq", sw_c, self.eq_on, accent, true) {
            self.eq_on = !self.eq_on;
            changed = true;
        }
        painter.text(
            pos2(head.right() - 42.0, head.center().y),
            Align2::RIGHT_CENTER,
            if self.eq_on { "Activado" } else { "Apagado" },
            regular(11.0),
            TEXT_FAINT,
        );
        let (note, _) = ui.allocate_exact_size(vec2(w, 18.0), Sense::hover());
        let note_text = if self.eq_on {
            "Mientras está activado, el ecualizador normal (graves, medios y agudos) queda apagado."
        } else {
            "Al activarlo, reemplaza al ecualizador normal (graves, medios y agudos)."
        };
        painter.text(
            note.left_center(),
            Align2::LEFT_CENTER,
            fit_text(&painter, note_text, regular(10.5), w),
            regular(10.5),
            TEXT_FAINT,
        );

        /* --- perfiles (en filas que se acomodan al ancho) --- */
        ui.add_space(6.0);
        let gap = 6.0;
        let widths: Vec<f32> = EQ_PRESETS.iter().map(|(n, _)| pill_w(ui, n)).collect();
        let mut rows = 1;
        let mut x = 0.0;
        for cw in &widths {
            if x > 0.0 && x + cw > w {
                rows += 1;
                x = 0.0;
            }
            x += cw + gap;
        }
        let (area, _) = ui.allocate_exact_size(vec2(w, rows as f32 * 30.0), Sense::hover());
        let (mut x, mut y) = (area.left(), area.top());
        for (i, (name, gains)) in EQ_PRESETS.iter().enumerate() {
            let cw = widths[i];
            if x > area.left() && x + cw > area.right() {
                x = area.left();
                y += 30.0;
            }
            let r = Rect::from_min_size(pos2(x, y + 2.0), vec2(cw, 24.0));
            let selected = self.eq_on && self.eq.iter().zip(gains.iter()).all(|(a, b)| (a - b).abs() < 0.05);
            let sel = ctx.animate_bool_with_time(Id::unique(("capyro-studio-preset", i)), selected, crate::system::perf::t(0.15));
            if pill(ui, &format!("preset-{i}"), r, name, false, sel, accent, true) {
                self.preset_changed |= !selected;
                self.eq = *gains;
                self.eq_on = true;
                changed = true;
            }
            x += cw + gap;
        }

        /* --- las bandas --- */
        ui.add_space(10.0);
        let (area, _) = ui.allocate_exact_size(vec2(w, 200.0), Sense::hover());
        let label_w = 30.0;
        let bands = Rect::from_min_max(pos2(area.left() + label_w, area.top()), area.max);
        let (yt, yb) = (bands.top() + 14.0, bands.bottom() - 34.0);
        let y0 = (yt + yb) / 2.0;
        let half = (yb - yt) / 2.0;
        let y_of = |db: f32| y0 - db.clamp(-12.0, 12.0) / 12.0 * half;
        let col_w = bands.width() / EQ15_BANDS as f32;
        let cx_of = |i: usize| bands.left() + col_w * (i as f32 + 0.5);

        // interacción primero (para dibujar ya con el valor nuevo)
        let mut hovers = [0.0f32; EQ15_BANDS];
        let mut active = [false; EQ15_BANDS];
        for i in 0..EQ15_BANDS {
            let col = Rect::from_min_max(pos2(cx_of(i) - col_w / 2.0, yt - 10.0), pos2(cx_of(i) + col_w / 2.0, yb + 10.0));
            let id = Id::unique(("capyro-studio-band", i));
            let resp = ui.interact(col, id, Sense::click_and_drag());
            let hot = resp.hovered() || resp.dragged();
            hovers[i] = ctx.animate_bool_with_time(id.with("h"), hot, crate::system::perf::t(0.12));
            active[i] = resp.dragged();
            if hot {
                ctx.set_cursor_icon(egui::CursorIcon::ResizeVertical);
            }
            if resp.double_clicked() {
                self.eq[i] = 0.0;
                self.eq_on = true;
                changed = true;
            } else if resp.dragged() || resp.clicked() {
                if let Some(p) = resp.interact_pointer_pos() {
                    let v = (((y0 - p.y) / half * 12.0) * 2.0).round() / 2.0;
                    let v = v.clamp(-12.0, 12.0);
                    if (v - self.eq[i]).abs() > f32::EPSILON || !self.eq_on {
                        self.eq[i] = v;
                        self.eq_on = true;
                        changed = true;
                    }
                }
            }
            resp.on_hover_text("Arrastrá para subir o bajar · doble clic: 0 dB");
        }

        // rejilla: +12, +6, 0, -6, -12
        for db in [12.0f32, 6.0, 0.0, -6.0, -12.0] {
            let y = y_of(db);
            let alpha = if db == 0.0 { 22 } else { 9 };
            painter.line_segment([pos2(bands.left(), y), pos2(bands.right(), y)], Stroke::new(1.0, Color32::from_white_alpha(alpha)));
            if db.abs() > 0.1 && (db.abs() - 6.0).abs() < 0.1 {
                continue; // ±6 sin número, para no amontonar
            }
            let text = if db == 0.0 { "0".to_owned() } else { format!("{db:+.0}") };
            painter.text(pos2(area.left() + label_w - 8.0, y), Align2::RIGHT_CENTER, text, regular(9.5), TEXT_FAINT);
        }

        let on = self.eq_on;
        let fill_col = if on { accent } else { Color32::from_white_alpha(60) };
        let points: Vec<Pos2> = (0..EQ15_BANDS).map(|i| pos2(cx_of(i), y_of(self.eq[i]))).collect();
        let curve = if anim { smooth(&points) } else { Vec::new() };

        // Alto: el área bajo la curva, con un poco de color
        if alto && on && curve.len() > 1 {
            let mut mesh = egui::Mesh::default();
            let top = accent.gamma_multiply(0.16);
            let base = accent.gamma_multiply(0.02);
            for p in &curve {
                mesh.colored_vertex(*p, top);
                mesh.colored_vertex(pos2(p.x, y0), base);
            }
            for k in 0..(curve.len() as u32 - 1) {
                let a = k * 2;
                mesh.add_triangle(a, a + 1, a + 2);
                mesh.add_triangle(a + 1, a + 3, a + 2);
            }
            painter.add(Shape::mesh(mesh));
        }

        // rieles y relleno desde 0
        for i in 0..EQ15_BANDS {
            let cx = cx_of(i);
            let tw = 4.0 + 1.5 * hovers[i];
            painter.rect_filled(Rect::from_min_max(pos2(cx - tw / 2.0, yt), pos2(cx + tw / 2.0, yb)), tw / 2.0, TRACK);
            let yv = y_of(self.eq[i]);
            if (yv - y0).abs() > 0.5 {
                let r = Rect::from_min_max(pos2(cx - tw / 2.0, yv.min(y0)), pos2(cx + tw / 2.0, yv.max(y0)));
                painter.rect_filled(r, tw / 2.0, fill_col.gamma_multiply(if on { 0.85 } else { 1.0 }));
            }
        }

        // la curva (Medio y Alto); en Alto, con brillo
        if curve.len() > 1 {
            let col = if on { accent } else { Color32::from_white_alpha(70) };
            if alto && on {
                painter.add(Shape::line(curve.clone(), Stroke::new(7.0, accent.gamma_multiply(0.16))));
            }
            painter.add(Shape::line(curve, Stroke::new(2.0, col.gamma_multiply(0.9))));
        }

        // perillas, valores y frecuencias
        for i in 0..EQ15_BANDS {
            let c = points[i];
            let r = 6.0 + 1.5 * hovers[i] + if active[i] { 1.0 } else { 0.0 };
            if alto && on && hovers[i] > 0.01 {
                crate::ui::alto::glow(&painter, c, 18.0, accent, 0.35 * hovers[i]);
            }
            painter.circle_filled(c, r, Color32::WHITE);
            painter.circle_stroke(c, r, Stroke::new(1.5, if on { accent } else { TEXT_FAINT }));
            if hovers[i] > 0.01 {
                let v = self.eq[i];
                let text = if v.abs() < 0.05 { "0".to_owned() } else { format!("{v:+.1}") };
                let ty = (c.y - 16.0).max(area.top() + 6.0);
                painter.text(pos2(c.x, ty), Align2::CENTER_CENTER, text, bold(10.5), TEXT.gamma_multiply(hovers[i]));
            }
            let lc = TEXT_FAINT.lerp_to_gamma(TEXT, hovers[i]);
            painter.text(pos2(c.x, yb + 18.0), Align2::CENTER_CENTER, BAND_LABELS[i], regular(10.0), lc);
        }
        painter.text(pos2(area.left() + label_w - 8.0, yb + 18.0), Align2::RIGHT_CENTER, "Hz", regular(9.5), TEXT_FAINT);

        if changed {
            self.apply_eq(core);
        }
    }
}

//! Los datos: título y artista en grande, y el resto de las etiquetas.

use super::*;

impl Studio {
    /// Portada (a la izquierda, o arriba si es angosto) + título, artista y el resto.
    pub(super) fn info_section(&mut self, ui: &mut Ui, ctx: &Context, accent: Color32, now: f64) {
        let w = ui.available_width();
        let narrow = w < 560.0;
        let side = if narrow { (w * 0.55).clamp(120.0, 190.0) } else { (w * 0.28).clamp(140.0, 210.0) };
        let fields_h = 44.0 + 32.0 + 12.0 + 3.0 * 48.0 + 70.0;
        let cover_h = side + 34.0;
        let h = if narrow { cover_h + 14.0 + fields_h } else { cover_h.max(fields_h) };
        let (sec, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());

        let cover_rect = if narrow {
            Rect::from_min_size(pos2(sec.center().x - side / 2.0, sec.top()), vec2(side, side))
        } else {
            Rect::from_min_size(sec.min, vec2(side, side))
        };
        self.cover_widget(ui, ctx, cover_rect, accent, now);

        let fx0 = if narrow { sec.left() } else { cover_rect.right() + 24.0 };
        let fy0 = if narrow { cover_rect.bottom() + 48.0 } else { sec.top() };
        let fw = sec.right() - fx0;
        let mut y = fy0;
        let video = self.video;
        let t = &mut self.tags;
        let (title_hint, artist_hint) =
            if video { ("Título del video", "Autor, canal o director") } else { ("Nombre de la canción", "Artista o intérprete") };
        big_field(ui, Rect::from_min_size(pos2(fx0, y), vec2(fw, 42.0)), "title", &mut t.title, title_hint, bold(24.0), TEXT, accent);
        y += 46.0;
        big_field(ui, Rect::from_min_size(pos2(fx0, y), vec2(fw, 30.0)), "artist", &mut t.artist, artist_hint, regular(15.0), TEXT_DIM, accent);
        y += 30.0 + 14.0;
        let row = |y: f32, h: f32| Rect::from_min_size(pos2(fx0, y), vec2(fw, h));
        let c = columns(row(y, 44.0), &[1.0, 1.0], 12.0);
        field(ui, c[0], "album", "Álbum", &mut t.album, if video { "Serie o colección" } else { "Álbum al que pertenece" }, accent, 1);
        field(ui, c[1], "album_artist", "Artista del álbum", &mut t.album_artist, "Artista principal del álbum", accent, 1);
        y += 48.0;
        let c = columns(row(y, 44.0), &[1.6, 0.8, 0.7, 0.7], 12.0);
        field(ui, c[0], "genre", "Género", &mut t.genre, "Género musical", accent, 1);
        field(ui, c[1], "year", "Año", &mut t.year, "—", accent, 1);
        field(ui, c[2], "track", "Pista", &mut t.track, "—", accent, 1);
        field(ui, c[3], "disc", "Disco", &mut t.disc, "—", accent, 1);
        y += 48.0;
        field(ui, row(y, 44.0), "composer", "Compositor", &mut t.composer, if video { "Quién hizo la música" } else { "Quién la compuso" }, accent, 1);
        y += 48.0;
        field(ui, row(y, 66.0), "comment", "Comentarios", &mut t.comment, if video { "Una nota, de qué trata, de dónde lo sacaste…" } else { "Una nota, la letra, de dónde la sacaste…" }, accent, 2);

        // WAV no guarda estas tres etiquetas (su formato no tiene dónde ponerlas)
        if self.file.extension().is_some_and(|e| e.eq_ignore_ascii_case("wav")) {
            ui.painter().text(
                pos2(fx0 + 2.0, y + 74.0),
                Align2::LEFT_CENTER,
                "En WAV no se guardan el artista del álbum, el disco, el compositor ni la portada.",
                regular(10.5),
                TEXT_FAINT,
            );
            ui.add_space(16.0);
        }
    }
}

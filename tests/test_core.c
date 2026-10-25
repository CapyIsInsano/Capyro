/*
 * Pruebas del núcleo, sin GUI. Se corren con `ctest`.
 *
 * Uso: test_core [carpeta_con_media]
 * La carpeta la genera CMake con el ffmpeg de línea de comandos (si está
 * instalado). Sin ella solo se corren las pruebas que no decodifican.
 */
#ifndef _POSIX_C_SOURCE
#  define _POSIX_C_SOURCE 200809L
#endif

#include "capyro_core.h"
#include "internal.h" /* caja blanca: para ver si el reloj sigue al audio */

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <time.h>

static void sleep_ms(int ms) {
    struct timespec ts = { ms / 1000, (long)(ms % 1000) * 1000000L };
    nanosleep(&ts, NULL);
}

static int failures = 0;

#define CHECK(cond)                                                          \
    do {                                                                     \
        if (!(cond)) {                                                       \
            fprintf(stderr, "FALLA %s:%d: %s\n", __FILE__, __LINE__, #cond); \
            failures++;                                                      \
        }                                                                    \
    } while (0)

#define NEAR(a, b, tol) (fabs((double)(a) - (double)(b)) <= (tol))

static void touch(const char *path) {
    FILE *f = fopen(path, "wb");
    if (f) {
        fclose(f);
    }
}

/* Corre capyro_core_update() durante `ms` milisegundos (como haría la GUI). */
static void pump(CapyroCore *core, int ms) {
    for (int t = 0; t < ms; t += 10) {
        capyro_core_update(core);
        sleep_ms(10);
    }
}

static void test_basics(void) {
    CHECK(capyro_core_probe_extension("a.mp4") == CAPYRO_MEDIA_VIDEO);
    CHECK(capyro_core_probe_extension("A.MP3") == CAPYRO_MEDIA_AUDIO);
    CHECK(capyro_core_probe_extension("a.mkv") == CAPYRO_MEDIA_NONE);
    CHECK(capyro_core_probe_extension("clip.MOV") == CAPYRO_MEDIA_VIDEO);
    CHECK(capyro_core_probe_extension("tono.wav") == CAPYRO_MEDIA_AUDIO);
    CHECK(capyro_core_probe_extension("tono.Ogg") == CAPYRO_MEDIA_AUDIO);
    CHECK(capyro_core_probe_extension("tono.flac") == CAPYRO_MEDIA_NONE);
    CHECK(strcmp(capyro_core_supported_video(), "mp4,mov") == 0);
    CHECK(strcmp(capyro_core_supported_audio(), "mp3,wav,ogg") == 0);
    CHECK(capyro_core_probe_extension("mp4") == CAPYRO_MEDIA_NONE);
    CHECK(capyro_core_probe_extension(NULL) == CAPYRO_MEDIA_NONE);

    CapyroCore *core = capyro_core_init();
    CHECK(core != NULL);
    if (!core) {
        return;
    }
    CHECK(capyro_core_get_state(core) == CAPYRO_STATE_IDLE);
    capyro_core_play(core);
    CHECK(capyro_core_get_state(core) == CAPYRO_STATE_IDLE);

    CHECK(capyro_core_open(core, "video.mkv") == CAPYRO_ERR_UNSUPPORTED);
    CHECK(capyro_core_playlist_count(core) == 0);

    /* Archivo vacío con extensión válida: FFmpeg no lo puede leer. */
    touch("capyro_test_vacio.mp3");
    CHECK(capyro_core_open(core, "capyro_test_vacio.mp3") == CAPYRO_ERR_DECODE);
    CHECK(capyro_core_get_state(core) == CAPYRO_STATE_IDLE);
    CHECK(capyro_core_playlist_current(core) == -1);

    CHECK(capyro_core_playlist_add(core, "no_existe.mp3") == CAPYRO_OK);
    CHECK(capyro_core_playlist_select(core, 1) == CAPYRO_ERR_NOT_FOUND);
    CHECK(capyro_core_playlist_select(core, 9) == CAPYRO_ERR_OUT_OF_RANGE);

    capyro_core_set_volume(core, 1.7f);
    CHECK(capyro_core_get_volume(core) == 1.0f);
    capyro_core_set_volume(core, -1.0f);
    CHECK(capyro_core_get_volume(core) == 0.0f);

    capyro_core_playlist_clear(core);
    CHECK(capyro_core_playlist_count(core) == 0);
    capyro_core_shutdown(core);
    remove("capyro_test_vacio.mp3");
}

static bool exists(const char *path) {
    FILE *f = fopen(path, "rb");
    if (f) {
        fclose(f);
        return true;
    }
    printf("SALTEO: no está %s (tu ffmpeg no pudo generarlo)\n", path);
    return false;
}

static void test_media(const char *dir) {
    char tone[1024], cover[1024], clip[1024];
    snprintf(tone, sizeof(tone), "%s/tono.mp3", dir);
    snprintf(cover, sizeof(cover), "%s/caratula.mp3", dir);
    snprintf(clip, sizeof(clip), "%s/clip.mp4", dir);

    CapyroCore *core = capyro_core_init();
    CHECK(core != NULL);
    if (!core) {
        return;
    }
    printf("backend de audio: \"%s\"\n", capyro_core_audio_backend(core));

    bool has_tone = exists(tone), has_cover = exists(cover), has_clip = exists(clip);
    CapyroVideoFrame vf;

    /* --- mp3: duración, metadatos, reproducción, seek, fin --- */
    if (has_tone) {
    CHECK(capyro_core_open(core, tone) == CAPYRO_OK);
    CHECK(capyro_core_get_media_kind(core) == CAPYRO_MEDIA_AUDIO);
    CHECK(NEAR(capyro_core_get_duration(core), 3.0, 0.15));
    CHECK(strcmp(capyro_core_get_title(core), "Tono de prueba") == 0);
    CHECK(strcmp(capyro_core_get_artist(core), "Capyro") == 0);
    CHECK(!capyro_core_lock_video_frame(core, &vf)); /* sin carátula */

    capyro_core_play(core);
    pump(core, 600);
    double pos = capyro_core_get_position(core);
    printf("posición tras 0.6 s: %.3f\n", pos);
    CHECK(pos > 0.3 && pos < 0.9);

    capyro_core_pause(core);
    double paused_at = capyro_core_get_position(core);
    pump(core, 300);
    CHECK(NEAR(capyro_core_get_position(core), paused_at, 0.02));

    capyro_core_seek(core, 2.5);
    CHECK(NEAR(capyro_core_get_position(core), 2.5, 0.01));
    capyro_core_play(core);
    pump(core, 1500);
    CHECK(capyro_core_get_state(core) == CAPYRO_STATE_PAUSED); /* llegó al final */
    CHECK(NEAR(capyro_core_get_position(core), capyro_core_get_duration(core), 0.01));

    capyro_core_play(core); /* play al final = desde el principio */
    pump(core, 100);
    CHECK(capyro_core_get_position(core) < 0.5);
    capyro_core_stop(core);
    CHECK(capyro_core_get_state(core) == CAPYRO_STATE_STOPPED);
    CHECK(capyro_core_get_position(core) == 0.0);
    }

    /* --- mp3 con carátula --- */
    if (has_cover) {
    CHECK(capyro_core_open(core, cover) == CAPYRO_OK);
    CHECK(capyro_core_get_media_kind(core) == CAPYRO_MEDIA_AUDIO);
    if (capyro_core_lock_video_frame(core, &vf)) {
        CHECK(vf.width == 64 && vf.height == 64 && vf.rgba != NULL);
        capyro_core_unlock_video_frame(core);
    } else {
        CHECK(!"la carátula debería verse como frame");
    }
    }

    if (!has_clip) {
        capyro_core_shutdown(core);
        return;
    }

    /* --- mp4: primer frame visible sin play, frames nuevos al reproducir --- */
    CHECK(capyro_core_open(core, clip) == CAPYRO_OK);
    CHECK(capyro_core_get_media_kind(core) == CAPYRO_MEDIA_VIDEO);
    CHECK(NEAR(capyro_core_get_duration(core), 2.0, 0.15));
    bool ready = false;
    for (int t = 0; t < 3000 && !ready; t += 10) { /* holgado para sanitizers */
        ready = capyro_core_lock_video_frame(core, &vf);
        if (!ready) {
            pump(core, 10);
        }
    }
    uint64_t first_serial = 0;
    if (ready) {
        CHECK(vf.width == 320 && vf.height == 240);
        CHECK(NEAR(vf.aspect, 320.0 / 240.0, 0.01));
        first_serial = vf.serial;
        capyro_core_unlock_video_frame(core);
    } else {
        CHECK(!"el primer frame debería estar listo sin dar play");
    }
    capyro_core_play(core);
    pump(core, 400);
    if (capyro_core_lock_video_frame(core, &vf)) {
        CHECK(vf.serial > first_serial);
        capyro_core_unlock_video_frame(core);
    }

    /* --- seek en pausa muestra el frame nuevo --- */
    capyro_core_pause(core);
    if (capyro_core_lock_video_frame(core, &vf)) {
        first_serial = vf.serial;
        capyro_core_unlock_video_frame(core);
    }
    capyro_core_seek(core, 1.0);
    pump(core, 200);
    if (capyro_core_lock_video_frame(core, &vf)) {
        CHECK(vf.serial > first_serial);
        capyro_core_unlock_video_frame(core);
    }

    /* --- auto_next: al terminar el mp4 pasa al siguiente de la lista --- */
    capyro_core_playlist_clear(core);
    CHECK(capyro_core_playlist_add(core, clip) == CAPYRO_OK);
    CHECK(capyro_core_playlist_add(core, has_tone ? tone : clip) == CAPYRO_OK);
    CHECK(capyro_core_playlist_select(core, 0) == CAPYRO_OK);
    capyro_core_seek(core, 1.7);
    capyro_core_play(core);
    pump(core, 1200);
    CHECK(capyro_core_playlist_current(core) == 1);
    CHECK(capyro_core_get_state(core) == CAPYRO_STATE_PLAYING);

    /* --- velocidad: a 2x el reloj avanza el doble, sin saltos al cambiarla --- */
    capyro_core_playlist_clear(core);
    CHECK(capyro_core_open(core, has_tone ? tone : clip) == CAPYRO_OK);
    capyro_core_set_speed(core, 5.0);
    CHECK(capyro_core_get_speed(core) == CAPYRO_SPEED_MAX);
    capyro_core_set_speed(core, 2.0);
    capyro_core_play(core);
    pump(core, 500);
    double fast = capyro_core_get_position(core);
    printf("posición tras 0.5 s a 2x: %.3f\n", fast);
    CHECK(fast > 0.7 && fast < 1.3);
    double before = capyro_core_get_position(core);
    capyro_core_set_speed(core, 1.0);
    CHECK(NEAR(capyro_core_get_position(core), before, 0.05));
    capyro_core_set_speed(core, 0.5);
    double slow_start = capyro_core_get_position(core);
    pump(core, 600);
    double slow = capyro_core_get_position(core) - slow_start;
    printf("avance en 0.6 s a 0.5x: %.3f\n", slow);
    CHECK(slow > 0.15 && slow < 0.45);
    capyro_core_set_speed(core, 1.0);

    /* --- repetir y aleatorio (misma media 4 veces: importa el índice) --- */
    capyro_core_playlist_clear(core);
    for (int i = 0; i < 4; i++) {
        CHECK(capyro_core_playlist_add(core, clip) == CAPYRO_OK);
    }
    CHECK(capyro_core_playlist_select(core, 3) == CAPYRO_OK);
    CHECK(capyro_core_playlist_next(core) == CAPYRO_ERR_OUT_OF_RANGE); /* repeat off */
    capyro_core_set_repeat(core, CAPYRO_REPEAT_ALL);
    CHECK(capyro_core_get_repeat(core) == CAPYRO_REPEAT_ALL);
    CHECK(capyro_core_playlist_next(core) == CAPYRO_OK);
    CHECK(capyro_core_playlist_current(core) == 0);              /* dio la vuelta */
    CHECK(capyro_core_playlist_prev(core) == CAPYRO_OK);
    CHECK(capyro_core_playlist_current(core) == 3);              /* y para atrás también */

    capyro_core_set_shuffle(core, true);
    CHECK(capyro_core_get_shuffle(core));
    int seen[4] = {0, 0, 0, 0};
    seen[capyro_core_playlist_current(core)]++;
    for (int i = 0; i < 3; i++) {
        CHECK(capyro_core_playlist_next(core) == CAPYRO_OK);
        long c = capyro_core_playlist_current(core);
        if (c >= 0 && c < 4) {
            seen[c]++;
        }
    }
    CHECK(seen[0] == 1 && seen[1] == 1 && seen[2] == 1 && seen[3] == 1); /* todos, sin repetir */
    capyro_core_set_shuffle(core, false);
    capyro_core_set_repeat(core, CAPYRO_REPEAT_OFF);

    /* --- eliminar de la lista --- */
    CHECK(capyro_core_playlist_select(core, 1) == CAPYRO_OK);
    capyro_core_play(core);
    capyro_core_playlist_remove(core, 0);                  /* otro: corre el índice */
    CHECK(capyro_core_playlist_count(core) == 3);
    CHECK(capyro_core_playlist_current(core) == 0);
    capyro_core_playlist_remove(core, 0);                  /* el actual: pasa al siguiente */
    CHECK(capyro_core_playlist_count(core) == 2);
    CHECK(capyro_core_playlist_current(core) == 0);
    CHECK(capyro_core_get_state(core) == CAPYRO_STATE_PLAYING);
    CHECK(capyro_core_playlist_select(core, 1) == CAPYRO_OK);
    capyro_core_playlist_remove(core, 1);                  /* el actual y último */
    CHECK(capyro_core_playlist_current(core) == 0);
    capyro_core_playlist_remove(core, 0);                  /* lista vacía */
    CHECK(capyro_core_playlist_count(core) == 0);
    CHECK(capyro_core_get_state(core) == CAPYRO_STATE_IDLE);

    /* --- repetir uno: al terminar vuelve a empezar el mismo, Y SE ESCUCHA ---
     * Bug de la v0.2.2: al volver a empezar, el reloj pasaba a ser "de pared"
     * y el callback de audio dejaba de sacar sonido. */
    CHECK(capyro_core_playlist_add(core, clip) == CAPYRO_OK);
    CHECK(capyro_core_playlist_add(core, clip) == CAPYRO_OK);
    CHECK(capyro_core_playlist_select(core, 0) == CAPYRO_OK);
    capyro_core_set_repeat(core, CAPYRO_REPEAT_ONE);
    capyro_core_seek(core, 1.7);
    capyro_core_play(core);
    pump(core, 900);
    CHECK(capyro_core_playlist_current(core) == 0);
    CHECK(capyro_core_get_state(core) == CAPYRO_STATE_PLAYING);
    pump(core, 400);
    double looped = capyro_core_get_position(core);
    printf("repetir uno: posición tras volver a empezar: %.3f\n", looped);
    CHECK(looped > 0.1 && looped < 1.2);
    if (core->audio_ok) {
        cp_mutex_lock(&core->lock);
        bool audio_clock = core->clock_audio;
        cp_mutex_unlock(&core->lock);
        CHECK(audio_clock); /* el reloj sigue al audio = el audio está sonando */
    }
    capyro_core_set_repeat(core, CAPYRO_REPEAT_OFF);

    /* --- formatos nuevos: wav, ogg y mov --- */
    const char *extra[] = { "tono.wav", "tono.ogg", "clip.mov" };
    const int   kinds[] = { CAPYRO_MEDIA_AUDIO, CAPYRO_MEDIA_AUDIO, CAPYRO_MEDIA_VIDEO };
    for (int i = 0; i < 3; i++) {
        char path[1024];
        snprintf(path, sizeof(path), "%s/%s", dir, extra[i]);
        if (!exists(path)) {
            continue;
        }
        capyro_core_playlist_clear(core);
        CHECK(capyro_core_open(core, path) == CAPYRO_OK);
        CHECK(capyro_core_get_media_kind(core) == kinds[i]);
        CHECK(NEAR(capyro_core_get_duration(core), 2.0, 0.2));
        capyro_core_play(core);
        pump(core, 400);
        double p = capyro_core_get_position(core);
        printf("%s: posición tras 0.4 s: %.3f\n", extra[i], p);
        CHECK(p > 0.2 && p < 0.7);
    }

    capyro_core_shutdown(core);
}

int main(int argc, char **argv) {
    test_basics();
    if (argc > 1) {
        test_media(argv[1]);
    } else {
        printf("sin carpeta de media: salteo las pruebas de decodificación\n");
    }

    if (failures) {
        fprintf(stderr, "%d prueba(s) fallaron\n", failures);
        return EXIT_FAILURE;
    }
    printf("core: todas las pruebas pasaron\n");
    return EXIT_SUCCESS;
}

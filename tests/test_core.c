/*
 * Pruebas del núcleo, sin GUI. Se corren con `ctest`.
 *
 * Uso: test_core [carpeta_con_media]
 * La carpeta la genera CMake con el ffmpeg de línea de comandos (si está
 * instalado). Sin ella solo se corren las pruebas que no decodifican.
 */
#if !defined(_WIN32) && !defined(_POSIX_C_SOURCE)
#  define _POSIX_C_SOURCE 200809L
#endif

#include "capyro_core.h"

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#  include <windows.h>
static void sleep_ms(int ms) { Sleep((DWORD)ms); }
#else
#  include <time.h>
static void sleep_ms(int ms) {
    struct timespec ts = { ms / 1000, (long)(ms % 1000) * 1000000L };
    nanosleep(&ts, NULL);
}
#endif

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

    /* --- mp3: duración, metadatos, reproducción, seek, fin --- */
    CHECK(capyro_core_open(core, tone) == CAPYRO_OK);
    CHECK(capyro_core_get_media_kind(core) == CAPYRO_MEDIA_AUDIO);
    CHECK(NEAR(capyro_core_get_duration(core), 3.0, 0.15));
    CHECK(strcmp(capyro_core_get_title(core), "Tono de prueba") == 0);
    CHECK(strcmp(capyro_core_get_artist(core), "Capyro") == 0);
    CapyroVideoFrame vf;
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

    /* --- mp3 con carátula --- */
    CHECK(capyro_core_open(core, cover) == CAPYRO_OK);
    CHECK(capyro_core_get_media_kind(core) == CAPYRO_MEDIA_AUDIO);
    if (capyro_core_lock_video_frame(core, &vf)) {
        CHECK(vf.width == 64 && vf.height == 64 && vf.rgba != NULL);
        capyro_core_unlock_video_frame(core);
    } else {
        CHECK(!"la carátula debería verse como frame");
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
    CHECK(capyro_core_playlist_add(core, tone) == CAPYRO_OK);
    CHECK(capyro_core_playlist_select(core, 0) == CAPYRO_OK);
    capyro_core_seek(core, 1.7);
    capyro_core_play(core);
    pump(core, 1200);
    CHECK(capyro_core_playlist_current(core) == 1);
    CHECK(capyro_core_get_state(core) == CAPYRO_STATE_PLAYING);

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

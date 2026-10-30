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

static void sleep_ms(int ms) {
    cp_sleep_ms(ms); /* de platform.h: anda en Linux y en Windows */
}

static int failures = 0;

#define CHECK(cond)                                                          \
    do {                                                                     \
        if (!(cond)) {                                                       \
            fprintf(stderr, "FALLA %s:%d: %s\n", __FILE__, __LINE__, #cond); \
            failures++;                                                      \
        }                                                                    \
    } while (0)

#define CLOSE_TO(a, b, tol) (fabs((double)(a) - (double)(b)) <= (tol))

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

/* Sistema operativo detectado */
static void test_os(void) {
    CapyroOsInfo os;
    capyro_core_os_info(&os);
    printf("sistema: %s · %s · %s\n", os.name, os.version, os.code);
    CHECK(os.name[0] != '\0');
#ifdef _WIN32
    CHECK(os.family == CAPYRO_OS_WINDOWS && strcmp(os.code, "Win") == 0);
#else
    CHECK(os.family == CAPYRO_OS_LINUX && strcmp(os.code, "Lin") == 0);
#endif
    CHECK(strcmp(capyro_core_build_info()->system, os.code) == 0);
}

static void test_basics(void) {
    CHECK(capyro_core_probe_extension("a.mp4") == CAPYRO_MEDIA_VIDEO);
    CHECK(capyro_core_probe_extension("A.MP3") == CAPYRO_MEDIA_AUDIO);
    CHECK(capyro_core_probe_extension("a.mkv") == CAPYRO_MEDIA_VIDEO);
    CHECK(capyro_core_probe_extension("Peli.WebM") == CAPYRO_MEDIA_VIDEO);
    CHECK(capyro_core_probe_extension("a.mka") == CAPYRO_MEDIA_NONE);
    CHECK(capyro_core_probe_extension("clip.MOV") == CAPYRO_MEDIA_VIDEO);
    CHECK(capyro_core_probe_extension("tono.wav") == CAPYRO_MEDIA_AUDIO);
    CHECK(capyro_core_probe_extension("tono.Ogg") == CAPYRO_MEDIA_AUDIO);
    CHECK(capyro_core_probe_extension("tono.flac") == CAPYRO_MEDIA_AUDIO);
    CHECK(capyro_core_probe_extension("voz.OPUS") == CAPYRO_MEDIA_AUDIO);
    CHECK(capyro_core_probe_extension("tema.m4a") == CAPYRO_MEDIA_AUDIO);
    CHECK(capyro_core_probe_extension("estudio.aiff") == CAPYRO_MEDIA_AUDIO);
    CHECK(capyro_core_probe_extension("estudio.AIF") == CAPYRO_MEDIA_AUDIO);
    CHECK(capyro_core_probe_extension("estudio.aifc") == CAPYRO_MEDIA_AUDIO);
    CHECK(capyro_core_probe_extension("estudio.aiffx") == CAPYRO_MEDIA_NONE);
    CHECK(capyro_core_probe_extension("tono.wma") == CAPYRO_MEDIA_NONE);
    CHECK(capyro_core_probe_extension("viejo.AVI") == CAPYRO_MEDIA_VIDEO);
    CHECK(capyro_core_probe_extension("tema.aac") == CAPYRO_MEDIA_AUDIO);
    CHECK(strcmp(capyro_core_supported_video(), "mp4,mov,avi,mkv,webm") == 0);
    CHECK(strcmp(capyro_core_supported_audio(), "mp3,wav,ogg,aac,flac,opus,m4a,aiff") == 0);
    CHECK(capyro_core_probe_extension("mp4") == CAPYRO_MEDIA_NONE);
    CHECK(capyro_core_probe_extension(NULL) == CAPYRO_MEDIA_NONE);

    /* datos del build: siempre cadenas válidas (vacías si no hay git) */
    const CapyroBuildInfo *bi = capyro_core_build_info();
    CHECK(bi != NULL && bi->commit && bi->branch && bi->remote && bi->subject);
    CHECK(bi->prefix && bi->prefix[0] == '/');
    CHECK(bi->bindir && strstr(bi->bindir, bi->prefix) == bi->bindir);
    CHECK(bi->commit[0] == '\0' || strlen(bi->commit) == 40);

    CapyroCore *core = capyro_core_init();
    CHECK(core != NULL);
    if (!core) {
        return;
    }
    CHECK(capyro_core_get_state(core) == CAPYRO_STATE_IDLE);
    capyro_core_play(core);
    CHECK(capyro_core_get_state(core) == CAPYRO_STATE_IDLE);

    CHECK(capyro_core_open(core, "video.wmv") == CAPYRO_ERR_UNSUPPORTED);
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

/* ---------- efectos de audio (DSP puro, sin dispositivo) ---------- */

#define FX_RATE 48000u
#define FX_N    (FX_RATE / 2) /* medio segundo */

static void sine(float *buf, size_t frames, double hz, float amp) {
    for (size_t i = 0; i < frames; i++) {
        float v = amp * (float)sin(2.0 * 3.14159265358979 * hz * (double)i / FX_RATE);
        buf[2 * i] = buf[2 * i + 1] = v;
    }
}

/* RMS del canal izquierdo, salteando el arranque del filtro. */
static double rms_left(const float *buf, size_t frames) {
    double acc = 0.0;
    size_t from = frames / 4;
    for (size_t i = from; i < frames; i++) {
        acc += (double)buf[2 * i] * buf[2 * i];
    }
    return sqrt(acc / (double)(frames - from));
}

static double fx_gain_db(const CapyroAudioFx *fx, double hz, float amp) {
    static float buf[FX_N * 2];
    Dsp d;
    dsp_init(&d, FX_RATE);
    dsp_set(&d, fx);
    sine(buf, FX_N, hz, amp);
    double before = rms_left(buf, FX_N);
    dsp_process(&d, buf, FX_N);
    return 20.0 * log10(rms_left(buf, FX_N) / before);
}

static void test_dsp(void) {
    CapyroAudioFx neutral = { .enabled = true, .width = 1.0f };
    static float buf[FX_N * 2], copy[FX_N * 2];

    /* neutro = no toca ni un bit */
    Dsp d;
    dsp_init(&d, FX_RATE);
    dsp_set(&d, &neutral);
    sine(buf, FX_N, 440.0, 0.5f);
    memcpy(copy, buf, sizeof(buf));
    dsp_process(&d, buf, FX_N);
    CHECK(memcmp(buf, copy, sizeof(buf)) == 0);

    /* graves +12: los 50 Hz suben ~12 dB más que los 5 kHz */
    CapyroAudioFx bass = neutral;
    bass.bass_db = 12.0f;
    double low = fx_gain_db(&bass, 50.0, 0.05f), high = fx_gain_db(&bass, 5000.0, 0.05f);
    printf("dsp graves +12: 50 Hz %+.1f dB, 5 kHz %+.1f dB\n", low, high);
    CHECK(low - high > 10.0 && low - high < 13.0);

    /* agudos -12: 10 kHz bajan, 100 Hz casi no */
    CapyroAudioFx treble = neutral;
    treble.treble_db = -12.0f;
    double t_hi = fx_gain_db(&treble, 10000.0, 0.3f), t_lo = fx_gain_db(&treble, 100.0, 0.3f);
    printf("dsp agudos -12: 10 kHz %+.1f dB, 100 Hz %+.1f dB\n", t_hi, t_lo);
    CHECK(t_hi < -9.0 && fabs(t_lo) < 1.0);

    /* medios +9: pico en 1,5 kHz */
    CapyroAudioFx mid = neutral;
    mid.mid_db = 9.0f;
    double m_c = fx_gain_db(&mid, 1500.0, 0.05f), m_far = fx_gain_db(&mid, 60.0, 0.05f);
    CHECK(m_c - m_far > 7.5);

    /* ecualizador de 15 bandas activado: el de 3 bandas no suena (uno u otro) */
    CapyroAudioFx adv = bass;
    adv.eq15_on = true;
    dsp_set(&d, &adv);
    sine(buf, FX_N, 50.0, 0.5f);
    memcpy(copy, buf, sizeof(buf));
    dsp_process(&d, buf, FX_N);
    CHECK(memcmp(buf, copy, sizeof(buf)) == 0); /* graves +12 ignorados, 15 bandas planas */
    adv.eq15[2] = 12.0f;                        /* banda de 63 Hz */
    double a_low = fx_gain_db(&adv, 63.0, 0.05f), a_high = fx_gain_db(&adv, 5000.0, 0.05f);
    printf("dsp 15 bandas (63 Hz +12, graves +12 ignorados): 63 Hz %+.1f dB, 5 kHz %+.1f dB\n", a_low, a_high);
    CHECK(a_low - a_high > 10.0 && a_low - a_high < 13.0);

    /* estéreo 0 = mono: izquierda == derecha */
    CapyroAudioFx mono = neutral;
    mono.width = 0.0f;
    dsp_set(&d, &mono);
    for (size_t i = 0; i < 1000; i++) {
        buf[2 * i]     = (float)(i % 7) * 0.05f;
        buf[2 * i + 1] = -(float)(i % 5) * 0.05f;
    }
    dsp_process(&d, buf, 1000);
    bool same = true;
    for (size_t i = 0; i < 1000; i++) {
        same = same && fabsf(buf[2 * i] - buf[2 * i + 1]) < 1e-6f;
    }
    CHECK(same);

    /* modo noche: un tono fuerte y uno suave quedan mucho más parejos */
    CapyroAudioFx night = neutral;
    night.night = true;
    double loud = fx_gain_db(&night, 440.0, 0.9f), soft = fx_gain_db(&night, 440.0, 0.03f);
    printf("dsp modo noche: fuerte %+.1f dB, suave %+.1f dB\n", loud, soft);
    CHECK(soft - loud > 15.0); /* rango dinámico: de 29,5 dB pasa a menos de 15 */
    CHECK(soft > 5.0);         /* lo bajito sube */

    /* interruptor general apagado = no se toca nada aunque haya ajustes */
    CapyroAudioFx off = bass;
    off.enabled = false;
    off.night = true;
    dsp_set(&d, &off);
    sine(buf, FX_N, 60.0, 0.5f);
    memcpy(copy, buf, sizeof(buf));
    dsp_process(&d, buf, FX_N);
    CHECK(memcmp(buf, copy, sizeof(buf)) == 0);

    /* nunca satura: graves y agudos al máximo con señal a tope */
    CapyroAudioFx max = neutral;
    max.bass_db = max.mid_db = max.treble_db = 12.0f;
    max.width = 2.0f;
    dsp_set(&d, &max);
    sine(buf, FX_N, 80.0, 1.0f);
    dsp_process(&d, buf, FX_N);
    float peak = 0.0f;
    for (size_t i = 0; i < FX_N * 2; i++) {
        peak = fabsf(buf[i]) > peak ? fabsf(buf[i]) : peak;
    }
    CHECK(peak <= 1.0f);

    /* rangos */
    CapyroAudioFx wild = { .enabled = true, .bass_db = 99.0f, .mid_db = -99.0f,
                           .treble_db = NAN, .width = 7.0f };
    dsp_clamp(&wild);
    CHECK(wild.bass_db == 12.0f && wild.mid_db == -12.0f && wild.treble_db == 0.0f && wild.width == 2.0f);
}

/* Frecuencia dominante contando cruces por cero (sirve para un tono puro). */
static double zero_cross_hz(const float *x, size_t n, unsigned rate) {
    size_t crossings = 0;
    for (size_t i = 1; i < n; i++) {
        if ((x[i - 1] < 0.0f) != (x[i] < 0.0f)) {
            crossings++;
        }
    }
    return (double)crossings / 2.0 * (double)rate / (double)n;
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
    CHECK(CLOSE_TO(capyro_core_get_duration(core), 3.0, 0.15));
    CHECK(strcmp(capyro_core_get_title(core), "Tono de prueba") == 0);
    CHECK(strcmp(capyro_core_get_artist(core), "Capyro") == 0);
    CHECK(!capyro_core_lock_video_frame(core, &vf)); /* sin carátula */
    {
        CapyroVideoProps ap;
        CHECK(!capyro_core_get_video_props(core, &ap)); /* un audio no es video */
    }

    capyro_core_play(core);
    pump(core, 600);
    double pos = capyro_core_get_position(core);
    printf("posición tras 0.6 s: %.3f\n", pos);
    CHECK(pos > 0.3 && pos < 0.9);

    capyro_core_pause(core);
    double paused_at = capyro_core_get_position(core);
    pump(core, 300);
    CHECK(CLOSE_TO(capyro_core_get_position(core), paused_at, 0.02));

    capyro_core_seek(core, 2.5);
    CHECK(CLOSE_TO(capyro_core_get_position(core), 2.5, 0.01));
    capyro_core_play(core);
    pump(core, 1500);
    CHECK(capyro_core_get_state(core) == CAPYRO_STATE_PAUSED); /* llegó al final */
    CHECK(CLOSE_TO(capyro_core_get_position(core), capyro_core_get_duration(core), 0.01));

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
    CHECK(CLOSE_TO(capyro_core_get_duration(core), 2.0, 0.15));
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
        CHECK(CLOSE_TO(vf.aspect, 320.0 / 240.0, 0.01));
        first_serial = vf.serial;
        capyro_core_unlock_video_frame(core);
    } else {
        CHECK(!"el primer frame debería estar listo sin dar play");
    }
    CapyroVideoProps vp;
    CHECK(capyro_core_get_video_props(core, &vp));
    printf("video: %dx%d a %.2f fps\n", vp.width, vp.height, vp.fps);
    CHECK(vp.width == 320 && vp.height == 240 && vp.fps > 1.0);

    /* la captura sale en tamaño original aunque se muestre más chico */
    CapyroImage shot;
    CHECK(capyro_core_capture_frame(core, &shot));
    CHECK(shot.width == 320 && shot.height == 240 && shot.rgba != NULL);
    capyro_core_free_image(&shot);
    CHECK(shot.rgba == NULL);

    /* en una ventana chica los cuadros se convierten al tamaño de pantalla */
    capyro_core_set_video_target(core, 160, 120);
    capyro_core_play(core);
    pump(core, 400);
    double due = capyro_core_video_next_due(core);
    printf("video: próximo cuadro en %.3f s\n", due);
    CHECK(due >= -1.0 && due < 0.5);
    if (capyro_core_lock_video_frame(core, &vf)) {
        CHECK(vf.serial > first_serial);
        printf("video: cuadro de %dx%d con objetivo 160x120\n", vf.width, vf.height);
        CHECK(vf.width == 160 && vf.height == 120);
        capyro_core_unlock_video_frame(core);
    }
    capyro_core_set_video_target(core, 0, 0);

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
    CHECK(CLOSE_TO(capyro_core_get_position(core), before, 0.05));
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

    /* --- el resto de los formatos: abren, duran lo que tienen que durar y
     * avanzan al reproducir (un .webm con solo audio es audio) --- */
    const char *extra[] = { "tono.wav", "tono.ogg", "clip.mov", "tono.aac", "clip.avi",
                            "clip.mkv", "clip.webm", "solo-audio.webm", "tono.flac",
                            "tono.opus", "tono.m4a", "tono.aiff" };
    const int   kinds[] = { CAPYRO_MEDIA_AUDIO, CAPYRO_MEDIA_AUDIO, CAPYRO_MEDIA_VIDEO,
                            CAPYRO_MEDIA_AUDIO, CAPYRO_MEDIA_VIDEO, CAPYRO_MEDIA_VIDEO,
                            CAPYRO_MEDIA_VIDEO, CAPYRO_MEDIA_AUDIO, CAPYRO_MEDIA_AUDIO,
                            CAPYRO_MEDIA_AUDIO, CAPYRO_MEDIA_AUDIO, CAPYRO_MEDIA_AUDIO };
    for (int i = 0; i < (int)(sizeof(extra) / sizeof(extra[0])); i++) {
        char path[1024];
        snprintf(path, sizeof(path), "%s/%s", dir, extra[i]);
        if (!exists(path)) {
            continue;
        }
        capyro_core_playlist_clear(core);
        CHECK(capyro_core_open(core, path) == CAPYRO_OK);
        CHECK(capyro_core_get_media_kind(core) == kinds[i]);
        /* (el .aac suelto no trae duración: antes se estimaba por el bitrate;
         * ahora se cuenta al abrir, y tiene que dar justa) */
        CHECK(CLOSE_TO(capyro_core_get_duration(core), 2.0, 0.07));
        capyro_core_play(core);
        pump(core, 400);
        double p = capyro_core_get_position(core);
        printf("%s: posición tras 0.4 s: %.3f\n", extra[i], p);
        CHECK(p > 0.2 && p < 0.7);
    }

    /* --- bugs de la revisión de formatos --- */
    char p2[1024];
    /* el "title" de una pista de Matroska es su nombre, no el del archivo */
    snprintf(p2, sizeof(p2), "%s/clip.mkv", dir);
    if (exists(p2)) {
        capyro_core_playlist_clear(core);
        CHECK(capyro_core_open(core, p2) == CAPYRO_OK);
        CHECK(strcmp(capyro_core_get_title(core), "") == 0);
    }
    /* video de celular: se ve derecho (girado), con su resolución "parada" */
    snprintf(p2, sizeof(p2), "%s/rotado.mp4", dir);
    if (exists(p2)) {
        capyro_core_playlist_clear(core);
        CHECK(capyro_core_open(core, p2) == CAPYRO_OK);
        pump(core, 150);
        CapyroVideoProps vp;
        CHECK(capyro_core_get_video_props(core, &vp) && vp.width == 240 && vp.height == 320);
        CapyroVideoFrame vf;
        if (capyro_core_lock_video_frame(core, &vf)) {
            CHECK(vf.width < vf.height && vf.aspect < 1.0f);
            capyro_core_unlock_video_frame(core);
        } else {
            CHECK(!"rotado.mp4 sin cuadro");
        }
        CapyroImage im;
        if (capyro_core_capture_frame(core, &im)) {
            CHECK(im.width == 240 && im.height == 320);
            capyro_core_free_image(&im);
        }
    }
    /* carátula de 300 px y video vertical: swscale escribía de más */
    const char *odd[] = { "tapa.flac", "vertical.mp4" };
    for (int i = 0; i < 2; i++) {
        snprintf(p2, sizeof(p2), "%s/%s", dir, odd[i]);
        if (!exists(p2)) {
            continue;
        }
        capyro_core_playlist_clear(core);
        capyro_core_set_video_target(core, 300, 170); /* achica a anchos impares */
        CHECK(capyro_core_open(core, p2) == CAPYRO_OK);
        capyro_core_play(core);
        pump(core, 300);
        CapyroVideoFrame vf;
        if (capyro_core_lock_video_frame(core, &vf)) {
            printf("%s: cuadro %dx%d\n", odd[i], vf.width, vf.height);
            capyro_core_unlock_video_frame(core);
        } else {
            CHECK(!"sin cuadro");
        }
        capyro_core_set_video_target(core, 0, 0);
    }
    /* al terminar uno, si el siguiente está roto se saltea (antes se cortaba) */
    char a1[1024], a3[1024];
    snprintf(a1, sizeof(a1), "%s/tono.wav", dir);
    snprintf(a3, sizeof(a3), "%s/tono.ogg", dir);
    if (exists(a1) && exists(a3)) {
        touch("capyro_test_roto.flac");
        capyro_core_playlist_clear(core);
        CHECK(capyro_core_playlist_add(core, a1) == CAPYRO_OK);
        CHECK(capyro_core_playlist_add(core, "capyro_test_roto.flac") == CAPYRO_OK);
        CHECK(capyro_core_playlist_add(core, "capyro_test_no_existe.flac") == CAPYRO_OK);
        CHECK(capyro_core_playlist_add(core, a3) == CAPYRO_OK);
        CHECK(capyro_core_playlist_select(core, 0) == CAPYRO_OK);
        capyro_core_seek(core, 1.8);
        capyro_core_play(core);
        pump(core, 900);
        CHECK(capyro_core_playlist_current(core) == 3);
        CHECK(capyro_core_get_state(core) == CAPYRO_STATE_PLAYING);
        /* y al terminar el último (sin siguiente) queda ese, en pausa al final */
        capyro_core_seek(core, 1.8);
        pump(core, 700);
        CHECK(capyro_core_playlist_current(core) == 3);
        CHECK(capyro_core_get_state(core) == CAPYRO_STATE_PAUSED);
        remove("capyro_test_roto.flac");
    }

    /* --- tono: +12 semitonos = una octava arriba, misma velocidad --- */
    char wav_tone[1024];
    snprintf(wav_tone, sizeof(wav_tone), "%s/tono.wav", dir); /* 440 Hz, 2 s */
    if (core->audio_ok && exists(wav_tone)) {
        static float sc[4096];
        unsigned rate = capyro_core_audio_rate(core);
        capyro_core_playlist_clear(core);
        CHECK(capyro_core_open(core, wav_tone) == CAPYRO_OK);

        capyro_core_set_pitch(core, 99.0);
        CHECK(capyro_core_get_pitch(core) == 12.0);
        capyro_core_play(core);
        pump(core, 500);
        double pos = capyro_core_get_position(core);
        CHECK(capyro_core_get_scope(core, sc, 4096) == 4096);
        double hz = zero_cross_hz(sc, 4096, rate);
        printf("tono +12: %.0f Hz (esperado 880), posición tras 0,5 s: %.2f\n", hz, pos);
        CHECK(hz > 820.0 && hz < 940.0);
        CHECK(pos > 0.3 && pos < 0.75); /* la velocidad no cambió */

        /* con el interruptor general apagado el tono vuelve a 440 */
        CapyroAudioFx fx;
        capyro_core_get_fx(core, &fx);
        fx.enabled = false;
        capyro_core_set_fx(core, &fx);
        pump(core, 400);
        capyro_core_get_scope(core, sc, 4096);
        hz = zero_cross_hz(sc, 4096, rate);
        printf("efectos apagados: %.0f Hz (esperado 440)\n", hz);
        CHECK(hz > 410.0 && hz < 470.0);
        CHECK(capyro_core_get_pitch(core) == 12.0); /* el ajuste se recuerda */

        fx.enabled = true;
        capyro_core_set_fx(core, &fx);
        capyro_core_set_pitch(core, 0.0);
        capyro_core_pause(core);
    }

    /* --- visualizador: el núcleo guarda lo último que sonó --- */
    if (core->audio_ok) {
        capyro_core_playlist_clear(core);
        CHECK(capyro_core_open(core, has_tone ? tone : clip) == CAPYRO_OK);
        capyro_core_play(core);
        pump(core, 400);
        static float scope[4096];
        CHECK(capyro_core_get_scope(core, scope, 1024) == 1024);
        CHECK(capyro_core_get_scope(core, scope, 999999) == 4096); /* se limita a 4096 */
        CHECK(capyro_core_get_scope(core, scope, 1024) == 1024);
        double energy = 0.0;
        for (int i = 0; i < 1024; i++) {
            energy += (double)scope[i] * scope[i];
        }
        printf("visualizador: energía de las últimas 1024 muestras = %.2f\n", energy);
        CHECK(energy > 1.0); /* un tono, no silencio */
        CHECK(capyro_core_audio_rate(core) == core->out_rate);
        capyro_core_pause(core);
    }

    /* --- efectos de sonido: se cargan desde memoria y se mezclan --- */
    char wav[1024];
    snprintf(wav, sizeof(wav), "%s/tono.wav", dir);
    FILE *f = fopen(wav, "rb");
    if (f && core->audio_ok) {
        fseek(f, 0, SEEK_END);
        long size = ftell(f);
        fseek(f, 0, SEEK_SET);
        uint8_t *data = malloc((size_t)size);
        CHECK(data && fread(data, 1, (size_t)size, f) == (size_t)size);
        CHECK(capyro_core_sfx_load(core, 0, data, (size_t)size) == CAPYRO_OK);
        free(data); /* el núcleo ya lo decodificó: no guarda el puntero */
        CHECK(core->sfx_len[0] > (size_t)(core->out_rate * 1.9)); /* ~2 s a la frecuencia del dispositivo */
        CHECK(capyro_core_sfx_load(core, 1, (const uint8_t *)"basura", 6) != CAPYRO_OK);
        capyro_core_sfx_play(core, 1);           /* slot vacío: no hace nada */
        CHECK(core->sfx_playing == -1);
        capyro_core_playlist_clear(core);        /* suena aunque no haya nada cargado */
        capyro_core_sfx_play(core, 0);
        pump(core, 300);
        cp_mutex_lock(&core->lock);
        size_t pos = core->sfx_pos;
        int playing = core->sfx_playing;
        cp_mutex_unlock(&core->lock);
        printf("sfx: %zu frames mezclados en 0.3 s\n", pos);
        CHECK(playing == 0 && pos > 0);
        pump(core, 2000);
        CHECK(core->sfx_playing == -1);          /* terminó solo */
    }
    if (f) {
        fclose(f);
    }

    capyro_core_shutdown(core);
}

/* Decodificación GPU/CPU: el modo se respeta, cambiarlo con el video abierto
 * rearma el decodificador sin cortar, y sin GPU todo sigue por CPU. */
static void test_hwdec(const char *dir) {
    char clip[1024];
    snprintf(clip, sizeof(clip), "%s/clip.mp4", dir);
    if (!exists(clip)) {
        return;
    }
    CapyroCore *core = capyro_core_init();
    CHECK(core != NULL);
    if (!core) {
        return;
    }
    CapyroDecodeInfo di;
    CHECK(capyro_core_get_hwdec(core) == CAPYRO_HWDEC_AUTO);
    capyro_core_get_decode_info(core, &di);
    CHECK(!di.active);

    /* CPU forzada */
    capyro_core_set_hwdec(core, CAPYRO_HWDEC_CPU);
    CHECK(capyro_core_get_hwdec(core) == CAPYRO_HWDEC_CPU);
    CHECK(capyro_core_open(core, clip) == CAPYRO_OK);
    capyro_core_get_decode_info(core, &di);
    CHECK(di.active && !di.hw && strcmp(di.api, "CPU") == 0 && di.codec[0] && di.note[0]);
    printf("hwdec cpu: %s · %s · %s\n", di.api, di.codec, di.note);

    /* pasar a automático con el video sonando: se rearma y sigue */
    uint64_t s0 = di.serial;
    capyro_core_play(core);
    pump(core, 200);
    capyro_core_set_hwdec(core, CAPYRO_HWDEC_AUTO);
    for (int t = 0; t < 3000; t += 20) {
        capyro_core_get_decode_info(core, &di);
        if (di.serial != s0) {
            break;
        }
        pump(core, 20);
    }
    CHECK(di.serial != s0 && di.active);
    printf("hwdec auto: %s%s%s\n", di.api, di.note[0] ? " · " : "", di.note);
    CHECK(di.hw || di.note[0]); /* sin GPU, la nota dice por qué */

    CapyroVideoFrame vf;
    uint64_t before = 0;
    if (capyro_core_lock_video_frame(core, &vf)) {
        before = vf.serial;
        capyro_core_unlock_video_frame(core);
    }
    pump(core, 500);
    double pos = capyro_core_get_position(core);
    CHECK(pos > 0.4); /* siguió avanzando después de rearmar */
    if (capyro_core_lock_video_frame(core, &vf)) {
        CHECK(vf.serial >= before && vf.width > 0);
        capyro_core_unlock_video_frame(core);
    } else {
        CHECK(!"después de rearmar el decodificador tiene que haber imagen");
    }

    capyro_core_set_hwdec(core, 7); /* inválido: se ignora */
    CHECK(capyro_core_get_hwdec(core) == CAPYRO_HWDEC_AUTO);

    /* un audio no usa decodificador de video */
    char wav[1024];
    snprintf(wav, sizeof(wav), "%s/tono.wav", dir);
    if (exists(wav)) {
        CHECK(capyro_core_open(core, wav) == CAPYRO_OK);
        capyro_core_get_decode_info(core, &di);
        CHECK(!di.active);
    }
    capyro_core_shutdown(core);
}

int main(int argc, char **argv) {
    test_os();
    test_basics();
    test_dsp();
    if (argc > 1) {
        test_media(argv[1]);
        test_hwdec(argv[1]);
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

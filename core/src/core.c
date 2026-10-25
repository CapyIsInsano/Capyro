/*
 * Capyro — núcleo: API pública, estado, transporte y lista de reproducción.
 * La parte de FFmpeg/miniaudio vive en player.c.
 */
#ifndef _POSIX_C_SOURCE
#  define _POSIX_C_SOURCE 200809L
#endif

#include "internal.h"

#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define CAPYRO_PLAYLIST_INITIAL_CAP 16
#define PREV_RESTART_THRESHOLD      3.0 /* "anterior" con más de 3 s reinicia */

/* ------------------------------------------------------------------ */
/* utilidades                                                          */
/* ------------------------------------------------------------------ */

static char *dup_string(const char *s) {
    size_t n = strlen(s) + 1;
    char *out = malloc(n);
    if (out) {
        memcpy(out, s, n);
    }
    return out;
}

static bool ext_equals(const char *ext, const char *want) {
    while (*ext && *want) {
        if (tolower((unsigned char)*ext) != *want) {
            return false;
        }
        ext++;
        want++;
    }
    return *ext == '\0' && *want == '\0';
}

/* xorshift64: suficiente para mezclar una lista. */
static uint64_t next_random(CapyroCore *core) {
    uint64_t x = core->rng;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    core->rng = x;
    return x;
}

static size_t random_below(CapyroCore *core, size_t n) {
    return n ? (size_t)(next_random(core) % n) : 0;
}

/* ---- orden de reproducción ---------------------------------------- */

static long order_pos_of(const CapyroCore *core, long index) {
    for (size_t i = 0; i < core->playlist_len; i++) {
        if ((long)core->order[i] == index) {
            return (long)i;
        }
    }
    return -1;
}

static void order_identity(CapyroCore *core) {
    for (size_t i = 0; i < core->playlist_len; i++) {
        core->order[i] = i;
    }
}

/* Mezcla todo; si hay uno sonando queda primero (lo que sigue es aleatorio). */
static void order_shuffle(CapyroCore *core) {
    size_t n = core->playlist_len;
    order_identity(core);
    for (size_t i = n; i > 1; i--) {
        size_t j = random_below(core, i);
        size_t tmp = core->order[i - 1];
        core->order[i - 1] = core->order[j];
        core->order[j] = tmp;
    }
    long pos = order_pos_of(core, core->playlist_current);
    if (pos > 0) {
        size_t tmp = core->order[0];
        core->order[0] = core->order[pos];
        core->order[pos] = tmp;
    }
}

/* Índice del siguiente según el orden, o -1. `wrap` = volver al principio. */
static long order_next(CapyroCore *core, bool wrap) {
    size_t n = core->playlist_len;
    if (n == 0) {
        return -1;
    }
    long pos = order_pos_of(core, core->playlist_current);
    if (pos < 0) {
        return (long)core->order[0];
    }
    if ((size_t)pos + 1 < n) {
        return (long)core->order[pos + 1];
    }
    if (!wrap) {
        return -1;
    }
    if (core->shuffle && n > 1) {
        /* Nueva vuelta con otro orden, sin repetir el que acaba de sonar. */
        long last = core->playlist_current;
        core->playlist_current = -1;
        order_shuffle(core);
        core->playlist_current = last;
        if ((long)core->order[0] == last) {
            core->order[0] = core->order[1];
            core->order[1] = (size_t)last;
        }
    }
    return (long)core->order[0];
}

static long order_prev(CapyroCore *core, bool wrap) {
    size_t n = core->playlist_len;
    long pos = order_pos_of(core, core->playlist_current);
    if (n == 0 || pos < 0) {
        return -1;
    }
    if (pos > 0) {
        return (long)core->order[pos - 1];
    }
    return wrap ? (long)core->order[n - 1] : -1;
}

static double clamp_pos(const CapyroCore *core, double t) {
    if (t < 0.0) {
        return 0.0;
    }
    if (core->duration > 0.0 && t > core->duration) {
        return core->duration;
    }
    return t;
}

/* Deja el núcleo sin nada cargado (el hilo ya tiene que estar parado). */
static void set_idle(CapyroCore *core) {
    cp_mutex_lock(&core->lock);
    core->state = CAPYRO_STATE_IDLE;
    core->kind  = CAPYRO_MEDIA_NONE;
    cp_mutex_unlock(&core->lock);
    core->playlist_current = -1;
}

/* Carga el archivo `index` de la lista (sin reproducir). */
static int load_index(CapyroCore *core, size_t index) {
    if (index >= core->playlist_len) {
        return CAPYRO_ERR_OUT_OF_RANGE;
    }
    const char *path = core->playlist[index];

    FILE *f = fopen(path, "rb");
    if (!f) {
        player_close(core);
        set_idle(core);
        return CAPYRO_ERR_NOT_FOUND;
    }
    fclose(f);

    int err = player_open(core, path);
    if (err != CAPYRO_OK) {
        set_idle(core);
        return err;
    }
    cp_mutex_lock(&core->lock);
    core->state = CAPYRO_STATE_STOPPED;
    cp_mutex_unlock(&core->lock);
    core->playlist_current = (long)index;
    return CAPYRO_OK;
}

/* ------------------------------------------------------------------ */
/* ciclo de vida                                                       */
/* ------------------------------------------------------------------ */

CapyroCore *capyro_core_init(void) {
    CapyroCore *core = calloc(1, sizeof(*core));
    if (!core) {
        return NULL;
    }
    core->playlist = calloc(CAPYRO_PLAYLIST_INITIAL_CAP, sizeof(char *));
    core->order    = calloc(CAPYRO_PLAYLIST_INITIAL_CAP, sizeof(size_t));
    if (!core->playlist || !core->order) {
        free(core->playlist);
        free(core->order);
        free(core);
        return NULL;
    }
    cp_mutex_init(&core->lock);
    core->playlist_cap     = CAPYRO_PLAYLIST_INITIAL_CAP;
    core->playlist_current = -1;
    core->volume           = 1.0f;
    core->speed            = 1.0;
    core->repeat           = CAPYRO_REPEAT_OFF;
    core->rng              = (uint64_t)(cp_now() * 1e9) ^ 0x9E3779B97F4A7C15ull;
    if (core->rng == 0) {
        core->rng = 88172645463325252ull;
    }
    core->auto_next        = true;
    core->shown            = -1;
    core->player.aidx      = -1;
    core->player.vidx      = -1;
    core->state            = CAPYRO_STATE_IDLE;

    if (player_global_init(core) != CAPYRO_OK) {
        cp_mutex_destroy(&core->lock);
        free(core->playlist);
        free(core->order);
        free(core);
        return NULL;
    }
    return core;
}

void capyro_core_shutdown(CapyroCore *core) {
    if (!core) {
        return;
    }
    player_global_shutdown(core);
    for (size_t i = 0; i < core->playlist_len; i++) {
        free(core->playlist[i]);
    }
    free(core->playlist);
    free(core->order);
    cp_mutex_destroy(&core->lock);
    free(core);
}

const char *capyro_core_version(void) {
    return CAPYRO_VERSION;
}

const char *capyro_core_error_string(int err) {
    switch (err) {
    case CAPYRO_OK:               return "ok";
    case CAPYRO_ERR_INVALID_ARG:  return "argumento inválido";
    case CAPYRO_ERR_UNSUPPORTED:  return "formato no soportado (solo .mp4 y .mp3)";
    case CAPYRO_ERR_NOT_FOUND:    return "no se pudo abrir el archivo";
    case CAPYRO_ERR_NO_MEMORY:    return "sin memoria";
    case CAPYRO_ERR_OUT_OF_RANGE: return "fuera de rango";
    case CAPYRO_ERR_DECODE:       return "no se pudo leer el archivo (¿está dañado?)";
    case CAPYRO_ERR_NO_STREAMS:   return "el archivo no tiene audio ni video";
    case CAPYRO_ERR_THREAD:       return "no se pudo iniciar la decodificación";
    default:                      return "error desconocido";
    }
}

const char *capyro_core_audio_backend(CapyroCore *core) {
    return (core && core->audio_ok) ? core->backend : "";
}

void capyro_core_set_auto_next(CapyroCore *core, bool enabled) {
    if (core) {
        core->auto_next = enabled;
    }
}

void capyro_core_update(CapyroCore *core) {
    if (!core) {
        return;
    }
    cp_mutex_lock(&core->lock);
    bool ended = core->state == CAPYRO_STATE_PLAYING && player_ended_locked(core);
    if (ended) {
        /* Queda en pausa al final para que se vea el 100 %. */
        core->clock_base     = core->duration > 0.0 ? core->duration : player_clock_locked(core);
        core->clock_audio    = false;
        core->audio_consumed = 0;
        core->state          = CAPYRO_STATE_PAUSED;
    }
    cp_mutex_unlock(&core->lock);

    if (!ended) {
        return;
    }
    if (core->repeat == CAPYRO_REPEAT_ONE) {
        capyro_core_seek(core, 0.0);
        capyro_core_play(core);
        return;
    }
    if (core->auto_next) {
        long next = order_next(core, core->repeat == CAPYRO_REPEAT_ALL);
        if (next >= 0 && load_index(core, (size_t)next) == CAPYRO_OK) {
            capyro_core_play(core);
        }
    }
}

/* ------------------------------------------------------------------ */
/* abrir / formato                                                     */
/* ------------------------------------------------------------------ */

int capyro_core_probe_extension(const char *path) {
    if (!path) {
        return CAPYRO_MEDIA_NONE;
    }
    const char *dot = strrchr(path, '.');
    if (!dot) {
        return CAPYRO_MEDIA_NONE;
    }
    if (ext_equals(dot + 1, "mp4")) {
        return CAPYRO_MEDIA_VIDEO;
    }
    if (ext_equals(dot + 1, "mp3")) {
        return CAPYRO_MEDIA_AUDIO;
    }
    return CAPYRO_MEDIA_NONE;
}

int capyro_core_open(CapyroCore *core, const char *path) {
    if (!core || !path) {
        return CAPYRO_ERR_INVALID_ARG;
    }
    int err = capyro_core_playlist_add(core, path);
    if (err != CAPYRO_OK) {
        return err;
    }
    return capyro_core_playlist_select(core, core->playlist_len - 1);
}

/* ------------------------------------------------------------------ */
/* transporte                                                          */
/* ------------------------------------------------------------------ */

void capyro_core_play(CapyroCore *core) {
    if (!core) {
        return;
    }
    cp_mutex_lock(&core->lock);
    if (core->state == CAPYRO_STATE_IDLE || core->state == CAPYRO_STATE_PLAYING) {
        cp_mutex_unlock(&core->lock);
        return;
    }
    /* Si estaba al final, volver a empezar. */
    if (core->duration > 0.0 && player_clock_locked(core) >= core->duration - 0.05) {
        player_request_seek_locked(core, 0.0);
    }
    core->wall_started = cp_now();
    core->state        = CAPYRO_STATE_PLAYING;
    cp_mutex_unlock(&core->lock);

    player_ensure_device_started(core);
}

void capyro_core_pause(CapyroCore *core) {
    if (!core) {
        return;
    }
    cp_mutex_lock(&core->lock);
    if (core->state == CAPYRO_STATE_PLAYING) {
        if (!core->clock_audio) {
            core->clock_base = player_clock_locked(core);
        }
        core->state = CAPYRO_STATE_PAUSED;
    }
    cp_mutex_unlock(&core->lock);
}

void capyro_core_toggle(CapyroCore *core) {
    if (!core) {
        return;
    }
    if (capyro_core_get_state(core) == CAPYRO_STATE_PLAYING) {
        capyro_core_pause(core);
    } else {
        capyro_core_play(core);
    }
}

void capyro_core_stop(CapyroCore *core) {
    if (!core) {
        return;
    }
    cp_mutex_lock(&core->lock);
    if (core->state != CAPYRO_STATE_IDLE) {
        core->state = CAPYRO_STATE_STOPPED;
        player_request_seek_locked(core, 0.0);
    }
    cp_mutex_unlock(&core->lock);
}

void capyro_core_seek(CapyroCore *core, double seconds) {
    if (!core) {
        return;
    }
    cp_mutex_lock(&core->lock);
    if (core->state != CAPYRO_STATE_IDLE) {
        player_request_seek_locked(core, clamp_pos(core, seconds));
    }
    cp_mutex_unlock(&core->lock);
}

void capyro_core_seek_relative(CapyroCore *core, double delta) {
    if (!core) {
        return;
    }
    cp_mutex_lock(&core->lock);
    if (core->state != CAPYRO_STATE_IDLE) {
        player_request_seek_locked(core, clamp_pos(core, player_clock_locked(core) + delta));
    }
    cp_mutex_unlock(&core->lock);
}

void capyro_core_set_speed(CapyroCore *core, double speed) {
    if (!core) {
        return;
    }
    if (speed < CAPYRO_SPEED_MIN) speed = CAPYRO_SPEED_MIN;
    if (speed > CAPYRO_SPEED_MAX) speed = CAPYRO_SPEED_MAX;
    cp_mutex_lock(&core->lock);
    if (speed != core->speed) {
        double pos  = player_clock_locked(core);
        core->speed = speed;
        /* Un seek al mismo punto vacía lo ya decodificado a la velocidad
         * vieja; el hilo rearma el filtro atempo con la nueva. */
        if (core->state != CAPYRO_STATE_IDLE) {
            player_request_seek_locked(core, clamp_pos(core, pos));
        }
    }
    cp_mutex_unlock(&core->lock);
}

double capyro_core_get_speed(CapyroCore *core) {
    if (!core) {
        return 1.0;
    }
    cp_mutex_lock(&core->lock);
    double s = core->speed;
    cp_mutex_unlock(&core->lock);
    return s;
}

/* ------------------------------------------------------------------ */
/* volumen                                                             */
/* ------------------------------------------------------------------ */

void capyro_core_set_volume(CapyroCore *core, float volume) {
    if (!core) {
        return;
    }
    cp_mutex_lock(&core->lock);
    core->volume = volume < 0.0f ? 0.0f : (volume > 1.0f ? 1.0f : volume);
    cp_mutex_unlock(&core->lock);
}

float capyro_core_get_volume(CapyroCore *core) {
    if (!core) {
        return 0.0f;
    }
    cp_mutex_lock(&core->lock);
    float v = core->volume;
    cp_mutex_unlock(&core->lock);
    return v;
}

void capyro_core_set_muted(CapyroCore *core, bool muted) {
    if (!core) {
        return;
    }
    cp_mutex_lock(&core->lock);
    core->muted = muted;
    cp_mutex_unlock(&core->lock);
}

bool capyro_core_get_muted(CapyroCore *core) {
    if (!core) {
        return false;
    }
    cp_mutex_lock(&core->lock);
    bool m = core->muted;
    cp_mutex_unlock(&core->lock);
    return m;
}

/* ------------------------------------------------------------------ */
/* estado                                                              */
/* ------------------------------------------------------------------ */

int capyro_core_get_state(CapyroCore *core) {
    if (!core) {
        return CAPYRO_STATE_IDLE;
    }
    cp_mutex_lock(&core->lock);
    int s = core->state;
    cp_mutex_unlock(&core->lock);
    return s;
}

int capyro_core_get_media_kind(CapyroCore *core) {
    if (!core) {
        return CAPYRO_MEDIA_NONE;
    }
    cp_mutex_lock(&core->lock);
    int k = core->kind;
    cp_mutex_unlock(&core->lock);
    return k;
}

double capyro_core_get_position(CapyroCore *core) {
    if (!core) {
        return 0.0;
    }
    cp_mutex_lock(&core->lock);
    double t = core->state == CAPYRO_STATE_IDLE ? 0.0 : clamp_pos(core, player_clock_locked(core));
    cp_mutex_unlock(&core->lock);
    return t;
}

double capyro_core_get_duration(CapyroCore *core) {
    if (!core) {
        return 0.0;
    }
    cp_mutex_lock(&core->lock);
    double d = core->duration;
    cp_mutex_unlock(&core->lock);
    return d;
}

/* title/artist solo cambian al abrir, que pasa en el hilo de la GUI. */
const char *capyro_core_get_title(CapyroCore *core) {
    return core ? core->title : "";
}

const char *capyro_core_get_artist(CapyroCore *core) {
    return core ? core->artist : "";
}

const char *capyro_core_get_current_file(CapyroCore *core) {
    if (!core || core->playlist_current < 0) {
        return "";
    }
    return core->playlist[core->playlist_current];
}

/* ------------------------------------------------------------------ */
/* lista de reproducción                                               */
/* ------------------------------------------------------------------ */

int capyro_core_playlist_add(CapyroCore *core, const char *path) {
    if (!core || !path || !*path) {
        return CAPYRO_ERR_INVALID_ARG;
    }
    if (capyro_core_probe_extension(path) == CAPYRO_MEDIA_NONE) {
        return CAPYRO_ERR_UNSUPPORTED;
    }
    if (core->playlist_len == core->playlist_cap) {
        size_t new_cap = core->playlist_cap * 2;
        char **grown   = realloc(core->playlist, new_cap * sizeof(char *));
        if (!grown) {
            return CAPYRO_ERR_NO_MEMORY;
        }
        core->playlist = grown;
        size_t *grown_order = realloc(core->order, new_cap * sizeof(size_t));
        if (!grown_order) {
            return CAPYRO_ERR_NO_MEMORY;
        }
        core->order        = grown_order;
        core->playlist_cap = new_cap;
    }
    char *copy = dup_string(path);
    if (!copy) {
        return CAPYRO_ERR_NO_MEMORY;
    }
    size_t idx = core->playlist_len++;
    core->playlist[idx] = copy;

    /* En aleatorio, el nuevo cae en un lugar al azar de lo que falta sonar. */
    size_t at = idx;
    if (core->shuffle) {
        long pos = order_pos_of(core, core->playlist_current); /* idx todavía no está */
        size_t from = pos < 0 ? 0 : (size_t)pos + 1;
        at = from + random_below(core, idx - from + 1);
        memmove(&core->order[at + 1], &core->order[at], (idx - at) * sizeof(size_t));
    }
    core->order[at] = idx;
    return CAPYRO_OK;
}

size_t capyro_core_playlist_count(CapyroCore *core) {
    return core ? core->playlist_len : 0;
}

const char *capyro_core_playlist_get(CapyroCore *core, size_t index) {
    if (!core || index >= core->playlist_len) {
        return NULL;
    }
    return core->playlist[index];
}

long capyro_core_playlist_current(CapyroCore *core) {
    return core ? core->playlist_current : -1;
}

int capyro_core_playlist_select(CapyroCore *core, size_t index) {
    if (!core) {
        return CAPYRO_ERR_INVALID_ARG;
    }
    if (index >= core->playlist_len) {
        return CAPYRO_ERR_OUT_OF_RANGE;
    }
    bool was_playing = capyro_core_get_state(core) == CAPYRO_STATE_PLAYING;
    int err = load_index(core, index);
    if (err == CAPYRO_OK && was_playing) {
        capyro_core_play(core);
    }
    return err;
}

int capyro_core_playlist_next(CapyroCore *core) {
    if (!core) {
        return CAPYRO_ERR_INVALID_ARG;
    }
    /* Con repetir activo (todo o uno), "siguiente" a mano da la vuelta. */
    long next = order_next(core, core->repeat != CAPYRO_REPEAT_OFF);
    if (next < 0) {
        return CAPYRO_ERR_OUT_OF_RANGE;
    }
    return capyro_core_playlist_select(core, (size_t)next);
}

int capyro_core_playlist_prev(CapyroCore *core) {
    if (!core || core->playlist_len == 0) {
        return CAPYRO_ERR_OUT_OF_RANGE;
    }
    /* Como cualquier reproductor: si ya avanzó un poco, "anterior" reinicia. */
    long prev = order_prev(core, core->repeat != CAPYRO_REPEAT_OFF);
    if (capyro_core_get_position(core) > PREV_RESTART_THRESHOLD || prev < 0) {
        capyro_core_seek(core, 0.0);
        return CAPYRO_OK;
    }
    return capyro_core_playlist_select(core, (size_t)prev);
}

void capyro_core_playlist_remove(CapyroCore *core, size_t index) {
    if (!core || index >= core->playlist_len) {
        return;
    }
    bool removing_current = core->playlist_current == (long)index;
    bool was_playing      = removing_current && capyro_core_get_state(core) == CAPYRO_STATE_PLAYING;
    /* Qué viene después del que se borra (antes de tocar el orden). */
    long pos = order_pos_of(core, (long)index);

    free(core->playlist[index]);
    memmove(&core->playlist[index], &core->playlist[index + 1],
            (core->playlist_len - index - 1) * sizeof(char *));
    memmove(&core->order[pos], &core->order[pos + 1],
            (core->playlist_len - (size_t)pos - 1) * sizeof(size_t));
    core->playlist_len--;
    for (size_t i = 0; i < core->playlist_len; i++) {
        if (core->order[i] > index) {
            core->order[i]--;
        }
    }

    if (!removing_current) {
        if (core->playlist_current > (long)index) {
            core->playlist_current--;
        }
        return;
    }

    /* Se borró el que estaba cargado: pasar al que le seguía en el orden
     * (o al último si era el final), y seguir sonando si estaba sonando. */
    player_close(core);
    set_idle(core);
    if (core->playlist_len == 0) {
        return;
    }
    size_t next_pos = (size_t)pos < core->playlist_len ? (size_t)pos : core->playlist_len - 1;
    if (load_index(core, core->order[next_pos]) == CAPYRO_OK && was_playing) {
        capyro_core_play(core);
    }
}

void capyro_core_playlist_clear(CapyroCore *core) {
    if (!core) {
        return;
    }
    player_close(core);
    for (size_t i = 0; i < core->playlist_len; i++) {
        free(core->playlist[i]);
    }
    core->playlist_len = 0;
    set_idle(core);
}

/* ------------------------------------------------------------------ */
/* repetir / aleatorio                                                 */
/* ------------------------------------------------------------------ */

void capyro_core_set_repeat(CapyroCore *core, int mode) {
    if (core && mode >= CAPYRO_REPEAT_OFF && mode <= CAPYRO_REPEAT_ONE) {
        core->repeat = mode;
    }
}

int capyro_core_get_repeat(CapyroCore *core) {
    return core ? core->repeat : CAPYRO_REPEAT_OFF;
}

void capyro_core_set_shuffle(CapyroCore *core, bool enabled) {
    if (!core || core->shuffle == enabled) {
        return;
    }
    core->shuffle = enabled;
    if (enabled) {
        order_shuffle(core);
    } else {
        order_identity(core);
    }
}

bool capyro_core_get_shuffle(CapyroCore *core) {
    return core ? core->shuffle : false;
}

/*
 * Capyro — Just press play.
 *
 * API pública del núcleo (C). Todo lo que la GUI (Rust) necesita pasa por acá.
 * Reglas de la frontera FFI:
 *   - CapyroCore es opaco: la GUI sólo maneja el puntero.
 *   - Los enums viajan como int (la GUI los convierte), así un valor nuevo
 *     del lado de C nunca rompe al lado de Rust.
 *   - Los const char* devueltos pertenecen al core; la GUI los copia si los
 *     necesita más allá de la llamada siguiente.
 */
#ifndef CAPYRO_CORE_H
#define CAPYRO_CORE_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define CAPYRO_VERSION "0.2.0-alpha"

typedef struct CapyroCore CapyroCore;

typedef enum {
    CAPYRO_STATE_IDLE    = 0, /* nada cargado */
    CAPYRO_STATE_PLAYING = 1,
    CAPYRO_STATE_PAUSED  = 2,
    CAPYRO_STATE_STOPPED = 3  /* archivo cargado, posición en 0 */
} CapyroState;

typedef enum {
    CAPYRO_MEDIA_NONE  = 0,
    CAPYRO_MEDIA_VIDEO = 1, /* .mp4 */
    CAPYRO_MEDIA_AUDIO = 2  /* .mp3 */
} CapyroMediaKind;

typedef enum {
    CAPYRO_OK               = 0,
    CAPYRO_ERR_INVALID_ARG  = 1,
    CAPYRO_ERR_UNSUPPORTED  = 2, /* extensión distinta de mp4/mp3 */
    CAPYRO_ERR_NOT_FOUND    = 3, /* no se pudo abrir el archivo */
    CAPYRO_ERR_NO_MEMORY    = 4,
    CAPYRO_ERR_OUT_OF_RANGE = 5, /* índice de lista inválido / fin de lista */
    CAPYRO_ERR_DECODE       = 6, /* FFmpeg no pudo leer el archivo */
    CAPYRO_ERR_NO_STREAMS   = 7, /* el archivo no tiene audio ni video */
    CAPYRO_ERR_THREAD       = 8  /* no se pudo crear el hilo de decodificación */
} CapyroError;

/* Último frame de video listo para mostrar (RGBA8, sin premultiplicar). */
typedef struct {
    const uint8_t *rgba;
    int32_t        width;
    int32_t        height;
    int32_t        stride;   /* bytes por fila */
    float          aspect;   /* relación de aspecto de pantalla (ancho/alto) */
    uint64_t       serial;   /* cambia cada vez que hay un frame nuevo */
} CapyroVideoFrame;

/* ---------- ciclo de vida ---------- */
CapyroCore *capyro_core_init(void);
void        capyro_core_shutdown(CapyroCore *core);
const char *capyro_core_version(void);
const char *capyro_core_error_string(int err);

/* Llamar seguido (la GUI lo hace en cada frame): detecta el fin del archivo
 * y pasa al siguiente de la lista si auto_next está activo. */
void capyro_core_update(CapyroCore *core);
void capyro_core_set_auto_next(CapyroCore *core, bool enabled);

/* Nombre del backend de audio ("PulseAudio", "ALSA", "WASAPI"...) o "" si
 * no hay dispositivo: en ese caso Capyro sigue andando con reloj de pared. */
const char *capyro_core_audio_backend(CapyroCore *core);

/* ---------- abrir / formato ---------- */
/* Agrega el archivo a la lista y lo deja cargado (estado STOPPED). */
int  capyro_core_open(CapyroCore *core, const char *path);
/* 0 = no soportado, si no un CapyroMediaKind. No toca el disco. */
int  capyro_core_probe_extension(const char *path);

/* ---------- transporte ---------- */
void capyro_core_play(CapyroCore *core);
void capyro_core_pause(CapyroCore *core);
void capyro_core_toggle(CapyroCore *core);
void capyro_core_stop(CapyroCore *core);
void capyro_core_seek(CapyroCore *core, double seconds);
void capyro_core_seek_relative(CapyroCore *core, double delta);

/* ---------- volumen ---------- */
void  capyro_core_set_volume(CapyroCore *core, float volume); /* 0.0 .. 1.0 */
float capyro_core_get_volume(CapyroCore *core);
void  capyro_core_set_muted(CapyroCore *core, bool muted);
bool  capyro_core_get_muted(CapyroCore *core);

/* ---------- estado (la GUI hace polling cada frame) ---------- */
int         capyro_core_get_state(CapyroCore *core);      /* CapyroState */
int         capyro_core_get_media_kind(CapyroCore *core); /* CapyroMediaKind */
double      capyro_core_get_position(CapyroCore *core);   /* segundos */
double      capyro_core_get_duration(CapyroCore *core);   /* segundos, 0 = desconocida */
const char *capyro_core_get_current_file(CapyroCore *core); /* "" si nada */
const char *capyro_core_get_title(CapyroCore *core);  /* metadato o "" */
const char *capyro_core_get_artist(CapyroCore *core); /* metadato o "" */

/* ---------- video ----------
 * Si devuelve true, `out` apunta a memoria del núcleo y el núcleo queda
 * bloqueado hasta capyro_core_unlock_video_frame(): copiá rápido y soltá.
 * Si devuelve false no hay frame (y no hay que llamar a unlock).
 * En un .mp3 con carátula, el frame es la carátula. */
bool capyro_core_lock_video_frame(CapyroCore *core, CapyroVideoFrame *out);
void capyro_core_unlock_video_frame(CapyroCore *core);

/* ---------- lista de reproducción ---------- */
int         capyro_core_playlist_add(CapyroCore *core, const char *path);
size_t      capyro_core_playlist_count(CapyroCore *core);
const char *capyro_core_playlist_get(CapyroCore *core, size_t index); /* NULL si fuera de rango */
long        capyro_core_playlist_current(CapyroCore *core);           /* -1 si ninguno */
int         capyro_core_playlist_select(CapyroCore *core, size_t index);
int         capyro_core_playlist_next(CapyroCore *core);
int         capyro_core_playlist_prev(CapyroCore *core);
void        capyro_core_playlist_remove(CapyroCore *core, size_t index);
void        capyro_core_playlist_clear(CapyroCore *core);

#ifdef __cplusplus
}
#endif

#endif /* CAPYRO_CORE_H */

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

#define CAPYRO_VERSION "0.2.9-alpha"

typedef struct CapyroCore CapyroCore;

typedef enum {
    CAPYRO_STATE_IDLE    = 0, /* nada cargado */
    CAPYRO_STATE_PLAYING = 1,
    CAPYRO_STATE_PAUSED  = 2,
    CAPYRO_STATE_STOPPED = 3  /* archivo cargado, posición en 0 */
} CapyroState;

typedef enum {
    CAPYRO_MEDIA_NONE  = 0,
    CAPYRO_MEDIA_VIDEO = 1, /* .mp4, .mov, .avi */
    CAPYRO_MEDIA_AUDIO = 2  /* .mp3, .wav, .ogg, .aac */
} CapyroMediaKind;

typedef enum {
    CAPYRO_OK               = 0,
    CAPYRO_ERR_INVALID_ARG  = 1,
    CAPYRO_ERR_UNSUPPORTED  = 2, /* extensión que Capyro no abre (ver supported_*) */
    CAPYRO_ERR_NOT_FOUND    = 3, /* no se pudo abrir el archivo */
    CAPYRO_ERR_NO_MEMORY    = 4,
    CAPYRO_ERR_OUT_OF_RANGE = 5, /* índice de lista inválido / fin de lista */
    CAPYRO_ERR_DECODE       = 6, /* FFmpeg no pudo leer el archivo */
    CAPYRO_ERR_NO_STREAMS   = 7, /* el archivo no tiene audio ni video */
    CAPYRO_ERR_THREAD       = 8  /* no se pudo crear el hilo de decodificación */
} CapyroError;

typedef enum {
    CAPYRO_REPEAT_OFF = 0, /* al terminar la lista, para */
    CAPYRO_REPEAT_ALL = 1, /* al terminar la lista, vuelve a empezar */
    CAPYRO_REPEAT_ONE = 2  /* repite el archivo actual */
} CapyroRepeat;

#define CAPYRO_SPEED_MIN 0.5
#define CAPYRO_SPEED_MAX 2.0

/* Efectos de audio (ecualizador, estéreo, modo noche). Se aplican en tiempo
 * real sobre lo que está sonando: los cambios se escuchan al instante. */
#define CAPYRO_EQ_MIN_DB  (-12.0f)
#define CAPYRO_EQ_MAX_DB  ( 12.0f)
#define CAPYRO_WIDTH_MAX  ( 2.0f)
#define CAPYRO_PITCH_MIN  (-12.0)   /* semitonos */
#define CAPYRO_PITCH_MAX  ( 12.0)

typedef struct {
    bool  enabled;    /* interruptor general (false = todo apagado, tono incluido) */
    float bass_db;    /* graves  (low shelf 120 Hz),  -12 .. +12 dB */
    float mid_db;     /* medios  (peaking 1,5 kHz),   -12 .. +12 dB */
    float treble_db;  /* agudos  (high shelf 6 kHz),  -12 .. +12 dB */
    float width;      /* estéreo: 0 = mono, 1 = normal, 2 = muy abierto */
    bool  night;      /* modo noche: compresor (lo fuerte baja, lo bajito sube) */
} CapyroAudioFx;

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
/* Frecuencia de muestreo del dispositivo (0 si no hay audio). */
unsigned    capyro_core_audio_rate(CapyroCore *core);
/* Copia en `out` las últimas `n` muestras que sonaron (mono, antes del
 * volumen, de la más vieja a la más nueva; máximo 4096). Devuelve cuántas
 * copió. Es lo que usa la GUI para dibujar el visualizador. */
size_t      capyro_core_get_scope(CapyroCore *core, float *out, size_t n);

/* ---------- abrir / formato ---------- */
/* Agrega el archivo a la lista y lo deja cargado (estado STOPPED). */
int  capyro_core_open(CapyroCore *core, const char *path);
/* 0 = no soportado, si no un CapyroMediaKind según la extensión (el tipo
 * real se decide al abrir: un .ogg puede traer video). No toca el disco. */
int  capyro_core_probe_extension(const char *path);
/* Extensiones soportadas, en minúscula y separadas por comas, sin punto.
 * Video: "mp4,mov,avi"   Audio: "mp3,wav,ogg,aac". Son la única fuente de verdad:
 * la GUI arma con esto el filtro del selector y el menú de información. */
const char *capyro_core_supported_video(void);
const char *capyro_core_supported_audio(void);

/* ---------- transporte ---------- */
void capyro_core_play(CapyroCore *core);
void capyro_core_pause(CapyroCore *core);
void capyro_core_toggle(CapyroCore *core);
void capyro_core_stop(CapyroCore *core);
void capyro_core_seek(CapyroCore *core, double seconds);
void capyro_core_seek_relative(CapyroCore *core, double delta);

/* Velocidad de reproducción (0.5 .. 2.0). Cambia el tempo sin cambiar el
 * tono (filtro atempo de FFmpeg). */
void   capyro_core_set_speed(CapyroCore *core, double speed);
double capyro_core_get_speed(CapyroCore *core);

/* ---------- efectos de audio ---------- */
void capyro_core_set_fx(CapyroCore *core, const CapyroAudioFx *fx);
void capyro_core_get_fx(CapyroCore *core, CapyroAudioFx *out);
/* Tono en semitonos (-12 .. +12) sin cambiar la velocidad (asetrate +
 * atempo de FFmpeg). A diferencia del ecualizador tarda un instante en
 * aplicarse: se vuelve a decodificar desde la posición actual. */
void   capyro_core_set_pitch(CapyroCore *core, double semitones);
double capyro_core_get_pitch(CapyroCore *core);

/* ---------- decodificación de video: GPU o CPU ----------
 * AUTO: usa la GPU si se puede (VA-API, NVDEC/CUDA o VDPAU) y si no, la CPU.
 * GPU:  igual que AUTO, pero si la GPU no puede, la nota lo explica (la GUI avisa).
 * CPU:  siempre por software.
 * Cambiarlo con un video abierto rearma el decodificador y sigue donde estaba.
 * Si la GPU falla a mitad del video, se pasa sola a la CPU. */
typedef enum {
    CAPYRO_HWDEC_AUTO = 0,
    CAPYRO_HWDEC_GPU  = 1,
    CAPYRO_HWDEC_CPU  = 2
} CapyroHwdec;

typedef struct CapyroDecodeInfo {
    bool     active;     /* hay un video abierto */
    bool     hw;         /* lo está decodificando la GPU */
    char     api[16];    /* "VA-API", "NVDEC", "VDPAU" o "CPU" */
    char     codec[24];  /* "H.264", "H.265 (HEVC)", "AV1"... */
    char     note[128];  /* por qué no se usa la GPU ("" si se usa) */
    uint64_t serial;     /* cambia cada vez que se arma un decodificador */
} CapyroDecodeInfo;

void capyro_core_set_hwdec(CapyroCore *core, int mode); /* CapyroHwdec */
int  capyro_core_get_hwdec(CapyroCore *core);
void capyro_core_get_decode_info(CapyroCore *core, CapyroDecodeInfo *out);

/* ---------- datos del build ----------
 * De qué commit de git sale este binario y dónde se instala. Los usa el
 * actualizador. Si se compiló sin git (desde un .zip), commit es "". */
typedef struct CapyroBuildInfo {
    const char *commit;     /* hash completo o "" */
    const char *branch;     /* rama al compilar o "" */
    const char *remote;     /* URL del "origin" o "" */
    const char *subject;    /* título del commit */
    long long   commit_time;/* fecha del commit (unix), 0 = desconocida */
    bool        dirty;      /* había cambios sin commitear */
    const char *prefix;     /* CMAKE_INSTALL_PREFIX */
    const char *bindir;     /* carpeta donde se instala el ejecutable */
    const char *build_type; /* Release, Debug... */
    bool        dist;       /* -DCAPYRO_DIST=ON */
    bool        native;     /* -DCAPYRO_NATIVE=ON */
} CapyroBuildInfo;

const CapyroBuildInfo *capyro_core_build_info(void);

/* ---------- efectos de sonido ----------
 * Sonidos cortos (logo, error...) que se mezclan encima de lo que esté
 * sonando. `data` es el archivo entero en memoria (mp3, wav, ogg...): se
 * decodifica al cargar. Slots 0..15. Suenan con el volumen y el silencio
 * generales. Si no hay dispositivo de audio no hacen nada. */
int  capyro_core_sfx_load(CapyroCore *core, int slot, const uint8_t *data, size_t size);
void capyro_core_sfx_play(CapyroCore *core, int slot);

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
 * En un audio con carátula (mp3, ogg...), el frame es la carátula. */
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

/* Repetir (CapyroRepeat) y aleatorio. Con aleatorio, siguiente/anterior
 * siguen un orden mezclado que no repite archivos hasta recorrer la lista. */
void capyro_core_set_repeat(CapyroCore *core, int mode);
int  capyro_core_get_repeat(CapyroCore *core);
void capyro_core_set_shuffle(CapyroCore *core, bool enabled);
bool capyro_core_get_shuffle(CapyroCore *core);

#ifdef __cplusplus
}
#endif

#endif /* CAPYRO_CORE_H */

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

#define CAPYRO_VERSION "0.4.0-alpha"

typedef struct CapyroCore CapyroCore;

typedef enum {
    CAPYRO_STATE_IDLE    = 0, /* nada cargado */
    CAPYRO_STATE_PLAYING = 1,
    CAPYRO_STATE_PAUSED  = 2,
    CAPYRO_STATE_STOPPED = 3  /* archivo cargado, posición en 0 */
} CapyroState;

typedef enum {
    CAPYRO_MEDIA_NONE  = 0,
    CAPYRO_MEDIA_VIDEO = 1, /* .mp4, .mov, .avi, .mkv, .webm */
    CAPYRO_MEDIA_AUDIO = 2  /* .mp3, .wav, .ogg, .aac, .flac, .opus, .m4a, .aiff */
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
    CAPYRO_ERR_THREAD       = 8, /* no se pudo crear el hilo de decodificación */
    CAPYRO_ERR_WRITE        = 9, /* no se pudo escribir (etiquetas o conversión: ¿disco lleno, archivo en uso?) */
    CAPYRO_ERR_CANCELLED    = 10, /* se canceló (Capyro Converter) */
    CAPYRO_ERR_ENCODER      = 11  /* no hay con qué codificar a ese formato (Capyro Converter) */
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
#define CAPYRO_EQ15_BANDS 15
#define CAPYRO_PITCH_MIN  (-12.0)   /* semitonos */
#define CAPYRO_PITCH_MAX  ( 12.0)

typedef struct {
    bool  enabled;    /* interruptor general (false = todo apagado, tono incluido) */
    float bass_db;    /* graves  (low shelf 120 Hz),  -12 .. +12 dB */
    float mid_db;     /* medios  (peaking 1,5 kHz),   -12 .. +12 dB */
    float treble_db;  /* agudos  (high shelf 6 kHz),  -12 .. +12 dB */
    float width;      /* estéreo: 0 = mono, 1 = normal, 2 = muy abierto */
    bool  night;      /* modo noche: compresor (lo fuerte baja, lo bajito sube) */
    /* ecualizador avanzado de 15 bandas (25 Hz a 16 kHz), aparte de los tres
     * controles de arriba. Ganancia de cada banda en dB, -12 .. +12. */
    bool  eq15_on;
    float eq15[CAPYRO_EQ15_BANDS];
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
 * Video: "mp4,mov,avi,mkv,webm"   Audio: "mp3,wav,ogg,aac,flac,opus,m4a,aiff".
 * Son la única fuente de verdad: la GUI arma con esto el filtro del selector
 * y el menú de información. (También se aceptan .aif y .aifc, que son AIFF.) */
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
 * AUTO: usa la GPU si se puede (Linux: VA-API, NVDEC/CUDA o VDPAU; Windows:
 * D3D11VA, NVDEC o DXVA2; macOS: VideoToolbox) y si no, la CPU.
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
    char     api[16];    /* "VA-API", "NVDEC", "VDPAU", "D3D11VA", "VideoToolbox" o "CPU" */
    char     codec[24];  /* "H.264", "H.265 (HEVC)", "AV1"... */
    char     note[128];  /* por qué no se usa la GPU ("" si se usa) */
    uint64_t serial;     /* cambia cada vez que se arma un decodificador */
} CapyroDecodeInfo;

void capyro_core_set_hwdec(CapyroCore *core, int mode); /* CapyroHwdec */

/* Segundo plano (sin ventana): false = el video no se decodifica, suena solo
 * el audio. Al volver a true salta a donde va (el video sigue en sincronía). */
void capyro_core_set_video_enabled(CapyroCore *core, bool enabled);
int  capyro_core_get_hwdec(CapyroCore *core);
void capyro_core_get_decode_info(CapyroCore *core, CapyroDecodeInfo *out);

/* ---------- sistema operativo ----------
 * En qué sistema corre Capyro: Linux, Windows (10 u 11) o macOS. */
enum {
    CAPYRO_OS_LINUX   = 1,
    CAPYRO_OS_WINDOWS = 2,
    CAPYRO_OS_MACOS   = 3
};

typedef struct CapyroOsInfo {
    int  family;       /* CAPYRO_OS_LINUX, CAPYRO_OS_WINDOWS o CAPYRO_OS_MACOS */
    int  major;        /* Windows: 10 u 11; macOS: 11, 12... 15, 26; Linux: 0 */
    char code[4];      /* "Lin", "Win" o "Mac" (como en capyro.lua) */
    char name[64];     /* "Windows 11", "Gentoo Linux", "macOS 15 Sequoia"... */
    char version[64];  /* "23H2 · build 22631", "kernel 6.18.0", "15.5 · Apple Silicon"... */
} CapyroOsInfo;

void capyro_core_os_info(CapyroOsInfo *out);

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
    const char *system;     /* para qué sistema se compiló: "Lin" o "Win" */
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
const char *capyro_core_get_album(CapyroCore *core);  /* metadato o "" */
/* Cambia cada vez que cambia el archivo cargado o la lista de reproducción:
 * la GUI guarda lo que pidió y solo lo vuelve a pedir cuando esto cambia. */
uint64_t    capyro_core_get_change_serial(CapyroCore *core);

/* ---------- video ----------
 * Si devuelve true, `out` apunta a memoria del núcleo y el núcleo queda
 * bloqueado hasta capyro_core_unlock_video_frame(): copiá rápido y soltá.
 * Si devuelve false no hay frame (y no hay que llamar a unlock).
 * En un audio con carátula (mp3, ogg...), el frame es la carátula. */
bool capyro_core_lock_video_frame(CapyroCore *core, CapyroVideoFrame *out);
void capyro_core_unlock_video_frame(CapyroCore *core);

/* Tamaño en pantalla del área de video, en píxeles reales. Los cuadros se
 * convierten a ese tamaño (nunca más grandes que el original ni que
 * 1920×1080): en una ventana chica se ahorra casi todo el trabajo de
 * convertir y subir cada cuadro. 0×0 = sin dato. */
void capyro_core_set_video_target(CapyroCore *core, int width, int height);

/* El video original abierto: su resolución (la del archivo, no la que se ve
 * en la ventana) y sus cuadros por segundo (0 si el archivo no lo dice).
 * false = no hay video (nada abierto, o un audio). */
typedef struct {
    int32_t width;
    int32_t height;
    double  fps;
} CapyroVideoProps;
bool capyro_core_get_video_props(CapyroCore *core, CapyroVideoProps *out);

/* Segundos hasta que toque mostrar el próximo cuadro de video (para no
 * redibujar más seguido que el video), o -1 si no hay ninguno en cola. */
double capyro_core_video_next_due(CapyroCore *core);

/* Imagen suelta en RGBA8 (la libera capyro_core_free_image). */
typedef struct {
    uint8_t *rgba;
    int32_t  width;
    int32_t  height;
} CapyroImage;

/* El cuadro visible en su tamaño original (hasta 8192 de lado: 8K), para las
 * capturas: no depende del tamaño de la ventana. false = no hay cuadro. */
bool capyro_core_capture_frame(CapyroCore *core, CapyroImage *out);
void capyro_core_free_image(CapyroImage *img);

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


/* ---- etiquetas (Capyro Studio) -----------------------------------------
 * Leer y escribir los metadatos de un archivo de audio. Escribir reescribe
 * el archivo SIN recodificar el audio (copia los paquetes) y deja el
 * original como "<archivo>.bak". Hay que tenerlo cerrado: no se puede
 * reemplazar un archivo que Capyro está reproduciendo (en Windows sobre
 * todo). Los campos vacíos se borran. */
typedef struct {
    char title[256];
    char artist[256];
    char album[256];
    char album_artist[256];
    char genre[128];
    char year[16];
    char track[32];    /* "3" o "3/12" */
    char disc[32];     /* "1" o "1/2" */
    char composer[256];
    char comment[1024];
} CapyroTags;

/* Suelta el archivo que está abierto (sin quitarlo de la lista), para que
 * se pueda reescribir. Después, capyro_core_playlist_select lo vuelve a abrir. */
void capyro_core_release_file(CapyroCore *core);

int capyro_tags_read(const char *path, CapyroTags *out);
int capyro_tags_write(const char *path, const CapyroTags *tags);

/* Datos técnicos del audio de un archivo (vista de música). */
typedef struct {
    char    codec[32];    /* "MP3", "FLAC", "AAC", "OPUS"... */
    int32_t sample_rate;  /* Hz */
    int32_t channels;
    int64_t bit_rate;     /* bits por segundo (0 = no se sabe) */
    int32_t bits;         /* bits por muestra (FLAC, WAV); 0 si no aplica */
} CapyroAudioInfo;

int capyro_audio_info(const char *path, CapyroAudioInfo *out);

/* ---- portada (Capyro Studio) --------------------------------------------
 * Al guardar las etiquetas la portada se deja como está, se cambia por una
 * imagen (jpg, png, webp, bmp...) o se quita. La imagen nueva se guarda
 * como JPEG de 1200 px como mucho (si ya es un JPG o PNG chico, tal cual).
 * Se puede en MP3, FLAC, M4A/MP4 y OGG/Opus; en WAV no. */
typedef enum {
    CAPYRO_COVER_KEEP = 0,
    CAPYRO_COVER_SET = 1,
    CAPYRO_COVER_REMOVE = 2,
} CapyroCoverMode;

/* Como capyro_tags_write, y además la portada (`cover_image` solo se usa con
 * CAPYRO_COVER_SET). */
int capyro_tags_write_ex(const char *path, const CapyroTags *tags, int cover_mode, const char *cover_image);

/* ¿El formato de este archivo puede guardar una portada? */
bool capyro_cover_supported(const char *path);

/* La portada del archivo en RGBA (CapyroImage, se libera con
 * capyro_core_free_image), achicada para que su lado más largo no pase de
 * `max_side` (nunca se agranda). CAPYRO_ERR_NOT_FOUND si no tiene. */
int capyro_cover_read(const char *path, int max_side, CapyroImage *out);

/* Una imagen cualquiera (para mostrar la portada nueva antes de guardar). */
int capyro_image_read(const char *path, int max_side, CapyroImage *out);


/* ---- Capyro Converter ---------------------------------------------------
 * Pasa un archivo a otro formato: video a audio, audio a video (con la
 * portada de fondo), cambiar de códec, achicar, recortar, emparejar el
 * volumen. El formato de salida sale de la extensión de `out_path`
 * (.mp4, .mkv, .webm, .mov, .mp3, .m4a, .flac, .ogg, .opus, .wav).
 * Corre en el hilo que la llama (la GUI la llama desde otro hilo). */

/* Qué tiene un archivo (para que la GUI ofrezca lo que se puede). */
typedef struct {
    bool    has_video;        /* video de verdad (no una portada) */
    bool    has_audio;
    bool    has_cover;        /* tiene portada */
    double  duration;         /* segundos (0 = no se sabe) */
    int32_t width, height;    /* del video (ya girado, como se ve) */
    double  fps;
    char    video_codec[32];  /* "h264", "hevc"... ("" si no hay) */
    char    audio_codec[32];  /* "aac", "mp3", "opus"... */
    int32_t sample_rate;
    int32_t channels;
    int32_t bits;             /* bits por muestra (FLAC, WAV); 0 si no aplica */
    int64_t bit_rate;         /* del archivo entero, bits por segundo */
    char    audio_copy_ext[8];/* extensión para sacar el audio sin recodificar ("m4a", "mp3", "opus", "flac", "mka"...) */
} CapyroMediaProbe;

int capyro_media_probe(const char *path, CapyroMediaProbe *out);

typedef struct {
    /* "" = sin video / sin audio; "copy" = tal cual (sin recodificar); o el
     * códec: "h264", "hevc", "vp9", "av1", "mpeg4" / "aac", "mp3", "opus",
     * "vorbis", "flac", "alac", "pcm". */
    char    video_codec[16];
    char    audio_codec[16];
    int32_t quality;          /* 0..100 (100 = la mejor): calidad del video */
    int32_t max_height;       /* lado corto máximo (480, 720, 1080...); 0 = el original */
    int32_t fps;              /* 0 = los del original */
    int32_t speed;            /* 0 rápido, 1 normal, 2 lento (archivo más chico) */
    int32_t audio_kbps;       /* 0 = lo normal de cada códec */
    int32_t sample_rate;      /* 0 = la del original */
    int32_t channels;         /* 0 = los del original; 1 mono, 2 estéreo */
    bool    normalize;        /* emparejar el volumen (EBU R128) */
    bool    gpu;              /* probar primero el codificador de la placa de video */
    bool    compat;           /* máxima compatibilidad (H.264 High 4.1, 8 bits, 4:2:0) */
    bool    keep_tags;        /* etiquetas, capítulos y portada (si el formato la guarda) */
    double  start;            /* recortar: desde (segundos) */
    double  end;              /* hasta (<= 0: hasta el final) */
    char    cover_image[1024];/* audio → video: la imagen ("" = la portada del archivo) */
} CapyroConvertOptions;

/* Se llama seguido con cuánto va (0..1). Devolver distinto de 0 cancela. */
typedef int (*CapyroProgressFn)(void *user, double fraction);

/* Qué se usó y, si salió mal, por qué (en castellano). */
typedef struct {
    char video_encoder[32];   /* "libx264", "h264_nvenc", "copiar"... ("" sin video) */
    char audio_encoder[32];
    char message[256];
} CapyroConvertResult;

int capyro_convert(const char *in_path, const char *out_path, const CapyroConvertOptions *opt,
                   CapyroProgressFn progress, void *user, CapyroConvertResult *result);

/* ¿Hay con qué codificar a `codec` ("h264", "mp3"...)? Con `gpu`, solo los
 * de la placa de video (que estén en FFmpeg no quiere decir que la placa
 * los tenga: eso se sabe al convertir). */
bool capyro_convert_has_encoder(const char *codec, bool gpu);

#ifdef __cplusplus
}
#endif

#endif /* CAPYRO_CORE_H */

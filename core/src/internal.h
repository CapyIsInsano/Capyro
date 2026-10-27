/* Capyro — estructuras internas del núcleo (no se exponen a la GUI). */
#ifndef CAPYRO_INTERNAL_H
#define CAPYRO_INTERNAL_H

#include "capyro_core.h"
#include "dsp.h"
#include "platform.h"

#include <stdbool.h>
#include <stdint.h>

struct AVFormatContext;
struct AVCodecContext;
struct AVFrame;
struct AVPacket;
struct AVFilterGraph;
struct AVFilterContext;
struct SwsContext;
struct ma_device;

#define VQ_MAX   3              /* frames decodificados por delante */
#define CAPYRO_SFX_SLOTS 16     /* efectos de sonido cargados a la vez */
#define CAPYRO_SCOPE_LEN 4096   /* muestras guardadas para el visualizador */
#define VQ_SLOTS (VQ_MAX + 2)   /* + el que se está mostrando + uno de repuesto */
#define HWDEV_COUNT 3           /* Linux: VA-API, NVDEC, VDPAU · Windows: D3D11VA, NVDEC, DXVA2 */
struct AVBufferRef;
struct AVStream;

struct AVFrame;

typedef struct {
    uint8_t *data;
    size_t   cap;
    int      w, h;
    double   pts;
    float    aspect;
    /* el cuadro decodificado original (para capturas en tamaño completo) */
    struct AVFrame *src;
} VideoSlot;

/* Archivo en memoria leído por FFmpeg (sfx.c). */
typedef struct {
    const uint8_t *data;
    size_t         size;
    size_t         pos;
} MemReader;
int     mem_read(void *opaque, uint8_t *buf, int n);
int64_t mem_seek(void *opaque, int64_t offset, int whence);

typedef struct PktNode {
    struct AVPacket *pkt;
    struct PktNode  *next;
} PktNode;

typedef struct {
    PktNode *head, *tail;
    size_t   bytes;
    int      count;
} PktQueue;

/* Todo lo de acá lo toca SOLO el hilo de decodificación mientras corre
 * (y el hilo de la GUI cuando el de decodificación está parado). */
typedef struct {
    struct AVFormatContext *fmt;
    struct AVCodecContext  *actx, *vctx;
    int                     aidx, vidx;  /* -1 = no se decodifica */
    struct AVFilterGraph   *agraph;   /* atempo + aresample + aformat */
    struct AVFilterContext *asrc, *asink;
    struct AVFrame         *afilt;
    bool                    af_flushed;
    double                  speed;    /* velocidad con la que se armó agraph */
    double                  pitch;    /* y el tono (semitonos) */
    struct SwsContext      *sws;
    struct AVFrame         *frame;
    /* decodificación por GPU: el frame llega en memoria de la GPU (hw_fmt) y
     * se copia a swframe antes de convertirlo a RGBA */
    bool                    hw;
    int                     hw_fmt;      /* enum AVPixelFormat, -1 = ninguno */
    bool                    hw_sw_noted; /* la GPU rechazó el video y siguió la CPU */
    struct AVFrame         *swframe;
    struct AVPacket        *pkt;
    PktQueue                aq, vq;
    bool                    demux_eof, a_flushed, v_flushed, a_done, v_done;
    double                  start_time;
    double                  a_drop_until, v_drop_until; /* < 0 = no descartar */
    uint64_t                seek_serial_done;
    cp_thread               thread;
    bool                    thread_running;
} Player;

struct CapyroCore {
    cp_mutex lock; /* protege todo lo de abajo salvo `player` y la lista */

    CapyroState     state;
    CapyroMediaKind kind;
    double          duration;
    bool            auto_next;

    /* reloj: si clock_audio, posición = clock_base + audio_consumed / out_rate */
    bool     clock_audio;
    double   clock_base;
    double   wall_started;
    uint64_t audio_consumed;
    double   speed;
    double   pitch;         /* semitonos pedidos por el usuario */

    /* efectos de audio en tiempo real (se aplican en el callback) */
    Dsp      dsp;

    /* audio */
    struct ma_device *device;
    bool              audio_ok;
    bool              device_started;
    unsigned          out_rate;
    char              backend[32];
    float            *ring;      /* estéreo intercalado */
    size_t            ring_cap;  /* en frames */
    size_t            ring_read;
    size_t            ring_len;
    float             volume;
    bool              muted;

    /* efectos de sonido: PCM float estéreo a out_rate, ya decodificados */
    float            *sfx[CAPYRO_SFX_SLOTS];
    size_t            sfx_len[CAPYRO_SFX_SLOTS]; /* en frames */
    int               sfx_playing;               /* -1 = ninguno */
    size_t            sfx_pos;

    /* "osciloscopio": últimas muestras que sonaron (mono, antes del volumen)
     * para el visualizador de la GUI */
    float             scope[CAPYRO_SCOPE_LEN];
    size_t            scope_pos;

    /* video */
    VideoSlot slots[VQ_SLOTS];
    int       vq[VQ_SLOTS];
    int       vq_head, vq_len;
    int       shown;        /* slot visible, -1 = ninguno */
    uint64_t  serial;
    bool      force_frame;  /* mostrar el próximo frame aunque esté en pausa */
    /* tamaño en pantalla del área de video (píxeles reales): los cuadros se
     * convierten a ese tamaño y no más (0 = sin dato, hasta 1920×1080) */
    int       video_target_w, video_target_h;
    /* el video original: resolución y cuadros por segundo (0 = no hay) */
    int       video_w, video_h;
    double    video_fps;

    /* control del hilo */
    bool     quit;
    bool     seek_req;
    double   seek_target;
    uint64_t seek_serial;
    bool     media_eof;     /* el hilo terminó de decodificar todo */

    char title[256];
    char artist[256];

    /* decodificación de video (hwdec.c) */
    int              hwdec_mode;    /* CapyroHwdec */
    bool             hwdec_reinit;  /* la GUI cambió el modo: rearmar el decodificador */
    CapyroDecodeInfo decinfo;
    /* dispositivos de GPU ya abiertos (se reusan entre archivos). Los toca
     * el hilo de la GUI al abrir y el de decodificación al rearmar, nunca a
     * la vez (el de decodificación no existe mientras se abre un archivo). */
    struct AVBufferRef *hwdev[HWDEV_COUNT];
    bool                hwdev_tried[HWDEV_COUNT];

    Player player;

    /* lista de reproducción (solo el hilo de la GUI) */
    char  **playlist;
    size_t  playlist_len;
    size_t  playlist_cap;
    long    playlist_current;
    size_t *order;          /* orden de reproducción (mezclado si shuffle) */
    int     repeat;
    bool    shuffle;
    uint64_t rng;
};

/* player.c */
int    player_global_init(CapyroCore *core);
void   player_global_shutdown(CapyroCore *core);
int    player_open(CapyroCore *core, const char *path);
void   player_close(CapyroCore *core);
double player_clock_locked(CapyroCore *core);
bool   player_ended_locked(CapyroCore *core);
void   player_request_seek_locked(CapyroCore *core, double target);
void   player_ensure_device_started(CapyroCore *core); /* sin el lock tomado */
double player_effective_pitch_locked(const CapyroCore *core);
/* hwdec.c */
struct AVCodecContext *hwdec_open(CapyroCore *core, const struct AVStream *st, int *hw_fmt,
                                  char *api, size_t api_cap, char *why, size_t why_cap);
void   hwdec_codec_name(int codec_id, char *out, size_t cap);
void   hwdec_free_devices(CapyroCore *core);
bool   audio_graph_create(const struct AVFrame *f, double speed, double pitch, unsigned out_rate,
                          struct AVFilterGraph **graph, struct AVFilterContext **src,
                          struct AVFilterContext **sink);

#endif /* CAPYRO_INTERNAL_H */

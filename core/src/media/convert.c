/* Capyro Converter: pasa un archivo a otro formato con las librerías de
 * FFmpeg (sin el programa ffmpeg).
 *
 *   entrada ─▶ demuxer ─▶ decodificador ─▶ filtros ─▶ codificador ─▶ muxer ─▶ salida
 *                     └──────────────── (copiar tal cual) ───────────────┘
 *
 * Por pista (una de video y una de audio):
 *   - "copy": los paquetes pasan tal cual (rápido y sin pérdida);
 *   - si no, se decodifica, se filtra (girar, achicar, cuadros por segundo,
 *     recortar, emparejar el volumen, remuestrear) y se vuelve a codificar.
 * Audio → video: el video sale de una imagen (la portada, o la que se elija)
 * sobre un fondo con la misma imagen desenfocada, repetida a 5 cuadros por
 * segundo hasta que termina el audio.
 *
 * Los codificadores se prueban en orden: primero los de la placa de video
 * (si se pidió), después los de la CPU. FFmpeg puede traer el de NVIDIA y la
 * PC no tener la placa: eso se sabe recién al abrirlo, y se pasa al
 * siguiente. En Windows el FFmpeg de Capyro (LGPL) no trae x264: ahí H.264
 * sale de OpenH264 o de Media Foundation. */

#include "capyro_core.h"
#include "media/cover.h"

#include <libavcodec/avcodec.h>
#include <libavfilter/avfilter.h>
#include <libavfilter/buffersink.h>
#include <libavfilter/buffersrc.h>
#include <libavformat/avformat.h>
#include <libavutil/avstring.h>
#include <libavutil/channel_layout.h>
#include <libavutil/display.h>
#include <libavutil/mem.h>
#include <libavutil/opt.h>
#include <libavutil/pixdesc.h>
#include <libavutil/time.h>

#include <math.h>
#include <stdarg.h>
#include <stdio.h>
#include <string.h>

/* audio → video: cuadros por segundo de la imagen fija */
#define COVER_FPS 5

/* ------------------------------------------------------------------ */
/* codificadores                                                       */
/* ------------------------------------------------------------------ */

/* En la Mac, H.265 sale de VideoToolbox (el de macOS) aunque no se pida la
 * placa de video: el FFmpeg de Capyro para Mac no trae x265, y VideoToolbox
 * tiene uno por software para las Mac sin H.265 en el chip. */
#ifdef __APPLE__
#  define MAC_HEVC "hevc_videotoolbox",
#else
#  define MAC_HEVC
#endif

typedef struct {
    const char *codec;
    const char *gpu[5];
    const char *cpu[5];
} EncChain;

static const EncChain CHAINS[] = {
    { "h264", { "h264_nvenc", "h264_qsv", "h264_amf", "h264_videotoolbox", NULL },
              { "libx264", "libopenh264", "h264_mf", NULL } },
    { "hevc", { "hevc_nvenc", "hevc_qsv", "hevc_amf", "hevc_videotoolbox", NULL }, { "libx265", "hevc_mf", MAC_HEVC NULL } },
    { "av1", { "av1_nvenc", "av1_qsv", "av1_amf", NULL }, { "libsvtav1", "libaom-av1", "librav1e", NULL } },
    { "vp9", { "vp9_qsv", NULL }, { "libvpx-vp9", NULL } },
    { "mpeg4", { NULL }, { "mpeg4", NULL } },
    { "aac", { NULL }, { "libfdk_aac", "aac", NULL } },
    { "mp3", { NULL }, { "libmp3lame", "mp3_mf", "libshine", NULL } },
    { "opus", { NULL }, { "libopus", "opus", NULL } },
    { "vorbis", { NULL }, { "libvorbis", "vorbis", NULL } },
    { "flac", { NULL }, { "flac", NULL } },
    { "alac", { NULL }, { "alac", NULL } },
    { "pcm", { NULL }, { "pcm_s16le", NULL } },
};

static const EncChain *find_chain(const char *codec) {
    for (size_t i = 0; i < sizeof(CHAINS) / sizeof(CHAINS[0]); i++) {
        if (!strcmp(CHAINS[i].codec, codec)) {
            return &CHAINS[i];
        }
    }
    return NULL;
}

/* Los nombres a probar, en orden (hasta 10). */
static int candidates(const char *codec, bool gpu, const char **out) {
    const EncChain *ch = find_chain(codec);
    int n = 0;
    if (!ch) {
        return 0;
    }
    if (gpu) {
        for (int i = 0; ch->gpu[i]; i++) {
            out[n++] = ch->gpu[i];
        }
    }
    for (int i = 0; ch->cpu[i]; i++) {
        out[n++] = ch->cpu[i];
    }
    return n;
}

bool capyro_convert_has_encoder(const char *codec, bool gpu) {
    if (!codec) {
        return false;
    }
    const EncChain *ch = find_chain(codec);
    if (!ch) {
        return false;
    }
    const char *const *list = gpu ? ch->gpu : ch->cpu;
    for (int i = 0; list[i]; i++) {
        /* Media Foundation (Windows) no cuenta: el de H.265 necesita una
         * extensión paga de la Microsoft Store, y en Wine no anda. Se prueba
         * igual al convertir, después de los otros. */
        if (!gpu && strstr(list[i], "_mf")) {
            continue;
        }
        if (avcodec_find_encoder_by_name(list[i])) {
            return true;
        }
    }
    return false;
}

/* Lo que acepta un codificador (la API cambió en FFmpeg 7.1). */
static const enum AVPixelFormat *enc_pix_fmts(const AVCodec *c) {
#if LIBAVCODEC_VERSION_INT >= AV_VERSION_INT(61, 13, 100)
    const void *cfg = NULL;
    int n = 0;
    if (avcodec_get_supported_config(NULL, c, AV_CODEC_CONFIG_PIX_FORMAT, 0, &cfg, &n) < 0) {
        return NULL;
    }
    return cfg;
#else
    return c->pix_fmts;
#endif
}

static const enum AVSampleFormat *enc_sample_fmts(const AVCodec *c) {
#if LIBAVCODEC_VERSION_INT >= AV_VERSION_INT(61, 13, 100)
    const void *cfg = NULL;
    int n = 0;
    if (avcodec_get_supported_config(NULL, c, AV_CODEC_CONFIG_SAMPLE_FORMAT, 0, &cfg, &n) < 0) {
        return NULL;
    }
    return cfg;
#else
    return c->sample_fmts;
#endif
}

static const int *enc_sample_rates(const AVCodec *c) {
#if LIBAVCODEC_VERSION_INT >= AV_VERSION_INT(61, 13, 100)
    const void *cfg = NULL;
    int n = 0;
    if (avcodec_get_supported_config(NULL, c, AV_CODEC_CONFIG_SAMPLE_RATE, 0, &cfg, &n) < 0) {
        return NULL;
    }
    return cfg;
#else
    return c->supported_samplerates;
#endif
}

/* El formato de píxel: 4:2:0 de 8 bits si lo acepta (lo que se ve en todos
 * lados); si no, el primero que diga el codificador. */
static enum AVPixelFormat pick_pix_fmt(const AVCodec *c) {
    const enum AVPixelFormat *list = enc_pix_fmts(c);
    if (!list) {
        return AV_PIX_FMT_YUV420P;
    }
    for (int i = 0; list[i] != AV_PIX_FMT_NONE; i++) {
        if (list[i] == AV_PIX_FMT_YUV420P) {
            return AV_PIX_FMT_YUV420P;
        }
    }
    for (int i = 0; list[i] != AV_PIX_FMT_NONE; i++) {
        if (list[i] == AV_PIX_FMT_NV12) {
            return AV_PIX_FMT_NV12;
        }
    }
    return list[0];
}

/* Formato de muestras: el que prefiere el códec; en FLAC/ALAC/WAV, uno que
 * no pierda los bits del original (24 bits → s32). */
static enum AVSampleFormat pick_sample_fmt(const AVCodec *c, int src_bits) {
    const enum AVSampleFormat *list = enc_sample_fmts(c);
    if (!list) {
        return AV_SAMPLE_FMT_FLTP;
    }
    if (src_bits > 16) {
        for (int i = 0; list[i] != AV_SAMPLE_FMT_NONE; i++) {
            if (list[i] == AV_SAMPLE_FMT_S32 || list[i] == AV_SAMPLE_FMT_S32P) {
                return list[i];
            }
        }
    }
    return list[0];
}

/* La frecuencia más parecida que acepta el códec (Opus: solo 48 kHz). */
static int pick_rate(const AVCodec *c, int want) {
    const int *list = enc_sample_rates(c);
    if (!list) {
        return want;
    }
    int best = list[0];
    for (int i = 0; list[i]; i++) {
        if (abs(list[i] - want) < abs(best - want)) {
            best = list[i];
        }
    }
    return best;
}

static double lerp(double a, double b, double t) {
    return a + (b - a) * t;
}

/* ------------------------------------------------------------------ */
/* estado de una conversión                                            */
/* ------------------------------------------------------------------ */

typedef struct {
    int              in_index; /* pista de entrada (-1: imagen fija) */
    AVStream        *in_st, *out_st;
    AVCodecContext  *dec, *enc;
    AVFilterGraph   *graph;
    AVFilterContext *src, *sink;
    bool             copy;
    int64_t          off;      /* cuánto restarle a los tiempos (en la base de la entrada) */
    bool             past_end; /* ya pasó el fin del recorte */
} OStream;

typedef struct {
    AVFormatContext *in, *out;
    const CapyroConvertOptions *opt;
    CapyroConvertResult *res;
    OStream v, a;
    int     *sub_map;          /* pistas que se copian (subtítulos, adjuntos): entrada → salida */
    /* audio → video: la imagen ya lista y el próximo cuadro */
    AVFrame *cover;
    int64_t  cover_next;
    /* recorte y avance */
    double start, dur;         /* dur: lo que dura la salida (0 = no se sabe) */
    double done_t;             /* hasta dónde se escribió */
    int64_t last_report;
    CapyroProgressFn progress;
    void   *user;
    bool    cancelled;
    /* portada de un audio (se escribe después del encabezado) */
    int      cover_out;
    AVPacket cover_pkt;
    bool     have_cover_pkt;
    AVPacket *pkt;
    AVFrame  *frame, *filt;
} Conv;

static void say(Conv *c, const char *fmt, ...) {
    if (!c->res || c->res->message[0]) {
        return; /* el primer error es el que importa */
    }
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(c->res->message, sizeof(c->res->message), fmt, ap);
    va_end(ap);
}

static void free_ostream(OStream *os) {
    avcodec_free_context(&os->dec);
    avcodec_free_context(&os->enc);
    avfilter_graph_free(&os->graph);
    os->src = os->sink = NULL;
}

/* Avisa cuánto va (como mucho 10 veces por segundo). false = cancelaron. */
static bool report(Conv *c, bool force) {
    if (!c->progress) {
        return true;
    }
    int64_t now = av_gettime_relative();
    if (!force && now - c->last_report < 100000) {
        return true;
    }
    c->last_report = now;
    double f = c->dur > 0.0 ? c->done_t / c->dur : 0.0;
    if (f < 0.0) f = 0.0;
    if (f > 1.0) f = 1.0;
    if (c->progress(c->user, f) != 0) {
        c->cancelled = true;
        return false;
    }
    return true;
}

/* Escribe un paquete ya en la base de tiempo de la salida. */
static int write_packet(Conv *c, AVPacket *pkt, AVStream *out_st) {
    if (pkt->pts != AV_NOPTS_VALUE) {
        double t = pkt->pts * av_q2d(out_st->time_base);
        if (t > c->done_t && (out_st->codecpar->codec_type == AVMEDIA_TYPE_AUDIO ||
                              out_st->codecpar->codec_type == AVMEDIA_TYPE_VIDEO)) {
            c->done_t = t;
        }
    }
    int err = av_interleaved_write_frame(c->out, pkt);
    if (err < 0) {
        say(c, "No pude escribir el archivo (¿disco lleno?)");
        return CAPYRO_ERR_WRITE;
    }
    return CAPYRO_OK;
}

/* ------------------------------------------------------------------ */
/* codificar                                                           */
/* ------------------------------------------------------------------ */

static int emit_cover(Conv *c, double up_to);

/* Manda un cuadro (o NULL para vaciar) al codificador y escribe lo que salga. */
static int encode(Conv *c, OStream *os, AVFrame *frame) {
    int err = avcodec_send_frame(os->enc, frame);
    if (err < 0 && err != AVERROR_EOF) {
        say(c, "El codificador %s no aceptó un cuadro", os->enc->codec->name);
        return CAPYRO_ERR_ENCODER;
    }
    for (;;) {
        err = avcodec_receive_packet(os->enc, c->pkt);
        if (err == AVERROR(EAGAIN) || err == AVERROR_EOF) {
            return CAPYRO_OK;
        }
        if (err < 0) {
            say(c, "El codificador %s falló", os->enc->codec->name);
            return CAPYRO_ERR_ENCODER;
        }
        c->pkt->stream_index = os->out_st->index;
        av_packet_rescale_ts(c->pkt, os->enc->time_base, os->out_st->time_base);
        int w = write_packet(c, c->pkt, os->out_st);
        av_packet_unref(c->pkt);
        if (w != CAPYRO_OK) {
            return w;
        }
        /* audio → video: la imagen acompaña al audio */
        if (os == &c->a && c->cover) {
            int e = emit_cover(c, c->done_t);
            if (e != CAPYRO_OK) {
                return e;
            }
        }
    }
}

/* Saca de los filtros todo lo que haya y lo codifica. */
static int drain_filters(Conv *c, OStream *os) {
    for (;;) {
        int err = av_buffersink_get_frame(os->sink, c->filt);
        if (err == AVERROR(EAGAIN) || err == AVERROR_EOF) {
            return CAPYRO_OK;
        }
        if (err < 0) {
            say(c, "Los filtros fallaron");
            return CAPYRO_ERR_DECODE;
        }
        if (c->filt->pts != AV_NOPTS_VALUE) {
            c->filt->pts = av_rescale_q(c->filt->pts, av_buffersink_get_time_base(os->sink), os->enc->time_base);
        }
        if (os == &c->v) {
            c->filt->pict_type = AV_PICTURE_TYPE_NONE;
        }
        int e = encode(c, os, c->filt);
        av_frame_unref(c->filt);
        if (e != CAPYRO_OK) {
            return e;
        }
    }
}

/* Un cuadro decodificado: tiempos desde el inicio del recorte, a los filtros. */
static int push_frame(Conv *c, OStream *os, AVFrame *f) {
    int64_t pts = f->best_effort_timestamp;
    if (pts == AV_NOPTS_VALUE) {
        pts = f->pts;
    }
    if (pts != AV_NOPTS_VALUE) {
        pts -= os->off;
        if (os == &c->a) {
            pts = av_rescale_q(pts, os->in_st->time_base, (AVRational){ 1, f->sample_rate });
        }
    }
    f->pts = pts;
    int err = av_buffersrc_add_frame_flags(os->src, f, AV_BUFFERSRC_FLAG_KEEP_REF);
    if (err < 0) {
        say(c, "Los filtros no aceptaron un cuadro");
        return CAPYRO_ERR_DECODE;
    }
    return drain_filters(c, os);
}

static int decode(Conv *c, OStream *os, const AVPacket *pkt) {
    int err = avcodec_send_packet(os->dec, pkt);
    if (err < 0 && err != AVERROR_EOF && pkt) {
        return CAPYRO_OK; /* un paquete roto se saltea */
    }
    for (;;) {
        err = avcodec_receive_frame(os->dec, c->frame);
        if (err == AVERROR(EAGAIN) || err == AVERROR_EOF) {
            return CAPYRO_OK;
        }
        if (err < 0) {
            return CAPYRO_OK;
        }
        int e = push_frame(c, os, c->frame);
        av_frame_unref(c->frame);
        if (e != CAPYRO_OK) {
            return e;
        }
    }
}

/* audio → video: repite la imagen hasta `up_to` segundos. */
static int emit_cover(Conv *c, double up_to) {
    while ((double)c->cover_next / COVER_FPS <= up_to) {
        AVFrame *f = av_frame_clone(c->cover);
        if (!f) {
            return CAPYRO_ERR_NO_MEMORY;
        }
        f->pts = c->cover_next++;
        f->pict_type = AV_PICTURE_TYPE_NONE;
        int e = encode(c, &c->v, f);
        av_frame_free(&f);
        if (e != CAPYRO_OK) {
            return e;
        }
    }
    return CAPYRO_OK;
}

/* Copia un paquete tal cual (con los tiempos corridos al recorte). */
static int copy_packet(Conv *c, AVPacket *pkt, AVStream *in_st, AVStream *out_st, int64_t off, bool *past_end) {
    if (pkt->pts != AV_NOPTS_VALUE) {
        pkt->pts -= off;
    }
    if (pkt->dts != AV_NOPTS_VALUE) {
        pkt->dts -= off;
    }
    double t = pkt->pts != AV_NOPTS_VALUE ? pkt->pts * av_q2d(in_st->time_base) : 0.0;
    double d = pkt->duration > 0 ? pkt->duration * av_q2d(in_st->time_base) : 0.0;
    bool trimming = c->start > 0.0 || c->opt->end > 0.0;
    if (trimming && c->dur > 0.0 && t >= c->dur) {
        if (past_end) {
            *past_end = true;
        }
        return CAPYRO_OK;
    }
    /* antes del inicio: el audio y los subtítulos se tiran; el video desde
     * el cuadro clave anterior (si no, no se puede ver) */
    if (trimming && t + d < 0.0 && in_st->codecpar->codec_type != AVMEDIA_TYPE_VIDEO) {
        return CAPYRO_OK;
    }
    pkt->stream_index = out_st->index;
    av_packet_rescale_ts(pkt, in_st->time_base, out_st->time_base);
    pkt->pos = -1;
    return write_packet(c, pkt, out_st);
}

/* ------------------------------------------------------------------ */
/* armar las pistas                                                    */
/* ------------------------------------------------------------------ */

static int stream_rotation(const AVStream *st) {
    const int32_t *m = NULL;
#if LIBAVCODEC_VERSION_INT >= AV_VERSION_INT(60, 29, 100)
    const AVPacketSideData *sd =
        av_packet_side_data_get(st->codecpar->coded_side_data, st->codecpar->nb_coded_side_data, AV_PKT_DATA_DISPLAYMATRIX);
    if (sd && sd->size >= 9 * sizeof(int32_t)) {
        m = (const int32_t *)sd->data;
    }
#else
    size_t size = 0;
    const uint8_t *d = av_stream_get_side_data((AVStream *)st, AV_PKT_DATA_DISPLAYMATRIX, &size);
    if (d && size >= 9 * sizeof(int32_t)) {
        m = (const int32_t *)d;
    }
#endif
    if (!m) {
        return 0;
    }
    double theta = -round(av_display_rotation_get(m));
    if (isnan(theta)) {
        return 0;
    }
    theta -= 360.0 * floor(theta / 360.0 + 0.9 / 360.0);
    if (fabs(theta - 90.0) < 1.0) return 90;
    if (fabs(theta - 180.0) < 1.0) return 180;
    if (fabs(theta - 270.0) < 1.0) return 270;
    return 0;
}

/* La mejor pista de video que sea video de verdad (no una portada). */
static int best_video(AVFormatContext *in) {
    int best = -1;
    int64_t best_px = -1;
    for (unsigned i = 0; i < in->nb_streams; i++) {
        const AVStream *st = in->streams[i];
        if (st->codecpar->codec_type != AVMEDIA_TYPE_VIDEO || (st->disposition & AV_DISPOSITION_ATTACHED_PIC)) {
            continue;
        }
        int64_t px = (int64_t)st->codecpar->width * st->codecpar->height;
        if (px > best_px) {
            best_px = px;
            best = (int)i;
        }
    }
    return best;
}

static int attached_pic(AVFormatContext *in) {
    for (unsigned i = 0; i < in->nb_streams; i++) {
        const AVStream *st = in->streams[i];
        if ((st->disposition & AV_DISPOSITION_ATTACHED_PIC) && st->attached_pic.size > 0) {
            return (int)i;
        }
    }
    return -1;
}

static int open_decoder(Conv *c, OStream *os) {
    const AVCodec *dec = avcodec_find_decoder(os->in_st->codecpar->codec_id);
    if (!dec) {
        say(c, "No sé leer el %s de este archivo", avcodec_get_name(os->in_st->codecpar->codec_id));
        return CAPYRO_ERR_DECODE;
    }
    os->dec = avcodec_alloc_context3(dec);
    if (!os->dec || avcodec_parameters_to_context(os->dec, os->in_st->codecpar) < 0) {
        return CAPYRO_ERR_NO_MEMORY;
    }
    os->dec->thread_count = 0;
    os->dec->pkt_timebase = os->in_st->time_base;
    if (avcodec_open2(os->dec, dec, NULL) < 0) {
        say(c, "No pude abrir el decodificador de %s", dec->name);
        return CAPYRO_ERR_DECODE;
    }
    return CAPYRO_OK;
}

/* Arma un grafo: [fuente] → `chain` → [salida]. */
static int make_graph(OStream *os, const char *src_name, const char *src_args, const char *chain) {
    avfilter_graph_free(&os->graph);
    os->graph = avfilter_graph_alloc();
    if (!os->graph) {
        return CAPYRO_ERR_NO_MEMORY;
    }
    os->graph->nb_threads = 0;
    bool audio = !strcmp(src_name, "abuffer");
    if (avfilter_graph_create_filter(&os->src, avfilter_get_by_name(src_name), "in", src_args, NULL, os->graph) < 0 ||
        avfilter_graph_create_filter(&os->sink, avfilter_get_by_name(audio ? "abuffersink" : "buffersink"), "out", NULL,
                                     NULL, os->graph) < 0) {
        return CAPYRO_ERR_DECODE;
    }
    AVFilterInOut *outputs = avfilter_inout_alloc();
    AVFilterInOut *inputs = avfilter_inout_alloc();
    if (!outputs || !inputs) {
        avfilter_inout_free(&outputs);
        avfilter_inout_free(&inputs);
        return CAPYRO_ERR_NO_MEMORY;
    }
    outputs->name = av_strdup("in");
    outputs->filter_ctx = os->src;
    inputs->name = av_strdup("out");
    inputs->filter_ctx = os->sink;
    int err = avfilter_graph_parse_ptr(os->graph, chain, &inputs, &outputs, NULL);
    avfilter_inout_free(&outputs);
    avfilter_inout_free(&inputs);
    if (err < 0 || avfilter_graph_config(os->graph, NULL) < 0) {
        return CAPYRO_ERR_DECODE;
    }
    return CAPYRO_OK;
}

/* Calidad (0..100) → opciones de cada codificador de video. */
static void video_options(const char *name, const CapyroConvertOptions *o, bool still, AVCodecContext *enc,
                          AVDictionary **d) {
    double q = (o->quality < 0 ? 0 : o->quality > 100 ? 100 : o->quality) / 100.0;
    int speed = o->speed < 0 ? 0 : o->speed > 2 ? 2 : o->speed;
    char buf[32];
    double fps = enc->framerate.num > 0 && enc->framerate.den > 0 ? av_q2d(enc->framerate) : 30.0;
    /* para los que piden bitrate: bits por píxel según la calidad */
    double bpp = still ? 0.02 : lerp(0.035, 0.16, q);
    int64_t br = (int64_t)(enc->width * (double)enc->height * fps * bpp);
    if (br < 250000) {
        br = 250000;
    }

    if (!strcmp(name, "libx264")) {
        snprintf(buf, sizeof(buf), "%.1f", lerp(34.0, 17.0, q));
        av_dict_set(d, "crf", buf, 0);
        av_dict_set(d, "preset", speed == 0 ? "veryfast" : speed == 1 ? "medium" : "slow", 0);
        if (o->compat) {
            av_dict_set(d, "profile", "high", 0);
            av_dict_set(d, "level", "4.1", 0);
        }
        if (still) {
            av_dict_set(d, "tune", "stillimage", 0);
        }
    } else if (!strcmp(name, "libx265")) {
        snprintf(buf, sizeof(buf), "%.1f", lerp(37.0, 20.0, q));
        av_dict_set(d, "crf", buf, 0);
        av_dict_set(d, "preset", speed == 0 ? "veryfast" : speed == 1 ? "medium" : "slow", 0);
        av_dict_set(d, "x265-params", "log-level=error", 0);
    } else if (!strcmp(name, "libvpx-vp9")) {
        snprintf(buf, sizeof(buf), "%d", (int)lerp(48.0, 24.0, q));
        av_dict_set(d, "crf", buf, 0);
        enc->bit_rate = 0;
        av_dict_set(d, "deadline", "good", 0);
        av_dict_set(d, "cpu-used", speed == 0 ? "5" : speed == 1 ? "3" : "1", 0);
        av_dict_set(d, "row-mt", "1", 0);
    } else if (!strcmp(name, "libsvtav1")) {
        snprintf(buf, sizeof(buf), "%d", (int)lerp(50.0, 22.0, q));
        av_dict_set(d, "crf", buf, 0);
        av_dict_set(d, "preset", speed == 0 ? "10" : speed == 1 ? "8" : "6", 0);
    } else if (!strcmp(name, "libaom-av1")) {
        snprintf(buf, sizeof(buf), "%d", (int)lerp(50.0, 22.0, q));
        av_dict_set(d, "crf", buf, 0);
        enc->bit_rate = 0;
        av_dict_set(d, "cpu-used", speed == 0 ? "8" : speed == 1 ? "6" : "4", 0);
        av_dict_set(d, "row-mt", "1", 0);
    } else if (strstr(name, "_nvenc")) {
        snprintf(buf, sizeof(buf), "%d", (int)lerp(34.0, 19.0, q));
        av_dict_set(d, "rc", "vbr", 0);
        av_dict_set(d, "cq", buf, 0);
        enc->bit_rate = 0;
        av_dict_set(d, "preset", speed == 0 ? "p2" : speed == 1 ? "p4" : "p6", 0);
    } else if (strstr(name, "_qsv")) {
        enc->global_quality = (int)lerp(34.0, 19.0, q);
        av_dict_set(d, "preset", speed == 0 ? "veryfast" : speed == 1 ? "medium" : "veryslow", 0);
    } else if (strstr(name, "_amf")) {
        snprintf(buf, sizeof(buf), "%d", (int)lerp(34.0, 18.0, q));
        av_dict_set(d, "rc", "cqp", 0);
        av_dict_set(d, "qp_i", buf, 0);
        av_dict_set(d, "qp_p", buf, 0);
        av_dict_set(d, "qp_b", buf, 0);
        av_dict_set(d, "quality", speed == 0 ? "speed" : speed == 1 ? "balanced" : "quality", 0);
    } else if (!strcmp(name, "mpeg4")) {
        enc->flags |= AV_CODEC_FLAG_QSCALE;
        enc->global_quality = (int)(FF_QP2LAMBDA * lerp(12.0, 2.0, q));
    } else {
        /* OpenH264, Media Foundation, VideoToolbox, rav1e: por bitrate */
        enc->bit_rate = br;
        enc->rc_max_rate = br * 2;
        enc->rc_buffer_size = (int)(br * 2);
        if (strstr(name, "_videotoolbox")) {
            /* sin el chip para ese códec, el codificador por software de macOS */
            av_dict_set(d, "allow_sw", "1", 0);
            if (!strcmp(name, "h264_videotoolbox")) {
                av_dict_set(d, "profile", "high", 0);
            }
        }
        if (!strcmp(name, "libopenh264")) {
            av_dict_set(d, "rc_mode", "bitrate", 0);
            av_dict_set(d, "allow_skip_frames", "0", 0);
            enc->profile = 100; /* High (CABAC): comprime mejor y lo abre todo */
        }
    }
}

/* Abre el codificador de video `name` para cuadros w×h (ya filtrados). */
static int open_video_encoder(Conv *c, OStream *os, const AVCodec *codec, int w, int h, AVRational sar, AVRational tb,
                              AVRational fr, enum AVPixelFormat pix, bool still) {
    avcodec_free_context(&os->enc);
    os->enc = avcodec_alloc_context3(codec);
    if (!os->enc) {
        return CAPYRO_ERR_NO_MEMORY;
    }
    AVCodecContext *enc = os->enc;
    enc->width = w;
    enc->height = h;
    enc->sample_aspect_ratio = sar.num > 0 ? sar : (AVRational){ 1, 1 };
    enc->pix_fmt = pix;
    enc->time_base = tb;
    enc->framerate = fr;
    double fps = fr.num > 0 && fr.den > 0 ? av_q2d(fr) : 30.0;
    /* imagen fija: un cuadro completo cada 10 s alcanza (el resto casi no pesa) */
    enc->gop_size = still ? COVER_FPS * 10 : (int)(fps * 2.0 + 0.5);
    enc->thread_count = 0;
    if (c->out->oformat->flags & AVFMT_GLOBALHEADER) {
        enc->flags |= AV_CODEC_FLAG_GLOBAL_HEADER;
    }
    AVDictionary *d = NULL;
    video_options(codec->name, c->opt, still, enc, &d);
    int err = avcodec_open2(enc, codec, &d);
    av_dict_free(&d);
    return err < 0 ? CAPYRO_ERR_ENCODER : CAPYRO_OK;
}

/* Los filtros del video: girar, achicar (lado corto), cuadros por segundo,
 * recortar y el formato del codificador. */
static void video_chain(Conv *c, const AVStream *st, enum AVPixelFormat pix, bool mpeg4, char *buf, size_t cap) {
    const CapyroConvertOptions *o = c->opt;
    buf[0] = '\0';
    int rot = stream_rotation(st);
    if (rot == 90) av_strlcat(buf, "transpose=clock,", cap);
    if (rot == 180) av_strlcat(buf, "hflip,vflip,", cap);
    if (rot == 270) av_strlcat(buf, "transpose=cclock,", cap);
    char tmp[256];
    int n = o->max_height;
    if (n > 0) {
        snprintf(tmp, sizeof(tmp),
                 "scale=w='if(gte(iw,ih),-2,min(iw,%d))':h='if(gte(iw,ih),min(ih,%d),-2)':flags=bicubic,", n, n);
    } else {
        /* 4:2:0 necesita lados pares */
        snprintf(tmp, sizeof(tmp), "scale=w='trunc(iw/2)*2':h='trunc(ih/2)*2':flags=bicubic,");
    }
    av_strlcat(buf, tmp, cap);
    double src_fps = st->avg_frame_rate.num > 0 && st->avg_frame_rate.den > 0 ? av_q2d(st->avg_frame_rate) : 0.0;
    double want = o->fps > 0 ? o->fps : 0.0;
    if (o->compat && (want <= 0.0 || want > 60.0) && src_fps > 60.5) {
        want = 60.0;
    }
    if (want > 0.0 && (src_fps <= 0.0 || src_fps > want + 0.5)) {
        snprintf(tmp, sizeof(tmp), "fps=%d,", (int)want);
        av_strlcat(buf, tmp, cap);
    } else if (mpeg4) {
        /* MPEG-4 no acepta bases de tiempo finas: cuadros por segundo fijos */
        snprintf(tmp, sizeof(tmp), "fps=%.3f,", src_fps > 0.0 ? src_fps : 30.0);
        av_strlcat(buf, tmp, cap);
    }
    if (c->start > 0.0 || o->end > 0.0) {
        if (c->dur > 0.0) {
            snprintf(tmp, sizeof(tmp), "trim=start=0:end=%.6f,", c->dur);
        } else {
            snprintf(tmp, sizeof(tmp), "trim=start=0,");
        }
        av_strlcat(buf, tmp, cap);
    }
    snprintf(tmp, sizeof(tmp), "format=%s", av_get_pix_fmt_name(pix));
    av_strlcat(buf, tmp, cap);
}

static int setup_video_encode(Conv *c, OStream *os, const char *codec) {
    int e = open_decoder(c, os);
    if (e != CAPYRO_OK) {
        return e;
    }
    const char *names[10];
    int n = candidates(codec, c->opt->gpu, names);
    AVCodecContext *dec = os->dec;
    AVRational tb = os->in_st->time_base;
    AVRational sar = dec->sample_aspect_ratio;
    AVRational fr = av_guess_frame_rate(c->in, os->in_st, NULL);
    char args[512];
    snprintf(args, sizeof(args), "video_size=%dx%d:pix_fmt=%d:time_base=%d/%d:pixel_aspect=%d/%d", dec->width,
             dec->height, dec->pix_fmt, tb.num, tb.den, sar.num > 0 ? sar.num : 1, sar.num > 0 ? sar.den : 1);
    if (fr.num > 0 && fr.den > 0) {
        char t[64];
        snprintf(t, sizeof(t), ":frame_rate=%d/%d", fr.num, fr.den);
        av_strlcat(args, t, sizeof(args));
    }
    bool found = false;
    for (int i = 0; i < n; i++) {
        const AVCodec *enc = avcodec_find_encoder_by_name(names[i]);
        if (!enc) {
            continue;
        }
        found = true;
        enum AVPixelFormat pix = pick_pix_fmt(enc);
        char chain[1024];
        video_chain(c, os->in_st, pix, !strcmp(names[i], "mpeg4"), chain, sizeof(chain));
        if (make_graph(os, "buffer", args, chain) != CAPYRO_OK) {
            continue;
        }
        AVRational otb = av_buffersink_get_time_base(os->sink);
        AVRational ofr = av_buffersink_get_frame_rate(os->sink);
        if (ofr.num <= 0) {
            ofr = fr;
        }
        if (open_video_encoder(c, os, enc, av_buffersink_get_w(os->sink), av_buffersink_get_h(os->sink),
                               av_buffersink_get_sample_aspect_ratio(os->sink), otb, ofr, pix, false) == CAPYRO_OK) {
            snprintf(c->res->video_encoder, sizeof(c->res->video_encoder), "%s", enc->name);
            return CAPYRO_OK;
        }
    }
    avfilter_graph_free(&os->graph);
    if (!found) {
        say(c, "Este FFmpeg no trae con qué codificar video a %s", codec);
    } else {
        say(c, "No pude abrir ningún codificador de %s", codec);
    }
    return CAPYRO_ERR_ENCODER;
}

/* audio → video: arma la imagen del tamaño del video, con la portada al
 * medio y de fondo la misma desenfocada y oscurecida. */
static AVFrame *compose_cover(AVFrame *img, int W, int H, enum AVPixelFormat pix) {
    OStream tmp;
    memset(&tmp, 0, sizeof(tmp));
    char args[256];
    snprintf(args, sizeof(args), "video_size=%dx%d:pix_fmt=%d:time_base=1/%d:pixel_aspect=1/1", img->width, img->height,
             img->format, COVER_FPS);
    int fh = (int)(H * 0.82) & ~1;
    int fw = (int)(W * 0.9) & ~1;
    char chain[1024];
    const char *pixname = av_get_pix_fmt_name(pix);
    snprintf(chain, sizeof(chain),
             "split[a][b];"
             "[a]scale=%d:%d:force_original_aspect_ratio=increase,crop=%d:%d,format=gbrp,gblur=sigma=28,"
             "format=rgb24,colorchannelmixer=rr=0.42:gg=0.42:bb=0.42[bg];"
             "[b]scale=%d:%d:force_original_aspect_ratio=decrease:force_divisible_by=2,format=rgb24[fg];"
             "[bg][fg]overlay=(W-w)/2:(H-h)/2,setsar=1,format=%s",
             W, H, W, H, fw, fh, pixname);
    AVFrame *out = NULL;
    if (make_graph(&tmp, "buffer", args, chain) != CAPYRO_OK) {
        /* sin alguno de esos filtros: la imagen al medio sobre negro */
        snprintf(chain, sizeof(chain),
                 "scale=%d:%d:force_original_aspect_ratio=decrease:force_divisible_by=2,"
                 "pad=%d:%d:(ow-iw)/2:(oh-ih)/2:color=0x141418,setsar=1,format=%s",
                 fw, fh, W, H, pixname);
        if (make_graph(&tmp, "buffer", args, chain) != CAPYRO_OK) {
            avfilter_graph_free(&tmp.graph);
            return NULL;
        }
    }
    img->pts = 0;
    out = av_frame_alloc();
    if (out && av_buffersrc_add_frame_flags(tmp.src, img, AV_BUFFERSRC_FLAG_KEEP_REF) >= 0 &&
        av_buffersrc_add_frame(tmp.src, NULL) >= 0 && av_buffersink_get_frame(tmp.sink, out) >= 0) {
        avfilter_graph_free(&tmp.graph);
        return out;
    }
    av_frame_free(&out);
    avfilter_graph_free(&tmp.graph);
    return NULL;
}

/* Un fondo liso (si el audio no tiene portada y no se eligió imagen). */
static AVFrame *plain_image(void) {
    AVFrame *f = av_frame_alloc();
    if (!f) {
        return NULL;
    }
    f->format = AV_PIX_FMT_RGB24;
    f->width = 640;
    f->height = 640;
    if (av_frame_get_buffer(f, 0) < 0) {
        av_frame_free(&f);
        return NULL;
    }
    for (int y = 0; y < f->height; y++) {
        uint8_t *row = f->data[0] + (size_t)y * f->linesize[0];
        for (int x = 0; x < f->width; x++) {
            double d = hypot(x - 320.0, y - 320.0) / 452.0;
            row[3 * x + 0] = (uint8_t)(40 - 22 * d);
            row[3 * x + 1] = (uint8_t)(30 - 16 * d);
            row[3 * x + 2] = (uint8_t)(34 - 16 * d);
        }
    }
    return f;
}

static int setup_cover_video(Conv *c, OStream *os, const char *codec) {
    const CapyroConvertOptions *o = c->opt;
    AVFrame *img = NULL;
    if (o->cover_image[0]) {
        img = cover_image_frame(o->cover_image);
        if (!img) {
            say(c, "No pude abrir la imagen elegida");
            return CAPYRO_ERR_DECODE;
        }
    } else {
        int pic = attached_pic(c->in);
        img = pic >= 0 ? cover_attached_frame(c->in->streams[pic]) : NULL;
        if (!img) {
            img = plain_image();
        }
    }
    if (!img) {
        return CAPYRO_ERR_NO_MEMORY;
    }
    int H = o->max_height > 0 ? o->max_height : 1080;
    if (H < 240) H = 240;
    if (H > 2160) H = 2160;
    int W = ((H * 16 / 9) + 1) & ~1;
    H &= ~1;

    const char *names[10];
    int n = candidates(codec, o->gpu, names);
    bool found = false;
    for (int i = 0; i < n; i++) {
        const AVCodec *enc = avcodec_find_encoder_by_name(names[i]);
        if (!enc) {
            continue;
        }
        found = true;
        enum AVPixelFormat pix = pick_pix_fmt(enc);
        AVFrame *frame = compose_cover(img, W, H, pix);
        if (!frame) {
            continue;
        }
        if (open_video_encoder(c, os, enc, frame->width, frame->height, (AVRational){ 1, 1 },
                               (AVRational){ 1, COVER_FPS }, (AVRational){ COVER_FPS, 1 }, pix, true) == CAPYRO_OK) {
            snprintf(c->res->video_encoder, sizeof(c->res->video_encoder), "%s", enc->name);
            c->cover = frame;
            av_frame_free(&img);
            return CAPYRO_OK;
        }
        av_frame_free(&frame);
    }
    av_frame_free(&img);
    if (!found) {
        say(c, "Este FFmpeg no trae con qué codificar video a %s", codec);
    } else {
        say(c, "No pude armar el video con la portada");
    }
    return CAPYRO_ERR_ENCODER;
}

static int setup_audio_encode(Conv *c, OStream *os, const char *codec) {
    int e = open_decoder(c, os);
    if (e != CAPYRO_OK) {
        return e;
    }
    const CapyroConvertOptions *o = c->opt;
    AVCodecContext *dec = os->dec;
    int src_bits = dec->bits_per_raw_sample > 0 ? dec->bits_per_raw_sample : dec->bits_per_coded_sample;
    const char *names[10];
    int n = candidates(codec, false, names);
    if (!strcmp(codec, "pcm") && src_bits > 16) {
        names[0] = "pcm_s24le";
    }
    AVChannelLayout in_layout = { 0 };
    if (dec->ch_layout.order == AV_CHANNEL_ORDER_UNSPEC || dec->ch_layout.nb_channels == 0) {
        av_channel_layout_default(&in_layout, dec->ch_layout.nb_channels > 0 ? dec->ch_layout.nb_channels : 2);
    } else if (av_channel_layout_copy(&in_layout, &dec->ch_layout) < 0) {
        return CAPYRO_ERR_NO_MEMORY;
    }
    bool found = false;
    for (int i = 0; i < n; i++) {
        const AVCodec *enc = avcodec_find_encoder_by_name(names[i]);
        if (!enc) {
            continue;
        }
        found = true;
        avcodec_free_context(&os->enc);
        os->enc = avcodec_alloc_context3(enc);
        if (!os->enc) {
            break;
        }
        AVCodecContext *ec = os->enc;
        int rate = pick_rate(enc, o->sample_rate > 0 ? o->sample_rate : dec->sample_rate);
        int ch = o->channels > 0 ? o->channels : in_layout.nb_channels;
        /* MP3 y Vorbis nativo: como mucho estéreo */
        if ((!strcmp(codec, "mp3") || !strcmp(names[i], "vorbis")) && ch > 2) {
            ch = 2;
        }
        if (ch > 8) {
            ch = 8;
        }
        av_channel_layout_default(&ec->ch_layout, ch);
        ec->sample_rate = rate;
        ec->sample_fmt = pick_sample_fmt(enc, src_bits);
        ec->time_base = (AVRational){ 1, rate };
        ec->thread_count = 0;
        if (src_bits > 16 && (!strcmp(codec, "flac") || !strcmp(codec, "alac"))) {
            ec->bits_per_raw_sample = 24;
        }
        int kbps = o->audio_kbps;
        if (kbps <= 0) {
            kbps = !strcmp(codec, "opus") ? 128 : !strcmp(codec, "vorbis") ? 192 : !strcmp(codec, "aac") ? 192 : 192;
        }
        bool lossless = !strcmp(codec, "flac") || !strcmp(codec, "alac") || !strcmp(codec, "pcm");
        if (!lossless) {
            ec->bit_rate = (int64_t)kbps * 1000 * (ch > 2 ? ch / 2 : 1);
        }
        if (!strcmp(codec, "flac")) {
            ec->compression_level = o->speed >= 2 ? 8 : 5;
        }
        if (!strcmp(names[i], "opus") || !strcmp(names[i], "vorbis")) {
            ec->strict_std_compliance = FF_COMPLIANCE_EXPERIMENTAL;
        }
        if (c->out->oformat->flags & AVFMT_GLOBALHEADER) {
            ec->flags |= AV_CODEC_FLAG_GLOBAL_HEADER;
        }
        if (avcodec_open2(ec, enc, NULL) < 0) {
            continue;
        }
        /* los filtros: recortar, emparejar volumen y pasar a lo que pide el códec */
        char inl[128], outl[128], args[512], chain[1024], tmp[256];
        av_channel_layout_describe(&in_layout, inl, sizeof(inl));
        av_channel_layout_describe(&ec->ch_layout, outl, sizeof(outl));
        snprintf(args, sizeof(args), "time_base=1/%d:sample_rate=%d:sample_fmt=%s:channel_layout=%s", dec->sample_rate,
                 dec->sample_rate, av_get_sample_fmt_name(dec->sample_fmt), inl);
        chain[0] = '\0';
        if (c->start > 0.0 || o->end > 0.0) {
            if (c->dur > 0.0) {
                snprintf(tmp, sizeof(tmp), "atrim=start=0:end=%.6f,", c->dur);
            } else {
                snprintf(tmp, sizeof(tmp), "atrim=start=0,");
            }
            av_strlcat(chain, tmp, sizeof(chain));
        }
        if (o->normalize) {
            av_strlcat(chain, "loudnorm=I=-16:TP=-1.5:LRA=11,", sizeof(chain));
        }
        snprintf(tmp, sizeof(tmp), "aresample=%d,aformat=sample_fmts=%s:sample_rates=%d:channel_layouts=%s", rate,
                 av_get_sample_fmt_name(ec->sample_fmt), rate, outl);
        av_strlcat(chain, tmp, sizeof(chain));
        if (make_graph(os, "abuffer", args, chain) != CAPYRO_OK) {
            continue;
        }
        if (!(enc->capabilities & AV_CODEC_CAP_VARIABLE_FRAME_SIZE) && ec->frame_size > 0) {
            av_buffersink_set_frame_size(os->sink, (unsigned)ec->frame_size);
        }
        snprintf(c->res->audio_encoder, sizeof(c->res->audio_encoder), "%s", enc->name);
        av_channel_layout_uninit(&in_layout);
        return CAPYRO_OK;
    }
    av_channel_layout_uninit(&in_layout);
    if (!found) {
        say(c, "Este FFmpeg no trae con qué codificar audio a %s", codec);
    } else {
        say(c, "No pude abrir el codificador de %s", codec);
    }
    return CAPYRO_ERR_ENCODER;
}

/* Pista de salida para una de entrada que se copia tal cual. */
static AVStream *copy_stream(Conv *c, const AVStream *in_st) {
    AVStream *so = avformat_new_stream(c->out, NULL);
    if (!so || avcodec_parameters_copy(so->codecpar, in_st->codecpar) < 0) {
        return NULL;
    }
    so->codecpar->codec_tag = 0;
    so->time_base = in_st->time_base;
    so->disposition = in_st->disposition & ~AV_DISPOSITION_ATTACHED_PIC;
    av_dict_copy(&so->metadata, in_st->metadata, 0);
    return so;
}

static bool is_ogg(const AVOutputFormat *f) {
    return cover_muxer_is_ogg(f);
}

/* Etiquetas: las del archivo y, si no están, las de la pista de audio (en
 * Ogg/Opus las etiquetas van en la pista). */
static void copy_tags(Conv *c) {
    AVDictionary **dst = &c->out->metadata;
    av_dict_copy(dst, c->in->metadata, 0);
    if (c->a.in_st) {
        const AVDictionaryEntry *e = NULL;
        while ((e = av_dict_get(c->a.in_st->metadata, "", e, AV_DICT_IGNORE_SUFFIX))) {
            if (!av_strcasecmp(e->key, "encoder") || !av_strcasecmp(e->key, "handler_name") ||
                !av_strcasecmp(e->key, "vendor_id") || !av_strcasecmp(e->key, "language") ||
                !av_strncasecmp(e->key, "duration", 8) || !av_strcasecmp(e->key, "METADATA_BLOCK_PICTURE")) {
                continue;
            }
            av_dict_set(dst, e->key, e->value, AV_DICT_DONT_OVERWRITE);
        }
    }
    av_dict_set(dst, "encoder", NULL, 0);
    av_dict_set(dst, "major_brand", NULL, 0);
    av_dict_set(dst, "minor_version", NULL, 0);
    av_dict_set(dst, "compatible_brands", NULL, 0);
    /* en Ogg el muxer escribe las de la pista */
    if (is_ogg(c->out->oformat) && c->a.out_st) {
        av_dict_copy(&c->a.out_st->metadata, *dst, AV_DICT_DONT_OVERWRITE);
        av_dict_set(&c->a.out_st->metadata, "encoder", NULL, 0);
    }
}

/* Capítulos (corridos al recorte; los de afuera se van). */
static int copy_chapters(Conv *c) {
    for (unsigned i = 0; i < c->in->nb_chapters; i++) {
        const AVChapter *ci = c->in->chapters[i];
        double t0 = c->in->start_time != AV_NOPTS_VALUE ? (double)c->in->start_time / AV_TIME_BASE : 0.0;
        double s = ci->start * av_q2d(ci->time_base) - t0 - c->start;
        double e = ci->end * av_q2d(ci->time_base) - t0 - c->start;
        if (e <= 0.0 || (c->dur > 0.0 && s >= c->dur)) {
            continue;
        }
        if (s < 0.0) s = 0.0;
        if (c->dur > 0.0 && e > c->dur) e = c->dur;
        AVChapter *co = av_mallocz(sizeof(*co));
        if (!co) {
            return CAPYRO_ERR_NO_MEMORY;
        }
        co->id = ci->id;
        co->time_base = (AVRational){ 1, 1000 };
        co->start = (int64_t)(s * 1000.0);
        co->end = (int64_t)(e * 1000.0);
        av_dict_copy(&co->metadata, ci->metadata, 0);
        if (av_dynarray_add_nofree(&c->out->chapters, (int *)&c->out->nb_chapters, co) < 0) {
            av_dict_free(&co->metadata);
            av_free(co);
            return CAPYRO_ERR_NO_MEMORY;
        }
    }
    return CAPYRO_OK;
}

/* Un audio de salida con la portada del de entrada (si el formato la guarda). */
static int add_audio_cover(Conv *c) {
    int pic = attached_pic(c->in);
    if (pic < 0) {
        return CAPYRO_OK;
    }
    const AVStream *ps = c->in->streams[pic];
    const AVOutputFormat *f = c->out->oformat;
    if (is_ogg(f)) {
        const char *mime = cover_mime(ps->codecpar->codec_id);
        if (!mime || !c->a.out_st) {
            return CAPYRO_OK;
        }
        char *b64 = cover_ogg_picture(ps->attached_pic.data, (size_t)ps->attached_pic.size, mime,
                                      ps->codecpar->width, ps->codecpar->height);
        if (b64) {
            av_dict_set(&c->a.out_st->metadata, "METADATA_BLOCK_PICTURE", b64, AV_DICT_DONT_STRDUP_VAL);
        }
        return CAPYRO_OK;
    }
    if (cover_muxer_is_mkv(f)) {
        return cover_add_mkv_attachment(c->out, ps->attached_pic.data, (size_t)ps->attached_pic.size,
                                        ps->codecpar->codec_id);
    }
    if (!cover_muxer_ok(f)) {
        return CAPYRO_OK;
    }
    AVStream *so = avformat_new_stream(c->out, NULL);
    if (!so || avcodec_parameters_copy(so->codecpar, ps->codecpar) < 0) {
        return CAPYRO_ERR_NO_MEMORY;
    }
    so->codecpar->codec_tag = 0;
    so->time_base = (AVRational){ 1, 90000 };
    so->disposition = AV_DISPOSITION_ATTACHED_PIC;
    av_dict_set(&so->metadata, "comment", "Cover (front)", 0);
    if (av_packet_ref(&c->cover_pkt, &ps->attached_pic) < 0) {
        return CAPYRO_ERR_NO_MEMORY;
    }
    c->have_cover_pkt = true;
    c->cover_out = so->index;
    return CAPYRO_OK;
}

/* ------------------------------------------------------------------ */
/* la conversión                                                       */
/* ------------------------------------------------------------------ */

static int run(Conv *c, const char *in_path, const char *out_path) {
    const CapyroConvertOptions *o = c->opt;
    if (avformat_open_input(&c->in, in_path, NULL, NULL) < 0) {
        say(c, "No pude abrir el archivo");
        return CAPYRO_ERR_NOT_FOUND;
    }
    if (avformat_find_stream_info(c->in, NULL) < 0) {
        say(c, "No pude leer el archivo");
        return CAPYRO_ERR_DECODE;
    }
    if (avformat_alloc_output_context2(&c->out, NULL, NULL, out_path) < 0 || !c->out) {
        say(c, "No conozco ese formato de salida");
        return CAPYRO_ERR_UNSUPPORTED;
    }
    const AVOutputFormat *ofmt = c->out->oformat;

    int vi = best_video(c->in);
    int ai = av_find_best_stream(c->in, AVMEDIA_TYPE_AUDIO, -1, -1, NULL, 0);
    bool want_v = o->video_codec[0] && ofmt->video_codec != AV_CODEC_ID_NONE;
    bool want_a = o->audio_codec[0] && ai >= 0;
    bool cover_video = want_v && vi < 0;
    if (cover_video && ai < 0) {
        say(c, "El archivo no tiene ni video ni audio");
        return CAPYRO_ERR_NO_STREAMS;
    }
    if (cover_video && !strcmp(o->video_codec, "copy")) {
        say(c, "El archivo no tiene video para copiar");
        return CAPYRO_ERR_NO_STREAMS;
    }
    if (!want_v && !want_a) {
        say(c, ai < 0 ? "El archivo no tiene audio" : "No queda nada para guardar (ni video ni audio)");
        return CAPYRO_ERR_NO_STREAMS;
    }

    /* recorte y duración de la salida */
    double t0 = c->in->start_time != AV_NOPTS_VALUE ? (double)c->in->start_time / AV_TIME_BASE : 0.0;
    double total = c->in->duration > 0 ? (double)c->in->duration / AV_TIME_BASE : 0.0;
    c->start = o->start > 0.0 ? o->start : 0.0;
    double end = o->end > 0.0 ? o->end : total;
    if (total > 0.0 && end > total) {
        end = total;
    }
    c->dur = end > c->start ? end - c->start : 0.0;
    if ((o->end > 0.0 || c->start > 0.0) && c->dur <= 0.0 && total > 0.0) {
        say(c, "El recorte quedó vacío");
        return CAPYRO_ERR_INVALID_ARG;
    }
    int64_t off_us = (int64_t)((t0 + c->start) * AV_TIME_BASE);

    /* --- video --- */
    if (want_v) {
        OStream *os = &c->v;
        if (cover_video) {
            os->in_index = -1;
            os->out_st = avformat_new_stream(c->out, NULL);
            if (!os->out_st) {
                return CAPYRO_ERR_NO_MEMORY;
            }
            int e = setup_cover_video(c, os, o->video_codec);
            if (e != CAPYRO_OK) {
                return e;
            }
        } else {
            os->in_index = vi;
            os->in_st = c->in->streams[vi];
            os->off = av_rescale_q(off_us, AV_TIME_BASE_Q, os->in_st->time_base);
            if (!strcmp(o->video_codec, "copy")) {
                if (avformat_query_codec(ofmt, os->in_st->codecpar->codec_id, FF_COMPLIANCE_NORMAL) != 1) {
                    say(c, "El video %s no entra tal cual en .%s: elegí otro códec", avcodec_get_name(os->in_st->codecpar->codec_id),
                        ofmt->extensions ? ofmt->extensions : ofmt->name);
                    return CAPYRO_ERR_UNSUPPORTED;
                }
                os->copy = true;
                os->out_st = copy_stream(c, os->in_st);
                if (!os->out_st) {
                    return CAPYRO_ERR_NO_MEMORY;
                }
                snprintf(c->res->video_encoder, sizeof(c->res->video_encoder), "copiar");
            } else {
                os->out_st = avformat_new_stream(c->out, NULL);
                if (!os->out_st) {
                    return CAPYRO_ERR_NO_MEMORY;
                }
                int e = setup_video_encode(c, os, o->video_codec);
                if (e != CAPYRO_OK) {
                    return e;
                }
            }
        }
        if (!os->copy) {
            if (avcodec_parameters_from_context(os->out_st->codecpar, os->enc) < 0) {
                return CAPYRO_ERR_NO_MEMORY;
            }
            os->out_st->time_base = os->enc->time_base;
            os->out_st->avg_frame_rate = os->enc->framerate;
            /* H.265 en MP4/MOV: la etiqueta que entienden Apple y los celulares */
            if (os->enc->codec_id == AV_CODEC_ID_HEVC &&
                (!strcmp(ofmt->name, "mp4") || !strcmp(ofmt->name, "mov") || !strcmp(ofmt->name, "ipod"))) {
                os->out_st->codecpar->codec_tag = MKTAG('h', 'v', 'c', '1');
            }
            if (os->in_st) {
                av_dict_copy(&os->out_st->metadata, os->in_st->metadata, 0);
                av_dict_set(&os->out_st->metadata, "encoder", NULL, 0);
                av_dict_set(&os->out_st->metadata, "handler_name", NULL, 0);
            }
        }
    }

    /* --- audio --- */
    if (want_a) {
        OStream *os = &c->a;
        os->in_index = ai;
        os->in_st = c->in->streams[ai];
        os->off = av_rescale_q(off_us, AV_TIME_BASE_Q, os->in_st->time_base);
        if (!strcmp(o->audio_codec, "copy")) {
            if (avformat_query_codec(ofmt, os->in_st->codecpar->codec_id, FF_COMPLIANCE_NORMAL) != 1) {
                say(c, "El audio %s no entra tal cual en .%s: elegí otro códec", avcodec_get_name(os->in_st->codecpar->codec_id),
                    ofmt->extensions ? ofmt->extensions : ofmt->name);
                return CAPYRO_ERR_UNSUPPORTED;
            }
            os->copy = true;
            os->out_st = copy_stream(c, os->in_st);
            if (!os->out_st) {
                return CAPYRO_ERR_NO_MEMORY;
            }
            snprintf(c->res->audio_encoder, sizeof(c->res->audio_encoder), "copiar");
        } else {
            os->out_st = avformat_new_stream(c->out, NULL);
            if (!os->out_st) {
                return CAPYRO_ERR_NO_MEMORY;
            }
            int e = setup_audio_encode(c, os, o->audio_codec);
            if (e != CAPYRO_OK) {
                return e;
            }
            if (avcodec_parameters_from_context(os->out_st->codecpar, os->enc) < 0) {
                return CAPYRO_ERR_NO_MEMORY;
            }
            os->out_st->time_base = os->enc->time_base;
            av_dict_copy(&os->out_st->metadata, os->in_st->metadata, 0);
            av_dict_set(&os->out_st->metadata, "encoder", NULL, 0);
            av_dict_set(&os->out_st->metadata, "handler_name", NULL, 0);
            av_dict_set(&os->out_st->metadata, "METADATA_BLOCK_PICTURE", NULL, 0);
        }
    }

    /* --- subtítulos y adjuntos (fuentes de los ASS): si el formato los acepta --- */
    c->sub_map = av_malloc_array(c->in->nb_streams, sizeof(int));
    if (!c->sub_map) {
        return CAPYRO_ERR_NO_MEMORY;
    }
    for (unsigned i = 0; i < c->in->nb_streams; i++) {
        c->sub_map[i] = -1;
        const AVStream *st = c->in->streams[i];
        enum AVMediaType t = st->codecpar->codec_type;
        if (!want_v || cover_video || (t != AVMEDIA_TYPE_SUBTITLE && t != AVMEDIA_TYPE_ATTACHMENT)) {
            continue;
        }
        if (t == AVMEDIA_TYPE_ATTACHMENT && !cover_muxer_is_mkv(ofmt)) {
            continue;
        }
        if (t == AVMEDIA_TYPE_SUBTITLE && avformat_query_codec(ofmt, st->codecpar->codec_id, FF_COMPLIANCE_NORMAL) != 1) {
            continue;
        }
        AVStream *so = copy_stream(c, st);
        if (!so) {
            return CAPYRO_ERR_NO_MEMORY;
        }
        c->sub_map[i] = so->index;
    }

    /* --- etiquetas, capítulos y portada --- */
    if (o->keep_tags) {
        copy_tags(c);
        int e = copy_chapters(c);
        if (e != CAPYRO_OK) {
            return e;
        }
        if (!want_v) {
            e = add_audio_cover(c);
            if (e != CAPYRO_OK) {
                return e;
            }
        }
    }

    /* --- abrir la salida --- */
    if (!(ofmt->flags & AVFMT_NOFILE) && avio_open(&c->out->pb, out_path, AVIO_FLAG_WRITE) < 0) {
        say(c, "No pude crear el archivo de salida (¿la carpeta se puede escribir?)");
        return CAPYRO_ERR_WRITE;
    }
    AVDictionary *mux = NULL;
    if (!strcmp(ofmt->name, "mp4") || !strcmp(ofmt->name, "mov") || !strcmp(ofmt->name, "ipod")) {
        av_dict_set(&mux, "movflags", "+faststart", 0); /* empieza a verse antes de bajarse entero */
    }
    c->out->avoid_negative_ts = AVFMT_AVOID_NEG_TS_MAKE_ZERO;
    int herr = avformat_write_header(c->out, &mux);
    av_dict_free(&mux);
    if (herr < 0) {
        say(c, "Esa combinación de formato y códecs no se puede guardar");
        return CAPYRO_ERR_WRITE;
    }
    if (c->have_cover_pkt) {
        c->cover_pkt.stream_index = c->cover_out;
        c->cover_pkt.pts = c->cover_pkt.dts = 0;
        c->cover_pkt.flags |= AV_PKT_FLAG_KEY;
        if (av_interleaved_write_frame(c->out, &c->cover_pkt) < 0) {
            say(c, "No pude escribir la portada");
            return CAPYRO_ERR_WRITE;
        }
    }

    /* recortar desde el medio: saltar cerca del inicio */
    if (c->start > 0.0) {
        av_seek_frame(c->in, -1, off_us, AVSEEK_FLAG_BACKWARD);
    }

    report(c, true);
    /* --- leer, convertir, escribir --- */
    int ret = CAPYRO_OK;
    for (;;) {
        int r = av_read_frame(c->in, c->pkt);
        if (r < 0) {
            break; /* fin (o error de lectura: se cierra lo que hay) */
        }
        int idx = c->pkt->stream_index;
        if (idx == c->v.in_index && c->v.out_st && !cover_video) {
            if (c->v.copy) {
                ret = copy_packet(c, c->pkt, c->v.in_st, c->v.out_st, c->v.off, &c->v.past_end);
            } else {
                ret = decode(c, &c->v, c->pkt);
                if (c->dur > 0.0 && c->pkt->pts != AV_NOPTS_VALUE &&
                    (c->pkt->pts - c->v.off) * av_q2d(c->v.in_st->time_base) > c->dur + 2.0) {
                    c->v.past_end = true;
                }
            }
        } else if (idx == c->a.in_index && c->a.out_st) {
            if (c->a.copy) {
                ret = copy_packet(c, c->pkt, c->a.in_st, c->a.out_st, c->a.off, &c->a.past_end);
                if (ret == CAPYRO_OK && c->cover) {
                    ret = emit_cover(c, c->done_t);
                }
            } else {
                ret = decode(c, &c->a, c->pkt);
                if (c->dur > 0.0 && c->pkt->pts != AV_NOPTS_VALUE &&
                    (c->pkt->pts - c->a.off) * av_q2d(c->a.in_st->time_base) > c->dur + 2.0) {
                    c->a.past_end = true;
                }
            }
        } else if (idx >= 0 && (unsigned)idx < c->in->nb_streams && c->sub_map[idx] >= 0) {
            AVStream *in_st = c->in->streams[idx];
            ret = copy_packet(c, c->pkt, in_st, c->out->streams[c->sub_map[idx]],
                              av_rescale_q(off_us, AV_TIME_BASE_Q, in_st->time_base), NULL);
        }
        av_packet_unref(c->pkt);
        if (ret != CAPYRO_OK) {
            return ret;
        }
        if (!report(c, false)) {
            return CAPYRO_ERR_CANCELLED;
        }
        /* recortado: cuando todas las pistas pasaron el fin, no hace falta leer más */
        bool trimming = o->end > 0.0;
        if (trimming && (!c->v.out_st || cover_video || c->v.past_end) && (!c->a.out_st || c->a.past_end)) {
            break;
        }
    }

    /* --- vaciar decodificadores, filtros y codificadores --- */
    OStream *order[2] = { &c->a, &c->v };
    for (int k = 0; k < 2; k++) {
        OStream *os = order[k];
        if (!os->out_st || os->copy) {
            continue;
        }
        if (os->dec) {
            int e = decode(c, os, NULL);
            if (e != CAPYRO_OK) {
                return e;
            }
        }
        if (os->src) {
            if (av_buffersrc_add_frame(os->src, NULL) < 0) {
                say(c, "Los filtros no se pudieron vaciar");
                return CAPYRO_ERR_DECODE;
            }
            int e = drain_filters(c, os);
            if (e != CAPYRO_OK) {
                return e;
            }
        }
        if (os == &c->v && c->cover) {
            /* la imagen hasta el final del audio */
            int e = emit_cover(c, c->dur > 0.0 ? c->dur : c->done_t);
            if (e != CAPYRO_OK) {
                return e;
            }
        }
        int e = encode(c, os, NULL);
        if (e != CAPYRO_OK) {
            return e;
        }
    }
    if (c->cover && c->a.copy) {
        int e = emit_cover(c, c->dur > 0.0 ? c->dur : c->done_t);
        if (e == CAPYRO_OK) {
            e = encode(c, &c->v, NULL);
        }
        if (e != CAPYRO_OK) {
            return e;
        }
    }
    if (av_write_trailer(c->out) < 0) {
        say(c, "No pude terminar de escribir el archivo");
        return CAPYRO_ERR_WRITE;
    }
    c->done_t = c->dur;
    report(c, true);
    return c->cancelled ? CAPYRO_ERR_CANCELLED : CAPYRO_OK;
}

int capyro_convert(const char *in_path, const char *out_path, const CapyroConvertOptions *opt,
                   CapyroProgressFn progress, void *user, CapyroConvertResult *result) {
    CapyroConvertResult dummy;
    if (!result) {
        result = &dummy;
    }
    memset(result, 0, sizeof(*result));
    if (!in_path || !out_path || !opt || !in_path[0] || !out_path[0]) {
        return CAPYRO_ERR_INVALID_ARG;
    }
    if (!strcmp(in_path, out_path)) {
        snprintf(result->message, sizeof(result->message), "La salida no puede ser el mismo archivo");
        return CAPYRO_ERR_INVALID_ARG;
    }
    Conv c;
    memset(&c, 0, sizeof(c));
    c.opt = opt;
    c.res = result;
    c.progress = progress;
    c.user = user;
    c.v.in_index = c.a.in_index = -1;
    c.cover_out = -1;
    c.pkt = av_packet_alloc();
    c.frame = av_frame_alloc();
    c.filt = av_frame_alloc();
    int ret = (c.pkt && c.frame && c.filt) ? run(&c, in_path, out_path) : CAPYRO_ERR_NO_MEMORY;
    if (c.cancelled) {
        ret = CAPYRO_ERR_CANCELLED;
    }

    free_ostream(&c.v);
    free_ostream(&c.a);
    av_frame_free(&c.cover);
    av_packet_unref(&c.cover_pkt);
    av_free(c.sub_map);
    av_packet_free(&c.pkt);
    av_frame_free(&c.frame);
    av_frame_free(&c.filt);
    avformat_close_input(&c.in);
    bool opened = c.out && c.out->pb;
    if (c.out) {
        if (c.out->pb && !(c.out->oformat->flags & AVFMT_NOFILE)) {
            avio_closep(&c.out->pb);
        }
        avformat_free_context(c.out);
    }
    if (ret != CAPYRO_OK) {
        if (opened) {
            remove(out_path); /* nada de archivos a medias */
        }
        if (ret == CAPYRO_ERR_CANCELLED) {
            snprintf(result->message, sizeof(result->message), "Cancelado");
        } else if (!result->message[0]) {
            snprintf(result->message, sizeof(result->message), "No se pudo convertir (error %d)", ret);
        }
    }
    return ret;
}

/* ------------------------------------------------------------------ */
/* qué tiene un archivo                                                */
/* ------------------------------------------------------------------ */

/* Extensión para sacar el audio sin recodificar. */
static const char *copy_ext(enum AVCodecID id) {
    switch (id) {
    case AV_CODEC_ID_AAC:    return "m4a";
    case AV_CODEC_ID_ALAC:   return "m4a";
    case AV_CODEC_ID_MP3:    return "mp3";
    case AV_CODEC_ID_OPUS:   return "opus";
    case AV_CODEC_ID_VORBIS: return "ogg";
    case AV_CODEC_ID_FLAC:   return "flac";
    case AV_CODEC_ID_AC3:    return "ac3";
    case AV_CODEC_ID_EAC3:   return "eac3";
    default:
        if (id >= AV_CODEC_ID_PCM_S16LE && id <= AV_CODEC_ID_PCM_F64LE) {
            return "wav";
        }
        return "mka"; /* Matroska acepta casi todo */
    }
}

int capyro_media_probe(const char *path, CapyroMediaProbe *out) {
    if (!out) {
        return CAPYRO_ERR_INVALID_ARG;
    }
    memset(out, 0, sizeof(*out));
    if (!path || !path[0]) {
        return CAPYRO_ERR_INVALID_ARG;
    }
    AVFormatContext *fmt = NULL;
    if (avformat_open_input(&fmt, path, NULL, NULL) < 0) {
        return CAPYRO_ERR_NOT_FOUND;
    }
    if (avformat_find_stream_info(fmt, NULL) < 0) {
        avformat_close_input(&fmt);
        return CAPYRO_ERR_DECODE;
    }
    out->duration = fmt->duration > 0 ? (double)fmt->duration / AV_TIME_BASE : 0.0;
    out->bit_rate = fmt->bit_rate;
    int vi = best_video(fmt);
    if (vi >= 0) {
        const AVStream *st = fmt->streams[vi];
        out->has_video = true;
        int rot = stream_rotation(st);
        bool turn = rot == 90 || rot == 270;
        out->width = turn ? st->codecpar->height : st->codecpar->width;
        out->height = turn ? st->codecpar->width : st->codecpar->height;
        AVRational fr = av_guess_frame_rate(fmt, (AVStream *)st, NULL);
        out->fps = fr.num > 0 && fr.den > 0 ? av_q2d(fr) : 0.0;
        snprintf(out->video_codec, sizeof(out->video_codec), "%s", avcodec_get_name(st->codecpar->codec_id));
    }
    out->has_cover = attached_pic(fmt) >= 0;
    int ai = av_find_best_stream(fmt, AVMEDIA_TYPE_AUDIO, -1, -1, NULL, 0);
    if (ai >= 0) {
        const AVCodecParameters *p = fmt->streams[ai]->codecpar;
        out->has_audio = true;
        snprintf(out->audio_codec, sizeof(out->audio_codec), "%s", avcodec_get_name(p->codec_id));
        out->sample_rate = p->sample_rate;
        out->channels = p->ch_layout.nb_channels;
        out->bits = p->bits_per_raw_sample > 0 ? p->bits_per_raw_sample : 0;
        snprintf(out->audio_copy_ext, sizeof(out->audio_copy_ext), "%s", copy_ext(p->codec_id));
    }
    avformat_close_input(&fmt);
    return CAPYRO_OK;
}

/*
 * Capyro — reproducción: FFmpeg (demux + decode) y miniaudio (salida).
 *
 * Hilos:
 *   - GUI:            llama a la API pública (abrir, play, seek, pedir frames).
 *   - decodificación: lee paquetes, decodifica audio y video y llena las colas.
 *   - audio:          callback de miniaudio; consume el ring de audio y es el
 *                     reloj maestro (la posición sale de las muestras que sonaron).
 *
 * Un solo mutex (core->lock) protege todo lo compartido. Nunca se llama a
 * FFmpeg ni a miniaudio con el lock tomado.
 */
#if !defined(_WIN32) && !defined(_POSIX_C_SOURCE)
#  define _POSIX_C_SOURCE 200809L
#endif

#include "internal.h"

#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include <libavutil/channel_layout.h>
#include <libavutil/imgutils.h>
#include <libswresample/swresample.h>
#include <libswscale/swscale.h>

#include "miniaudio.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define MAX_VIDEO_W      1920
#define MAX_VIDEO_H      1080
#define MAX_COVER_SIDE   1024
#define AUDIO_AHEAD_SEC  0.5    /* cuánto audio decodificado mantener listo */
#define PKT_QUEUE_BYTES  (32u * 1024u * 1024u)
#define PKT_QUEUE_MIN    8      /* paquetes por stream que intentamos tener */

/* ================================================================== */
/* colas de paquetes (solo hilo de decodificación)                    */
/* ================================================================== */

static void pq_push(PktQueue *q, AVPacket *src) {
    PktNode *n = malloc(sizeof(*n));
    AVPacket *p = av_packet_alloc();
    if (!n || !p) {
        free(n);
        av_packet_free(&p);
        av_packet_unref(src);
        return;
    }
    av_packet_move_ref(p, src);
    n->pkt  = p;
    n->next = NULL;
    if (q->tail) {
        q->tail->next = n;
    } else {
        q->head = n;
    }
    q->tail = n;
    q->bytes += (size_t)p->size;
    q->count++;
}

static AVPacket *pq_pop(PktQueue *q) {
    PktNode *n = q->head;
    if (!n) {
        return NULL;
    }
    q->head = n->next;
    if (!q->head) {
        q->tail = NULL;
    }
    AVPacket *p = n->pkt;
    q->bytes -= (size_t)p->size;
    q->count--;
    free(n);
    return p;
}

static void pq_clear(PktQueue *q) {
    AVPacket *p;
    while ((p = pq_pop(q))) {
        av_packet_free(&p);
    }
}

/* ================================================================== */
/* audio: ring + callback                                             */
/* ================================================================== */

static void ring_clear_locked(CapyroCore *core) {
    core->ring_read = 0;
    core->ring_len  = 0;
}

static void ring_push_locked(CapyroCore *core, const float *src, size_t frames) {
    size_t space = core->ring_cap - core->ring_len;
    if (frames > space) {
        frames = space;
    }
    size_t w     = (core->ring_read + core->ring_len) % core->ring_cap;
    size_t first = frames < core->ring_cap - w ? frames : core->ring_cap - w;
    memcpy(core->ring + w * 2, src, first * 2 * sizeof(float));
    memcpy(core->ring, src + first * 2, (frames - first) * 2 * sizeof(float));
    core->ring_len += frames;
}

static void audio_callback(ma_device *dev, void *output, const void *input, ma_uint32 frames) {
    (void)input;
    CapyroCore *core = dev->pUserData;
    float *out = output;
    ma_uint32 done = 0;

    cp_mutex_lock(&core->lock);
    if (core->state == CAPYRO_STATE_PLAYING && core->clock_audio && !core->seek_req) {
        /* curva cuadrática: el control de volumen se siente más natural */
        float gain = core->muted ? 0.0f : core->volume * core->volume;
        while (done < frames && core->ring_len > 0) {
            size_t chunk = frames - done;
            size_t until_wrap = core->ring_cap - core->ring_read;
            if (chunk > core->ring_len) chunk = core->ring_len;
            if (chunk > until_wrap)     chunk = until_wrap;
            const float *src = core->ring + core->ring_read * 2;
            float *dst = out + (size_t)done * 2;
            for (size_t i = 0; i < chunk * 2; i++) {
                dst[i] = src[i] * gain;
            }
            core->ring_read = (core->ring_read + chunk) % core->ring_cap;
            core->ring_len -= chunk;
            done += (ma_uint32)chunk;
        }
        core->audio_consumed += done;
    }
    cp_mutex_unlock(&core->lock);

    if (done < frames) {
        memset(out + (size_t)done * 2, 0, (size_t)(frames - done) * 2 * sizeof(float));
    }
}

int player_global_init(CapyroCore *core) {
    av_log_set_level(AV_LOG_FATAL); /* los errores los reportamos nosotros */

    ma_device *dev = calloc(1, sizeof(*dev));
    if (!dev) {
        return CAPYRO_ERR_NO_MEMORY;
    }
    ma_device_config cfg   = ma_device_config_init(ma_device_type_playback);
    cfg.playback.format    = ma_format_f32;
    cfg.playback.channels  = 2;
    cfg.sampleRate         = 0; /* la nativa del dispositivo: FFmpeg remuestrea */
    cfg.dataCallback       = audio_callback;
    cfg.pUserData          = core;

    if (ma_device_init(NULL, &cfg, dev) != MA_SUCCESS) {
        free(dev);
        fprintf(stderr, "capyro: no hay dispositivo de audio, sigo sin sonido\n");
        return CAPYRO_OK;
    }

    core->out_rate = dev->sampleRate;
    core->ring_cap = (size_t)core->out_rate * 2; /* 2 s */
    core->ring     = malloc(core->ring_cap * 2 * sizeof(float));
    if (!core->ring) {
        ma_device_uninit(dev);
        free(dev);
        return CAPYRO_ERR_NO_MEMORY;
    }
    core->device   = dev;
    core->audio_ok = true;
    snprintf(core->backend, sizeof(core->backend), "%s",
             ma_get_backend_name(dev->pContext->backend));
    return CAPYRO_OK;
}

void player_global_shutdown(CapyroCore *core) {
    player_close(core);
    if (core->device) {
        ma_device_uninit(core->device); /* para el dispositivo y espera al callback */
        free(core->device);
        core->device = NULL;
    }
    free(core->ring);
    core->ring = NULL;
    for (int i = 0; i < VQ_SLOTS; i++) {
        free(core->slots[i].data);
        core->slots[i].data = NULL;
        core->slots[i].cap  = 0;
    }
    free(core->player.abuf);
    core->player.abuf = NULL;
}

/* El dispositivo se arranca recién en el primer play (y nunca con el lock
 * tomado: ma_device_start espera al hilo de audio, que pide el lock). */
void player_ensure_device_started(CapyroCore *core) {
    bool start = false;
    cp_mutex_lock(&core->lock);
    if (core->audio_ok && !core->device_started) {
        core->device_started = true;
        start = true;
    }
    cp_mutex_unlock(&core->lock);
    if (start && ma_device_start(core->device) != MA_SUCCESS) {
        fprintf(stderr, "capyro: no se pudo arrancar el dispositivo de audio\n");
    }
}

/* ================================================================== */
/* reloj                                                              */
/* ================================================================== */

double player_clock_locked(CapyroCore *core) {
    if (core->clock_audio) {
        return core->clock_base + (double)core->audio_consumed / (double)core->out_rate;
    }
    if (core->state == CAPYRO_STATE_PLAYING) {
        return core->clock_base + (cp_now() - core->wall_started);
    }
    return core->clock_base;
}

/* Cambia del reloj de audio al de pared sin saltos (cuando el audio se
 * terminó antes que el video). */
static void clock_to_wall_locked(CapyroCore *core) {
    double now_pos       = player_clock_locked(core);
    core->clock_audio    = false;
    core->clock_base     = now_pos;
    core->wall_started   = cp_now();
    core->audio_consumed = 0;
}

/* ================================================================== */
/* video: cola de frames                                              */
/* ================================================================== */

static void vq_clear_locked(CapyroCore *core) {
    core->vq_head = 0;
    core->vq_len  = 0;
}

/* Pasa a "visible" el frame más nuevo cuyo tiempo ya llegó. */
static void advance_video_locked(CapyroCore *core) {
    double clock = player_clock_locked(core);
    while (core->vq_len > 0) {
        int idx = core->vq[core->vq_head];
        if (!core->force_frame && core->slots[idx].pts > clock + 0.005) {
            break;
        }
        core->shown       = idx;
        core->vq_head     = (core->vq_head + 1) % VQ_SLOTS;
        core->vq_len--;
        core->serial++;
        core->force_frame = false;
    }
}

static int free_slot_locked(CapyroCore *core) {
    for (int s = 0; s < VQ_SLOTS; s++) {
        if (s == core->shown) {
            continue;
        }
        bool used = false;
        for (int i = 0; i < core->vq_len; i++) {
            if (core->vq[(core->vq_head + i) % VQ_SLOTS] == s) {
                used = true;
                break;
            }
        }
        if (!used) {
            return s;
        }
    }
    return -1;
}

/* Convierte `frame` a RGBA dentro de `slot`, achicando si no entra en max_w×max_h. */
static bool convert_frame(Player *p, AVFrame *frame, VideoSlot *slot, int max_w, int max_h) {
    int w = frame->width, h = frame->height;
    if (w <= 0 || h <= 0) {
        return false;
    }
    double scale = 1.0;
    if (w > max_w) scale = (double)max_w / w;
    if (h * scale > max_h) scale = (double)max_h / h;
    int dw = (int)(w * scale + 0.5), dh = (int)(h * scale + 0.5);
    if (dw < 1) dw = 1;
    if (dh < 1) dh = 1;

    p->sws = sws_getCachedContext(p->sws, w, h, (enum AVPixelFormat)frame->format,
                                  dw, dh, AV_PIX_FMT_RGBA, SWS_BILINEAR, NULL, NULL, NULL);
    if (!p->sws) {
        return false;
    }
    size_t need = (size_t)dw * (size_t)dh * 4;
    if (slot->cap < need) {
        uint8_t *grown = realloc(slot->data, need);
        if (!grown) {
            return false;
        }
        slot->data = grown;
        slot->cap  = need;
    }
    uint8_t *dst[4]    = { slot->data, NULL, NULL, NULL };
    int      stride[4] = { dw * 4, 0, 0, 0 };
    sws_scale(p->sws, (const uint8_t *const *)frame->data, frame->linesize, 0, h, dst, stride);
    slot->w = dw;
    slot->h = dh;
    return true;
}

static float display_aspect(AVFormatContext *fmt, AVStream *st, AVFrame *frame) {
    AVRational sar = av_guess_sample_aspect_ratio(fmt, st, frame);
    double a = (double)frame->width / (double)frame->height;
    if (sar.num > 0 && sar.den > 0) {
        a *= av_q2d(sar);
    }
    return (float)a;
}

/* Carátula de un mp3 (stream "attached_pic"): se decodifica una sola vez. */
static void decode_cover(CapyroCore *core, AVStream *st) {
    Player *p = &core->player;
    const AVCodec *codec = avcodec_find_decoder(st->codecpar->codec_id);
    if (!codec) {
        return;
    }
    AVCodecContext *ctx = avcodec_alloc_context3(codec);
    AVFrame *frame = av_frame_alloc();
    if (!ctx || !frame || avcodec_parameters_to_context(ctx, st->codecpar) < 0 ||
        avcodec_open2(ctx, codec, NULL) < 0) {
        goto out;
    }
    if (avcodec_send_packet(ctx, &st->attached_pic) < 0) {
        goto out;
    }
    avcodec_send_packet(ctx, NULL);
    if (avcodec_receive_frame(ctx, frame) == 0) {
        VideoSlot *slot = &core->slots[0];
        if (convert_frame(p, frame, slot, MAX_COVER_SIDE, MAX_COVER_SIDE)) {
            slot->pts    = 0.0;
            slot->aspect = display_aspect(p->fmt, st, frame);
            cp_mutex_lock(&core->lock);
            core->shown = 0;
            core->serial++;
            cp_mutex_unlock(&core->lock);
        }
    }
out:
    av_frame_free(&frame);
    avcodec_free_context(&ctx);
}

/* ================================================================== */
/* pasos del hilo de decodificación                                   */
/* ================================================================== */

static double frame_time(Player *p, int stream, AVFrame *f) {
    int64_t ts = f->best_effort_timestamp;
    if (ts == AV_NOPTS_VALUE) {
        return -1.0;
    }
    return (double)ts * av_q2d(p->fmt->streams[stream]->time_base) - p->start_time;
}

static bool demux_one(CapyroCore *core) {
    Player *p = &core->player;
    int ret = av_read_frame(p->fmt, p->pkt);
    if (ret < 0) {
        p->demux_eof = true; /* EOF o error de lectura: lo tratamos igual */
        return true;
    }
    if (p->pkt->stream_index == p->aidx) {
        pq_push(&p->aq, p->pkt);
    } else if (p->pkt->stream_index == p->vidx) {
        pq_push(&p->vq, p->pkt);
    } else {
        av_packet_unref(p->pkt);
    }
    return true;
}

/* Devuelve: 1 = obtuvo frame, 0 = hizo algo, -1 = nada que hacer. */
static int pull_frame(AVCodecContext *ctx, PktQueue *q, AVFrame *frame,
                      bool demux_eof, bool *flushed, bool *done) {
    int ret = avcodec_receive_frame(ctx, frame);
    if (ret == 0) {
        return 1;
    }
    if (ret == AVERROR(EAGAIN)) {
        AVPacket *pkt = pq_pop(q);
        if (pkt) {
            avcodec_send_packet(ctx, pkt); /* paquetes corruptos: se ignoran */
            av_packet_free(&pkt);
            return 0;
        }
        if (demux_eof && !*flushed) {
            avcodec_send_packet(ctx, NULL); /* vaciar el decodificador */
            *flushed = true;
            return 0;
        }
        return -1;
    }
    *done = true; /* AVERROR_EOF u otro error: este stream terminó */
    return -1;
}

static bool audio_step(CapyroCore *core) {
    Player *p = &core->player;
    int r = pull_frame(p->actx, &p->aq, p->frame, p->demux_eof, &p->a_flushed, &p->a_done);
    if (r < 1) {
        return r == 0;
    }

    AVFrame *f = p->frame;
    double t = frame_time(p, p->aidx, f);
    int out_max = swr_get_out_samples(p->swr, f->nb_samples);
    if (out_max > p->abuf_frames) {
        float *grown = realloc(p->abuf, (size_t)out_max * 2 * sizeof(float));
        if (!grown) {
            av_frame_unref(f);
            return true;
        }
        p->abuf        = grown;
        p->abuf_frames = out_max;
    }
    uint8_t *out[1] = { (uint8_t *)p->abuf };
    int n = swr_convert(p->swr, out, out_max, (const uint8_t **)f->extended_data, f->nb_samples);
    av_frame_unref(f);
    if (n <= 0) {
        return true;
    }

    /* Después de un seek: tirar lo que quedó antes del punto pedido. */
    int skip = 0;
    if (p->a_drop_until >= 0.0 && t >= 0.0) {
        double s = (p->a_drop_until - t) * core->out_rate;
        if (s >= n) {
            return true;
        }
        skip = s > 0 ? (int)s : 0;
        p->a_drop_until = -1.0;
    }

    cp_mutex_lock(&core->lock);
    if (!core->seek_req) {
        ring_push_locked(core, p->abuf + (size_t)skip * 2, (size_t)(n - skip));
    }
    cp_mutex_unlock(&core->lock);
    return true;
}

static bool video_step(CapyroCore *core) {
    Player *p = &core->player;
    int r = pull_frame(p->vctx, &p->vq, p->frame, p->demux_eof, &p->v_flushed, &p->v_done);
    if (r < 1) {
        return r == 0;
    }

    AVFrame *f = p->frame;
    AVStream *st = p->fmt->streams[p->vidx];
    double t = frame_time(p, p->vidx, f);
    if (p->v_drop_until >= 0.0 && t >= 0.0) {
        AVRational fr = st->avg_frame_rate;
        double frame_dur = (fr.num > 0 && fr.den > 0) ? 1.0 / av_q2d(fr) : 0.04;
        if (t + frame_dur <= p->v_drop_until) {
            av_frame_unref(f);
            return true;
        }
        p->v_drop_until = -1.0;
    }

    cp_mutex_lock(&core->lock);
    int idx = free_slot_locked(core);
    cp_mutex_unlock(&core->lock);
    if (idx < 0) {
        av_frame_unref(f);
        return true;
    }

    /* El slot libre no lo ve nadie más: se puede escribir sin lock. */
    VideoSlot *slot = &core->slots[idx];
    bool ok = convert_frame(p, f, slot, MAX_VIDEO_W, MAX_VIDEO_H);
    slot->pts    = t < 0 ? 0.0 : t;
    slot->aspect = display_aspect(p->fmt, st, f);
    av_frame_unref(f);

    if (ok) {
        cp_mutex_lock(&core->lock);
        if (!core->seek_req && core->vq_len < VQ_MAX) {
            core->vq[(core->vq_head + core->vq_len) % VQ_SLOTS] = idx;
            core->vq_len++;
        }
        cp_mutex_unlock(&core->lock);
    }
    return true;
}

static void do_seek(CapyroCore *core, double target, uint64_t serial) {
    Player *p = &core->player;
    int64_t ts = (int64_t)((target + p->start_time) * AV_TIME_BASE);
    av_seek_frame(p->fmt, -1, ts, AVSEEK_FLAG_BACKWARD);

    pq_clear(&p->aq);
    pq_clear(&p->vq);
    if (p->actx) {
        avcodec_flush_buffers(p->actx);
        swr_close(p->swr);
        swr_init(p->swr);
    }
    if (p->vctx) {
        avcodec_flush_buffers(p->vctx);
    }
    p->demux_eof = p->a_flushed = p->v_flushed = p->a_done = p->v_done = false;
    p->a_drop_until = p->v_drop_until = target;
    p->seek_serial_done = serial;

    cp_mutex_lock(&core->lock);
    if (core->seek_serial == serial) { /* si llegó otro seek, lo atiende la próxima vuelta */
        core->seek_req  = false;
        core->media_eof = false;
        ring_clear_locked(core);
        vq_clear_locked(core);
    }
    cp_mutex_unlock(&core->lock);
}

static void decode_thread(void *arg) {
    CapyroCore *core = arg;
    Player *p = &core->player;

    for (;;) {
        cp_mutex_lock(&core->lock);
        bool     quit    = core->quit;
        bool     seek    = core->seek_req && core->seek_serial != p->seek_serial_done;
        double   target  = core->seek_target;
        uint64_t serial  = core->seek_serial;
        size_t   ring    = core->ring_len;
        int      vq_len  = core->vq_len;
        /* Si la GUI no está pidiendo frames (ventana minimizada), igual hay
         * que dejar pasar los viejos: si no, el video frena al audio. */
        advance_video_locked(core);
        vq_len = core->vq_len;
        /* El audio terminó pero el video sigue: el reloj pasa a ser de pared. */
        if (core->clock_audio && p->a_done && ring == 0) {
            clock_to_wall_locked(core);
        }
        cp_mutex_unlock(&core->lock);

        if (quit) {
            break;
        }
        if (seek) {
            do_seek(core, target, serial);
            continue;
        }

        bool worked = false;

        bool want_audio = p->aidx >= 0 && !p->a_done && ring < (size_t)(core->out_rate * AUDIO_AHEAD_SEC);
        bool want_video = p->vidx >= 0 && !p->v_done && vq_len < VQ_MAX;

        if (!p->demux_eof && p->aq.bytes + p->vq.bytes < PKT_QUEUE_BYTES &&
            ((p->aidx >= 0 && p->aq.count < PKT_QUEUE_MIN) ||
             (p->vidx >= 0 && p->vq.count < PKT_QUEUE_MIN) ||
             (p->aidx < 0 && p->vidx < 0))) {
            worked |= demux_one(core);
        }
        if (want_audio) {
            worked |= audio_step(core);
        }
        if (want_video) {
            worked |= video_step(core);
        }

        if (!worked) {
            bool all_done = p->demux_eof && (p->aidx < 0 || p->a_done) && (p->vidx < 0 || p->v_done);
            if (all_done) {
                cp_mutex_lock(&core->lock);
                if (!core->seek_req) {
                    core->media_eof = true;
                }
                cp_mutex_unlock(&core->lock);
            }
            cp_sleep_ms(all_done ? 15 : 4);
        }
    }
}

/* ================================================================== */
/* abrir / cerrar                                                     */
/* ================================================================== */

static void copy_tag(char *dst, size_t cap, AVDictionary *d1, AVDictionary *d2, const char *key) {
    AVDictionaryEntry *e = av_dict_get(d1, key, NULL, 0);
    if (!e && d2) {
        e = av_dict_get(d2, key, NULL, 0);
    }
    snprintf(dst, cap, "%s", e ? e->value : "");
}

static AVCodecContext *open_decoder(AVStream *st, bool threads) {
    const AVCodec *codec = avcodec_find_decoder(st->codecpar->codec_id);
    if (!codec) {
        return NULL;
    }
    AVCodecContext *ctx = avcodec_alloc_context3(codec);
    if (!ctx) {
        return NULL;
    }
    if (threads) {
        ctx->thread_count = 0; /* automático */
    }
    if (avcodec_parameters_to_context(ctx, st->codecpar) < 0 || avcodec_open2(ctx, codec, NULL) < 0) {
        avcodec_free_context(&ctx);
        return NULL;
    }
    ctx->pkt_timebase = st->time_base;
    return ctx;
}

static SwrContext *open_resampler(AVCodecContext *actx, unsigned out_rate) {
    AVChannelLayout in_layout;
    if (actx->ch_layout.order == AV_CHANNEL_ORDER_UNSPEC) {
        av_channel_layout_default(&in_layout, actx->ch_layout.nb_channels);
    } else if (av_channel_layout_copy(&in_layout, &actx->ch_layout) < 0) {
        return NULL;
    }
    AVChannelLayout out_layout = AV_CHANNEL_LAYOUT_STEREO;
    SwrContext *swr = NULL;
    int ret = swr_alloc_set_opts2(&swr, &out_layout, AV_SAMPLE_FMT_FLT, (int)out_rate,
                                  &in_layout, actx->sample_fmt, actx->sample_rate, 0, NULL);
    av_channel_layout_uninit(&in_layout);
    if (ret < 0 || swr_init(swr) < 0) {
        swr_free(&swr);
        return NULL;
    }
    return swr;
}

static void free_player(Player *p) {
    pq_clear(&p->aq);
    pq_clear(&p->vq);
    avcodec_free_context(&p->actx);
    avcodec_free_context(&p->vctx);
    swr_free(&p->swr);
    sws_freeContext(p->sws);
    p->sws = NULL;
    av_frame_free(&p->frame);
    av_packet_free(&p->pkt);
    avformat_close_input(&p->fmt);
    p->aidx = p->vidx = -1;
}

void player_close(CapyroCore *core) {
    Player *p = &core->player;
    if (p->thread_running) {
        cp_mutex_lock(&core->lock);
        core->quit = true;
        cp_mutex_unlock(&core->lock);
        cp_thread_join(p->thread);
        p->thread_running = false;
    }
    free_player(p);

    cp_mutex_lock(&core->lock);
    core->quit        = false;
    core->seek_req    = false;
    core->media_eof   = false;
    core->clock_audio = false;
    core->clock_base  = 0.0;
    core->audio_consumed = 0;
    core->duration    = 0.0;
    core->shown       = -1;
    core->force_frame = false;
    core->serial++;
    core->title[0]  = '\0';
    core->artist[0] = '\0';
    if (core->ring) {
        ring_clear_locked(core);
    }
    vq_clear_locked(core);
    cp_mutex_unlock(&core->lock);
}

int player_open(CapyroCore *core, const char *path) {
    player_close(core);
    Player *p = &core->player;
    memset(p, 0, offsetof(Player, abuf)); /* conserva el buffer de audio */
    p->aidx = p->vidx = -1;

    if (avformat_open_input(&p->fmt, path, NULL, NULL) < 0) {
        return CAPYRO_ERR_DECODE;
    }
    if (avformat_find_stream_info(p->fmt, NULL) < 0) {
        free_player(p);
        return CAPYRO_ERR_DECODE;
    }

    int aidx = av_find_best_stream(p->fmt, AVMEDIA_TYPE_AUDIO, -1, -1, NULL, 0);
    int vidx = av_find_best_stream(p->fmt, AVMEDIA_TYPE_VIDEO, -1, -1, NULL, 0);
    if (aidx < 0 && vidx < 0) {
        free_player(p);
        return CAPYRO_ERR_NO_STREAMS;
    }

    p->frame = av_frame_alloc();
    p->pkt   = av_packet_alloc();
    if (!p->frame || !p->pkt) {
        free_player(p);
        return CAPYRO_ERR_NO_MEMORY;
    }

    /* Audio: solo si hay dispositivo; si no, el reloj es de pared. */
    if (aidx >= 0 && core->audio_ok) {
        p->actx = open_decoder(p->fmt->streams[aidx], false);
        if (p->actx) {
            p->swr = open_resampler(p->actx, core->out_rate);
        }
        if (p->actx && p->swr) {
            p->aidx = aidx;
        } else {
            avcodec_free_context(&p->actx);
            swr_free(&p->swr);
            fprintf(stderr, "capyro: %s: no se pudo abrir el audio\n", path);
        }
    }

    bool real_video = false;
    if (vidx >= 0) {
        AVStream *st = p->fmt->streams[vidx];
        if (st->disposition & AV_DISPOSITION_ATTACHED_PIC) {
            decode_cover(core, st);
        } else {
            p->vctx = open_decoder(st, true);
            if (p->vctx) {
                p->vidx    = vidx;
                real_video = true;
            } else {
                fprintf(stderr, "capyro: %s: no se pudo abrir el video\n", path);
            }
        }
    }
    if (p->aidx < 0 && p->vidx < 0 && aidx < 0) {
        free_player(p);
        return CAPYRO_ERR_DECODE;
    }

    p->start_time   = p->fmt->start_time != AV_NOPTS_VALUE ? (double)p->fmt->start_time / AV_TIME_BASE : 0.0;
    p->a_drop_until = p->v_drop_until = -1.0;

    cp_mutex_lock(&core->lock);
    core->kind        = real_video ? CAPYRO_MEDIA_VIDEO : CAPYRO_MEDIA_AUDIO;
    core->duration    = p->fmt->duration != AV_NOPTS_VALUE ? (double)p->fmt->duration / AV_TIME_BASE : 0.0;
    core->clock_audio = p->aidx >= 0;
    core->clock_base  = 0.0;
    core->audio_consumed = 0;
    core->wall_started   = cp_now();
    core->force_frame    = real_video; /* mostrar el primer frame aunque no esté sonando */
    copy_tag(core->title, sizeof(core->title), p->fmt->metadata,
             aidx >= 0 ? p->fmt->streams[aidx]->metadata : NULL, "title");
    copy_tag(core->artist, sizeof(core->artist), p->fmt->metadata,
             aidx >= 0 ? p->fmt->streams[aidx]->metadata : NULL, "artist");
    cp_mutex_unlock(&core->lock);

    if (cp_thread_start(&p->thread, decode_thread, core) != 0) {
        free_player(p);
        return CAPYRO_ERR_THREAD;
    }
    p->thread_running = true;
    return CAPYRO_OK;
}

/* ================================================================== */
/* seek / fin                                                         */
/* ================================================================== */

void player_request_seek_locked(CapyroCore *core, double target) {
    core->seek_req       = true;
    core->seek_serial++;
    core->seek_target    = target;
    core->media_eof      = false;
    core->clock_audio    = core->player.aidx >= 0;
    core->clock_base     = target;
    core->audio_consumed = 0;
    core->wall_started   = cp_now();
    core->force_frame    = core->kind == CAPYRO_MEDIA_VIDEO;
    if (core->ring) {
        ring_clear_locked(core);
    }
    vq_clear_locked(core);
}

bool player_ended_locked(CapyroCore *core) {
    if (core->seek_req || !core->media_eof) {
        return false;
    }
    advance_video_locked(core);
    if (core->ring_len > 0 || core->vq_len > 0) {
        return false;
    }
    /* Sin audio ni video que marquen el ritmo (p. ej. mp3 sin dispositivo
     * de audio): termina cuando el reloj de pared llega a la duración. */
    if (!core->clock_audio && core->player.vidx < 0 && core->duration > 0.0) {
        return player_clock_locked(core) >= core->duration;
    }
    return true;
}

/* ================================================================== */
/* frames para la GUI                                                 */
/* ================================================================== */

bool capyro_core_lock_video_frame(CapyroCore *core, CapyroVideoFrame *out) {
    if (!core || !out) {
        return false;
    }
    cp_mutex_lock(&core->lock);
    advance_video_locked(core);
    if (core->shown < 0) {
        cp_mutex_unlock(&core->lock);
        return false;
    }
    const VideoSlot *s = &core->slots[core->shown];
    out->rgba   = s->data;
    out->width  = s->w;
    out->height = s->h;
    out->stride = s->w * 4;
    out->aspect = s->aspect;
    out->serial = core->serial;
    return true; /* el lock queda tomado hasta unlock_video_frame */
}

void capyro_core_unlock_video_frame(CapyroCore *core) {
    if (core) {
        cp_mutex_unlock(&core->lock);
    }
}

/*
 * Capyro — efectos de sonido.
 *
 * Decodifica un archivo corto que está en memoria (la GUI los trae embebidos
 * en el binario) a PCM float estéreo a la frecuencia del dispositivo, y lo
 * deja listo para que el callback de audio lo mezcle (ver player.c).
 */
#ifndef _POSIX_C_SOURCE
#  define _POSIX_C_SOURCE 200809L
#endif

#include "internal.h"

#include <libavcodec/avcodec.h>
#include <libavfilter/avfilter.h>
#include <libavfilter/buffersink.h>
#include <libavfilter/buffersrc.h>
#include <libavformat/avformat.h>

#include <stdlib.h>
#include <string.h>

#define AVIO_BUF 4096

/* Lectores de memoria para AVIO. */
int mem_read(void *opaque, uint8_t *buf, int n) {
    MemReader *m = opaque;
    size_t left = m->size - m->pos;
    if (left == 0) {
        return AVERROR_EOF;
    }
    size_t k = (size_t)n < left ? (size_t)n : left;
    memcpy(buf, m->data + m->pos, k);
    m->pos += k;
    return (int)k;
}

int64_t mem_seek(void *opaque, int64_t offset, int whence) {
    MemReader *m = opaque;
    int64_t base;
    switch (whence & ~AVSEEK_FORCE) {
    case AVSEEK_SIZE: return (int64_t)m->size;
    case SEEK_SET:    base = 0; break;
    case SEEK_CUR:    base = (int64_t)m->pos; break;
    case SEEK_END:    base = (int64_t)m->size; break;
    default:          return -1;
    }
    int64_t to = base + offset;
    if (to < 0 || to > (int64_t)m->size) {
        return -1;
    }
    m->pos = (size_t)to;
    return to;
}

typedef struct {
    float  *buf;
    size_t  len; /* frames */
    size_t  cap;
} PcmBuf;

static bool pcm_append(PcmBuf *b, const float *src, size_t frames) {
    if (b->len + frames > b->cap) {
        size_t cap = b->cap ? b->cap * 2 : 48000;
        while (cap < b->len + frames) {
            cap *= 2;
        }
        float *grown = realloc(b->buf, cap * 2 * sizeof(float));
        if (!grown) {
            return false;
        }
        b->buf = grown;
        b->cap = cap;
    }
    memcpy(b->buf + b->len * 2, src, frames * 2 * sizeof(float));
    b->len += frames;
    return true;
}

static void drain(AVFilterContext *sink, AVFrame *tmp, PcmBuf *out) {
    while (av_buffersink_get_frame(sink, tmp) >= 0) {
        pcm_append(out, (const float *)tmp->data[0], (size_t)tmp->nb_samples);
        av_frame_unref(tmp);
    }
}

int capyro_core_sfx_load(CapyroCore *core, int slot, const uint8_t *data, size_t size) {
    if (!core || !data || size == 0 || slot < 0 || slot >= CAPYRO_SFX_SLOTS) {
        return CAPYRO_ERR_INVALID_ARG;
    }
    if (!core->audio_ok) {
        return CAPYRO_OK; /* sin dispositivo no hay nada que cargar */
    }

    MemReader mr = { data, size, 0 };
    AVFormatContext *fmt   = NULL;
    AVCodecContext  *ctx   = NULL;
    AVFilterGraph   *graph = NULL;
    AVFilterContext *src = NULL, *sink = NULL;
    AVPacket *pkt   = av_packet_alloc();
    AVFrame  *frame = av_frame_alloc();
    AVFrame  *tmp   = av_frame_alloc();
    PcmBuf    pcm   = { 0 };
    int       err   = CAPYRO_ERR_DECODE;

    uint8_t *iobuf = av_malloc(AVIO_BUF);
    AVIOContext *avio = iobuf ? avio_alloc_context(iobuf, AVIO_BUF, 0, &mr, mem_read, NULL, mem_seek) : NULL;
    fmt = avformat_alloc_context();
    if (!pkt || !frame || !tmp || !avio || !fmt) {
        if (!avio) {
            av_free(iobuf);
        }
        err = CAPYRO_ERR_NO_MEMORY;
        goto out;
    }
    fmt->pb     = avio;
    fmt->flags |= AVFMT_FLAG_CUSTOM_IO;
    if (avformat_open_input(&fmt, NULL, NULL, NULL) < 0) {
        fmt = NULL; /* avformat_open_input lo libera si falla */
        goto out;
    }
    if (avformat_find_stream_info(fmt, NULL) < 0) {
        goto out;
    }
    int idx = av_find_best_stream(fmt, AVMEDIA_TYPE_AUDIO, -1, -1, NULL, 0);
    if (idx < 0) {
        err = CAPYRO_ERR_NO_STREAMS;
        goto out;
    }
    const AVCodec *codec = avcodec_find_decoder(fmt->streams[idx]->codecpar->codec_id);
    ctx = codec ? avcodec_alloc_context3(codec) : NULL;
    if (!ctx || avcodec_parameters_to_context(ctx, fmt->streams[idx]->codecpar) < 0 ||
        avcodec_open2(ctx, codec, NULL) < 0) {
        goto out;
    }

    bool eof = false;
    while (!eof) {
        if (av_read_frame(fmt, pkt) < 0) {
            eof = true;
            avcodec_send_packet(ctx, NULL);
        } else {
            if (pkt->stream_index == idx) {
                avcodec_send_packet(ctx, pkt);
            }
            av_packet_unref(pkt);
        }
        while (avcodec_receive_frame(ctx, frame) == 0) {
            if (!graph && !audio_graph_create(frame, 1.0, 0.0, core->out_rate, &graph, &src, &sink)) {
                goto out;
            }
            if (av_buffersrc_add_frame(src, frame) >= 0) {
                drain(sink, tmp, &pcm);
            }
            av_frame_unref(frame);
        }
    }
    if (graph && av_buffersrc_add_frame(src, NULL) >= 0) {
        drain(sink, tmp, &pcm);
    }
    if (pcm.len == 0) {
        goto out;
    }

    /* Cambiar el buffer del slot (si estaba sonando ese, se corta). */
    cp_mutex_lock(&core->lock);
    float *old = core->sfx[slot];
    core->sfx[slot]     = pcm.buf;
    core->sfx_len[slot] = pcm.len;
    if (core->sfx_playing == slot) {
        core->sfx_playing = -1;
    }
    cp_mutex_unlock(&core->lock);
    free(old);
    pcm.buf = NULL;
    err = CAPYRO_OK;

out:
    free(pcm.buf);
    avfilter_graph_free(&graph);
    avcodec_free_context(&ctx);
    avformat_close_input(&fmt);
    if (avio) {
        av_freep(&avio->buffer);
        avio_context_free(&avio);
    }
    av_frame_free(&frame);
    av_frame_free(&tmp);
    av_packet_free(&pkt);
    return err;
}

void capyro_core_sfx_play(CapyroCore *core, int slot) {
    if (!core || slot < 0 || slot >= CAPYRO_SFX_SLOTS) {
        return;
    }
    bool ok = false;
    cp_mutex_lock(&core->lock);
    if (core->sfx[slot]) {
        core->sfx_playing = slot; /* uno a la vez: el nuevo pisa al anterior */
        core->sfx_pos     = 0;
        ok = true;
    }
    cp_mutex_unlock(&core->lock);
    if (ok) {
        player_ensure_device_started(core);
    }
}

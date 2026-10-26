/*
 * Capyro — música de fondo (la del tutorial).
 *
 * A diferencia de los efectos (que se decodifican enteros), la música dura
 * minutos: se decodifica de a poco en un hilo propio a un ring de ~2 s y el
 * callback de audio la mezcla. Con loop vuelve a empezar sola. Entra y sale
 * con un fundido.
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

#define MUSIC_AVIO_BUF 8192
#define MUSIC_RING_SEC 2

/* Empuja lo que haya en el sink al ring; espera si está lleno.
 * Devuelve false si hay que terminar. */
static bool push_pcm(CapyroCore *core, const float *src, size_t frames) {
    Music *m = &core->music;
    while (frames > 0) {
        cp_mutex_lock(&core->lock);
        if (m->quit) {
            cp_mutex_unlock(&core->lock);
            return false;
        }
        size_t space = m->ring_cap - m->ring_len;
        size_t n = frames < space ? frames : space;
        if (n > 0) {
            size_t w = (m->ring_read + m->ring_len) % m->ring_cap;
            size_t first = n < m->ring_cap - w ? n : m->ring_cap - w;
            memcpy(m->ring + w * 2, src, first * 2 * sizeof(float));
            memcpy(m->ring, src + first * 2, (n - first) * 2 * sizeof(float));
            m->ring_len += n;
        }
        cp_mutex_unlock(&core->lock);
        src += n * 2;
        frames -= n;
        if (frames > 0) {
            cp_sleep_ms(20); /* lleno: esperar a que el callback consuma */
        }
    }
    return true;
}

static void music_thread(void *arg) {
    CapyroCore *core = arg;
    Music *m = &core->music;
    MemReader mr = { m->data, m->size, 0 };
    AVFormatContext *fmt   = NULL;
    AVCodecContext  *ctx   = NULL;
    AVFilterGraph   *graph = NULL;
    AVFilterContext *src = NULL, *sink = NULL;
    AVPacket *pkt   = av_packet_alloc();
    AVFrame  *frame = av_frame_alloc();
    AVFrame  *tmp   = av_frame_alloc();
    uint8_t  *iobuf = av_malloc(MUSIC_AVIO_BUF);
    AVIOContext *avio = iobuf ? avio_alloc_context(iobuf, MUSIC_AVIO_BUF, 0, &mr, mem_read, NULL, mem_seek) : NULL;
    int idx = -1;

    fmt = avformat_alloc_context();
    if (!pkt || !frame || !tmp || !avio || !fmt) {
        if (!avio) {
            av_free(iobuf);
        }
        goto out;
    }
    fmt->pb     = avio;
    fmt->flags |= AVFMT_FLAG_CUSTOM_IO;
    if (avformat_open_input(&fmt, NULL, NULL, NULL) < 0) {
        fmt = NULL;
        goto out;
    }
    if (avformat_find_stream_info(fmt, NULL) < 0) {
        goto out;
    }
    idx = av_find_best_stream(fmt, AVMEDIA_TYPE_AUDIO, -1, -1, NULL, 0);
    if (idx < 0) {
        goto out;
    }
    const AVCodec *codec = avcodec_find_decoder(fmt->streams[idx]->codecpar->codec_id);
    ctx = codec ? avcodec_alloc_context3(codec) : NULL;
    if (!ctx || avcodec_parameters_to_context(ctx, fmt->streams[idx]->codecpar) < 0 ||
        avcodec_open2(ctx, codec, NULL) < 0) {
        goto out;
    }

    for (;;) {
        int r = av_read_frame(fmt, pkt);
        if (r < 0) {
            /* fin del archivo: vaciar el decodificador */
            avcodec_send_packet(ctx, NULL);
        } else if (pkt->stream_index == idx) {
            avcodec_send_packet(ctx, pkt);
        }
        av_packet_unref(pkt);
        while (avcodec_receive_frame(ctx, frame) == 0) {
            if (!graph && !audio_graph_create(frame, 1.0, 0.0, core->out_rate, &graph, &src, &sink)) {
                goto out;
            }
            if (av_buffersrc_add_frame(src, frame) >= 0) {
                while (av_buffersink_get_frame(sink, tmp) >= 0) {
                    bool go = push_pcm(core, (const float *)tmp->data[0], (size_t)tmp->nb_samples);
                    av_frame_unref(tmp);
                    if (!go) {
                        goto out;
                    }
                }
            }
            av_frame_unref(frame);
        }
        if (r < 0) {
            cp_mutex_lock(&core->lock);
            bool again = m->loop && !m->quit;
            cp_mutex_unlock(&core->lock);
            if (!again) {
                break;
            }
            /* de nuevo desde el principio */
            av_seek_frame(fmt, idx, 0, AVSEEK_FLAG_BACKWARD);
            avcodec_flush_buffers(ctx);
        }
    }

out:
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
    cp_mutex_lock(&core->lock);
    m->ended = true;
    cp_mutex_unlock(&core->lock);
}

/* Para el hilo (si hay) y libera todo. Sin el lock tomado. */
static void music_join(CapyroCore *core) {
    Music *m = &core->music;
    cp_mutex_lock(&core->lock);
    bool active = m->active;
    m->quit = true;
    cp_mutex_unlock(&core->lock);
    if (active) {
        cp_thread_join(m->thread);
    }
    cp_mutex_lock(&core->lock);
    float *ring = m->ring;
    uint8_t *data = m->data;
    memset(m, 0, sizeof(*m));
    cp_mutex_unlock(&core->lock);
    free(ring);
    free(data);
}

int capyro_core_music_play(CapyroCore *core, const uint8_t *data, size_t size, bool loop, int fade_ms) {
    if (!core || !data || size == 0) {
        return CAPYRO_ERR_INVALID_ARG;
    }
    if (!core->audio_ok) {
        return CAPYRO_OK; /* sin dispositivo, en silencio */
    }
    music_join(core);
    Music *m = &core->music;
    uint8_t *copy = malloc(size);
    size_t cap = (size_t)core->out_rate * MUSIC_RING_SEC;
    float *ring = malloc(cap * 2 * sizeof(float));
    if (!copy || !ring) {
        free(copy);
        free(ring);
        return CAPYRO_ERR_NO_MEMORY;
    }
    memcpy(copy, data, size);
    float fade_frames = (float)core->out_rate * (float)(fade_ms > 0 ? fade_ms : 1) / 1000.0f;

    cp_mutex_lock(&core->lock);
    m->data     = copy;
    m->size     = size;
    m->ring     = ring;
    m->ring_cap = cap;
    m->loop     = loop;
    m->level    = 0.0f;
    m->target   = 1.0f;
    m->step     = 1.0f / fade_frames;
    m->quit     = false;
    m->ended    = false;
    m->active   = true;
    cp_mutex_unlock(&core->lock);

    if (cp_thread_start(&m->thread, music_thread, core) != 0) {
        cp_mutex_lock(&core->lock);
        m->active = false;
        cp_mutex_unlock(&core->lock);
        music_join(core);
        return CAPYRO_ERR_THREAD;
    }
    player_ensure_device_started(core);
    return CAPYRO_OK;
}

void capyro_core_music_stop(CapyroCore *core, int fade_ms) {
    if (!core) {
        return;
    }
    cp_mutex_lock(&core->lock);
    Music *m = &core->music;
    if (m->active) {
        float fade_frames = (float)core->out_rate * (float)(fade_ms > 0 ? fade_ms : 1) / 1000.0f;
        m->target = 0.0f;
        m->step   = (m->level > 0.0f ? m->level : 1.0f) / fade_frames;
    }
    cp_mutex_unlock(&core->lock);
}

bool capyro_core_music_playing(CapyroCore *core) {
    if (!core) {
        return false;
    }
    cp_mutex_lock(&core->lock);
    bool on = core->music.active && !core->music.ended && core->music.target > 0.0f;
    cp_mutex_unlock(&core->lock);
    return on;
}

void music_mix_locked(CapyroCore *core, float *out, size_t frames, float gain) {
    Music *m = &core->music;
    if (!m->active || !m->ring) {
        return;
    }
    size_t n = frames < m->ring_len ? frames : m->ring_len;
    for (size_t i = 0; i < n; i++) {
        /* rampa de volumen (fundido de entrada / salida) */
        if (m->level < m->target) {
            m->level += m->step;
            if (m->level > m->target) m->level = m->target;
        } else if (m->level > m->target) {
            m->level -= m->step;
            if (m->level < m->target) m->level = m->target;
        }
        const float *src = m->ring + ((m->ring_read + i) % m->ring_cap) * 2;
        float g = gain * m->level;
        for (int c = 0; c < 2; c++) {
            float v = out[2 * i + c] + src[c] * g;
            out[2 * i + c] = v > 1.0f ? 1.0f : (v < -1.0f ? -1.0f : v);
        }
    }
    m->ring_read = (m->ring_read + n) % m->ring_cap;
    m->ring_len -= n;
    /* terminó el fundido de salida: que el hilo pare */
    if (m->target <= 0.0f && m->level <= 0.0f) {
        m->quit = true;
    }
}

void music_reap(CapyroCore *core) {
    cp_mutex_lock(&core->lock);
    Music *m = &core->music;
    bool done = m->active && (m->ended || (m->quit && m->target <= 0.0f));
    bool drained = m->ended && m->ring_len == 0;
    bool faded = m->target <= 0.0f && m->level <= 0.0f;
    cp_mutex_unlock(&core->lock);
    if (done && (drained || faded)) {
        music_join(core);
    }
}

void music_shutdown(CapyroCore *core) {
    music_join(core);
}

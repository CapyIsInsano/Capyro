/*
 * Capyro — decodificación de video por GPU.
 *
 * Se usa la "copia de vuelta": FFmpeg decodifica en la GPU (VA-API para
 * Intel/AMD, NVDEC por CUDA para NVIDIA, VDPAU como último recurso) y cada
 * cuadro se copia a memoria normal para convertirlo a RGBA como siempre.
 * Lo pesado (decodificar H.264/HEVC/VP9/AV1, sobre todo en 4K) lo hace la
 * GPU; la copia es barata en comparación.
 */
#include "internal.h"

#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include <libavutil/hwcontext.h>
#include <libavutil/pixdesc.h>

#include <stdint.h>
#include <stdio.h>
#include <string.h>

static const struct {
    enum AVHWDeviceType type;
    const char         *name;
} APIS[HWDEV_COUNT] = {
    { AV_HWDEVICE_TYPE_VAAPI, "VA-API" },
    { AV_HWDEVICE_TYPE_CUDA,  "NVDEC"  },
    { AV_HWDEVICE_TYPE_VDPAU, "VDPAU"  },
};

void hwdec_codec_name(int codec_id, char *out, size_t cap) {
    const char *n;
    switch (codec_id) {
    case AV_CODEC_ID_H264:       n = "H.264"; break;
    case AV_CODEC_ID_HEVC:       n = "H.265 (HEVC)"; break;
    case AV_CODEC_ID_VP8:        n = "VP8"; break;
    case AV_CODEC_ID_VP9:        n = "VP9"; break;
    case AV_CODEC_ID_AV1:        n = "AV1"; break;
    case AV_CODEC_ID_MPEG4:      n = "MPEG-4"; break;
    case AV_CODEC_ID_MPEG2VIDEO: n = "MPEG-2"; break;
    case AV_CODEC_ID_MJPEG:      n = "MJPEG"; break;
    case AV_CODEC_ID_PRORES:     n = "ProRes"; break;
    default:                     n = avcodec_get_name((enum AVCodecID)codec_id); break;
    }
    snprintf(out, cap, "%s", n);
}

/* FFmpeg ofrece formatos; elegimos el de la GPU. Si la GPU no puede con
 * este video (perfil no soportado), FFmpeg vuelve a preguntar sin él y
 * elegimos uno normal: sigue decodificando por CPU. */
static enum AVPixelFormat pick_format(AVCodecContext *ctx, const enum AVPixelFormat *fmts) {
    enum AVPixelFormat want = (enum AVPixelFormat)(intptr_t)ctx->opaque;
    for (const enum AVPixelFormat *p = fmts; *p != AV_PIX_FMT_NONE; p++) {
        if (*p == want) {
            return *p;
        }
    }
    for (const enum AVPixelFormat *p = fmts; *p != AV_PIX_FMT_NONE; p++) {
        const AVPixFmtDescriptor *d = av_pix_fmt_desc_get(*p);
        if (d && !(d->flags & AV_PIX_FMT_FLAG_HWACCEL)) {
            return *p;
        }
    }
    return AV_PIX_FMT_NONE;
}

AVCodecContext *hwdec_open(CapyroCore *core, const AVStream *st, int *hw_fmt,
                           char *api, size_t api_cap, char *why, size_t why_cap) {
    const AVCodec *codec = avcodec_find_decoder(st->codecpar->codec_id);
    char cname[24];
    hwdec_codec_name(st->codecpar->codec_id, cname, sizeof(cname));
    if (!codec) {
        snprintf(why, why_cap, "No hay decodificador para %s", cname);
        return NULL;
    }
    bool codec_ok = false, device_ok = false;
    for (int i = 0; i < HWDEV_COUNT; i++) {
        /* ¿este decodificador sabe usar esta API? */
        enum AVPixelFormat pix = AV_PIX_FMT_NONE;
        for (int j = 0;; j++) {
            const AVCodecHWConfig *cfg = avcodec_get_hw_config(codec, j);
            if (!cfg) {
                break;
            }
            if ((cfg->methods & AV_CODEC_HW_CONFIG_METHOD_HW_DEVICE_CTX) && cfg->device_type == APIS[i].type) {
                pix = cfg->pix_fmt;
                break;
            }
        }
        if (pix == AV_PIX_FMT_NONE) {
            continue;
        }
        codec_ok = true;

        /* el dispositivo se abre una sola vez y se reusa */
        if (!core->hwdev_tried[i]) {
            core->hwdev_tried[i] = true;
            int lvl = av_log_get_level();
            av_log_set_level(AV_LOG_QUIET); /* sin GPU de ese tipo FFmpeg se queja por stderr */
            if (av_hwdevice_ctx_create(&core->hwdev[i], APIS[i].type, NULL, NULL, 0) < 0) {
                core->hwdev[i] = NULL;
            }
            av_log_set_level(lvl);
        }
        if (!core->hwdev[i]) {
            continue;
        }
        device_ok = true;

        AVCodecContext *ctx = avcodec_alloc_context3(codec);
        if (!ctx) {
            continue;
        }
        if (avcodec_parameters_to_context(ctx, st->codecpar) < 0) {
            avcodec_free_context(&ctx);
            continue;
        }
        ctx->hw_device_ctx = av_buffer_ref(core->hwdev[i]);
        ctx->opaque        = (void *)(intptr_t)pix;
        ctx->get_format    = pick_format;
        ctx->thread_count  = 1; /* el trabajo lo hace la GPU */
        if (!ctx->hw_device_ctx || avcodec_open2(ctx, codec, NULL) < 0) {
            avcodec_free_context(&ctx);
            continue;
        }
        ctx->pkt_timebase = st->time_base;
        *hw_fmt = pix;
        snprintf(api, api_cap, "%s", APIS[i].name);
        why[0] = '\0';
        return ctx;
    }
    if (!codec_ok) {
        snprintf(why, why_cap, "Tu GPU o tu FFmpeg no decodifican %s", cname);
    } else if (!device_ok) {
        snprintf(why, why_cap, "No hay GPU compatible (VA-API, NVDEC, VDPAU)");
    } else {
        snprintf(why, why_cap, "La GPU no pudo abrir este video");
    }
    return NULL;
}

void hwdec_free_devices(CapyroCore *core) {
    for (int i = 0; i < HWDEV_COUNT; i++) {
        av_buffer_unref(&core->hwdev[i]);
        core->hwdev_tried[i] = false;
    }
}

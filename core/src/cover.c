/*
 * Capyro — portadas de los audios (Capyro Studio).
 *
 * - Leer la portada de un archivo o una imagen cualquiera como RGBA (para
 *   mostrarla en la interfaz).
 * - Preparar una imagen para guardarla como portada: si ya es un JPG o PNG
 *   de tamaño razonable se usa tal cual; si no (WebP, BMP, fotos enormes),
 *   se achica a 1200 px y se guarda como JPEG.
 * - En Ogg (Vorbis/Opus) la portada no es una pista: va en la etiqueta
 *   METADATA_BLOCK_PICTURE (un bloque de imagen de FLAC en base64).
 */
#include "capyro_core.h"
#include "cover.h"

#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include <libavutil/base64.h>
#include <libavutil/dict.h>
#include <libavutil/frame.h>
#include <libavutil/intreadwrite.h>
#include <libavutil/mem.h>
#include <libswscale/swscale.h>

#include <stdlib.h>
#include <string.h>

#define COVER_MAX_SIDE      1200              /* lado más largo de una portada recodificada */
#define COVER_RAW_MAX_SIDE  1600              /* JPG/PNG hasta este tamaño van tal cual */
#define COVER_RAW_MAX_BYTES (4 * 1024 * 1024) /* ... y hasta este peso */
#define COVER_JPEG_QUALITY  3                 /* qscale de MJPEG: 2 = máxima, 31 = mínima */

/* ------------------------------------------------------------------ */
/* utilidades                                                          */
/* ------------------------------------------------------------------ */

/* Achica (w, h) para que el lado más largo no pase de `max_side`. */
static void fit_side(int w, int h, int max_side, int *dw, int *dh) {
    *dw = w;
    *dh = h;
    if (max_side <= 0 || (w <= max_side && h <= max_side)) {
        return;
    }
    if (w >= h) {
        *dw = max_side;
        *dh = (int)((int64_t)h * max_side / w);
    } else {
        *dh = max_side;
        *dw = (int)((int64_t)w * max_side / h);
    }
    if (*dw < 1) *dw = 1;
    if (*dh < 1) *dh = 1;
}

const char *cover_mime(int codec_id) {
    switch (codec_id) {
    case AV_CODEC_ID_MJPEG: return "image/jpeg";
    case AV_CODEC_ID_PNG:   return "image/png";
    default:                return NULL;
    }
}

/* Decodifica una imagen que ya está en un paquete. */
static AVFrame *decode_packet(const AVCodecParameters *par, const AVPacket *pkt) {
    const AVCodec *codec = avcodec_find_decoder(par->codec_id);
    if (!codec) {
        return NULL;
    }
    AVCodecContext *ctx = avcodec_alloc_context3(codec);
    AVFrame *frame = av_frame_alloc();
    AVFrame *out = NULL;
    if (ctx && frame && avcodec_parameters_to_context(ctx, par) >= 0 && avcodec_open2(ctx, codec, NULL) >= 0 &&
        avcodec_send_packet(ctx, pkt) >= 0) {
        avcodec_send_packet(ctx, NULL);
        if (avcodec_receive_frame(ctx, frame) == 0 && frame->width > 0 && frame->height > 0) {
            out = frame;
            frame = NULL;
        }
    }
    av_frame_free(&frame);
    avcodec_free_context(&ctx);
    return out;
}

/* Abre una imagen y lee su (único) cuadro: el paquete y los parámetros. */
static int read_image_packet(const char *path, AVFormatContext **fmt_out, AVPacket **pkt_out, int *stream_out) {
    if (!path || !path[0]) {
        return CAPYRO_ERR_INVALID_ARG;
    }
    AVFormatContext *fmt = NULL;
    AVDictionary *opts = NULL;
    /* que un "%" en el nombre no se tome como patrón de secuencia (image2) */
    av_dict_set(&opts, "pattern_type", "none", 0);
    int err = avformat_open_input(&fmt, path, NULL, &opts);
    av_dict_free(&opts);
    if (err < 0) {
        return CAPYRO_ERR_NOT_FOUND;
    }
    if (avformat_find_stream_info(fmt, NULL) < 0) {
        avformat_close_input(&fmt);
        return CAPYRO_ERR_DECODE;
    }
    int s = av_find_best_stream(fmt, AVMEDIA_TYPE_VIDEO, -1, -1, NULL, 0);
    if (s < 0) {
        avformat_close_input(&fmt);
        return CAPYRO_ERR_UNSUPPORTED;
    }
    AVPacket *pkt = av_packet_alloc();
    if (!pkt) {
        avformat_close_input(&fmt);
        return CAPYRO_ERR_NO_MEMORY;
    }
    while (av_read_frame(fmt, pkt) >= 0) {
        if (pkt->stream_index == s) {
            *fmt_out = fmt;
            *pkt_out = pkt;
            *stream_out = s;
            return CAPYRO_OK;
        }
        av_packet_unref(pkt);
    }
    av_packet_free(&pkt);
    avformat_close_input(&fmt);
    return CAPYRO_ERR_DECODE;
}

/* Cuadro → RGBA (filas pegadas, 4 bytes por píxel) achicado a `max_side`. */
static int frame_to_rgba(const AVFrame *f, int max_side, CapyroImage *out) {
    int dw, dh;
    fit_side(f->width, f->height, max_side, &dw, &dh);
    size_t stride = (size_t)dw * 4;
    /* margen al final: swscale a veces escribe de a bloques */
    uint8_t *buf = malloc(stride * (size_t)dh + stride + 64);
    if (!buf) {
        return CAPYRO_ERR_NO_MEMORY;
    }
    struct SwsContext *sws = sws_getContext(f->width, f->height, (enum AVPixelFormat)f->format, dw, dh,
                                            AV_PIX_FMT_RGBA, SWS_BICUBIC, NULL, NULL, NULL);
    if (!sws) {
        free(buf);
        return CAPYRO_ERR_DECODE;
    }
    uint8_t *dst[4] = { buf, NULL, NULL, NULL };
    int dst_stride[4] = { (int)stride, 0, 0, 0 };
    int rows = sws_scale(sws, (const uint8_t *const *)f->data, f->linesize, 0, f->height, dst, dst_stride);
    sws_freeContext(sws);
    if (rows <= 0) {
        free(buf);
        return CAPYRO_ERR_DECODE;
    }
    out->width = dw;
    out->height = dh;
    out->rgba = buf;
    return CAPYRO_OK;
}

/* Recodifica el cuadro como JPEG (achicado a COVER_MAX_SIDE). */
static int encode_jpeg(const AVFrame *src, CoverData *out) {
    const AVCodec *enc = avcodec_find_encoder(AV_CODEC_ID_MJPEG);
    if (!enc) {
        return CAPYRO_ERR_UNSUPPORTED;
    }
    int dw, dh;
    fit_side(src->width, src->height, COVER_MAX_SIDE, &dw, &dh);

    int ret = CAPYRO_ERR_DECODE;
    AVCodecContext *ctx = avcodec_alloc_context3(enc);
    AVFrame *f = av_frame_alloc();
    AVPacket *pkt = av_packet_alloc();
    struct SwsContext *sws = NULL;
    if (!ctx || !f || !pkt) {
        ret = CAPYRO_ERR_NO_MEMORY;
        goto out;
    }
    ctx->width = dw;
    ctx->height = dh;
    /* yuv420p de rango completo = lo que usa un JPEG común */
    ctx->pix_fmt = AV_PIX_FMT_YUV420P;
    ctx->color_range = AVCOL_RANGE_JPEG;
    ctx->time_base = (AVRational){ 1, 25 };
    ctx->flags |= AV_CODEC_FLAG_QSCALE;
    ctx->global_quality = FF_QP2LAMBDA * COVER_JPEG_QUALITY;
    if (avcodec_open2(ctx, enc, NULL) < 0) {
        ret = CAPYRO_ERR_UNSUPPORTED;
        goto out;
    }
    f->format = AV_PIX_FMT_YUV420P;
    f->width = dw;
    f->height = dh;
    f->color_range = AVCOL_RANGE_JPEG;
    if (av_frame_get_buffer(f, 0) < 0) {
        ret = CAPYRO_ERR_NO_MEMORY;
        goto out;
    }
    sws = sws_getContext(src->width, src->height, (enum AVPixelFormat)src->format, dw, dh, AV_PIX_FMT_YUV420P,
                         SWS_BICUBIC, NULL, NULL, NULL);
    if (!sws) {
        goto out;
    }
    {
        /* salida en rango completo (0-255), como espera el JPEG */
        int *inv, *tab, src_range, dst_range, bri, con, sat;
        if (sws_getColorspaceDetails(sws, &inv, &src_range, &tab, &dst_range, &bri, &con, &sat) >= 0) {
            sws_setColorspaceDetails(sws, inv, src_range, tab, 1, bri, con, sat);
        }
    }
    if (sws_scale(sws, (const uint8_t *const *)src->data, src->linesize, 0, src->height, f->data, f->linesize) <= 0) {
        goto out;
    }
    f->pts = 0;
    f->quality = ctx->global_quality;
    if (avcodec_send_frame(ctx, f) < 0) {
        goto out;
    }
    avcodec_send_frame(ctx, NULL);
    if (avcodec_receive_packet(ctx, pkt) < 0 || pkt->size <= 0) {
        goto out;
    }
    out->data = av_malloc((size_t)pkt->size);
    if (!out->data) {
        ret = CAPYRO_ERR_NO_MEMORY;
        goto out;
    }
    memcpy(out->data, pkt->data, (size_t)pkt->size);
    out->size = (size_t)pkt->size;
    out->codec_id = AV_CODEC_ID_MJPEG;
    out->width = dw;
    out->height = dh;
    out->mime = "image/jpeg";
    ret = CAPYRO_OK;
out:
    sws_freeContext(sws);
    av_packet_free(&pkt);
    av_frame_free(&f);
    avcodec_free_context(&ctx);
    return ret;
}

/* ------------------------------------------------------------------ */
/* para tags.c                                                         */
/* ------------------------------------------------------------------ */

int cover_prepare(const char *image_path, CoverData *out) {
    memset(out, 0, sizeof(*out));
    AVFormatContext *fmt = NULL;
    AVPacket *pkt = NULL;
    int s = -1;
    int err = read_image_packet(image_path, &fmt, &pkt, &s);
    if (err != CAPYRO_OK) {
        return err;
    }
    const AVCodecParameters *par = fmt->streams[s]->codecpar;
    /* se decodifica siempre: así se sabe que es una imagen de verdad */
    AVFrame *frame = decode_packet(par, pkt);
    if (!frame) {
        err = CAPYRO_ERR_DECODE;
        goto done;
    }
    bool raw_ok = cover_mime(par->codec_id) != NULL && pkt->size > 0 && pkt->size <= COVER_RAW_MAX_BYTES &&
                  frame->width <= COVER_RAW_MAX_SIDE && frame->height <= COVER_RAW_MAX_SIDE;
    if (raw_ok) {
        out->data = av_malloc((size_t)pkt->size);
        if (!out->data) {
            err = CAPYRO_ERR_NO_MEMORY;
        } else {
            memcpy(out->data, pkt->data, (size_t)pkt->size);
            out->size = (size_t)pkt->size;
            out->codec_id = par->codec_id;
            out->width = frame->width;
            out->height = frame->height;
            out->mime = cover_mime(par->codec_id);
            err = CAPYRO_OK;
        }
    } else {
        err = encode_jpeg(frame, out);
    }
    av_frame_free(&frame);
done:
    av_packet_free(&pkt);
    avformat_close_input(&fmt);
    if (err != CAPYRO_OK) {
        cover_data_free(out);
    }
    return err;
}

void cover_data_free(CoverData *c) {
    if (c) {
        av_freep(&c->data);
        c->size = 0;
    }
}

bool cover_muxer_is_ogg(const AVOutputFormat *fmt) {
    if (!fmt || !fmt->name) {
        return false;
    }
    return !strcmp(fmt->name, "ogg") || !strcmp(fmt->name, "oga") || !strcmp(fmt->name, "opus") ||
           !strcmp(fmt->name, "spx");
}

bool cover_muxer_ok(const AVOutputFormat *fmt) {
    if (!fmt || !fmt->name) {
        return false;
    }
    if (cover_muxer_is_ogg(fmt)) {
        return true;
    }
    /* "mov,mp4,m4a..." no: av_guess_format devuelve el nombre de un muxer solo */
    const char *ok[] = { "mp3", "flac", "ipod", "mp4", "mov" };
    for (size_t i = 0; i < sizeof(ok) / sizeof(ok[0]); i++) {
        if (!strcmp(fmt->name, ok[i])) {
            return true;
        }
    }
    return false;
}

char *cover_ogg_picture(const uint8_t *data, size_t size, const char *mime, int width, int height) {
    if (!data || !size || !mime) {
        return NULL;
    }
    size_t mlen = strlen(mime);
    /* tipo, mime, descripción (vacía), ancho, alto, profundidad, colores, datos */
    size_t total = 4 + 4 + mlen + 4 + 4 * 4 + 4 + size;
    if (total > (size_t)(INT32_MAX / 4 * 3 - 16)) {
        return NULL;
    }
    uint8_t *blk = av_malloc(total);
    if (!blk) {
        return NULL;
    }
    uint8_t *p = blk;
    AV_WB32(p, 3); p += 4;                       /* 3 = portada (frente) */
    AV_WB32(p, (uint32_t)mlen); p += 4;
    memcpy(p, mime, mlen); p += mlen;
    AV_WB32(p, 0); p += 4;                       /* sin descripción */
    AV_WB32(p, (uint32_t)(width > 0 ? width : 0)); p += 4;
    AV_WB32(p, (uint32_t)(height > 0 ? height : 0)); p += 4;
    AV_WB32(p, 24); p += 4;                      /* bits por píxel */
    AV_WB32(p, 0); p += 4;                       /* sin paleta */
    AV_WB32(p, (uint32_t)size); p += 4;
    memcpy(p, data, size);

    int b64_size = AV_BASE64_SIZE((int)total);
    char *b64 = av_malloc((size_t)b64_size);
    if (b64 && !av_base64_encode(b64, b64_size, blk, (int)total)) {
        av_freep(&b64);
    }
    av_free(blk);
    return b64;
}

/* ------------------------------------------------------------------ */
/* API pública                                                         */
/* ------------------------------------------------------------------ */

bool capyro_cover_supported(const char *path) {
    if (!path || !path[0]) {
        return false;
    }
    return cover_muxer_ok(av_guess_format(NULL, path, NULL));
}

int capyro_cover_read(const char *path, int max_side, CapyroImage *out) {
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
    int err = CAPYRO_ERR_NOT_FOUND;
    for (unsigned i = 0; i < fmt->nb_streams; i++) {
        AVStream *st = fmt->streams[i];
        if (!(st->disposition & AV_DISPOSITION_ATTACHED_PIC) || st->attached_pic.size <= 0) {
            continue;
        }
        AVFrame *frame = decode_packet(st->codecpar, &st->attached_pic);
        if (frame) {
            err = frame_to_rgba(frame, max_side, out);
            av_frame_free(&frame);
            break;
        }
        err = CAPYRO_ERR_DECODE;
    }
    avformat_close_input(&fmt);
    return err;
}

int capyro_image_read(const char *path, int max_side, CapyroImage *out) {
    if (!out) {
        return CAPYRO_ERR_INVALID_ARG;
    }
    memset(out, 0, sizeof(*out));
    AVFormatContext *fmt = NULL;
    AVPacket *pkt = NULL;
    int s = -1;
    int err = read_image_packet(path, &fmt, &pkt, &s);
    if (err != CAPYRO_OK) {
        return err;
    }
    AVFrame *frame = decode_packet(fmt->streams[s]->codecpar, pkt);
    err = frame ? frame_to_rgba(frame, max_side, out) : CAPYRO_ERR_DECODE;
    av_frame_free(&frame);
    av_packet_free(&pkt);
    avformat_close_input(&fmt);
    return err;
}

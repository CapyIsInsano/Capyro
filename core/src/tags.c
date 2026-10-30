/*
 * Capyro — etiquetas de un archivo de audio (Capyro Studio).
 *
 * Leer: los metadatos que FFmpeg ya entiende (título, artista, álbum...).
 * Escribir: se reescribe el archivo copiando los paquetes tal cual (no hay
 * recodificación, así que no pierde calidad y es rápido) con las etiquetas
 * nuevas. El original queda como "<archivo>.bak" (y los anteriores como .bak2,
 * .bak3...) y el nuevo toma su nombre.
 */
#include "capyro_core.h"

#include <libavformat/avformat.h>
#include <libavutil/dict.h>

#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* Campo de CapyroTags, la clave de FFmpeg que lo guarda y las alternativas
 * que se miran al leer (en orden). */
typedef struct {
    size_t offset;
    size_t cap;
    const char *key;         /* clave al escribir */
    const char *alts[3];     /* al leer: primero "key", después estas (NULL = fin) */
    const char *drop[3];     /* al escribir: otras claves que se sacan */
} TagField;

#define FIELD(member, key_, ...) \
    { offsetof(CapyroTags, member), sizeof(((CapyroTags *)0)->member), key_, { key_, __VA_ARGS__ }, { __VA_ARGS__ } }

static const TagField FIELDS[] = {
    FIELD(title,        "title",        NULL),
    FIELD(artist,       "artist",       "author", NULL),
    FIELD(album,        "album",        NULL),
    FIELD(album_artist, "album_artist", "albumartist", NULL),
    FIELD(genre,        "genre",        NULL),
    FIELD(year,         "date",         "year", NULL),
    FIELD(track,        "track",        NULL),
    FIELD(disc,         "disc",         NULL),
    FIELD(composer,     "composer",     NULL),
    FIELD(comment,      "comment",      NULL),
};
#define N_FIELDS ((int)(sizeof(FIELDS) / sizeof(FIELDS[0])))

static char *field_ptr(CapyroTags *t, const TagField *f) {
    return (char *)t + f->offset;
}

/* Copia el valor (sin espacios de sobra) al campo. */
static void set_field(CapyroTags *t, const TagField *f, const char *v) {
    char *dst = field_ptr(t, f);
    if (!v) {
        dst[0] = '\0';
        return;
    }
    while (*v == ' ' || *v == '\t') {
        v++;
    }
    snprintf(dst, f->cap, "%s", v);
    size_t n = strlen(dst);
    while (n > 0 && (dst[n - 1] == ' ' || dst[n - 1] == '\t' || dst[n - 1] == '\r' || dst[n - 1] == '\n')) {
        dst[--n] = '\0';
    }
}

/* El primer valor de `key` en el diccionario, o NULL. */
static const char *dict_value(AVDictionary *d, const char *key) {
    AVDictionaryEntry *e = d ? av_dict_get(d, key, NULL, 0) : NULL;
    return e ? e->value : NULL;
}

static int open_input(const char *path, AVFormatContext **out) {
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
    *out = fmt;
    return CAPYRO_OK;
}

int capyro_tags_read(const char *path, CapyroTags *out) {
    if (!out) {
        return CAPYRO_ERR_INVALID_ARG;
    }
    memset(out, 0, sizeof(*out));
    AVFormatContext *fmt = NULL;
    int err = open_input(path, &fmt);
    if (err != CAPYRO_OK) {
        return err;
    }
    for (int i = 0; i < N_FIELDS; i++) {
        const TagField *f = &FIELDS[i];
        const char *v = NULL;
        for (int k = 0; !v && k < 3 && (k == 0 || f->alts[k]); k++) {
            const char *key = k == 0 ? f->key : f->alts[k];
            v = dict_value(fmt->metadata, key);
            /* en Ogg y WebM los datos van en el stream de audio */
            for (unsigned s = 0; !v && s < fmt->nb_streams; s++) {
                v = dict_value(fmt->streams[s]->metadata, key);
            }
        }
        set_field(out, f, v);
    }
    avformat_close_input(&fmt);
    return CAPYRO_OK;
}

/* Pone (o quita, si el valor es vacío) cada campo en el diccionario. */
static void apply_tags(AVDictionary **d, const CapyroTags *t) {
    for (int i = 0; i < N_FIELDS; i++) {
        const TagField *f = &FIELDS[i];
        for (int k = 0; f->drop[k]; k++) {
            av_dict_set(d, f->drop[k], NULL, 0);
        }
        const char *v = field_ptr((CapyroTags *)t, f);
        av_dict_set(d, f->key, v[0] ? v : NULL, 0);
    }
}

static void remove_if_exists(const char *path) {
    remove(path);
}

static bool file_exists(const char *path) {
    FILE *f = fopen(path, "rb");
    if (!f) {
        return false;
    }
    fclose(f);
    return true;
}

/* Primer nombre libre de respaldo: "<p>.bak", "<p>.bak2", "<p>.bak3", ... */
static bool free_backup_name(const char *path, char *out, size_t size) {
    snprintf(out, size, "%s.bak", path);
    for (int n = 2; file_exists(out) && n < 10000; n++) {
        snprintf(out, size, "%s.bak%d", path, n);
    }
    return !file_exists(out);
}

int capyro_tags_write(const char *path, const CapyroTags *tags) {
    if (!path || !path[0] || !tags) {
        return CAPYRO_ERR_INVALID_ARG;
    }
    AVFormatContext *in = NULL;
    int err = open_input(path, &in);
    if (err != CAPYRO_OK) {
        return err;
    }

    /* el temporal conserva la extensión: algunos muxers la miran */
    char tmp[4096];
    {
        const char *dot = strrchr(path, '.');
        const char *slash = strrchr(path, '/');
        if (dot && (!slash || dot > slash)) {
            snprintf(tmp, sizeof(tmp), "%.*s.capyro-tmp%s", (int)(dot - path), path, dot);
        } else {
            snprintf(tmp, sizeof(tmp), "%s.capyro-tmp", path);
        }
    }

    const AVOutputFormat *fmt = av_guess_format(NULL, path, NULL);
    AVFormatContext *out = NULL;
    AVPacket *pkt = NULL;
    int *map = NULL;
    int ret = CAPYRO_ERR_WRITE;

    if (!fmt) {
        ret = CAPYRO_ERR_UNSUPPORTED;
        goto done;
    }
    /* la ruta de salida va también: algunos muxers miran la extensión */
    if (avformat_alloc_output_context2(&out, (AVOutputFormat *)fmt, NULL, tmp) < 0) {
        ret = CAPYRO_ERR_UNSUPPORTED;
        goto done;
    }
    map = calloc(in->nb_streams ? in->nb_streams : 1, sizeof(int));
    pkt = av_packet_alloc();
    if (!map || !pkt) {
        ret = CAPYRO_ERR_NO_MEMORY;
        goto done;
    }

    /* las mismas pistas (audio, video, carátula), sin recodificar */
    for (unsigned i = 0; i < in->nb_streams; i++) {
        AVStream *si = in->streams[i];
        map[i] = -1;
        if (si->codecpar->codec_type != AVMEDIA_TYPE_AUDIO && si->codecpar->codec_type != AVMEDIA_TYPE_VIDEO &&
            si->codecpar->codec_type != AVMEDIA_TYPE_ATTACHMENT) {
            continue;
        }
        AVStream *so = avformat_new_stream(out, NULL);
        if (!so || avcodec_parameters_copy(so->codecpar, si->codecpar) < 0) {
            ret = CAPYRO_ERR_NO_MEMORY;
            goto done;
        }
        so->codecpar->codec_tag = 0;
        so->time_base = si->time_base;
        so->disposition = si->disposition;
        av_dict_copy(&so->metadata, si->metadata, 0);
        if (si->codecpar->codec_type == AVMEDIA_TYPE_AUDIO) {
            /* en Ogg/WebM las etiquetas viven en el stream de audio */
            apply_tags(&so->metadata, tags);
        }
        map[i] = (int)so->index;
    }
    av_dict_copy(&out->metadata, in->metadata, 0);
    apply_tags(&out->metadata, tags);

    if (avio_open(&out->pb, tmp, AVIO_FLAG_WRITE) < 0) {
        ret = CAPYRO_ERR_WRITE;
        goto done;
    }
    if (avformat_write_header(out, NULL) < 0) {
        ret = CAPYRO_ERR_WRITE;
        goto done;
    }
    while (av_read_frame(in, pkt) >= 0) {
        int idx = pkt->stream_index;
        if (idx < 0 || (unsigned)idx >= in->nb_streams || map[idx] < 0) {
            av_packet_unref(pkt);
            continue;
        }
        AVStream *si = in->streams[idx];
        AVStream *so = out->streams[map[idx]];
        pkt->stream_index = map[idx];
        av_packet_rescale_ts(pkt, si->time_base, so->time_base);
        if (av_interleaved_write_frame(out, pkt) < 0) {
            ret = CAPYRO_ERR_WRITE;
            goto done;
        }
    }
    if (av_write_trailer(out) < 0) {
        ret = CAPYRO_ERR_WRITE;
        goto done;
    }
    ret = CAPYRO_OK;

done:
    if (out) {
        if (out->pb) {
            avio_closep(&out->pb);
        }
        avformat_free_context(out);
    }
    av_packet_free(&pkt);
    free(map);
    avformat_close_input(&in);

    if (ret != CAPYRO_OK) {
        remove_if_exists(tmp);
        return ret;
    }
    /* original → .bak, nuevo → original. Si el segundo paso falla, se vuelve atrás.
     * Nunca se pisa un respaldo que ya existe: el primero es el original y los
     * siguientes (de guardados posteriores) van a .bak2, .bak3, ... */
    char bak[4096];
    if (!free_backup_name(path, bak, sizeof(bak))) {
        remove_if_exists(tmp);
        return CAPYRO_ERR_WRITE;
    }
    if (rename(path, bak) != 0) {
        remove_if_exists(tmp);
        return CAPYRO_ERR_WRITE;
    }
    if (rename(tmp, path) != 0) {
        rename(bak, path);
        remove_if_exists(tmp);
        return CAPYRO_ERR_WRITE;
    }
    return CAPYRO_OK;
}

/* Capyro — portadas (uso interno de tags.c). Ver cover.c. */
#ifndef CAPYRO_COVER_H
#define CAPYRO_COVER_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

struct AVOutputFormat;

/* Una portada lista para meter en un archivo. */
typedef struct {
    uint8_t    *data;      /* la imagen (JPEG o PNG), con av_malloc */
    size_t      size;
    int         codec_id;  /* AV_CODEC_ID_MJPEG o AV_CODEC_ID_PNG */
    int         width;
    int         height;
    const char *mime;      /* "image/jpeg" o "image/png" */
} CoverData;

/* Lee la imagen y la deja lista (achicada y en JPEG si hace falta). */
int  cover_prepare(const char *image_path, CoverData *out);
void cover_data_free(CoverData *c);

/* ¿Este muxer guarda portadas? ¿Y es de la familia Ogg (van como etiqueta)? */
bool cover_muxer_ok(const struct AVOutputFormat *fmt);
bool cover_muxer_is_ogg(const struct AVOutputFormat *fmt);

/* Bloque METADATA_BLOCK_PICTURE (FLAC) en base64, para Ogg. av_free al terminar. */
char *cover_ogg_picture(const uint8_t *data, size_t size, const char *mime, int width, int height);

/* MIME de una imagen por su códec (NULL si no es JPEG ni PNG). */
const char *cover_mime(int codec_id);

#endif /* CAPYRO_COVER_H */

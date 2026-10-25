/*
 * Capyro — efectos de audio en tiempo real (ecualizador, estéreo, modo noche).
 *
 * Se aplican en el callback de audio, sobre lo que está a punto de sonar, así
 * que cualquier cambio se escucha al instante (no hay que esperar a que se
 * vacíe el búfer de audio ya decodificado). El tono, que necesita estirar el
 * audio en el tiempo, va aparte: lo hace el grafo de FFmpeg (ver player.c).
 */
#ifndef CAPYRO_DSP_H
#define CAPYRO_DSP_H

#include "capyro_core.h"

#include <stdbool.h>
#include <stddef.h>

/* Filtro biquad (forma directa II transpuesta), estado por canal. */
typedef struct {
    double b0, b1, b2, a1, a2;
    double z1[2], z2[2];
} Biquad;

typedef struct {
    CapyroAudioFx fx;        /* parámetros pedidos */
    unsigned      rate;      /* frecuencia de muestreo de salida */
    bool          active;    /* false = no se toca la señal */
    float         preamp;    /* lineal: deja margen cuando hay realce */
    Biquad        bass, mid, treble;
    float         comp_env;  /* envolvente del compresor (modo noche) */
} Dsp;

void dsp_init(Dsp *d, unsigned rate);
/* Recalcula coeficientes; conserva el estado de los filtros (sin clics). */
void dsp_set(Dsp *d, const CapyroAudioFx *fx);
/* Procesa `frames` muestras estéreo intercaladas en el lugar. */
void dsp_process(Dsp *d, float *buf, size_t frames);
/* Lleva los parámetros a sus rangos válidos. */
void dsp_clamp(CapyroAudioFx *fx);
bool dsp_is_neutral(const CapyroAudioFx *fx);

#endif /* CAPYRO_DSP_H */

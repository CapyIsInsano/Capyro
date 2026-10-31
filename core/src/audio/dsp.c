/*
 * Capyro — efectos de audio en tiempo real. Ver dsp.h.
 *
 * Ecualizador de tres bandas con las fórmulas del "Audio EQ Cookbook"
 * (Robert Bristow-Johnson):
 *   graves  → low shelf  a 120 Hz
 *   medios  → peaking    a 1,5 kHz (Q 0,8): la zona de las voces
 *   agudos  → high shelf a 6 kHz
 */
#include "dsp.h"

#include <math.h>
#include <string.h>

#ifndef M_PI
#  define M_PI 3.14159265358979323846
#endif

#define BASS_HZ    120.0
#define MID_HZ     1500.0
#define MID_Q      0.8
#define TREBLE_HZ  6000.0

/* Modo noche: compresor 4:1 desde -28 dBFS, con +9 dB de ganancia de
 * compensación. Lo fuerte baja, lo bajito sube: se entienden los diálogos
 * sin que las explosiones despierten a nadie. */
#define NIGHT_THRESHOLD_DB (-28.0f)
#define NIGHT_RATIO        4.0f
#define NIGHT_MAKEUP_DB    9.0f
#define NIGHT_ATTACK_MS    8.0f
#define NIGHT_RELEASE_MS   250.0f

static float clampf(float v, float lo, float hi) {
    return v < lo ? lo : (v > hi ? hi : v);
}

/* Frecuencias centrales del ecualizador de 15 bandas (de graves a agudos). */
static const double EQ15_HZ[CAPYRO_EQ15_BANDS] = {
    25, 40, 63, 100, 160, 250, 400, 630, 1000, 1600, 2500, 4000, 6300, 10000, 16000
};
#define EQ15_Q 1.6

/* Hay alguna banda del de 15 que no esté en cero. */
static bool eq15_used(const CapyroAudioFx *fx) {
    if (!fx->eq15_on) {
        return false;
    }
    for (int i = 0; i < CAPYRO_EQ15_BANDS; i++) {
        if (fabsf(fx->eq15[i]) >= 0.05f) {
            return true;
        }
    }
    return false;
}

void dsp_clamp(CapyroAudioFx *fx) {
    for (int i = 0; i < CAPYRO_EQ15_BANDS; i++) {
        fx->eq15[i] = isnan(fx->eq15[i]) ? 0.0f : clampf(fx->eq15[i], CAPYRO_EQ_MIN_DB, CAPYRO_EQ_MAX_DB);
    }
    fx->bass_db   = clampf(fx->bass_db,   CAPYRO_EQ_MIN_DB, CAPYRO_EQ_MAX_DB);
    fx->mid_db    = clampf(fx->mid_db,    CAPYRO_EQ_MIN_DB, CAPYRO_EQ_MAX_DB);
    fx->treble_db = clampf(fx->treble_db, CAPYRO_EQ_MIN_DB, CAPYRO_EQ_MAX_DB);
    fx->width     = clampf(fx->width, 0.0f, CAPYRO_WIDTH_MAX);
    if (isnan(fx->bass_db))   fx->bass_db = 0.0f;
    if (isnan(fx->mid_db))    fx->mid_db = 0.0f;
    if (isnan(fx->treble_db)) fx->treble_db = 0.0f;
    if (isnan(fx->width))     fx->width = 1.0f;
}

/* Graves/medios/agudos en uso. Con el ecualizador de 15 bandas activado,
 * el de 3 bandas no suena (uno u otro, no los dos). */
static bool eq3_used(const CapyroAudioFx *fx) {
    return !fx->eq15_on &&
           (fabsf(fx->bass_db) >= 0.05f || fabsf(fx->mid_db) >= 0.05f || fabsf(fx->treble_db) >= 0.05f);
}

bool dsp_is_neutral(const CapyroAudioFx *fx) {
    return !eq3_used(fx) && fabsf(fx->width - 1.0f) < 0.005f && !fx->night && !eq15_used(fx);
}

/* --- coeficientes ---------------------------------------------------- */

static void biquad_norm(Biquad *q, double b0, double b1, double b2, double a0, double a1, double a2) {
    q->b0 = b0 / a0;
    q->b1 = b1 / a0;
    q->b2 = b2 / a0;
    q->a1 = a1 / a0;
    q->a2 = a2 / a0;
}

static void low_shelf(Biquad *q, double rate, double f0, double db) {
    double A = pow(10.0, db / 40.0), w0 = 2.0 * M_PI * f0 / rate;
    double cw = cos(w0), alpha = sin(w0) / 2.0 * sqrt(2.0); /* pendiente S = 1 */
    double sa = 2.0 * sqrt(A) * alpha;
    biquad_norm(q,
        A * ((A + 1) - (A - 1) * cw + sa),
        2 * A * ((A - 1) - (A + 1) * cw),
        A * ((A + 1) - (A - 1) * cw - sa),
        (A + 1) + (A - 1) * cw + sa,
        -2 * ((A - 1) + (A + 1) * cw),
        (A + 1) + (A - 1) * cw - sa);
}

static void high_shelf(Biquad *q, double rate, double f0, double db) {
    double A = pow(10.0, db / 40.0), w0 = 2.0 * M_PI * f0 / rate;
    double cw = cos(w0), alpha = sin(w0) / 2.0 * sqrt(2.0);
    double sa = 2.0 * sqrt(A) * alpha;
    biquad_norm(q,
        A * ((A + 1) + (A - 1) * cw + sa),
        -2 * A * ((A - 1) + (A + 1) * cw),
        A * ((A + 1) + (A - 1) * cw - sa),
        (A + 1) - (A - 1) * cw + sa,
        2 * ((A - 1) - (A + 1) * cw),
        (A + 1) - (A - 1) * cw - sa);
}

static void peaking(Biquad *q, double rate, double f0, double Q, double db) {
    double A = pow(10.0, db / 40.0), w0 = 2.0 * M_PI * f0 / rate;
    double alpha = sin(w0) / (2.0 * Q), cw = cos(w0);
    biquad_norm(q,
        1 + alpha * A, -2 * cw, 1 - alpha * A,
        1 + alpha / A, -2 * cw, 1 - alpha / A);
}

static inline double biquad_run(Biquad *q, int ch, double x) {
    double y = q->b0 * x + q->z1[ch];
    q->z1[ch] = q->b1 * x - q->a1 * y + q->z2[ch];
    q->z2[ch] = q->b2 * x - q->a2 * y;
    return y;
}

/* --- API -------------------------------------------------------------- */

void dsp_init(Dsp *d, unsigned rate) {
    memset(d, 0, sizeof(*d));
    d->rate     = rate ? rate : 48000;
    d->fx.width = 1.0f;
    d->fx.enabled = true;
    d->preamp   = 1.0f;
    dsp_set(d, &d->fx);
}

void dsp_set(Dsp *d, const CapyroAudioFx *fx) {
    bool was_active = d->active;
    d->fx = *fx;
    dsp_clamp(&d->fx);
    d->active = d->fx.enabled && !dsp_is_neutral(&d->fx);
    if (d->active && !was_active) {
        /* arrancar de cero: el estado viejo es de otra señal */
        memset(d->bass.z1, 0, sizeof(d->bass.z1));     memset(d->bass.z2, 0, sizeof(d->bass.z2));
        memset(d->mid.z1, 0, sizeof(d->mid.z1));       memset(d->mid.z2, 0, sizeof(d->mid.z2));
        memset(d->treble.z1, 0, sizeof(d->treble.z1)); memset(d->treble.z2, 0, sizeof(d->treble.z2));
        for (int i = 0; i < CAPYRO_EQ15_BANDS; i++) {
            memset(d->eq15[i].z1, 0, sizeof(d->eq15[i].z1));
            memset(d->eq15[i].z2, 0, sizeof(d->eq15[i].z2));
        }
        d->comp_env = 0.0f;
    }
    low_shelf(&d->bass, d->rate, BASS_HZ, d->fx.bass_db);
    peaking(&d->mid, d->rate, MID_HZ, MID_Q, d->fx.mid_db);
    high_shelf(&d->treble, d->rate, TREBLE_HZ, d->fx.treble_db);
    for (int i = 0; i < CAPYRO_EQ15_BANDS; i++) {
        peaking(&d->eq15[i], d->rate, EQ15_HZ[i], EQ15_Q, d->fx.eq15[i]);
    }

    /* Margen: si alguna banda sube, se baja todo un poco antes de filtrar
     * para que no sature (el resto lo absorbe el limitador suave). */
    float boost = 0.0f;
    if (eq3_used(&d->fx)) {
        if (d->fx.bass_db > boost)   boost = d->fx.bass_db;
        if (d->fx.mid_db > boost)    boost = d->fx.mid_db;
        if (d->fx.treble_db > boost) boost = d->fx.treble_db;
    }
    if (eq15_used(&d->fx)) {
        for (int i = 0; i < CAPYRO_EQ15_BANDS; i++) {
            if (d->fx.eq15[i] > boost) boost = d->fx.eq15[i];
        }
    }
    d->preamp = powf(10.0f, -0.6f * boost / 20.0f);
}

/* Limitador suave: lineal hasta 0,8 y después se curva hacia 1 sin cortar. */
static inline float soft_clip(float x) {
    float a = fabsf(x);
    if (a <= 0.8f) {
        return x;
    }
    float y = 0.8f + 0.2f * tanhf((a - 0.8f) / 0.2f);
    return x < 0 ? -y : y;
}

void dsp_process(Dsp *d, float *buf, size_t frames) {
    if (!d->active) {
        return;
    }
    const bool eq    = eq3_used(&d->fx);
    const bool eq15  = eq15_used(&d->fx);
    const bool width = fabsf(d->fx.width - 1.0f) >= 0.005f;
    const float w    = d->fx.width;

    const float rate   = (float)d->rate;
    const float att    = expf(-1.0f / (NIGHT_ATTACK_MS * 0.001f * rate));
    const float rel    = expf(-1.0f / (NIGHT_RELEASE_MS * 0.001f * rate));
    const float makeup = powf(10.0f, NIGHT_MAKEUP_DB / 20.0f);

    for (size_t i = 0; i < frames; i++) {
        float l = buf[2 * i], r = buf[2 * i + 1];

        if (eq) {
            double dl = l * d->preamp, dr = r * d->preamp;
            dl = biquad_run(&d->bass, 0, dl);   dr = biquad_run(&d->bass, 1, dr);
            dl = biquad_run(&d->mid, 0, dl);    dr = biquad_run(&d->mid, 1, dr);
            dl = biquad_run(&d->treble, 0, dl); dr = biquad_run(&d->treble, 1, dr);
            l = (float)dl;
            r = (float)dr;
        }

        if (eq15) {
            /* el preamp ya lo aplicó el bloque de arriba si estaba activo */
            double k = eq ? 1.0 : d->preamp;
            double dl = l * k, dr = r * k;
            for (int b = 0; b < CAPYRO_EQ15_BANDS; b++) {
                dl = biquad_run(&d->eq15[b], 0, dl);
                dr = biquad_run(&d->eq15[b], 1, dr);
            }
            l = (float)dl;
            r = (float)dr;
        }

        if (width) {
            /* medio/lados: 0 = mono, 1 = igual, 2 = el doble de "abierto" */
            float m = 0.5f * (l + r), s = 0.5f * (l - r) * w;
            l = m + s;
            r = m - s;
        }

        if (d->fx.night) {
            float peak = fabsf(l) > fabsf(r) ? fabsf(l) : fabsf(r);
            float coef = peak > d->comp_env ? att : rel;
            d->comp_env = coef * d->comp_env + (1.0f - coef) * peak;
            float env_db = 20.0f * log10f(d->comp_env + 1e-9f);
            float g_db = 0.0f;
            if (env_db > NIGHT_THRESHOLD_DB) {
                g_db = (NIGHT_THRESHOLD_DB - env_db) * (1.0f - 1.0f / NIGHT_RATIO);
            }
            float g = powf(10.0f, g_db / 20.0f) * makeup;
            l *= g;
            r *= g;
        }

        buf[2 * i]     = soft_clip(l);
        buf[2 * i + 1] = soft_clip(r);
    }
}

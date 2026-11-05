/* Capyro — capa mínima de plataforma: hilos, mutex, reloj, sleep, archivos
 * con nombres UTF-8 y datos del sistema operativo. Es lo único del núcleo
 * que cambia entre Linux (POSIX) y Windows (Win32). */
#ifndef CAPYRO_PLATFORM_H
#define CAPYRO_PLATFORM_H

#include <stdio.h>

#ifdef _WIN32
#  ifndef WIN32_LEAN_AND_MEAN
#    define WIN32_LEAN_AND_MEAN
#  endif
#  ifndef NOMINMAX
#    define NOMINMAX
#  endif
#  include <windows.h>
typedef HANDLE           cp_thread;
typedef CRITICAL_SECTION cp_mutex;
#else
#  include <pthread.h>
typedef pthread_t       cp_thread;
typedef pthread_mutex_t cp_mutex;
#endif

typedef void (*cp_thread_fn)(void *arg);

int    cp_thread_start(cp_thread *t, cp_thread_fn fn, void *arg); /* 0 = ok */
void   cp_thread_join(cp_thread t);

void   cp_mutex_init(cp_mutex *m);
void   cp_mutex_destroy(cp_mutex *m);
void   cp_mutex_lock(cp_mutex *m);
void   cp_mutex_unlock(cp_mutex *m);

double cp_now(void);         /* segundos, monotónico */
void   cp_sleep_ms(int ms);

/* fopen con la ruta en UTF-8 (en Windows, fopen a secas no entiende acentos). */
FILE  *cp_fopen(const char *utf8_path, const char *mode);
/* rename y remove con rutas UTF-8 (mismo motivo). 0 = ok. */
int    cp_rename(const char *from, const char *to);
int    cp_remove(const char *utf8_path);

/* Sistema operativo en el que corre Capyro (ver CapyroOsInfo). */
struct CapyroOsInfo;
void   cp_os_info(struct CapyroOsInfo *out);

#endif /* CAPYRO_PLATFORM_H */

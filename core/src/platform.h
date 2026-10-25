/* Capyro — capa mínima de plataforma: hilos, mutex, reloj y sleep (POSIX).
 * Capyro por ahora es solo para Linux; si algún día se porta, esto es lo
 * único del núcleo que hay que reimplementar. */
#ifndef CAPYRO_PLATFORM_H
#define CAPYRO_PLATFORM_H

#include <pthread.h>

typedef pthread_t       cp_thread;
typedef pthread_mutex_t cp_mutex;

typedef void (*cp_thread_fn)(void *arg);

int    cp_thread_start(cp_thread *t, cp_thread_fn fn, void *arg); /* 0 = ok */
void   cp_thread_join(cp_thread t);

void   cp_mutex_init(cp_mutex *m);
void   cp_mutex_destroy(cp_mutex *m);
void   cp_mutex_lock(cp_mutex *m);
void   cp_mutex_unlock(cp_mutex *m);

double cp_now(void);         /* segundos, monotónico */
void   cp_sleep_ms(int ms);

#endif /* CAPYRO_PLATFORM_H */

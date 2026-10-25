/* Capyro — capa mínima de plataforma: hilos, mutex, reloj y sleep.
 * POSIX (Linux/BSD/macOS) usa pthreads; Windows usa la API Win32. */
#ifndef CAPYRO_PLATFORM_H
#define CAPYRO_PLATFORM_H

#ifdef _WIN32
#  define WIN32_LEAN_AND_MEAN
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

#endif /* CAPYRO_PLATFORM_H */

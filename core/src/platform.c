#if !defined(_WIN32) && !defined(_POSIX_C_SOURCE)
#  define _POSIX_C_SOURCE 200809L
#endif

#include "platform.h"

#include <stdlib.h>

#ifndef _WIN32
#  include <time.h>
#endif

typedef struct {
    cp_thread_fn fn;
    void        *arg;
} ThreadStart;

#ifdef _WIN32

static DWORD WINAPI thread_trampoline(LPVOID p) {
    ThreadStart s = *(ThreadStart *)p;
    free(p);
    s.fn(s.arg);
    return 0;
}

int cp_thread_start(cp_thread *t, cp_thread_fn fn, void *arg) {
    ThreadStart *s = malloc(sizeof(*s));
    if (!s) {
        return -1;
    }
    s->fn  = fn;
    s->arg = arg;
    *t = CreateThread(NULL, 0, thread_trampoline, s, 0, NULL);
    if (!*t) {
        free(s);
        return -1;
    }
    return 0;
}

void cp_thread_join(cp_thread t) {
    WaitForSingleObject(t, INFINITE);
    CloseHandle(t);
}

void cp_mutex_init(cp_mutex *m)    { InitializeCriticalSection(m); }
void cp_mutex_destroy(cp_mutex *m) { DeleteCriticalSection(m); }
void cp_mutex_lock(cp_mutex *m)    { EnterCriticalSection(m); }
void cp_mutex_unlock(cp_mutex *m)  { LeaveCriticalSection(m); }

double cp_now(void) {
    static LARGE_INTEGER freq = {0};
    LARGE_INTEGER c;
    if (freq.QuadPart == 0) {
        QueryPerformanceFrequency(&freq);
    }
    QueryPerformanceCounter(&c);
    return (double)c.QuadPart / (double)freq.QuadPart;
}

void cp_sleep_ms(int ms) { Sleep((DWORD)ms); }

#else /* POSIX */

static void *thread_trampoline(void *p) {
    ThreadStart s = *(ThreadStart *)p;
    free(p);
    s.fn(s.arg);
    return NULL;
}

int cp_thread_start(cp_thread *t, cp_thread_fn fn, void *arg) {
    ThreadStart *s = malloc(sizeof(*s));
    if (!s) {
        return -1;
    }
    s->fn  = fn;
    s->arg = arg;
    if (pthread_create(t, NULL, thread_trampoline, s) != 0) {
        free(s);
        return -1;
    }
    return 0;
}

void cp_thread_join(cp_thread t) { pthread_join(t, NULL); }

void cp_mutex_init(cp_mutex *m)    { pthread_mutex_init(m, NULL); }
void cp_mutex_destroy(cp_mutex *m) { pthread_mutex_destroy(m); }
void cp_mutex_lock(cp_mutex *m)    { pthread_mutex_lock(m); }
void cp_mutex_unlock(cp_mutex *m)  { pthread_mutex_unlock(m); }

double cp_now(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (double)ts.tv_sec + (double)ts.tv_nsec / 1e9;
}

void cp_sleep_ms(int ms) {
    struct timespec ts = { ms / 1000, (long)(ms % 1000) * 1000000L };
    nanosleep(&ts, NULL);
}

#endif

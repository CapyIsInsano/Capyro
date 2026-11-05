/*
 * Capyro — capa de plataforma. Dos implementaciones en el mismo archivo:
 * POSIX (Linux y macOS) y Win32 (Windows). Se compila solo la que
 * corresponde según el sistema para el que se compila; de macOS solo cambia
 * cómo se averigua la versión del sistema (cp_os_info).
 */
#ifndef _WIN32
#  ifndef _POSIX_C_SOURCE
#    define _POSIX_C_SOURCE 200809L
#  endif
#endif
#ifdef __APPLE__
/* sysctlbyname: con _POSIX_C_SOURCE solo, macOS esconde lo que no es POSIX */
#  define _DARWIN_C_SOURCE
#endif

#include "platform.h"
#include "capyro_core.h"

#include <stdlib.h>
#include <string.h>

typedef struct {
    cp_thread_fn fn;
    void        *arg;
} ThreadStart;

#ifdef _WIN32
/* ====================================================================== */
/* Windows                                                                */
/* ====================================================================== */
#include <wchar.h>

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
    static LARGE_INTEGER freq;
    if (freq.QuadPart == 0) {
        QueryPerformanceFrequency(&freq);
    }
    LARGE_INTEGER c;
    QueryPerformanceCounter(&c);
    return (double)c.QuadPart / (double)freq.QuadPart;
}

void cp_sleep_ms(int ms) { Sleep((DWORD)(ms > 0 ? ms : 0)); }

static wchar_t *utf8_to_wide(const char *s) {
    int n = MultiByteToWideChar(CP_UTF8, 0, s, -1, NULL, 0);
    if (n <= 0) {
        return NULL;
    }
    wchar_t *w = malloc((size_t)n * sizeof(wchar_t));
    if (w) {
        MultiByteToWideChar(CP_UTF8, 0, s, -1, w, n);
    }
    return w;
}

FILE *cp_fopen(const char *utf8_path, const char *mode) {
    wchar_t *wp = utf8_to_wide(utf8_path);
    wchar_t *wm = utf8_to_wide(mode);
    FILE *f = (wp && wm) ? _wfopen(wp, wm) : NULL;
    free(wp);
    free(wm);
    return f;
}

int cp_rename(const char *from, const char *to) {
    wchar_t *wf = utf8_to_wide(from);
    wchar_t *wt = utf8_to_wide(to);
    int r = (wf && wt) ? _wrename(wf, wt) : -1;
    free(wf);
    free(wt);
    return r;
}

int cp_remove(const char *utf8_path) {
    wchar_t *wp = utf8_to_wide(utf8_path);
    int r = wp ? _wremove(wp) : -1;
    free(wp);
    return r;
}

typedef LONG(WINAPI *RtlGetVersionFn)(OSVERSIONINFOW *);

void cp_os_info(CapyroOsInfo *out) {
    memset(out, 0, sizeof(*out));
    out->family = CAPYRO_OS_WINDOWS;
    snprintf(out->code, sizeof(out->code), "Win");
    /* GetVersionEx miente desde Windows 8.1; RtlGetVersion dice la verdad */
    OSVERSIONINFOW v;
    memset(&v, 0, sizeof(v));
    v.dwOSVersionInfoSize = sizeof(v);
    HMODULE ntdll = GetModuleHandleW(L"ntdll.dll");
    RtlGetVersionFn rtl = ntdll ? (RtlGetVersionFn)(void (*)(void))GetProcAddress(ntdll, "RtlGetVersion") : NULL;
    if (rtl) {
        rtl(&v);
    }
    unsigned long major = v.dwMajorVersion, build = v.dwBuildNumber;
    if (major == 10 && build >= 22000) {
        snprintf(out->name, sizeof(out->name), "Windows 11");
        out->major = 11;
    } else if (major == 10) {
        snprintf(out->name, sizeof(out->name), "Windows 10");
        out->major = 10;
    } else {
        snprintf(out->name, sizeof(out->name), "Windows %lu.%lu", major, v.dwMinorVersion);
        out->major = (int)major;
    }
    /* "23H2", "22H2"... (no está en todas las versiones) */
    wchar_t disp[32] = { 0 };
    DWORD sz = sizeof(disp);
    char display[32] = "";
    if (RegGetValueW(HKEY_LOCAL_MACHINE, L"SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion", L"DisplayVersion",
                     RRF_RT_REG_SZ, NULL, disp, &sz) == ERROR_SUCCESS) {
        WideCharToMultiByte(CP_UTF8, 0, disp, -1, display, sizeof(display), NULL, NULL);
    }
    if (display[0]) {
        snprintf(out->version, sizeof(out->version), "%s · build %lu", display, build);
    } else {
        snprintf(out->version, sizeof(out->version), "build %lu", build);
    }
}

#else
/* ====================================================================== */
/* Linux y macOS (POSIX)                                                  */
/* ====================================================================== */
#include <sys/utsname.h>
#include <time.h>
#ifdef __APPLE__
#  include <sys/sysctl.h>
#endif

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

FILE *cp_fopen(const char *utf8_path, const char *mode) {
    return fopen(utf8_path, mode); /* en Linux las rutas ya son bytes UTF-8 */
}

int cp_rename(const char *from, const char *to) {
    return rename(from, to);
}

int cp_remove(const char *utf8_path) {
    return remove(utf8_path);
}

#ifdef __APPLE__
/* Un valor de texto de sysctl ("15.5"); "" si no existe. */
static void sysctl_text(const char *name, char *out, size_t cap) {
    size_t len = cap;
    out[0] = '\0';
    if (sysctlbyname(name, out, &len, NULL, 0) != 0 || len == 0) {
        out[0] = '\0';
    } else {
        out[len < cap ? len : cap - 1] = '\0';
    }
}

/* Un valor numérico de sysctl; -1 si no existe. */
static int sysctl_int(const char *name) {
    int v = 0;
    size_t len = sizeof(v);
    return sysctlbyname(name, &v, &len, NULL, 0) == 0 ? v : -1;
}

void cp_os_info(CapyroOsInfo *out) {
    memset(out, 0, sizeof(*out));
    out->family = CAPYRO_OS_MACOS;
    snprintf(out->code, sizeof(out->code), "Mac");
    char ver[32];
    sysctl_text("kern.osproductversion", ver, sizeof(ver)); /* "15.5" (macOS 10.13.4 o más nuevo) */
    out->major = atoi(ver);
    static const struct {
        int         major;
        const char *name;
    } NAMES[] = {
        { 11, "Big Sur" }, { 12, "Monterey" }, { 13, "Ventura" }, { 14, "Sonoma" },
        { 15, "Sequoia" }, { 26, "Tahoe" },
    };
    const char *nick = NULL;
    for (size_t i = 0; i < sizeof(NAMES) / sizeof(NAMES[0]); i++) {
        if (NAMES[i].major == out->major) {
            nick = NAMES[i].name;
        }
    }
    if (out->major > 0 && nick) {
        snprintf(out->name, sizeof(out->name), "macOS %d %s", out->major, nick);
    } else if (out->major > 0) {
        snprintf(out->name, sizeof(out->name), "macOS %d", out->major);
    } else {
        snprintf(out->name, sizeof(out->name), "macOS");
    }
    /* el chip: Apple Silicon o Intel (y si corre la versión de Intel con Rosetta) */
    const char *chip = sysctl_int("hw.optional.arm64") == 1 ? "Apple Silicon" : "Intel";
    const char *rosetta = sysctl_int("sysctl.proc_translated") == 1 ? " (Rosetta)" : "";
    if (ver[0]) {
        snprintf(out->version, sizeof(out->version), "%.16s · %s%s", ver, chip, rosetta);
    } else {
        struct utsname u;
        if (uname(&u) == 0) {
            snprintf(out->version, sizeof(out->version), "Darwin %.24s · %s%s", u.release, chip, rosetta);
        }
    }
}
#else
void cp_os_info(CapyroOsInfo *out) {
    memset(out, 0, sizeof(*out));
    out->family = CAPYRO_OS_LINUX;
    out->major  = 0;
    snprintf(out->code, sizeof(out->code), "Lin");
    snprintf(out->name, sizeof(out->name), "Linux");
    /* nombre de la distro: PRETTY_NAME de /etc/os-release */
    FILE *f = fopen("/etc/os-release", "r");
    if (!f) {
        f = fopen("/usr/lib/os-release", "r");
    }
    if (f) {
        char line[256];
        while (fgets(line, sizeof(line), f)) {
            if (strncmp(line, "PRETTY_NAME=", 12) == 0) {
                char *v = line + 12;
                v[strcspn(v, "\r\n")] = '\0';
                if (*v == '"' || *v == '\'') {
                    char q = *v++;
                    char *end = strrchr(v, q);
                    if (end) {
                        *end = '\0';
                    }
                }
                if (*v) {
                    snprintf(out->name, sizeof(out->name), "%.63s", v);
                }
                break;
            }
        }
        fclose(f);
    }
    struct utsname u;
    if (uname(&u) == 0) {
        snprintf(out->version, sizeof(out->version), "kernel %.56s", u.release);
    }
}
#endif /* __APPLE__ */
#endif

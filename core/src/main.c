/*
 * Capyro — Just press play.
 *
 * C es dueño del arranque y del apagado: crea el core, abre los archivos
 * pasados por línea de comandos y le entrega el control a la GUI (Rust),
 * que corre el event loop y llama de vuelta a capyro_core.h en cada frame.
 */
#include "capyro_core.h"

#include <stdio.h>
#ifdef __GLIBC__
#  include <malloc.h>
#endif
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#  ifndef WIN32_LEAN_AND_MEAN
#    define WIN32_LEAN_AND_MEAN
#  endif
#  include <windows.h>
#  include <shellapi.h>

/* En Windows argv viene en la página de códigos vieja (sin acentos ni ñ):
 * se piden los argumentos en UTF-16 y se pasan a UTF-8, que es lo que usan
 * el núcleo y FFmpeg. */
static char **utf8_argv(int *argc) {
    LPWSTR *w = CommandLineToArgvW(GetCommandLineW(), argc);
    if (!w) {
        return NULL;
    }
    char **out = calloc((size_t)*argc + 1, sizeof(char *));
    for (int i = 0; out && i < *argc; i++) {
        int n = WideCharToMultiByte(CP_UTF8, 0, w[i], -1, NULL, 0, NULL, NULL);
        out[i] = malloc((size_t)(n > 0 ? n : 1));
        if (out[i]) {
            out[i][0] = '\0';
            WideCharToMultiByte(CP_UTF8, 0, w[i], -1, out[i], n, NULL, NULL);
        }
    }
    LocalFree(w);
    return out;
}

/* Capyro es una app de ventana (sin consola): para --help y --version se
 * escribe en la consola desde la que se lo llamó, si hay una. */
static void attach_console(void) {
    if (AttachConsole(ATTACH_PARENT_PROCESS)) {
        freopen("CONOUT$", "w", stdout);
        freopen("CONOUT$", "w", stderr);
    }
}
#endif

/* Implementada en gui/src/lib.rs */
extern int capyro_gui_run(CapyroCore *core);

static void print_help(const char *argv0) {
    printf("Capyro %s — Just press play.\n\n", capyro_core_version());
    printf("Uso: %s [archivo ...]\n", argv0);
    printf("Formatos: video %s · audio %s\n\n", capyro_core_supported_video(), capyro_core_supported_audio());
    printf("  -h, --help       muestra esta ayuda\n");
    printf("  -v, --version    muestra la versión\n\n");
    printf("Los archivos pasados se agregan a la lista; el primero queda cargado.\n");
#ifdef _WIN32
    printf("Config: Documentos\\Capyro\\Config\\Capyro.lua (o la que elegiste al instalar)\n");
#else
    printf("Config: ~/.config/capyro/capyro.lua (ver config/capyro.lua)\n");
#endif
}

int main(int argc, char **argv) {
#ifdef __GLIBC__
    /* Los cuadros de video (de 1 MB para arriba) van directo a memoria
     * propia del sistema y se devuelven al soltarlos. Si no, glibc los deja
     * en el montón y la memoria del proceso queda inflada mientras se ve un
     * video (unos 15 MB menos con un 1080p). */
    mallopt(M_MMAP_THRESHOLD, 256 * 1024);
#endif
#ifdef _WIN32
    char **wargv = utf8_argv(&argc);
    if (wargv) {
        argv = wargv;
    }
    for (int i = 1; i < argc; i++) {
        if (argv[i][0] == '-') {
            attach_console();
            break;
        }
    }
#endif
    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "-h") == 0 || strcmp(argv[i], "--help") == 0) {
            print_help(argv[0]);
            return 0;
        }
        if (strcmp(argv[i], "-v") == 0 || strcmp(argv[i], "--version") == 0) {
            const CapyroBuildInfo *bi = capyro_core_build_info();
            printf("Capyro %s\n", capyro_core_version());
            if (bi->commit[0]) {
                printf("commit %.12s%s (%s)\n", bi->commit, bi->dirty ? "+cambios" : "",
                       bi->branch[0] ? bi->branch : "sin rama");
            }
            printf("instalado en %s\n", bi->bindir);
            CapyroOsInfo os;
            capyro_core_os_info(&os);
            printf("sistema: %s (%s) · compilado para %s\n", os.name, os.version, bi->system);
            return 0;
        }
    }

    CapyroCore *core = capyro_core_init();
    if (!core) {
        fprintf(stderr, "capyro: no se pudo inicializar el núcleo\n");
        return 1;
    }

    for (int i = 1; i < argc; i++) {
        int err = capyro_core_playlist_add(core, argv[i]);
        if (err != CAPYRO_OK) {
            fprintf(stderr, "capyro: %s: %s\n", argv[i], capyro_core_error_string(err));
        }
    }
    /* el primero que abra (si uno está roto, no se queda todo parado) */
    for (size_t i = 0; i < capyro_core_playlist_count(core); i++) {
        int err = capyro_core_playlist_select(core, i);
        if (err == CAPYRO_OK) {
            break;
        }
        fprintf(stderr, "capyro: %s: %s\n", capyro_core_playlist_get(core, i), capyro_core_error_string(err));
    }

    int ret = capyro_gui_run(core);

    capyro_core_shutdown(core);
    return ret;
}

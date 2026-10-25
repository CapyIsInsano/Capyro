/*
 * Capyro — Just press play.
 *
 * C es dueño del arranque y del apagado: crea el core, abre los archivos
 * pasados por línea de comandos y le entrega el control a la GUI (Rust),
 * que corre el event loop y llama de vuelta a capyro_core.h en cada frame.
 */
#include "capyro_core.h"

#include <stdio.h>
#include <string.h>

/* Implementada en gui/src/lib.rs */
extern int capyro_gui_run(CapyroCore *core);

static void print_help(const char *argv0) {
    printf("Capyro %s — Just press play.\n\n", capyro_core_version());
    printf("Uso: %s [archivo ...]\n", argv0);
    printf("Formatos: video %s · audio %s\n\n", capyro_core_supported_video(), capyro_core_supported_audio());
    printf("  -h, --help       muestra esta ayuda\n");
    printf("  -v, --version    muestra la versión\n\n");
    printf("Los archivos pasados se agregan a la lista; el primero queda cargado.\n");
    printf("Config: ~/.config/capyro/capyro.lua (ver config/capyro.lua)\n");
}

int main(int argc, char **argv) {
    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "-h") == 0 || strcmp(argv[i], "--help") == 0) {
            print_help(argv[0]);
            return 0;
        }
        if (strcmp(argv[i], "-v") == 0 || strcmp(argv[i], "--version") == 0) {
            printf("Capyro %s\n", capyro_core_version());
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
    if (capyro_core_playlist_count(core) > 0) {
        int err = capyro_core_playlist_select(core, 0);
        if (err != CAPYRO_OK) {
            fprintf(stderr, "capyro: %s: %s\n",
                    capyro_core_playlist_get(core, 0), capyro_core_error_string(err));
        }
    }

    int ret = capyro_gui_run(core);

    capyro_core_shutdown(core);
    return ret;
}

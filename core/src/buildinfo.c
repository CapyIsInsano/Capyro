/*
 * Capyro — datos del build (commit de git, prefijo de instalación).
 * capyro_buildinfo.h lo genera cmake/buildinfo.cmake en cada compilación.
 */
#include "capyro_core.h"
#include "capyro_buildinfo.h"

static const CapyroBuildInfo info = {
    .commit      = CAPYRO_GIT_COMMIT,
    .branch      = CAPYRO_GIT_BRANCH,
    .remote      = CAPYRO_GIT_REMOTE,
    .subject     = CAPYRO_GIT_SUBJECT,
    .commit_time = CAPYRO_GIT_TIME,
    .dirty       = CAPYRO_GIT_DIRTY,
    .prefix      = CAPYRO_INSTALL_PREFIX,
    .bindir      = CAPYRO_INSTALL_BINDIR,
    .build_type  = CAPYRO_BUILD_TYPE,
    .dist        = CAPYRO_BUILD_DIST,
    .native      = CAPYRO_BUILD_NATIVE,
    .system      = CAPYRO_SYSTEM_CODE,
};

const CapyroBuildInfo *capyro_core_build_info(void) {
    return &info;
}

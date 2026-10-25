# ---------------------------------------------------------------------------
# Genera capyro_buildinfo.h con el commit de git del que sale este build.
#
# Corre en CADA compilación (cmake -P, desde el target capyro_buildinfo),
# pero solo reescribe el header si algo cambió: así un build sin cambios no
# recompila nada. El actualizador usa estos datos para saber si hay una
# versión nueva en el repo y cómo volver a compilar e instalar Capyro.
#
# Entradas (-D): SRC, OUT, PREFIX, BINDIR, BUILD_TYPE, DIST, NATIVE
# ---------------------------------------------------------------------------

set(commit "")
set(branch "")
set(remote "")
set(subject "")
set(ctime "0")
set(dirty 0)

find_program(GIT_EXECUTABLE git)
if(GIT_EXECUTABLE AND EXISTS "${SRC}/.git")
    macro(git_out var)
        execute_process(COMMAND "${GIT_EXECUTABLE}" -C "${SRC}" ${ARGN}
            OUTPUT_VARIABLE ${var} ERROR_QUIET OUTPUT_STRIP_TRAILING_WHITESPACE
            RESULT_VARIABLE _rc)
        if(NOT _rc EQUAL 0)
            set(${var} "")
        endif()
    endmacro()
    git_out(commit rev-parse HEAD)
    git_out(branch rev-parse --abbrev-ref HEAD)
    git_out(remote remote get-url origin)
    git_out(subject log -1 --format=%s)
    git_out(ctime log -1 --format=%ct)
    git_out(status status --porcelain --untracked-files=no)
    if(NOT status STREQUAL "")
        set(dirty 1)
    endif()
    if(branch STREQUAL "HEAD")
        set(branch "")
    endif()
    if(ctime STREQUAL "")
        set(ctime "0")
    endif()
endif()

# Comillas y barras escapadas para que entren en un literal de C.
foreach(v commit branch remote subject PREFIX BINDIR BUILD_TYPE)
    string(REPLACE "\\" "\\\\" ${v} "${${v}}")
    string(REPLACE "\"" "\\\"" ${v} "${${v}}")
endforeach()
foreach(v DIST NATIVE)
    if(${v})
        set(${v} 1)
    else()
        set(${v} 0)
    endif()
endforeach()

set(content "/* Generado por cmake/buildinfo.cmake: no editar. */
#define CAPYRO_GIT_COMMIT  \"${commit}\"
#define CAPYRO_GIT_BRANCH  \"${branch}\"
#define CAPYRO_GIT_REMOTE  \"${remote}\"
#define CAPYRO_GIT_SUBJECT \"${subject}\"
#define CAPYRO_GIT_TIME    ${ctime}LL
#define CAPYRO_GIT_DIRTY   ${dirty}
#define CAPYRO_INSTALL_PREFIX \"${PREFIX}\"
#define CAPYRO_INSTALL_BINDIR \"${BINDIR}\"
#define CAPYRO_BUILD_TYPE  \"${BUILD_TYPE}\"
#define CAPYRO_BUILD_DIST  ${DIST}
#define CAPYRO_BUILD_NATIVE ${NATIVE}
")

if(EXISTS "${OUT}")
    file(READ "${OUT}" old)
else()
    set(old "")
endif()
if(NOT old STREQUAL content)
    file(WRITE "${OUT}" "${content}")
endif()

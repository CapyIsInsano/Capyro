# ---------------------------------------------------------------------------
# Núcleo de Capyro (C11): FFmpeg para leer/decodificar, miniaudio para sonar.
# ---------------------------------------------------------------------------

# FFmpeg >= 5.1 (API de AVChannelLayout). libavfilter: atempo para la velocidad.
find_package(PkgConfig REQUIRED)
pkg_check_modules(FFMPEG REQUIRED IMPORTED_TARGET
    libavformat>=59.27
    libavcodec>=59.37
    libavutil>=57.28
    libavfilter>=8.44
    libswscale>=6.7)

# miniaudio: header único. Usa el del sistema si existe; si no, lo baja.
find_path(MINIAUDIO_INCLUDE_DIR miniaudio.h PATH_SUFFIXES miniaudio)
if(NOT MINIAUDIO_INCLUDE_DIR)
    include(FetchContent)
    FetchContent_Declare(
        miniaudio
        GIT_REPOSITORY https://github.com/mackron/miniaudio.git
        GIT_TAG        0.11.25
        GIT_SHALLOW    TRUE
        SOURCE_SUBDIR  _no_cmake_ # solo queremos el header, no su build
    )
    FetchContent_MakeAvailable(miniaudio)
    set(MINIAUDIO_INCLUDE_DIR ${miniaudio_SOURCE_DIR})
endif()
message(STATUS "Capyro: miniaudio en ${MINIAUDIO_INCLUDE_DIR}")

find_package(Threads REQUIRED)

add_library(capyro_core STATIC
    src/core.c
    src/player.c
    src/sfx.c
    src/dsp.c
    src/hwdec.c
    src/buildinfo.c
    src/platform.c
    src/miniaudio_impl.c
)
# capyro_buildinfo.h lo genera el target capyro_buildinfo (CMakeLists raíz).
add_dependencies(capyro_core capyro_buildinfo)
target_include_directories(capyro_core PRIVATE ${CMAKE_BINARY_DIR}/generated)
target_include_directories(capyro_core
    PUBLIC  ${CMAKE_CURRENT_SOURCE_DIR}/include
    PRIVATE ${MINIAUDIO_INCLUDE_DIR})
target_compile_definitions(capyro_core PRIVATE
    MA_NO_DECODING MA_NO_ENCODING MA_NO_GENERATION
    MA_NO_RESOURCE_MANAGER MA_NO_NODE_GRAPH MA_NO_ENGINE)
target_link_libraries(capyro_core PUBLIC PkgConfig::FFMPEG Threads::Threads ${CMAKE_DL_LIBS})

target_link_libraries(capyro_core PUBLIC m)

# Warnings solo para nuestro código (miniaudio va aparte).
set_source_files_properties(src/core.c src/player.c src/sfx.c src/dsp.c src/hwdec.c src/buildinfo.c src/platform.c
    PROPERTIES COMPILE_OPTIONS "-Wall;-Wextra;-Wpedantic")
# miniaudio son ~4 MB de C: con -O2 compila un 40 % más rápido y lo que hace
# en tiempo real (copiar buffers al dispositivo) no nota la diferencia.
set_source_files_properties(src/miniaudio_impl.c PROPERTIES COMPILE_OPTIONS "-O2")

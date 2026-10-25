# ---------------------------------------------------------------------------
# Núcleo de Capyro (C11): FFmpeg para leer/decodificar, miniaudio para sonar.
# ---------------------------------------------------------------------------

# FFmpeg >= 5.1 (API de AVChannelLayout)
find_package(PkgConfig REQUIRED)
pkg_check_modules(FFMPEG REQUIRED IMPORTED_TARGET
    libavformat>=59.27
    libavcodec>=59.37
    libavutil>=57.28
    libswresample>=4.7
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
    src/platform.c
    src/miniaudio_impl.c
)
target_include_directories(capyro_core
    PUBLIC  ${CMAKE_CURRENT_SOURCE_DIR}/include
    PRIVATE ${MINIAUDIO_INCLUDE_DIR})
target_compile_definitions(capyro_core PRIVATE
    MA_NO_DECODING MA_NO_ENCODING MA_NO_GENERATION
    MA_NO_RESOURCE_MANAGER MA_NO_NODE_GRAPH MA_NO_ENGINE)
target_link_libraries(capyro_core PUBLIC PkgConfig::FFMPEG Threads::Threads ${CMAKE_DL_LIBS})

if(UNIX AND NOT APPLE)
    target_link_libraries(capyro_core PUBLIC m)
elseif(APPLE)
    target_link_libraries(capyro_core PUBLIC
        "-framework CoreFoundation" "-framework CoreAudio" "-framework AudioToolbox")
endif()

# Warnings solo para nuestro código (miniaudio va aparte).
set(CAPYRO_WARN $<IF:$<C_COMPILER_ID:MSVC>,/W4,-Wall -Wextra -Wpedantic>)
set_source_files_properties(src/core.c src/player.c src/platform.c
    PROPERTIES COMPILE_OPTIONS "${CAPYRO_WARN}")

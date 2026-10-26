# ---------------------------------------------------------------------------
# Toolchain para compilar Capyro para Windows desde Linux (mingw-w64).
#
#   cmake -B build-win -DCMAKE_TOOLCHAIN_FILE=cmake/mingw-w64.cmake \
#         -DCAPYRO_FFMPEG_DIR=/ruta/a/ffmpeg-win64-shared
#   cmake --build build-win && cmake --install build-win --prefix dist-win
#
# CAPYRO_FFMPEG_DIR: un FFmpeg "shared" para win64 (por ejemplo los de
# github.com/BtbN/FFmpeg-Builds) con include/, lib/ (los .dll.a),
# lib/pkgconfig/ y bin/ (las .dll, que se copian al instalar).
#
# Rust: hace falta el target x86_64-pc-windows-gnu (rustup target add ...).
# ---------------------------------------------------------------------------
set(CMAKE_SYSTEM_NAME Windows)
set(CMAKE_SYSTEM_PROCESSOR x86_64)

set(_mingw x86_64-w64-mingw32)
set(CMAKE_C_COMPILER   ${_mingw}-gcc)
set(CMAKE_CXX_COMPILER ${_mingw}-g++)
set(CMAKE_RC_COMPILER  ${_mingw}-windres)
set(CMAKE_AR           ${_mingw}-ar CACHE FILEPATH "")
set(CMAKE_RANLIB       ${_mingw}-ranlib CACHE FILEPATH "")

set(CMAKE_FIND_ROOT_PATH /usr/${_mingw})
set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_PACKAGE ONLY)

# FFmpeg para Windows: sus .pc (pkg-config) y no los del sistema Linux.
if(CAPYRO_FFMPEG_DIR)
    set(ENV{PKG_CONFIG_LIBDIR} "${CAPYRO_FFMPEG_DIR}/lib/pkgconfig")
    set(ENV{PKG_CONFIG_PATH} "")
    list(APPEND CMAKE_FIND_ROOT_PATH "${CAPYRO_FFMPEG_DIR}")
endif()

# Corrosion: el crate de la GUI se compila para Windows.
set(Rust_CARGO_TARGET x86_64-pc-windows-gnu)

# Para copiar las DLL al instalar (file(GET_RUNTIME_DEPENDENCIES)).
set(CMAKE_GET_RUNTIME_DEPENDENCIES_PLATFORM "windows+pe")
set(CMAKE_GET_RUNTIME_DEPENDENCIES_TOOL "objdump")
set(CMAKE_GET_RUNTIME_DEPENDENCIES_COMMAND ${_mingw}-objdump)

# Las pruebas (test_core.exe) se pueden correr con wine si está.
find_program(_wine NAMES wine64 wine PATHS /usr/lib/wine NO_CMAKE_FIND_ROOT_PATH)
if(_wine)
    set(CMAKE_CROSSCOMPILING_EMULATOR ${_wine})
endif()

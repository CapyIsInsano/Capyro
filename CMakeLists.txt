cmake_minimum_required(VERSION 3.22)
project(capyro
    VERSION 0.2.7
    DESCRIPTION "Capyro — Just press play."
    LANGUAGES C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS OFF)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

if(NOT CMAKE_SYSTEM_NAME STREQUAL "Linux")
    message(FATAL_ERROR "Capyro por ahora solo se puede compilar en Linux.")
endif()

if(NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)
    set(CMAKE_BUILD_TYPE Release CACHE STRING "Tipo de build" FORCE)
endif()

option(CAPYRO_BUILD_TESTS "Compilar las pruebas del núcleo" ON)
option(CAPYRO_DIST   "Build de distribución: LTO en C y Rust (más lento de compilar, binario más chico)" OFF)
option(CAPYRO_NATIVE "Optimizar para el procesador de esta máquina (el binario no sirve en otras)" OFF)
option(CAPYRO_CCACHE "Usar ccache para el C si está instalado" ON)

# --- velocidad de compilación y optimización ----------------------------
if(CAPYRO_CCACHE)
    find_program(CCACHE_PROGRAM ccache)
    if(CCACHE_PROGRAM)
        set(CMAKE_C_COMPILER_LAUNCHER ${CCACHE_PROGRAM})
        message(STATUS "Capyro: usando ccache")
    endif()
endif()

# Cada función en su propia sección: así el linker tira lo que no se usa.
add_compile_options(-ffunction-sections -fdata-sections)

if(CAPYRO_NATIVE)
    add_compile_options(-march=native)
endif()

if(CAPYRO_DIST)
    include(CheckIPOSupported)
    check_ipo_supported(RESULT CAPYRO_IPO OUTPUT CAPYRO_IPO_MSG LANGUAGES C)
    if(CAPYRO_IPO)
        set(CMAKE_INTERPROCEDURAL_OPTIMIZATION ON)
    else()
        message(WARNING "Capyro: el compilador no soporta LTO (${CAPYRO_IPO_MSG})")
    endif()
endif()
message(STATUS "Capyro: dist=${CAPYRO_DIST} native=${CAPYRO_NATIVE}")

# --- Corrosion: integra Cargo dentro de CMake --------------------------
# Usa el Corrosion del sistema si existe; si no, lo baja de GitHub.
find_package(Corrosion QUIET)
if(NOT Corrosion_FOUND)
    include(FetchContent)
    FetchContent_Declare(
        Corrosion
        GIT_REPOSITORY https://github.com/corrosion-rs/corrosion.git
        GIT_TAG        v0.5.1
        GIT_SHALLOW    TRUE
    )
    FetchContent_MakeAvailable(Corrosion)
endif()

include(GNUInstallDirs)

# --- datos del build (commit de git, prefijo de instalación) -------------
# Los usa el actualizador ("Buscar actualizaciones" en el menú ⓘ). Se
# regeneran en cada build, pero el header solo cambia si cambió el commit.
set(CAPYRO_BUILDINFO_H ${CMAKE_CURRENT_BINARY_DIR}/generated/capyro_buildinfo.h)
add_custom_target(capyro_buildinfo
    COMMAND ${CMAKE_COMMAND}
        -DSRC=${CMAKE_CURRENT_SOURCE_DIR}
        -DOUT=${CAPYRO_BUILDINFO_H}
        -DPREFIX=${CMAKE_INSTALL_PREFIX}
        -DBINDIR=${CMAKE_INSTALL_FULL_BINDIR}
        -DBUILD_TYPE=${CMAKE_BUILD_TYPE}
        -DDIST=${CAPYRO_DIST}
        -DNATIVE=${CAPYRO_NATIVE}
        -P ${CMAKE_CURRENT_SOURCE_DIR}/cmake/buildinfo.cmake
    BYPRODUCTS ${CAPYRO_BUILDINFO_H}
    COMMENT "Capyro: datos del build"
    VERBATIM)

add_subdirectory(core)
add_subdirectory(gui)

# --- ejecutable final ---------------------------------------------------
add_executable(capyro core/src/main.c)
target_link_libraries(capyro PRIVATE capyro_gui capyro_core)
# Tirar el código que nadie usa (de las dependencias de Rust sobre todo) y,
# en Release, sacar los símbolos: el ejecutable pasa de ~21 MB a ~10 MB.
target_link_options(capyro PRIVATE -Wl,--gc-sections -Wl,-O1 -Wl,--as-needed
    $<$<CONFIG:Release,MinSizeRel>:-s>)

# --- pruebas ------------------------------------------------------------
if(CAPYRO_BUILD_TESTS)
    enable_testing()
    add_executable(test_core tests/test_core.c)
    target_link_libraries(test_core PRIVATE capyro_core m)
    # Prueba de caja blanca: mira el estado interno del reloj de audio.
    target_include_directories(test_core PRIVATE core/src)

    # Media de prueba generada con el ffmpeg de línea de comandos (opcional).
    # Se genera al correr ctest, nunca durante el build.
    find_program(FFMPEG_EXECUTABLE ffmpeg)
    if(FFMPEG_EXECUTABLE)
        set(TEST_MEDIA ${CMAKE_CURRENT_BINARY_DIR}/test-media)
        add_test(NAME generar_media
            COMMAND ${CMAKE_COMMAND} -DFFMPEG=${FFMPEG_EXECUTABLE} -DOUT=${TEST_MEDIA}
                    -P ${CMAKE_CURRENT_SOURCE_DIR}/tests/gen_media.cmake)
        set_tests_properties(generar_media PROPERTIES FIXTURES_SETUP media)
        add_test(NAME core COMMAND test_core ${TEST_MEDIA} WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR})
        set_tests_properties(core PROPERTIES FIXTURES_REQUIRED media)
    else()
        message(STATUS "Capyro: no encontré ffmpeg (CLI); las pruebas no van a decodificar")
        add_test(NAME core COMMAND test_core WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR})
    endif()
endif()

# --- instalación --------------------------------------------------------
install(TARGETS capyro RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR})
install(FILES config/capyro.lua DESTINATION ${CMAKE_INSTALL_DATADIR}/capyro)
install(FILES packaging/capyro.desktop DESTINATION ${CMAKE_INSTALL_DATADIR}/applications)
# Ícono en todos los tamaños: la barra de tareas (Waybar, Niri, KDE...) busca
# "capyro" en el tema hicolor según el app_id de la ventana y el .desktop.
# Si falta el tamaño que pide o el caché de íconos está viejo, muestra el
# ícono de "imagen perdida".
install(DIRECTORY packaging/icons/hicolor DESTINATION ${CMAKE_INSTALL_DATADIR}/icons)
install(FILES assets/capyro_logo_256.png DESTINATION ${CMAKE_INSTALL_DATADIR}/pixmaps RENAME capyro.png)
# Refresca los cachés para que el ícono aparezca sin cerrar sesión.
# (Con DESTDIR, al armar un paquete, no: de eso se encarga el gestor de paquetes.)
install(CODE [[
    if(NOT "$ENV{DESTDIR}" STREQUAL "")
        return()
    endif()
    set(_share "${CMAKE_INSTALL_PREFIX}/share")
    find_program(_gtk_cache NAMES gtk-update-icon-cache gtk4-update-icon-cache)
    if(_gtk_cache AND EXISTS "${_share}/icons/hicolor")
        execute_process(COMMAND "${_gtk_cache}" -q -f -t "${_share}/icons/hicolor"
                        RESULT_VARIABLE _rc OUTPUT_QUIET ERROR_QUIET)
        message(STATUS "Capyro: caché de íconos actualizado")
    endif()
    find_program(_desktop_db update-desktop-database)
    if(_desktop_db)
        execute_process(COMMAND "${_desktop_db}" -q "${_share}/applications" OUTPUT_QUIET ERROR_QUIET)
    endif()
]])

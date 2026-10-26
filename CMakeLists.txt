cmake_minimum_required(VERSION 3.22)
project(capyro
    VERSION 0.3.0
    DESCRIPTION "Capyro — Just press play."
    LANGUAGES C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS OFF)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

# --- sistema: Linux ("Lin") o Windows ("Win") -----------------------------
# Se detecta solo según para qué se compila. Todo lo que es de un sistema
# (scripts, íconos, .desktop, DLL, recursos .rc...) solo se compila e instala
# en ese sistema. El código queda guardado en la config instalada
# (options.system) y el actualizador lo usa para instalar solo lo que va.
set(CAPYRO_SYSTEM "auto" CACHE STRING "Sistema destino: auto, Lin o Win")
set_property(CACHE CAPYRO_SYSTEM PROPERTY STRINGS auto Lin Win)
if(CMAKE_SYSTEM_NAME STREQUAL "Linux")
    set(_capyro_detected Lin)
elseif(CMAKE_SYSTEM_NAME STREQUAL "Windows")
    set(_capyro_detected Win)
else()
    message(FATAL_ERROR "Capyro se compila en Linux o Windows (este es ${CMAKE_SYSTEM_NAME}).")
endif()
string(TOLOWER "${CAPYRO_SYSTEM}" _capyro_req)
if(_capyro_req STREQUAL "auto" OR _capyro_req STREQUAL "")
    set(CAPYRO_SYSTEM_CODE ${_capyro_detected})
elseif(_capyro_req MATCHES "^(lin|linux)$")
    set(CAPYRO_SYSTEM_CODE Lin)
elseif(_capyro_req MATCHES "^(win|windows)$")
    set(CAPYRO_SYSTEM_CODE Win)
else()
    message(FATAL_ERROR "CAPYRO_SYSTEM='${CAPYRO_SYSTEM}': usá auto, Lin o Win")
endif()
if(NOT CAPYRO_SYSTEM_CODE STREQUAL _capyro_detected)
    message(FATAL_ERROR "CAPYRO_SYSTEM=${CAPYRO_SYSTEM_CODE} pero se está compilando para "
        "${CMAKE_SYSTEM_NAME} (${_capyro_detected}). Para Windows desde Linux usá "
        "-DCMAKE_TOOLCHAIN_FILE=cmake/mingw-w64.cmake")
endif()
message(STATUS "Capyro: sistema ${CAPYRO_SYSTEM_CODE} (${CMAKE_SYSTEM_NAME})")

if(WIN32)
    enable_language(RC)
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
        -DSYSTEM=${CAPYRO_SYSTEM_CODE}
        -P ${CMAKE_CURRENT_SOURCE_DIR}/cmake/buildinfo.cmake
    BYPRODUCTS ${CAPYRO_BUILDINFO_H}
    COMMENT "Capyro: datos del build"
    VERBATIM)

add_subdirectory(core)
add_subdirectory(gui)

# --- ejecutable final ---------------------------------------------------
if(WIN32)
    # Ícono, versión y manifiesto dentro del .exe. WIN32 = sin consola negra
    # (si se le pasan opciones, main.c se engancha a la consola que lo abrió).
    set(CAPYRO_ICO ${CMAKE_CURRENT_SOURCE_DIR}/packaging/windows/capyro.ico)
    set(CAPYRO_MANIFEST ${CMAKE_CURRENT_SOURCE_DIR}/packaging/windows/capyro.manifest)
    set(CAPYRO_VERSION_STRING ${PROJECT_VERSION})
    configure_file(packaging/windows/capyro.rc.in ${CMAKE_CURRENT_BINARY_DIR}/generated/capyro.rc @ONLY)
    add_executable(capyro WIN32 core/src/main.c ${CMAKE_CURRENT_BINARY_DIR}/generated/capyro.rc)
else()
    add_executable(capyro core/src/main.c)
endif()
target_link_libraries(capyro PRIVATE capyro_gui capyro_core)
# Tirar el código que nadie usa (de las dependencias de Rust sobre todo) y,
# en Release, sacar los símbolos: el ejecutable pasa de ~21 MB a ~10 MB.
target_link_options(capyro PRIVATE -Wl,--gc-sections -Wl,-O1
    $<$<CONFIG:Release,MinSizeRel>:-s>)
if(WIN32)
    # libgcc y winpthread adentro del .exe: dos DLL menos para llevar.
    target_link_options(capyro PRIVATE -static-libgcc
        -Wl,-Bstatic,--whole-archive -lwinpthread -Wl,--no-whole-archive,-Bdynamic)
else()
    target_link_options(capyro PRIVATE -Wl,--as-needed)
endif()

# --- config instalada: la de ejemplo + el sistema detectado --------------
# options.system = "auto" → "Lin" o "Win". Se regenera si cambia la de ejemplo.
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS ${CMAKE_CURRENT_SOURCE_DIR}/config/capyro.lua)
file(READ config/capyro.lua _capyro_cfg)
string(REGEX REPLACE "(\n[ \t]*system[ \t]*=[ \t]*)\"[A-Za-z]*\"" "\\1\"${CAPYRO_SYSTEM_CODE}\""
    _capyro_cfg_installed "${_capyro_cfg}")
if(_capyro_cfg_installed STREQUAL _capyro_cfg AND NOT _capyro_cfg MATCHES "system[ \t]*=[ \t]*\"${CAPYRO_SYSTEM_CODE}\"")
    message(FATAL_ERROR "config/capyro.lua no tiene la opción system = \"...\"")
endif()
set(CAPYRO_INSTALLED_CONFIG ${CMAKE_CURRENT_BINARY_DIR}/generated/capyro.lua)
file(CONFIGURE OUTPUT ${CAPYRO_INSTALLED_CONFIG} CONTENT "${_capyro_cfg_installed}" @ONLY)

# --- pruebas ------------------------------------------------------------
if(CAPYRO_BUILD_TESTS)
    enable_testing()
    add_executable(test_core tests/test_core.c)
    target_link_libraries(test_core PRIVATE capyro_core)
    if(WIN32)
        target_link_options(test_core PRIVATE -static-libgcc
            -Wl,-Bstatic,--whole-archive -lwinpthread -Wl,--no-whole-archive,-Bdynamic)
    endif()
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
if(WIN32)
    # Windows: todo en una carpeta (por defecto la elige el instalador,
    # %LOCALAPPDATA%\Programs\Capyro):
    #   capyro.exe, las DLL de FFmpeg, config\capyro.lua y capyro.ico
    install(TARGETS capyro RUNTIME DESTINATION .)
    install(FILES ${CAPYRO_INSTALLED_CONFIG} DESTINATION config)
    install(FILES packaging/windows/capyro.ico DESTINATION .)
    install(FILES LICENSE DESTINATION . RENAME LICENSE.txt)
    # La licencia de FFmpeg va con sus DLL.
    foreach(_lic "${CAPYRO_FFMPEG_DIR}/LICENSE.txt" "${FFMPEG_DLL_DIR}/../LICENSE.txt")
        if(EXISTS "${_lic}")
            install(FILES "${_lic}" DESTINATION . RENAME FFMPEG-LICENSE.txt)
            break()
        endif()
    endforeach()
    # La versión, para el instalador (CapyroInstaller la muestra al terminar).
    file(STRINGS core/include/capyro_core.h _capyro_ver_line REGEX "#define CAPYRO_VERSION ")
    string(REGEX REPLACE ".*\"(.*)\".*" "\\1" CAPYRO_VERSION_FULL "${_capyro_ver_line}")
    file(WRITE ${CMAKE_CURRENT_BINARY_DIR}/generated/version.txt "${CAPYRO_VERSION_FULL}\n")
    install(FILES ${CMAKE_CURRENT_BINARY_DIR}/generated/version.txt DESTINATION .)
    # Las DLL que necesita capyro.exe (FFmpeg y lo que FFmpeg pida), sacadas
    # de las carpetas bin/ de FFmpeg y del compilador. Las de Windows no.
    get_filename_component(_capyro_cc_dir "${CMAKE_C_COMPILER}" DIRECTORY)
    set(CAPYRO_DLL_DIRS ${FFMPEG_DLL_DIR} ${_capyro_cc_dir})
    if(CMAKE_CROSSCOMPILING)
        execute_process(COMMAND ${CMAKE_C_COMPILER} -print-file-name=libwinpthread-1.dll
            OUTPUT_VARIABLE _capyro_wp OUTPUT_STRIP_TRAILING_WHITESPACE ERROR_QUIET)
        if(_capyro_wp AND EXISTS "${_capyro_wp}")
            get_filename_component(_capyro_wp "${_capyro_wp}" DIRECTORY)
            list(APPEND CAPYRO_DLL_DIRS ${_capyro_wp})
        endif()
        list(APPEND CAPYRO_DLL_DIRS /usr/x86_64-w64-mingw32/bin /usr/x86_64-w64-mingw32/lib)
    endif()
    # Para saber si una DLL que no está es de Windows: mingw trae una
    # biblioteca de importación (libuser32.a...) para cada una del sistema.
    execute_process(COMMAND ${CMAKE_C_COMPILER} -print-file-name=libkernel32.a
        OUTPUT_VARIABLE _capyro_k32 OUTPUT_STRIP_TRAILING_WHITESPACE ERROR_QUIET)
    get_filename_component(CAPYRO_MINGW_LIBDIR "${_capyro_k32}" DIRECTORY)
    install(CODE "set(CAPYRO_DLL_DIRS \"${CAPYRO_DLL_DIRS}\")\nset(CAPYRO_MINGW_LIBDIR \"${CAPYRO_MINGW_LIBDIR}\")")
    install(CODE [[
        file(GET_RUNTIME_DEPENDENCIES
            EXECUTABLES "${CMAKE_INSTALL_PREFIX}/capyro.exe"
            RESOLVED_DEPENDENCIES_VAR _deps
            UNRESOLVED_DEPENDENCIES_VAR _missing
            DIRECTORIES ${CAPYRO_DLL_DIRS}
            PRE_EXCLUDE_REGEXES "^api-ms-" "^ext-ms-"
            POST_EXCLUDE_REGEXES "[Ss]ystem32" "[Ss]ys[Ww][Oo][Ww]64")
        foreach(_dll IN LISTS _deps)
            file(INSTALL "${_dll}" DESTINATION "${CMAKE_INSTALL_PREFIX}")
        endforeach()
        # Las que no se encontraron tienen que ser de Windows (kernel32,
        # user32, opengl32...): si falta otra, el .exe no va a abrir.
        set(_win_only bcryptprimitives nvcuda ucrtbase vcruntime140 combase)
        foreach(_m IN LISTS _missing)
            string(TOLOWER "${_m}" _ml)
            string(REGEX REPLACE "[.]dll$" "" _ml "${_ml}")
            list(FIND _win_only "${_ml}" _known)
            if(_known EQUAL -1 AND NOT _ml MATCHES "^(api|ext)-ms-"
               AND NOT EXISTS "${CAPYRO_MINGW_LIBDIR}/lib${_ml}.a")
                message(WARNING "Capyro: no encontré ${_m} (agregá su carpeta a CAPYRO_DLL_DIRS)")
            endif()
        endforeach()
    ]])
else()
    # Linux: binario, config de ejemplo, .desktop e íconos. Nada de Windows.
    install(TARGETS capyro RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR})
    install(FILES ${CAPYRO_INSTALLED_CONFIG} DESTINATION ${CMAKE_INSTALL_DATADIR}/capyro)
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
endif()

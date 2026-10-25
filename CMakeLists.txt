cmake_minimum_required(VERSION 3.22)
project(capyro
    VERSION 0.2.0
    DESCRIPTION "Capyro — Just press play."
    LANGUAGES C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS OFF)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

if(NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)
    set(CMAKE_BUILD_TYPE Release CACHE STRING "Tipo de build" FORCE)
endif()

option(CAPYRO_BUILD_TESTS "Compilar las pruebas del núcleo" ON)

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

add_subdirectory(core)
add_subdirectory(gui)

# --- ejecutable final ---------------------------------------------------
add_executable(capyro core/src/main.c)
target_link_libraries(capyro PRIVATE capyro_core capyro_gui)

if(WIN32)
    # Sin consola negra detrás de la ventana en builds Release.
    set_target_properties(capyro PROPERTIES WIN32_EXECUTABLE $<CONFIG:Release>)
endif()

# --- pruebas ------------------------------------------------------------
if(CAPYRO_BUILD_TESTS)
    enable_testing()
    add_executable(test_core tests/test_core.c)
    target_link_libraries(test_core PRIVATE capyro_core)
    if(NOT MSVC)
        target_link_libraries(test_core PRIVATE m)
    endif()

    # Media de prueba generada con el ffmpeg de línea de comandos (opcional).
    find_program(FFMPEG_EXECUTABLE ffmpeg)
    if(FFMPEG_EXECUTABLE)
        set(TEST_MEDIA ${CMAKE_CURRENT_BINARY_DIR}/test-media)
        set(FF ${FFMPEG_EXECUTABLE} -hide_banner -loglevel error -y)
        add_custom_command(
            OUTPUT ${TEST_MEDIA}/tono.mp3 ${TEST_MEDIA}/caratula.mp3 ${TEST_MEDIA}/clip.mp4
            COMMAND ${CMAKE_COMMAND} -E make_directory ${TEST_MEDIA}
            COMMAND ${FF} -f lavfi -i sine=frequency=440:duration=3
                    -metadata title=Tono\ de\ prueba -metadata artist=Capyro
                    ${TEST_MEDIA}/tono.mp3
            COMMAND ${FF} -f lavfi -i color=c=orange:s=64x64 -frames:v 1 ${TEST_MEDIA}/caratula.png
            COMMAND ${FF} -f lavfi -i sine=frequency=330:duration=2 -i ${TEST_MEDIA}/caratula.png
                    -map 0:a -map 1:v -c:v mjpeg -disposition:v attached_pic -id3v2_version 3
                    ${TEST_MEDIA}/caratula.mp3
            COMMAND ${FF} -f lavfi -i testsrc=size=320x240:rate=25:duration=2
                    -f lavfi -i sine=frequency=550:duration=2
                    -c:v mpeg4 -q:v 5 -c:a aac -shortest
                    ${TEST_MEDIA}/clip.mp4
            COMMENT "Generando media de prueba con ffmpeg"
            VERBATIM)
        add_custom_target(test_media ALL DEPENDS ${TEST_MEDIA}/clip.mp4)
        add_test(NAME core COMMAND test_core ${TEST_MEDIA} WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR})
    else()
        message(STATUS "Capyro: no encontré ffmpeg (CLI); las pruebas no van a decodificar")
        add_test(NAME core COMMAND test_core WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR})
    endif()
endif()

# --- instalación --------------------------------------------------------
include(GNUInstallDirs)
install(TARGETS capyro RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR})
install(FILES config/capyro.lua DESTINATION ${CMAKE_INSTALL_DATADIR}/capyro)
if(UNIX AND NOT APPLE)
    install(FILES packaging/capyro.desktop DESTINATION ${CMAKE_INSTALL_DATADIR}/applications)
    install(FILES assets/capyro_logo_256.png
            DESTINATION ${CMAKE_INSTALL_DATADIR}/icons/hicolor/256x256/apps
            RENAME capyro.png)
endif()

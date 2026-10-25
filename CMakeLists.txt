cmake_minimum_required(VERSION 3.22)
project(capyro
    VERSION 0.3.0
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

# --- pruebas ------------------------------------------------------------
if(CAPYRO_BUILD_TESTS)
    enable_testing()
    add_executable(test_core tests/test_core.c)
    target_link_libraries(test_core PRIVATE capyro_core m)

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
include(GNUInstallDirs)
install(TARGETS capyro RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR})
install(FILES config/capyro.lua DESTINATION ${CMAKE_INSTALL_DATADIR}/capyro)
install(FILES packaging/capyro.desktop DESTINATION ${CMAKE_INSTALL_DATADIR}/applications)
install(FILES assets/capyro_logo_256.png
        DESTINATION ${CMAKE_INSTALL_DATADIR}/icons/hicolor/256x256/apps
        RENAME capyro.png)

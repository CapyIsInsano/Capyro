# Importa el crate como target de CMake. Corrosion compila con cargo y
# agrega solas las librerías nativas que Rust necesita para linkear.
# Con -DCAPYRO_DIST=ON se usa el perfil "dist" (LTO completo, ver Cargo.toml).
if(CAPYRO_DIST)
    set(CAPYRO_CARGO_PROFILE PROFILE dist)
endif()
corrosion_import_crate(
    MANIFEST_PATH ${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml
    CRATES capyro_gui
    ${CAPYRO_CARGO_PROFILE}
)
if(CAPYRO_NATIVE)
    corrosion_add_target_rustflags(capyro_gui -Ctarget-cpu=native)
endif()


# La GUI (Rust) llama funciones del núcleo. Con librerías estáticas el linker
# solo resuelve "hacia adelante", así que hay que decirle a CMake que
# libcapyro_core.a va DESPUÉS de libcapyro_gui.a en la línea de link.
set_property(TARGET capyro_gui-static APPEND PROPERTY INTERFACE_LINK_LIBRARIES capyro_core)

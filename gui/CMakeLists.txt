# Importa el crate como target de CMake. Corrosion compila con cargo y
# agrega solas las librerías nativas que Rust necesita para linkear.
corrosion_import_crate(
    MANIFEST_PATH ${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml
    CRATES capyro_gui
)


# La GUI (Rust) llama funciones del núcleo. Con librerías estáticas el linker
# solo resuelve "hacia adelante", así que hay que decirle a CMake que
# libcapyro_core.a va DESPUÉS de libcapyro_gui.a en la línea de link.
set_property(TARGET capyro_gui-static APPEND PROPERTY INTERFACE_LINK_LIBRARIES capyro_core)

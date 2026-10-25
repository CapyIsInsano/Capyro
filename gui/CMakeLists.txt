# Importa el crate como target de CMake. Corrosion compila con cargo y
# agrega solas las librerías nativas que Rust necesita para linkear.
corrosion_import_crate(
    MANIFEST_PATH ${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml
    CRATES capyro_gui
)

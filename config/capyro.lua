-- ============================================================
--  Capyro — Just press play.
--  Archivo de configuración (Lua 5.4)
--
--  Capyro busca este archivo en este orden:
--    1. la ruta en la variable de entorno CAPYRO_CONFIG
--    2. $XDG_CONFIG_HOME/capyro/capyro.lua   (o ~/.config/capyro/capyro.lua)
--    3. ./config/capyro.lua                  (junto al binario o en el cwd)
--  Si no encuentra ninguno usa esta misma configuración, que va embebida
--  en el binario. No hace falta recompilar para cambiarla: editás y
--  reabrís Capyro.
-- ============================================================

-- ------------------------------------------------------------
--  Atajos de teclado
--
--  Formato:  accion = "Tecla"   o   accion = { "Tecla1", "Tecla2" }
--  Modificadores: "Ctrl+", "Shift+", "Alt+"   (ej. "Ctrl+Q", "Shift+Right")
--  Teclas: letras ("K"), números ("1"), "Space", "Enter", "Escape", "Tab",
--          "Left", "Right", "Up", "Down", "Home", "End",
--          "PageUp", "PageDown", "F1".."F12", "Comma", "Period", "Minus", "Plus"
--
--  Acciones disponibles:
--    play_pause, stop, seek_forward, seek_backward,
--    volume_up, volume_down, mute,
--    next_file, prev_file, toggle_playlist, fullscreen, open_files,
--    toggle_shuffle, cycle_repeat, speed_up, speed_down, speed_reset, quit
-- ------------------------------------------------------------
local keys = {
    play_pause      = { "Space", "K" },
    stop            = "S",
    seek_forward    = { "Right", "L" },
    seek_backward   = { "Left", "J" },
    volume_up       = "Up",
    volume_down     = "Down",
    mute            = "M",
    next_file       = "N",
    prev_file       = "P",
    toggle_playlist = "Ctrl+L",
    fullscreen      = { "F", "F11" },
    open_files      = "Ctrl+O",
    toggle_shuffle  = "Ctrl+S",
    cycle_repeat    = "Ctrl+R",        -- no repetir → repetir lista → repetir uno
    speed_down      = "[",
    speed_up        = "]",
    speed_reset     = "Backspace",
    quit            = "Ctrl+Q",
}

-- ------------------------------------------------------------
--  Opciones
-- ------------------------------------------------------------
local options = {
    seek_step    = 5,          -- segundos que avanza/retrocede seek_forward/backward
    volume_step  = 5,          -- porcentaje por cada volume_up/volume_down
    start_volume = 100,        -- volumen al abrir Capyro (0..100)
    auto_next    = true,       -- al terminar un archivo, pasar al siguiente de la lista
    speed        = 1.0,        -- velocidad inicial (0.5 .. 2.0), sin cambiar el tono
    shuffle      = false,      -- aleatorio al abrir
    ["repeat"]   = "off",      -- "off", "all" (toda la lista) u "one" (el actual)
    accent       = "#FF7417",  -- color de acento (botón play, barra de progreso)

    -- Solo Linux: "auto", "x11" o "wayland" (hay que reabrir Capyro).
    -- En Wayland nativo todavía no se pueden arrastrar archivos a la ventana
    -- (limitación de winit); con "x11" corre por XWayland y el arrastre sí
    -- funciona si tu compositor lo soporta. El botón "+ Agregar" anda siempre.
    backend      = "auto",
}

return {
    keys    = keys,
    options = options,
}

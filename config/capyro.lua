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
--    toggle_shuffle, cycle_repeat, speed_up, speed_down, speed_reset,
--    screenshot, toggle_effects, effects_bypass, quit
--
--  Además, las teclas 0..9 (sin modificadores) saltan al 0 %..90 % del archivo.
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
    screenshot      = "Shift+S",       -- guarda el fotograma actual como PNG
    toggle_effects  = "Ctrl+E",        -- abre/cierra el menú de efectos de audio
    effects_bypass  = "Shift+E",       -- prende/apaga todos los efectos (para comparar)
    quit            = "Ctrl+Q",
}

-- ------------------------------------------------------------
--  Opciones
-- ------------------------------------------------------------
local options = {
    seek_step    = 5,          -- segundos que avanza/retrocede seek_forward/backward
    volume_step  = 5,          -- porcentaje por cada volume_up/volume_down
    wheel_step   = 5,          -- porcentaje por cada "muesca" de la rueda sobre el volumen
    start_volume = 100,        -- volumen al abrir Capyro (0..100)
    auto_next    = true,       -- al terminar un archivo, pasar al siguiente de la lista
    speed        = 1.0,        -- velocidad inicial (0.5 .. 2.0), sin cambiar el tono
    shuffle      = false,      -- aleatorio al abrir
    ["repeat"]   = "off",      -- "off", "all" (toda la lista) u "one" (el actual)
    accent       = "#FF7417",  -- color de acento (botón play, barra de progreso)

    remember_session = true,   -- al abrir, retomar la lista, el archivo y la posición
    visualizer       = true,   -- barras de frecuencia mientras suena un audio
    screenshot_dir   = "",     -- "" = Imágenes/Capyro; o una ruta, ej. "~/Capturas"
    splash           = true,   -- pantalla de "Cargando" al abrir (con animación)
    startup_sound    = true,   -- sonido de inicio cuando termina de cargar
    click_sound      = true,   -- sonidos de la interfaz: clic en botones y al cerrar menús
    show_format      = true,   -- formato del archivo arriba a la izquierda del video (MP4 · H.264)
    show_millis      = true,   -- debajo, los milisegundos del segundo actual (000..999)

    -- Decodificación de video: "auto" (GPU si se puede, si no CPU),
    -- "gpu" (VA-API / NVDEC / VDPAU; si no hay, avisa y usa la CPU) o "cpu".
    -- También se cambia en ⚙ → Decodificación.
    decoding         = "auto",

    -- Actualizaciones (menú ⓘ → "Buscar actualizaciones").
    -- Vacío = el repo de git desde el que se compiló Capyro. Si lo compilaste
    -- desde un .zip, poné el tuyo: "usuario/Capyro" o la URL completa.
    update_repo   = "",
    update_branch = "",        -- vacío = la rama principal del repo

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

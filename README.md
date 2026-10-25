<p align="center">
  <img src="assets/capyro_logo_256.png" width="128" alt="Logo de Capyro">
</p>

<h1 align="center">Capyro</h1>
<p align="center"><b>Just press play.</b></p>

<p align="center">
  Reproductor multimedia liviano para <b>Linux</b>: video <code>mp4</code>, <code>mov</code> y <code>avi</code>; audio <code>mp3</code>, <code>wav</code>, <code>ogg</code> y <code>aac</code>.<br>
  Núcleo en <b>C</b> · interfaz en <b>Rust</b> · compilado con <b>CMake</b> · configurable en <b>Lua</b>.
</p>

<p align="center">
  <img src="docs/screenshot.png" width="600" alt="Capyro">
</p>

---

## Estado: `v0.2.6-alpha`

Reproduce audio y video con seek, carátulas y metadatos; tiene lista con **aleatorio** y
**repetir**, **velocidad** de 0.5× a 2× sin cambiar el tono, y animaciones en los botones.

> Por ahora Capyro es **solo para Linux**.

<p align="center">
  <img src="docs/screenshot-audio.png" width="420" alt="mp3 con carátula">
  <img src="docs/screenshot-lista.png" width="420" alt="Lista de reproducción">
</p>

| Funciona ✅ | Pendiente 🚧 |
|---|---|
| Video `.mp4` `.mov` `.avi` y audio `.mp3` `.wav` `.ogg` `.aac` (FFmpeg + miniaudio) | Subtítulos |
| Botón **+ Agregar archivo(s)** con el selector de archivos del sistema | Arrastrar archivos en Wayland nativo (limitación de winit) |
| Video sincronizado con el audio (el audio es el reloj maestro) | Más opciones en el menú de ajustes |
| Seek preciso (clic/arrastre en la barra, flechas) | Aceleración por hardware (VA-API) |
| Carátulas y metadatos (título, artista) | Recordar posición / archivos recientes |
| Lista con aleatorio, repetir (lista / uno) y paso automático | Otras plataformas (más adelante) |
| Velocidad 0.5×–2× sin cambiar el tono (menú ⚙ o `[` `]`) | |
| Botones animados (hover, clic, transiciones) | |
| Menú ⓘ con la versión y un resumen de cada formato | |
| Sonidos: capibara al tocar el logo, aviso de error | |
| Retoma la sesión, visualizador, capturas, tiempo restante, saltos 0–9 | |
| Efectos de audio: graves, medios, agudos, tono, estéreo/mono, modo noche, presets | |
| Volumen, silencio, pausa, stop, pantalla completa | |
| Atajos y opciones en `capyro.lua` (recarga en caliente) | |
| Sigue andando sin placa de sonido (reloj de pared, sin audio) | |

---

## Arquitectura

```
                ┌──────────────────────────────────────────────┐
  main.c ──────▶│  núcleo (C)              libcapyro_core      │
  (arranca)     │                                              │
                │  hilo de decodificación ─ FFmpeg             │
                │    demux → colas de paquetes → decode        │
                │      audio → tono/atempo/aresample → ring ───┼──▶ EQ/estéreo/noche ──▶ 🔊
                │      video → swscale → cola RGBA (3 frames)  │    (callback = reloj)
                └──────────────────────▲───────────────────────┘
                                       │ FFI: capyro_core.h
                ┌──────────────────────┴───────────────────────┐
                │  GUI (Rust)  libcapyro_gui — egui/eframe     │
                │  cada frame: update() + pedir frame → textura│
                └──────────────────────▲───────────────────────┘
                                       │ mlua (Lua 5.4 embebido)
                               ~/.config/capyro/capyro.lua
```

- **C es el dueño**: `main.c` crea el núcleo, carga los archivos de la línea de comandos,
  le pasa el puntero a la GUI y lo libera al cerrar.
- **Tres hilos**: la GUI, el de decodificación (uno por archivo abierto) y el callback de
  audio. Un único mutex protege el estado compartido, y nunca se llama a FFmpeg ni a
  miniaudio con el lock tomado.
- **Reloj maestro = audio**: la posición sale de las muestras que realmente sonaron, y el
  video muestra el frame más nuevo cuyo tiempo ya llegó. Si no hay audio (o se terminó
  antes que el video) el reloj pasa a ser de pared sin saltos.
- **Velocidad**: el audio pasa por el filtro `atempo` de libavfilter (cambia el tempo sin
  cambiar el tono) y el reloj avanza `velocidad` segundos por segundo, así el video lo sigue.
- **Efectos de audio**: el ecualizador (biquads del *Audio EQ Cookbook*), el estéreo y el
  compresor del modo noche están escritos en C (`core/src/dsp.c`) y corren en el callback
  de audio, justo antes de que suene: por eso responden al instante. El tono necesita
  estirar el audio en el tiempo, así que va en el grafo de FFmpeg (`asetrate` + `atempo`)
  y cambiarlo vuelve a decodificar desde la posición actual.
- **La lista vive en el núcleo**: el orden de reproducción (mezclado si hay aleatorio),
  repetir y el paso al siguiente se deciden en C; la GUI solo muestra y manda comandos.
- **Rust se compila como `staticlib`** y expone una sola función: `capyro_gui_run()`.
- **CMake** compila todo con un solo comando; el crate entra como un target más gracias a
  [Corrosion](https://github.com/corrosion-rs/corrosion).
- **Lua** queda del lado de la GUI (`mlua` con Lua vendorizado: no hace falta tenerlo instalado).

Reglas de la frontera C ↔ Rust:
- `CapyroCore` es opaco: Rust nunca ve sus campos.
- Los enums viajan como `int` y Rust los convierte, así un valor nuevo del lado de C nunca
  provoca comportamiento indefinido en Rust.
- El frame de video se presta con `lock`/`unlock`; en Rust eso es `Core::with_video_frame`,
  que suelta el lock aunque haya un `panic`.
- Todo el `unsafe` vive en `gui/src/ffi.rs`, y ningún `panic` cruza hacia C.

---

## Requisitos

| Herramienta | Versión |
|---|---|
| CMake | ≥ 3.22 |
| Sistema | **Linux** (X11 o Wayland) |
| Compilador C | C11 (GCC o Clang) |
| Rust + Cargo | **≥ 1.95** (egui 0.37 lo pide) |
| FFmpeg (librerías) | **≥ 5.1** (`libavformat`, `libavcodec`, `libavutil`, `libavfilter`, `libswscale`) |
| pkg-config / pkgconf | para encontrar FFmpeg |
| Git | CMake baja Corrosion y miniaudio la primera vez |
| `ffmpeg` (CLI) | opcional: solo para generar la media de las pruebas |

[miniaudio](https://miniaud.io/) es un header único: si tenés `miniaudio.h` en el sistema
se usa ese, si no CMake lo baja solo (versión fija 0.11.25).

El selector de archivos usa el **portal de escritorio** (`xdg-desktop-portal` con
un backend que tenga *FileChooser*: gtk, gnome o kde). Si no hay portal, usa `zenity` o
`kdialog` si están instalados. En Gentoo: `sys-apps/xdg-desktop-portal` +
`sys-apps/xdg-desktop-portal-gtk` (o `gnome-extra/zenity`).

El audio sale por PulseAudio/PipeWire, ALSA o JACK (miniaudio los carga en tiempo
de ejecución) y la ventana por X11 o Wayland: no hace falta ninguna librería `-dev` de eso.

### Instalar dependencias

**Gentoo**
```sh
sudo emerge --ask dev-build/cmake dev-vcs/git dev-lang/rust media-video/ffmpeg dev-util/pkgconf
rustc --version   # si es menor a 1.95, usá rustup (ver abajo)
```

**Arch / Artix**
```sh
sudo pacman -S --needed base-devel cmake git rustup ffmpeg pkgconf
rustup default stable
```

**Devuan / Debian / Ubuntu** — el Rust de `apt` es demasiado viejo, usá rustup:
```sh
sudo apt install build-essential cmake git curl pkg-config ffmpeg \
    libavformat-dev libavcodec-dev libavutil-dev libavfilter-dev libswscale-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

---

## Compilar

```sh
git clone https://github.com/CapyIsInsano/Capyro.git
cd Capyro
cmake -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build -j"$(nproc)"
./build/capyro
```

Si tenés [Ninja](https://ninja-build.org/) (`dev-build/ninja`), usalo: reparte mejor los
núcleos entre C y Rust. Agregá `-G Ninja` al primer comando.

### Tiempos de compilación

La **primera** vez se compilan ~160 dependencias de Rust (egui, winit...) y eso es lo que
más tarda: unos 4 minutos con 2 núcleos, bastante menos con más. Después:

| Cambiaste... | Recompila en |
|---|---|
| algo de la GUI (`gui/src/*.rs`) | ~3 s |
| algo del núcleo (`core/src/*.c`) | ~3 s |

(Antes de la v0.2.5 cualquier cambio en la GUI tardaba más de un minuto por el LTO.)

### Opciones de compilación

| Opción | Qué hace |
|---|---|
| *(nada)* | `-O3` en C y Rust, sin LTO: rápido de recompilar. Binario de ~10 MB. |
| `-DCAPYRO_DIST=ON` | LTO completo en C y Rust: binario ~15 % más chico (~8,4 MB). Para instalar o empaquetar; cada recompilación tarda ~1 min más. |
| `-DCAPYRO_NATIVE=ON` | Optimiza para **tu** procesador (`-march=native`). El binario no sirve en otras PCs. |
| `-DCAPYRO_CCACHE=OFF` | No usar `ccache` (se usa solo si está instalado). |
| `-DCAPYRO_BUILD_TESTS=OFF` | No compilar las pruebas. |

Por ejemplo, la versión "lo más optimizada posible" para tu máquina:

```sh
cmake -B build-dist -G Ninja -DCMAKE_BUILD_TYPE=Release -DCAPYRO_DIST=ON -DCAPYRO_NATIVE=ON
cmake --build build-dist
```

Siempre se tira el código que no se usa (`--gc-sections`) y en Release se sacan los
símbolos, así que el ejecutable queda chico aunque traiga egui adentro.

### Instalar

```sh
sudo cmake --install build          # por defecto en /usr/local
```

Instala:

| Archivo | Destino |
|---|---|
| binario | `/usr/local/bin/capyro` |
| config de ejemplo | `/usr/local/share/capyro/capyro.lua` |
| lanzador | `/usr/local/share/applications/capyro.desktop` |
| ícono | `/usr/local/share/icons/hicolor/256x256/apps/capyro.png` |

Para otro prefijo: `cmake --install build --prefix ~/.local`.

---

## Uso

```sh
capyro                                  # abre vacío; usá "+ Agregar archivo(s)"
capyro cancion.mp3 video.mov tema.ogg   # carga la lista y arranca a reproducir
capyro --help
capyro --version
```

- **+ Agregar archivo(s)** (el botón naranja de la derecha, `Ctrl+O`, o clic en el área de
  video vacía): abre el selector de archivos del sistema y permite elegir varios a la vez.
  Si no hay nada sonando arranca el primero; si ya hay algo, los agrega al final de la lista.
- **Arrastrar y soltar** uno o varios archivos: se agregan a la lista y arranca el primero
  (ver la nota sobre Wayland más abajo).
- Al terminar un archivo pasa solo al siguiente (`auto_next` en la config).
- **Aleatorio** (botón a la izquierda de los controles): mezcla la lista sin repetir
  archivos hasta recorrerla toda.
- **Repetir** (botón a la derecha de los controles): cada clic cambia entre *no repetir* →
  *repetir lista* → *repetir este archivo* (muestra un "1").
- **Lista**: clic en un archivo para reproducirlo; pasá el mouse y tocá la **✕** para
  quitarlo. Si quitás el que está sonando, sigue con el próximo.
- **Clic** en el área de video: play/pausa. **Doble clic**: pantalla completa.
- **Barra de progreso**: clic o arrastrar para saltar.
- **Volumen**: clic para silenciar, rueda del mouse encima para subir/bajar (un paso por
  muesca, configurable con `wheel_step`).
- **ⓘ Acerca de** (arriba de "Lista"): versión de Capyro, formatos soportados y backend de
  audio en uso. Tocá un formato para ver un resumen; solo se abre uno a la vez y tocarlo de
  nuevo lo cierra.
- **Logo**: tocalo y suena un capibara (uno de tres, al azar).
- **Retoma donde lo dejaste**: al abrir Capyro sin archivos vuelve la lista, el archivo y la
  posición de la última vez (en pausa), además del volumen, la velocidad, aleatorio y
  repetir. Se guarda en `~/.local/state/capyro/session.lua`; se apaga con
  `remember_session = false`.
- **Efectos de audio** (botón ≡ a la izquierda del volumen, `Ctrl+E`):
  - **Graves / Medios · voces / Agudos**: ecualizador de 3 bandas, ±12 dB.
  - **Tono**: ±12 semitonos *sin* cambiar la velocidad (se aplica al soltar el control).
  - **Estéreo**: de *Mono* (útil con un solo auricular) a 200 % (más abierto).
  - **Modo noche**: compresor que baja lo fuerte y sube lo bajito (diálogos claros a
    volumen bajo).
  - **Presets**: Normal, Bajos, Voces, Lo-fi y Noche.
  - **Interruptor general** (`Shift+E`) para comparar con y sin efectos, y **Restablecer**.
  - Doble clic en un control lo vuelve a 0; la rueda del mouse lo mueve de a un paso.
  - El ecualizador se escucha **al instante** y nunca satura (limitador suave). Los
    efectos se recuerdan entre sesiones, y el botón se pone naranja cuando hay alguno activo.
- **Visualizador**: mientras suena un audio, barras de frecuencia de fondo (FFT propia, sin
  dependencias). Se apaga con `visualizer = false`.
- **Tiempo restante**: clic en el tiempo de la derecha para alternar entre la duración total
  y lo que falta (`-03:21`).
- **Capturas**: `Shift+S` o ⚙ → *Capturar fotograma* guarda la imagen del video como PNG en
  `~/Imágenes/Capyro` (respeta tu carpeta de Imágenes; se cambia con `screenshot_dir`).
- Si intentás abrir un archivo no soportado o tocás subtítulos (todavía no disponibles),
  suena un aviso de error. Los sonidos siguen el volumen y el silencio generales.
- **⚙ Ajustes**: velocidad (0.5×, 0.75×, 1×, 1.25×, 1.5×, 2×) y recargar `capyro.lua` sin
  reiniciar. Se cierra con un clic afuera o `Esc`.
- **Header**: arrastrar para mover, doble clic para maximizar. Los bordes redimensionan.

### Atajos por defecto

| Acción | Tecla |
|---|---|
| Play / pausa | `Space`, `K` |
| Detener | `S` |
| Adelantar / retroceder | `→` / `←`, `L` / `J` |
| Volumen + / − | `↑` / `↓` |
| Silenciar | `M` |
| Siguiente / anterior | `N` / `P` |
| Mostrar lista | `Ctrl+L` |
| Agregar archivos | `Ctrl+O` |
| Aleatorio | `Ctrl+S` |
| Repetir (no → lista → uno) | `Ctrl+R` |
| Velocidad − / + / normal | `[` / `]` / `Backspace` |
| Pantalla completa | `F`, `F11` (salir: `Esc`) |
| Saltar al 0 %, 10 % ... 90 % | `0` ... `9` |
| Capturar fotograma (PNG) | `Shift+S` |
| Menú de efectos de audio | `Ctrl+E` |
| Prender / apagar todos los efectos | `Shift+E` |
| Salir | `Ctrl+Q` |

---

## Configuración (Lua)

Capyro busca `capyro.lua` en este orden y usa el primero que encuentra:

1. la ruta en `CAPYRO_CONFIG`
2. `$XDG_CONFIG_HOME/capyro/capyro.lua` → `~/.config/capyro/capyro.lua`
3. `./config/capyro.lua`, junto al binario, o `../share/capyro/capyro.lua`

Si no hay ninguno, usa la configuración por defecto que va **embebida en el binario**.
Si el archivo tiene un error, lo avisa en la terminal y también cae a la embebida.

Para empezar a personalizar:

```sh
mkdir -p ~/.config/capyro
cp config/capyro.lua ~/.config/capyro/
```

Ejemplo:

```lua
return {
    keys = {
        play_pause   = { "Space", "K" },     -- una tecla o una lista
        seek_forward = "Shift+Right",         -- modificadores: Ctrl+, Shift+, Alt+
        quit         = "Ctrl+Q",
    },
    options = {
        seek_step    = 10,         -- segundos
        volume_step  = 5,          -- % por tecla
        wheel_step   = 5,          -- % por muesca de la rueda sobre el volumen
        start_volume = 80,         -- %
        auto_next    = true,       -- pasar al siguiente al terminar
        speed        = 1.0,        -- velocidad inicial (0.5 .. 2.0)
        shuffle      = false,      -- aleatorio al abrir
        ["repeat"]   = "off",      -- "off", "all" u "one" (repeat es palabra reservada de Lua)
        backend      = "auto",     -- "auto", "x11" o "wayland"
        remember_session = true,   -- retomar lista y posición al abrir
        visualizer   = true,       -- barras de frecuencia en los audios
        screenshot_dir = "",       -- "" = ~/Imágenes/Capyro
        accent       = "#FF7417",  -- color del botón play y la barra
    },
}
```

Acciones disponibles: `play_pause`, `stop`, `seek_forward`, `seek_backward`, `volume_up`,
`volume_down`, `mute`, `next_file`, `prev_file`, `toggle_playlist`, `fullscreen`,
`open_files`, `toggle_shuffle`, `cycle_repeat`, `speed_up`, `speed_down`, `speed_reset`,
`screenshot`, `toggle_effects`, `effects_bypass`, `quit`.
La lista completa de nombres de teclas está comentada en [`config/capyro.lua`](config/capyro.lua).

---

## Pruebas

```sh
ctest --test-dir build --output-on-failure   # núcleo en C
cd gui && cargo test                         # config Lua, sesión y visualizador (FFT)
```

Si está el `ffmpeg` de línea de comandos, `ctest` genera primero archivos de prueba en todos
los formatos (mp3, mp3 con carátula, wav, ogg, aac, mp4, mov y avi; nunca durante el build) y después
prueba de punta a punta: duración, metadatos, reproducción, pausa,
seek (también en pausa), fin del archivo, carátula, frames de video, paso automático al
siguiente, velocidad (0.5× y 2×), repetir lista/uno, aleatorio sin repetidos y quitar
archivos de la lista (incluido el que está sonando). También verifica que al repetir un
archivo el audio vuelva a sonar (el bug de la v0.2.2) y que los efectos de sonido se
decodifiquen desde memoria y se mezclen. Los efectos de audio se prueban midiendo: graves
+12 suben ~12 dB más los 50 Hz que los 5 kHz, el modo noche achica el rango dinámico, mono
deja ambos canales iguales, nada satura al máximo, y tono +12 convierte un La de 440 Hz en
880 Hz sin cambiar la velocidad. Sin `ffmpeg` solo corren las pruebas que no decodifican.

Para generar los mp3 de prueba tu `ffmpeg` necesita un encoder de mp3 (LAME); si no lo
tiene, esas pruebas se saltean. En Gentoo se activa con el USE `lame` de
`media-video/ffmpeg`. Para *reproducir* mp3 no hace falta: el decoder viene con FFmpeg.

Sin placa de sonido (un contenedor, CI) miniaudio usa su backend *Null*, que consume
audio en tiempo real, así que las pruebas igual valen. El núcleo también se probó con
ThreadSanitizer y AddressSanitizer.

---

## Estructura

```
capyro/
├── CMakeLists.txt            # build principal (trae Corrosion)
├── core/                     # ── núcleo en C ──
│   ├── include/capyro_core.h #    API pública (lo único que ve Rust)
│   └── src/
│       ├── core.c            #    API, transporte, lista de reproducción
│       ├── player.c          #    FFmpeg + miniaudio: hilos, colas, reloj
│       ├── sfx.c             #    efectos de sonido (decodifica desde memoria)
│       ├── dsp.c/.h          #    efectos de audio en tiempo real (EQ, estéreo, noche)
│       ├── platform.c/.h     #    hilos y mutex (pthreads)
│       ├── internal.h        #    estructuras internas
│       ├── miniaudio_impl.c  #    implementación de miniaudio
│       └── main.c            #    entry point
├── gui/                      # ── interfaz en Rust ──
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs            #    capyro_gui_run() + ventana
│       ├── ffi.rs            #    bindings seguros al núcleo
│       ├── app.rs            #    layout y lógica de la UI
│       ├── app/fx_menu.rs    #    menú de efectos de audio
│       ├── icons.rs          #    íconos dibujados a mano
│       ├── theme.rs          #    colores y fuentes
│       ├── config.rs         #    lectura de capyro.lua (mlua)
│       ├── formats.rs        #    resúmenes de cada formato (menú ⓘ)
│       ├── session.rs        #    guardar/retomar la sesión
│       └── viz.rs            #    visualizador de audio (FFT)
├── config/capyro.lua         # configuración por defecto
├── assets/                   # logo en 128/256/512 px + sounds/ (efectos)
├── packaging/capyro.desktop  # lanzador para Linux
├── tests/                    # test_core.c + gen_media.cmake
└── docs/                     # capturas para este README
```

---

## Roadmap

- [x] FFmpeg en el núcleo: abrir, duración, seek, decodificar audio y video
- [x] miniaudio para la salida de audio
- [x] Frames de video → textura en egui, sincronizados con el audio
- [x] Carátulas y metadatos
- [x] wav, ogg y mov
- [x] avi y aac, resúmenes de formatos, efectos de sonido
- [x] Compilación rápida (sin LTO por defecto) y modo `CAPYRO_DIST`
- [x] Retomar sesión, visualizador, capturas, tiempo restante
- [x] Efectos de audio (ecualizador, tono, estéreo, modo noche, presets)
- [x] Paso automático al siguiente de la lista
- [x] Aleatorio y repetir (lista / uno)
- [x] Menú de ajustes con velocidad de reproducción
- [x] Animaciones en los botones
- [ ] Subtítulos `.srt`
- [ ] Más opciones en el menú de ajustes
- [ ] Hooks en Lua (`on_open`, `on_end`, acciones personalizadas)
- [ ] Decodificación por hardware para video 4K
- [ ] Otras plataformas (más adelante)

---

## Notas

- **Arrastrar archivos en Wayland**: winit (la librería de ventanas que usa egui) todavía
  no recibe archivos soltados en Wayland nativo, así que en Niri, Sway, Hyprland, GNOME o
  KDE sobre Wayland el arrastre no hace nada. Usá el botón **+ Agregar**, o poné
  `backend = "x11"` en `capyro.lua` para correr por XWayland, donde el arrastre funciona
  si tu compositor pasa el drag & drop entre Wayland y X11.
- La ventana usa esquinas redondeadas con fondo transparente. En un WM/compositor sin
  transparencia las esquinas se ven negras; es solo estético.
- El video se escala a como mucho 1920×1080 antes de subirlo a la GPU y se decodifica por
  software (con hilos). Para 4K a 60 fps hará falta decodificación por hardware.
- Probado en Linux x86_64 (Ubuntu en X11 con FFmpeg 6.1, y Gentoo). Por ahora Capyro es
  solo para Linux: CMake se niega a configurar en otros sistemas.

---

## Licencia

Capyro es software libre: podés redistribuirlo y modificarlo bajo los términos de la
**GNU General Public License v3.0** (o, a tu elección, cualquier versión posterior).
Ver [`LICENSE`](LICENSE).

Usa [FFmpeg](https://ffmpeg.org/) (LGPL/GPL), [miniaudio](https://miniaud.io/)
(dominio público / MIT-0), [egui](https://github.com/emilk/egui) (MIT/Apache-2.0) y
[mlua](https://github.com/mlua-rs/mlua) con Lua 5.4 (MIT).

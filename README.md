<div align="center">

<img src="docs/readme/hero.svg" width="900" alt="Capyro: reproductor multimedia para Linux y Windows">

<br>

<img src="https://img.shields.io/badge/versi%C3%B3n-0.3.0--alpha-FF7417?style=flat-square&labelColor=1B1C20" alt="versión 0.3.0-alpha">
<img src="https://img.shields.io/badge/plataforma-Linux%20%7C%20Windows-FF7417?style=flat-square&labelColor=1B1C20" alt="Linux y Windows">
<img src="https://img.shields.io/badge/licencia-GPL--3.0-FF7417?style=flat-square&labelColor=1B1C20" alt="GPL-3.0">
<img src="https://img.shields.io/badge/C11-%20-3A3B40?style=flat-square&labelColor=1B1C20" alt="C11">
<img src="https://img.shields.io/badge/Rust-1.95%2B-3A3B40?style=flat-square&labelColor=1B1C20" alt="Rust 1.95+">
<img src="https://img.shields.io/badge/Lua-5.4-3A3B40?style=flat-square&labelColor=1B1C20" alt="Lua 5.4">
<img src="https://img.shields.io/badge/FFmpeg-5.1%2B-3A3B40?style=flat-square&labelColor=1B1C20" alt="FFmpeg 5.1+">

**Capyro** es un reproductor de audio y video liviano para **Linux y Windows**, con un capibara de mascota.<br>
Abrís un archivo, apretás play y listo: *just press play*.

[Funciones](#funciones) ·
[Efectos de audio](#efectos-de-audio) ·
[Actualizaciones](#4-actualizar) ·
[Distribuciones](#distribuciones) ·
[Instalación](#instalación) ·
[Windows](#windows) ·
[Atajos](#cómo-se-usa) ·
[Configuración](#configuración) ·
[Problemas](#solución-de-problemas)

</div>

> [!TIP]
> **Novedades de la v0.3.0:** **el salto a Windows**. Capyro corre en **Windows 10 y 11**, con
> su propio instalador ([CapyroInstaller](#windows)). Cada sistema compila e instala **solo lo
> suyo**: la config guarda `system = "Lin"` o `"Win"` y el actualizador baja únicamente lo de
> tu sistema. El tutorial ahora te muestra qué sistema detectó.
> [Ver el historial completo →](CHANGELOG.md)

<br>

<img src="docs/readme/divider-1.svg" width="100%" alt="">

## Funciones

<table>
<tr>
<td width="50%" align="center"><img src="docs/screenshot.png" alt="Reproduciendo un video"><br><sub><b>Video</b> sincronizado con el audio</sub></td>
<td width="50%" align="center"><img src="docs/screenshot-audio.png" alt="Audio con carátula y visualizador"><br><sub><b>Audio</b> con carátula, título y visualizador</sub></td>
</tr>
<tr>
<td align="center"><img src="docs/screenshot-lista.png" alt="Lista de reproducción"><br><sub><b>Lista</b> con aleatorio, repetir y paso automático</sub></td>
<td align="center"><img src="docs/screenshot-efectos.png" alt="Menú de efectos de audio"><br><sub><b>Efectos de audio</b> que se escuchan al instante</sub></td>
</tr>
<tr>
<td align="center"><img src="docs/screenshot-info.png" alt="Menú de información"><br><sub><b>ⓘ</b> versión, formatos y buscar actualizaciones</sub></td>
<td align="center"><img src="docs/screenshot-ajustes.png" alt="Menú de ajustes"><br><sub><b>⚙</b> velocidad, decodificación GPU/CPU, configuración y capturas</sub></td>
</tr>
<tr>
<td align="center"><img src="docs/screenshot-playlists.png" alt="Playlists en la lista"><br><sub><b>Playlists</b>: hasta 3, además de la cola</sub></td>
<td align="center"><img src="docs/screenshot-atajos.png" alt="Editor de atajos"><br><sub><b>Editar configuración</b>: los atajos, sin tocar archivos</sub></td>
</tr>
<tr>
<td align="center"><img src="docs/screenshot-carga.png" alt="Pantalla de carga"><br><sub><b>Pantalla de carga</b> animada, con sonido de inicio</sub></td>
<td align="center"><img src="docs/screenshot-actualizacion.png" alt="Aviso de actualización instalada"><br><sub><b>Se actualiza solo</b> y te muestra el parche</sub></td>
</tr>
<tr>
<td align="center"><img src="docs/screenshot-tutorial.png" alt="Tutorial de bienvenida"><br><sub><b>Tutorial</b> de bienvenida (el destello gira con el mouse)</sub></td>
<td align="center"><img src="docs/screenshot-instalador.png" alt="CapyroInstaller"><br><sub><b>CapyroInstaller</b>: el instalador para Windows</sub></td>
</tr>
</table>

| | |
|---|---|
| ▶️ **Reproducción** | Audio y video con FFmpeg, video sincronizado con el audio, búsqueda precisa, velocidad de 0.5× a 2× **sin cambiar el tono**, saltos al 0–90 % con las teclas `0`–`9`. |
| 🎵 **Audio** | Carátula, título y artista, un visualizador de frecuencias propio y efectos de audio en tiempo real. |
| 📃 **Lista** | Una **cola** temporal y hasta **3 playlists** guardadas. Aleatorio sin repetidos, repetir la lista o un archivo, paso automático al siguiente y quitar archivos aunque estén sonando. |
| 💾 **Memoria** | Retoma la lista, el archivo, la posición, el volumen y los efectos de la última vez. |
| 🚀 **GPU** | Decodificación de video por **GPU** (Linux: VA-API, NVDEC o VDPAU; Windows: D3D11VA, NVDEC o DXVA2), con vuelta automática a la CPU si la placa no puede. |
| 🪟 **Linux y Windows** | El mismo Capyro en los dos: cada uno compila e instala solo lo suyo (`system = "Lin"` o `"Win"`). En Windows se instala con [CapyroInstaller](#windows). |
| 📸 **Capturas** | Guarda el fotograma del video como PNG con `Shift`+`S`. |
| 🔄 **Actualizaciones** | Un botón busca commits nuevos en GitHub, instala la versión nueva (en Linux la compila; en Windows baja la ya compilada) y te muestra qué cambió. |
| ✨ **Interfaz** | Ventana propia con esquinas redondeadas, pantalla de carga, botones animados, menús que aparecen deslizándose y avisos con fundido. |
| 🦫 **Detalles** | Tocá el logo y suena un capibara (hay tres, salen al azar). Al abrir suena el inicio, y al actualizarse, su propio sonido. |
| ⚙️ **Personalizable** | Los atajos se cambian **desde la app** (⚙ → Editar configuración) y todo queda en un archivo **Lua** que también podés editar a mano. |
| 🎓 **Tutorial** | La primera vez que lo abrís te presenta Capyro, te explica toda la interfaz (con música) y te dice qué sistema detectó. Se repite desde Ajustes. |
| 📰 **Novedades** | Un botón al lado de ⓘ con lo nuevo de la versión que tenés. |

<img src="docs/readme/divider-2.svg" width="100%" alt="">

## Efectos de audio

<img align="right" width="300" src="docs/screenshot-efectos.png" alt="Menú de efectos">

Se abren con el botón **≡** a la izquierda del volumen, o con <kbd>Ctrl</kbd>+<kbd>E</kbd>.

| Control | Qué hace |
|---|---|
| **Graves** | ±12 dB alrededor de 120 Hz |
| **Medios · voces** | ±12 dB alrededor de 1,5 kHz, donde están las voces |
| **Agudos** | ±12 dB desde 6 kHz |
| **Tono** | ±12 semitonos **sin** cambiar la velocidad |
| **Estéreo** | De *Mono* (ideal con un solo auricular) a 200 % |
| **Modo noche** | Baja lo fuerte y sube lo bajito: se entienden los diálogos sin que las explosiones despierten a nadie |

**Presets:** Normal · Bajos · Voces · Lo-fi · Noche.<br>
El **interruptor general** (<kbd>Shift</kbd>+<kbd>E</kbd>) los apaga y prende para comparar.<br>
**Doble clic** en un control lo vuelve a 0, y la **rueda** lo mueve de a un paso.

El ecualizador está escrito en C y se aplica justo antes de que suene el audio, así que
cada cambio se escucha **al instante**. Un limitador suave evita que sature, aunque
pongas todo al máximo.

<br clear="right">

<img src="docs/readme/divider-3.svg" width="100%" alt="">

## Formatos

<img src="docs/readme/formats.svg" width="100%" alt="Formatos soportados: video MP4, MOV y AVI; audio MP3, WAV, OGG y AAC">

> [!NOTE]
> Si intentás abrir otro formato, Capyro te avisa con un sonido y un mensaje. Los formatos
> soportados también se ven dentro de la app, en el menú **ⓘ**: tocá uno para leer su resumen.

<img src="docs/readme/divider-4.svg" width="100%" alt="">

## Distribuciones

<img src="docs/readme/distros.svg" width="100%" alt="Distribuciones soportadas">

| Estado | Qué significa |
|---|---|
| **✓ Probado** | Compilado y usado de verdad en esa distro. |
| **Compatible** | Tiene todo lo necesario (FFmpeg 5.1+ y Rust 1.95+), pero todavía nadie lo probó. Si lo probás, ¡contanos! |
| **Experimental** | Debería andar con algún ajuste (musl en Alpine, rutas de librerías en NixOS). |

> [!WARNING]
> **No soportadas:** Ubuntu 22.04, Debian 11, Linux Mint 21 y anteriores. Traen FFmpeg 4.4
> y Capyro necesita **5.1 o más nuevo**.

En Linux funciona en **X11 y Wayland** (Niri, Hyprland, Sway, GNOME, KDE...), con
**PipeWire, PulseAudio, ALSA o JACK**, y con **systemd o sin él** (OpenRC, runit, s6...): no
depende del sistema de inicio. También corre en **Windows 10 y 11** (x64): ver [Windows](#windows).

<img src="docs/readme/divider-5.svg" width="100%" alt="">

## Instalación

### 1. Dependencias

Necesitás CMake, un compilador de C, **Rust 1.95+**, las librerías de **FFmpeg 5.1+**, pkg-config y Git.
Elegí tu distro:

<details>
<summary><b>Gentoo</b> &nbsp;·&nbsp; probado</summary>

```sh
sudo emerge --ask dev-build/cmake dev-vcs/git dev-lang/rust media-video/ffmpeg dev-util/pkgconf
# selector de archivos (si no tenés uno):
sudo emerge --ask sys-apps/xdg-desktop-portal sys-apps/xdg-desktop-portal-gtk
# opcional, para compilar más rápido:
sudo emerge --ask dev-build/ninja dev-util/ccache
# decodificación por GPU: FFmpeg con USE="vaapi" (Intel/AMD) o "nvenc" (NVIDIA), y el driver:
#   Intel: media-libs/intel-media-driver · AMD: mesa con USE="vaapi" · NVIDIA: nvidia-drivers
#   para comprobar: sudo emerge --ask media-video/libva-utils && vainfo
rustc --version   # si es menor a 1.95, usá rustup
```
</details>

<details>
<summary><b>Arch Linux · Artix</b> &nbsp;·&nbsp; compatible</summary>

```sh
sudo pacman -S --needed base-devel cmake git rustup ffmpeg pkgconf xdg-desktop-portal-gtk
rustup default stable
```
</details>

<details>
<summary><b>Debian 12+ · Devuan 5+ · Ubuntu 24.04+ · Linux Mint 22+</b> &nbsp;·&nbsp; probado en Ubuntu</summary>

El Rust de `apt` es demasiado viejo, así que usá rustup:

```sh
sudo apt install build-essential cmake git curl pkg-config ffmpeg \
    libavformat-dev libavcodec-dev libavutil-dev libavfilter-dev libswscale-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```
</details>

<details>
<summary><b>Fedora</b> &nbsp;·&nbsp; compatible</summary>

El FFmpeg de Fedora (`ffmpeg-free`) no trae H.264/H.265, que es lo que usan casi todos
los MP4. Usá el de RPM Fusion:

```sh
sudo dnf install https://mirrors.rpmfusion.org/free/fedora/rpmfusion-free-release-$(rpm -E %fedora).noarch.rpm \
                 https://mirrors.rpmfusion.org/nonfree/fedora/rpmfusion-nonfree-release-$(rpm -E %fedora).noarch.rpm
sudo dnf swap ffmpeg-free ffmpeg --allowerasing
sudo dnf install gcc cmake git pkgconf-pkg-config ffmpeg-devel
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
</details>

<details>
<summary><b>openSUSE Tumbleweed</b> &nbsp;·&nbsp; compatible</summary>

```sh
sudo zypper install gcc cmake git pkgconf rustup \
    ffmpeg-7-libavformat-devel ffmpeg-7-libavcodec-devel ffmpeg-7-libavutil-devel \
    ffmpeg-7-libavfilter-devel ffmpeg-7-libswscale-devel
rustup default stable
```
Para H.264/H.265 (casi todos los MP4), instalá las versiones de FFmpeg del repositorio **Packman**.
</details>

<details>
<summary><b>Void Linux</b> &nbsp;·&nbsp; compatible</summary>

```sh
sudo xbps-install -S base-devel cmake git pkg-config ffmpeg-devel rust cargo
rustc --version   # si es menor a 1.95, usá rustup
```
</details>

<details>
<summary><b>Alpine</b> &nbsp;·&nbsp; experimental</summary>

```sh
doas apk add build-base cmake git pkgconf ffmpeg-dev rust cargo
```
Usa musl en vez de glibc y todavía no se probó. Con Alpine *edge* tenés un Rust lo
bastante nuevo.
</details>

<details>
<summary><b>NixOS</b> &nbsp;·&nbsp; experimental</summary>

```sh
nix-shell -p gcc cmake git pkg-config ffmpeg rustc cargo \
    libGL libxkbcommon wayland xorg.libX11 xorg.libXcursor xorg.libXi xorg.libXrandr
```
La ventana carga OpenGL, Wayland y xkbcommon en tiempo de ejecución: si no abre, exportá
`LD_LIBRARY_PATH` con esas librerías.
</details>

### 2. Compilar

```sh
git clone https://github.com/CapyIsInsano/Capyro.git
cd Capyro
cmake -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build -j"$(nproc)"
./build/capyro
```

> [!TIP]
> Con [Ninja](https://ninja-build.org/) agregá `-G Ninja` al primer comando: reparte mejor los
> núcleos entre C y Rust. Si tenés `ccache`, se usa solo.

**¿Cuánto tarda?** La **primera** compilación arma ~160 dependencias de Rust: unos 4 minutos
con 2 núcleos, bastante menos con más. Después, **cualquier cambio recompila en ~3 segundos**.

<details>
<summary><b>Opciones de compilación</b></summary>

| Opción | Qué hace |
|---|---|
| *(ninguna)* | `-O3` en C y Rust, sin LTO: rápido de recompilar. Binario de ~10 MB. |
| `-DCAPYRO_DIST=ON` | LTO completo en C y Rust: binario ~15 % más chico (~8,4 MB). Para instalar o empaquetar. |
| `-DCAPYRO_NATIVE=ON` | Optimiza para **tu** procesador (`-march=native`). El binario no sirve en otras PCs. |
| `-DCAPYRO_CCACHE=OFF` | No usar `ccache`. |
| `-DCAPYRO_BUILD_TESTS=OFF` | No compilar las pruebas. |

La versión más optimizada posible para tu máquina:

```sh
cmake -B build-dist -G Ninja -DCMAKE_BUILD_TYPE=Release -DCAPYRO_DIST=ON -DCAPYRO_NATIVE=ON
cmake --build build-dist
```
</details>

### 3. Instalar

```sh
sudo cmake --install build                  # en /usr/local
cmake --install build --prefix ~/.local     # o solo para tu usuario
```

Instala el binario, la config de ejemplo, el lanzador `.desktop` (Capyro aparece en el menú
de aplicaciones y en "Abrir con…" para todos sus formatos) y el ícono.

> [!TIP]
> Si lo instalás **en tu usuario** (`--prefix ~/.local`), se actualiza sin pedir contraseña.
> Fijate que `~/.local/bin` esté en tu `PATH`.

### 4. Actualizar

Una vez instalado, Capyro se actualiza solo: **ⓘ → Buscar actualizaciones**. En Linux lo
compila (abajo); en Windows baja la versión ya compilada ([ver Windows](#actualizar)).

1. Le pregunta al repo de GitHub cuál es el último commit (`git ls-remote`). Si es el mismo
   de tu Capyro, te dice que ya tenés la última.
2. Si hay algo nuevo, baja el código a `~/.cache/capyro/src` y te **muestra qué commits se van a
   instalar**: los que son más nuevos que el tuyo, con su hash, título y fecha. Recién cuando tocás
   **Instalar** sigue. Con **Ahora no** no se toca nada, y desde ⓘ lo volvés a ver con **Ver**.
3. Lo compila en `~/.cache/capyro/build` con las mismas opciones que usaste vos (prefijo,
   `CAPYRO_DIST`, `CAPYRO_NATIVE`) y **solo para tu sistema** (`-DCAPYRO_SYSTEM=Lin`), con
   prioridad baja para no trabar lo que estés mirando.
4. Lo instala en el mismo lugar donde está. Si es una carpeta del sistema (`/usr/local`), te
   pide la contraseña con `pkexec`.
5. Suena el aviso, aparece **el parche** (los commits nuevos) y te pide **reiniciar**: con un
   clic se vuelve a abrir en la versión nueva, con tu lista y tu posición.

La **primera** actualización compila todo (unos minutos, como la primera vez). Las
siguientes reutilizan lo compilado y tardan **segundos**. Mientras tanto podés seguir usándolo.

| Para que funcione | |
|---|---|
| Capyro **instalado** | Corriendo desde `build/` solo te avisa si hay algo nuevo. |
| `git`, `cmake` y `cargo` | Los mismos que usaste para compilarlo. |
| Saber de qué repo bajar | Lo toma del `git remote` con el que lo compilaste. Si lo bajaste como `.zip`, usa el oficial: [CapyIsInsano/Capyro](https://github.com/CapyIsInsano/Capyro). Para usar un fork, poné `update_repo = "usuario/Capyro"`. |
| Internet | Sin conexión te avisa con el sonido de error. |

<img src="docs/readme/divider-6.svg" width="100%" alt="">

## Windows

<img align="right" width="300" src="docs/screenshot-instalador.png" alt="CapyroInstaller">

Desde la v0.3.0 Capyro corre en **Windows 10 y 11** (64 bits).

### Instalar

Bajá **CapyroInstaller.exe** del repo [CapyroInstaller](https://github.com/CapyIsInsano/CapyroInstaller)
(Releases) y abrilo:

1. Elegís **dónde se instala** (por defecto `%LOCALAPPDATA%\Programs\Capyro`) y **dónde va la
   configuración** (por defecto `Documentos\Capyro\Config\Capyro.lua`). Las dos se cambian.
2. **Instalar**: barra verde y listo. No pide permisos de administrador.
3. Te dice qué versión quedó instalada y te pregunta **¿Lanzar Capyro?** Sí o No (los dos
   cierran el instalador). Después **podés borrar el instalador**.

Crea los accesos directos (escritorio y menú Inicio) y se desinstala desde
**Configuración → Aplicaciones**. La config se crea con `system = "Win"`.

> [!IMPORTANT]
> El instalador no está firmado, así que la primera vez **SmartScreen** puede decir
> *"Windows protegió tu PC"*: tocá **Más información → Ejecutar de todas formas**.

<br clear="right">

### Actualizar

Igual que en Linux, **ⓘ → Buscar actualizaciones**, pero en Windows **no se compila nada**:
baja `capyro-windows-x64.zip` de la release [`windows`](https://github.com/CapyIsInsano/Capyro/releases/tag/windows)
del repo (la arma el CI en cada push) y reemplaza los archivos instalados. Primero te muestra
los commits nuevos, como siempre. Usa `curl` y `tar`, que vienen con Windows.

### Compilar para Windows

El CI lo hace **desde Linux** con mingw-w64 ([`.github/workflows/windows.yml`](.github/workflows/windows.yml)),
y se puede hacer igual a mano:

```sh
sudo apt install mingw-w64 cmake ninja-build      # o los paquetes de tu distro
rustup target add x86_64-pc-windows-gnu
# FFmpeg para Windows "shared" (por ejemplo de github.com/BtbN/FFmpeg-Builds), descomprimido en ffmpeg-win/
cmake -S . -B build-win -G Ninja -DCMAKE_TOOLCHAIN_FILE=cmake/mingw-w64.cmake \
      -DCAPYRO_FFMPEG_DIR=$PWD/ffmpeg-win -DCMAKE_BUILD_TYPE=Release
cmake --build build-win
cmake --install build-win --prefix dist/Capyro    # capyro.exe + DLL + config\capyro.lua
```

En Windows mismo debería compilar desde **MSYS2 (UCRT64)** con sus paquetes de `cmake`,
`ninja`, `ffmpeg` y Rust (`cmake -S . -B build -G Ninja && cmake --build build`), pero ese
camino todavía no está probado: el probado es desde Linux.

> [!NOTE]
> La versión para Windows (Capyro y el instalador) se probó con **Wine** (como Windows 10 y
> como Windows 11): el núcleo pasa todas sus pruebas y la interfaz, el tutorial, la
> instalación y la desinstalación andan. Falta probarla en un Windows de verdad: si lo hacés,
> ¡contanos cómo te fue!

### Un sistema, sus cosas

CMake detecta para qué sistema se compila (`-DCAPYRO_SYSTEM=auto`, `Lin` o `Win`) y **solo
compila e instala lo de ese sistema**: en Linux, el `.desktop`, los íconos y nada de Windows;
en Windows, el `.exe` con su ícono, las DLL y nada de Linux. Las dependencias de Rust también
van por sistema. El código queda en la config instalada (`system = "Lin"` o `"Win"`) y el
actualizador lo respeta.

<img src="docs/screenshot-sistema.png" width="100%" alt="El tutorial muestra el sistema detectado: Linux o Windows 11">

<img src="docs/readme/divider-6.svg" width="100%" alt="">

## Cómo se usa

```sh
capyro                                  # abre vacío (o retoma la última sesión)
capyro cancion.mp3 video.mov tema.ogg   # carga la lista y arranca a sonar
capyro --help
```

- **Agregar archivos**: con el botón naranja, con <kbd>Ctrl</kbd>+<kbd>O</kbd> o haciendo clic en el área vacía.
- **Barra de progreso**: clic o arrastrar para saltar. Si hacés clic en el tiempo de la derecha, alterna entre la duración total y lo que falta.
- **Volumen**: clic para silenciar, y la rueda del mouse encima sube y baja un paso por muesca.
- **Área de video**: clic para play/pausa, doble clic para pantalla completa.
- **Ventana**: se arrastra desde arriba, se maximiza con doble clic y se redimensiona desde los bordes.

### Decodificación por GPU

En **⚙ → Decodificación** elegís cómo se decodifican los videos:

| | |
|---|---|
| **Automático** | Usa la GPU si se puede, y si no, la CPU, sin avisar. Es lo recomendado. |
| **GPU** | Igual, pero si la GPU no puede, te avisa (con el sonido de error) y sigue por CPU. |
| **CPU** | Siempre por software, como hasta la v0.2.8. |

El cambio se aplica **al instante**, aunque haya un video sonando: el decodificador se rearma y
el video sigue donde estaba. Abajo de los botones dice qué se está usando, por ejemplo
"H.265 (HEVC) decodificado por la GPU (VA-API)". Si la GPU falla a mitad de un video, Capyro
pasa solo a la CPU sin cortar. Queda guardado en `capyro.lua` como `decoding = "auto"`.

Prueba, en orden: **VA-API** (Intel, AMD, y NVIDIA con `nvidia-vaapi-driver`), **NVDEC** (NVIDIA
por CUDA) y **VDPAU**. La GPU decodifica, y el cuadro se copia a memoria para mostrarlo.

### Cola y playlists

La **Lista** (abajo a la izquierda) tiene pestañas: la **Cola**, que es temporal como
siempre, y tus **playlists** (hasta 3).

- En la Cola, pasá el mouse por una canción y tocá **+** para agregarla a una playlist.
  Con **Guardar en…** guardás la Cola entera.
- Con **+** al lado de las pestañas creás una playlist nueva (y le ponés nombre ahí mismo).
- En una playlist: **Reproducir** la carga en la Cola y arranca. Si tocás una canción,
  arranca desde esa. **Doble clic** en el nombre la renombra y el **tacho** la borra
  (pide confirmar).
- Se guardan en `~/.local/share/capyro/playlists.lua`.

### Atajos

| | | | |
|---|---|---|---|
| Play / pausa | <kbd>Espacio</kbd> <kbd>K</kbd> | Lista | <kbd>Ctrl</kbd>+<kbd>L</kbd> |
| Adelantar / retroceder | <kbd>→</kbd> <kbd>←</kbd> · <kbd>L</kbd> <kbd>J</kbd> | Agregar archivos | <kbd>Ctrl</kbd>+<kbd>O</kbd> |
| Saltar al 0 %…90 % | <kbd>0</kbd> … <kbd>9</kbd> | Aleatorio | <kbd>Ctrl</kbd>+<kbd>S</kbd> |
| Volumen | <kbd>↑</kbd> <kbd>↓</kbd> | Repetir (no → lista → uno) | <kbd>Ctrl</kbd>+<kbd>R</kbd> |
| Silenciar | <kbd>M</kbd> | Velocidad − / + / normal | <kbd>[</kbd> <kbd>]</kbd> <kbd>⌫</kbd> |
| Siguiente / anterior | <kbd>N</kbd> <kbd>P</kbd> | Efectos de audio | <kbd>Ctrl</kbd>+<kbd>E</kbd> |
| Detener | <kbd>S</kbd> | Prender / apagar efectos | <kbd>Shift</kbd>+<kbd>E</kbd> |
| Pantalla completa | <kbd>F</kbd> <kbd>F11</kbd> | Capturar fotograma | <kbd>Shift</kbd>+<kbd>S</kbd> |
| Cerrar menú / pantalla completa | <kbd>Esc</kbd> | Salir | <kbd>Ctrl</kbd>+<kbd>Q</kbd> |

Todos se cambian desde **⚙ → Editar configuración** (ver [Configuración](#configuración)).

<img src="docs/readme/divider-7.svg" width="100%" alt="">

## Configuración

Todo se configura en un archivo **Lua**. Para empezar, copiá el de ejemplo:

```sh
mkdir -p ~/.config/capyro
cp config/capyro.lua ~/.config/capyro/
```

```lua
return {
    keys = {
        play_pause   = { "Space", "K" },   -- una tecla o una lista
        seek_forward = "Shift+Right",       -- modificadores: Ctrl+, Shift+, Alt+
    },
    options = {
        seek_step        = 10,         -- segundos por salto
        wheel_step       = 5,          -- % de volumen por muesca de la rueda
        speed            = 1.0,        -- velocidad al abrir (0.5 .. 2.0)
        ["repeat"]       = "off",      -- "off", "all" u "one"
        accent           = "#FF7417",  -- ¡el color naranja también se cambia!
        remember_session = true,       -- retomar lista y posición
        visualizer       = true,       -- barras de frecuencia en los audios
        screenshot_dir   = "",         -- "" = ~/Imágenes/Capyro
        backend          = "auto",     -- "auto", "x11" o "wayland"
        splash           = true,       -- pantalla de carga al abrir
        startup_sound    = true,       -- sonido de inicio
        click_sound      = true,       -- clic en botones y sonido al cerrar menús
        show_format      = true,       -- "MP4 · H.264" arriba a la izquierda del video
        show_millis      = true,       -- y debajo, los milisegundos (000..999)
        decoding         = "auto",     -- "auto", "gpu" o "cpu"
        system           = "Lin",      -- "Lin" o "Win": lo pone la instalación ("auto" = detectarlo)
        update_repo      = "",         -- "" = el repo del que lo compilaste (o el oficial)
    },
}
```

Para los **atajos** no hace falta tocar el archivo: **⚙ → Editar configuración** muestra cada
función con sus teclas.
- Tocás una tecla para quitarla y **+** para agregar otra: apretás la combinación y listo.
- **Agregar atajo** ofrece las funciones que todavía no tienen tecla.
- Si la tecla la usaba otra función, se la pasa y te avisa.
- Se guarda solo en `~/.config/capyro/capyro.lua` (en Windows, en la que eligió el
  instalador), con una copia del anterior en `capyro.lua.bak`.

Si editás el archivo a mano, en el mismo editor está **Recargar archivo**. Todas las
opciones y acciones están comentadas en [`config/capyro.lua`](config/capyro.lua).

<details>
<summary><b>¿Dónde guarda Capyro sus cosas?</b></summary>

| Qué | Linux | Windows |
|---|---|---|
| Configuración | `~/.config/capyro/capyro.lua` | La que elegiste al instalar (por defecto `Documentos\Capyro\Config\Capyro.lua`; la ruta está en `capyro-config.txt`, al lado de `capyro.exe`) |
| Sesión (lista, posición, volumen, efectos) | `~/.local/state/capyro/session.lua` | `%LOCALAPPDATA%\Capyro\state\session.lua` |
| Playlists | `~/.local/share/capyro/playlists.lua` | `Documentos\Capyro\playlists.lua` |
| Tutorial ya visto | `~/.local/state/capyro/tutorial-visto` | `%LOCALAPPDATA%\Capyro\state\tutorial-visto` |
| Capturas | `~/Imágenes/Capyro` (según `xdg-user-dirs`) | `Imágenes\Capyro` |
| Actualizaciones (y `update.log`) | `~/.cache/capyro/` | `%LOCALAPPDATA%\Capyro\cache\` |

En los dos, la variable `CAPYRO_CONFIG` manda sobre todo.

Sin archivo de configuración usa la que trae adentro del binario. Si el archivo tiene un
error, te avisa en la terminal y usa la de adentro.
</details>

<img src="docs/readme/divider-8.svg" width="100%" alt="">

## Solución de problemas

<details>
<summary><b>No se escucha nada</b></summary>

Abrí **ⓘ**: abajo dice qué sistema de audio está usando Capyro. Si dice *sin dispositivo*,
no encontró PipeWire, PulseAudio, ALSA ni JACK. Revisá que tu servidor de sonido esté
corriendo y que Capyro no esté silenciado (<kbd>M</kbd>).

Para ver si los sonidos de la app se disparan, abrilo con `CAPYRO_DEBUG_SFX=1 capyro`: cada
sonido (clic, foto, ready, error...) se anota en la terminal.
</details>

<details>
<summary><b>El botón "Agregar archivo(s)" no abre nada</b></summary>

El selector usa el portal de escritorio. Instalá `xdg-desktop-portal` + `xdg-desktop-portal-gtk`
(o `-kde` / `-gnome`), o al menos `zenity` o `kdialog` como respaldo.
</details>

<details>
<summary><b>Arrastrar archivos a la ventana no hace nada (Wayland)</b></summary>

La librería de ventanas que usa Capyro (winit) todavía no recibe archivos soltados en
Wayland nativo. Usá el botón **Agregar**, o poné `backend = "x11"` en la config para correr
por XWayland, donde el arrastre funciona si tu compositor lo permite.
</details>

<details>
<summary><b><code>requires rustc 1.95</code> / <code>edition2024</code> al compilar</b></summary>

Tu Rust es viejo. Instalá [rustup](https://rustup.rs) y corré `rustup default stable`.
</details>

<details>
<summary><b><code>Package 'libavfilter' was not found</code></b></summary>

Faltan las librerías de desarrollo de FFmpeg (los paquetes `-dev` / `-devel`). Revisá los
comandos de tu distro en [Instalación](#instalación).
</details>

<details>
<summary><b>"Buscar actualizaciones" falla</b></summary>

| Mensaje | Qué hacer |
|---|---|
| *Sin conexión* | Revisá internet. Capyro necesita llegar a GitHub. |
| *No encontré el repo* | El repo es privado o la URL está mal. Revisá `update_repo` en la configuración. |
| *Necesita git / cmake / cargo* | Instalá lo que falte. Si usás rustup y abrís Capyro desde el menú, igual busca en `~/.cargo/bin`. |
| *Falló la compilación* | El detalle completo está en `~/.cache/capyro/update.log`. |
| *No hay agente de polkit* | Tu escritorio no tiene quién pida la contraseña (en Niri/Hyprland: `polkit-gnome`, `lxqt-policykit` o `hyprpolkitagent`). O instalá Capyro en `~/.local`. |
</details>

<details>
<summary><b>En la barra de tareas sale un ícono roto en vez del capibara</b></summary>

La barra busca el ícono `capyro` en el tema de íconos, a partir del lanzador `capyro.desktop`.
Desde la v0.2.7 se instala en todos los tamaños y `cmake --install` refresca el caché solo.
Si lo instalaste con una versión anterior, volvé a instalar. Si igual no aparece, reiniciá
la barra (o cerrá sesión): algunas leen los íconos una sola vez al arrancar. Corriendo desde
`build/` sin instalar no hay lanzador, así que no hay ícono.
</details>

<details>
<summary><b>Las esquinas de la ventana se ven negras</b></summary>

Tu gestor de ventanas no tiene transparencia (no hay compositor). Es solo estético.
</details>

<details>
<summary><b>Elegí GPU pero dice "GPU no disponible" (o un video 4K va a los saltos)</b></summary>

En **⚙ → Decodificación**, abajo de los botones, dice qué se está usando y por qué.

| Dice | Qué hacer |
|---|---|
| *No hay GPU compatible* | Falta el driver de VA-API o tu FFmpeg no lo trae. Corré `vainfo`: si da error, instalá el driver (Intel: `intel-media-driver`, AMD: Mesa con VA-API). En NVIDIA hace falta el driver propietario (o `nvidia-vaapi-driver`). |
| *Tu GPU o tu FFmpeg no decodifican …* | Ese códec no va por GPU (por ejemplo, el MPEG-4 viejo de muchos AVI, o AV1 en placas viejas). Se ve igual, por CPU. |
| *La GPU no soporta este perfil* | Por ejemplo, H.264 de 10 bits. Sigue por CPU solo. |

Con un video 4K y sin GPU, compilá con `-DCAPYRO_NATIVE=ON` para exprimir el procesador.
</details>

<details>
<summary><b>Las pruebas de mp3 dicen "SALTEO"</b></summary>

Tu `ffmpeg` no tiene encoder de mp3, y solo se usa para *generar* los archivos de prueba.
En Gentoo se activa con el USE `lame`. Para *reproducir* mp3 no hace falta.
</details>

<img src="docs/readme/divider-9.svg" width="100%" alt="">

## Por dentro

<img src="docs/readme/stack.svg" width="100%" alt="C, Rust, CMake y Lua">

<details>
<summary><b>Arquitectura</b></summary>

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

- **C es el dueño**: `main.c` crea el núcleo, le pasa el puntero a la GUI y lo libera al cerrar.
- **Tres hilos**: la GUI, el de decodificación y el callback de audio. Un único mutex protege
  lo compartido, y nunca se llama a FFmpeg ni a miniaudio con ese candado tomado.
- **El audio marca el tiempo**: la posición sale de las muestras que realmente sonaron, y el
  video muestra el fotograma que corresponde.
- **Velocidad y tono** van en el grafo de FFmpeg (`atempo`, `asetrate`). El **ecualizador,
  el estéreo y el modo noche** están en C (`core/src/dsp.c`) dentro del callback de audio,
  por eso responden al instante.
- **La lista vive en el núcleo**: orden, aleatorio y repetir se deciden en C. La GUI solo
  muestra y manda comandos.
- **Frontera C ↔ Rust segura**: `CapyroCore` es opaco, los enums viajan como `int`, todo el
  `unsafe` de la interfaz está en `gui/src/ffi.rs` y ningún `panic` cruza hacia C.
- **El binario sabe de dónde salió**: en cada compilación `cmake/buildinfo.cmake` guarda el
  commit, el repo, el prefijo de instalación y el sistema (`capyro --version` los muestra). El
  actualizador (`gui/src/updater.rs`) corre en su propio hilo y compara ese commit con el del repo.
- **Un código, dos sistemas**: lo que cambia entre Linux y Windows está aislado en
  `core/src/platform.c` (hilos, reloj, archivos, versión del sistema), `gui/src/paths.rs`
  (carpetas) y `gui/src/updater/` (`unix.rs` compila, `windows.rs` baja la release). Todo lo
  demás es igual en los dos.
</details>

<details>
<summary><b>Estructura del proyecto</b></summary>

```
capyro/
├── CMakeLists.txt            # build principal (trae Corrosion; detecta Lin/Win)
├── cmake/buildinfo.cmake     # guarda el commit de git y el sistema en cada build
├── cmake/mingw-w64.cmake     # compilar para Windows desde Linux
├── core/                     # ── núcleo en C ──
│   ├── include/capyro_core.h #    API pública (lo único que ve Rust)
│   └── src/
│       ├── core.c            #    API, transporte, lista de reproducción
│       ├── player.c          #    FFmpeg + miniaudio: hilos, colas, reloj
│       ├── dsp.c/.h          #    efectos de audio (EQ, estéreo, modo noche)
│       ├── sfx.c             #    sonidos de la app (decodifica desde memoria)
│       ├── buildinfo.c       #    commit y prefijo del build (para actualizar)
│       ├── hwdec.c           #    decodificación por GPU (VA-API, NVDEC, VDPAU / D3D11VA, DXVA2)
│       ├── platform.c/.h     #    hilos, reloj, archivos y sistema (POSIX o Win32)
│       ├── internal.h        #    estructuras internas
│       ├── miniaudio_impl.c  #    implementación de miniaudio
│       └── main.c            #    punto de entrada
├── gui/                      # ── interfaz en Rust ──
│   └── src/
│       ├── lib.rs            #    capyro_gui_run() + ventana
│       ├── app.rs            #    layout y lógica de la UI
│       ├── app/fx_menu.rs    #    menú de efectos de audio
│       ├── app/lists.rs      #    panel Lista: cola y playlists
│       ├── app/keys_editor.rs #   editor de atajos
│       ├── app/changes_menu.rs #  menú de novedades
│       ├── app/tutorial.rs   #    tutorial de bienvenida
│       ├── playlists.rs      #    guardar / leer las playlists
│       ├── changes.rs        #    novedades (lee el CHANGELOG embebido)
│       ├── ffi.rs            #    bindings seguros al núcleo
│       ├── viz.rs            #    visualizador (FFT propia)
│       ├── session.rs        #    guardar / retomar la sesión
│       ├── paths.rs          #    dónde se guarda cada cosa (Linux / Windows)
│       ├── updater.rs        #    buscar e instalar actualizaciones
│       ├── updater/unix.rs   #      Linux: git + cmake
│       ├── updater/windows.rs #     Windows: baja la release "windows"
│       ├── config.rs         #    lectura de capyro.lua (mlua)
│       ├── formats.rs        #    resúmenes de cada formato
│       ├── icons.rs          #    íconos dibujados a mano
│       └── theme.rs          #    colores y fuentes
├── config/capyro.lua         # configuración por defecto
├── assets/                   # logo + sounds/ (capibaras, error, inicio, actualización)
├── packaging/                # Linux: .desktop + íconos · windows/: .ico, .rc y manifiesto
├── .github/workflows/        # CI: compila para Windows y publica la release "windows"
├── tests/                    # pruebas del núcleo
└── docs/                     # capturas e ilustraciones de este README
```
</details>

<details>
<summary><b>Pruebas</b></summary>

```sh
ctest --test-dir build --output-on-failure   # núcleo en C
cd gui && cargo test                         # config, sesión, visualizador y actualizador
```

`ctest` genera archivos en **todos los formatos** con `ffmpeg` y prueba el núcleo de punta
a punta: reproducción, pausa, búsqueda, fin del archivo, carátulas, fotogramas de video,
lista, aleatorio, repetir y velocidad.

Los efectos se prueban **midiendo el sonido**:
- Graves +12 suben ~12 dB más los 50 Hz que los 5 kHz.
- Mono deja los dos canales iguales.
- Con todo al máximo, nada satura.
- El modo noche achica la diferencia entre lo fuerte y lo bajito.
- Tono +12 convierte un La de 440 Hz en 880 Hz sin cambiar la velocidad.

Sin placa de sonido (un servidor, CI) se usa un dispositivo "nulo" que igual consume el
audio en tiempo real. El núcleo también pasa por ThreadSanitizer y AddressSanitizer.

Compilado para Windows, `test_core.exe` corre las mismas pruebas (en Windows o con Wine),
más una que revisa que el sistema detectado sea el mismo para el que se compiló.
</details>

<img src="docs/readme/divider-10.svg" width="100%" alt="">

## Roadmap y licencia

**Lo que viene**
- [ ] Subtítulos `.srt`
- [ ] Ecualizador gráfico de más bandas y presets propios
- [ ] Más opciones en el menú de ajustes
- [ ] Hooks en Lua (`on_open`, `on_end`, acciones personalizadas)
- [x] Windows 10 y 11
- [ ] macOS, más adelante

**Lo que ya está:** [el historial completo de versiones](CHANGELOG.md).

<br>

Capyro es **software libre** bajo la [GNU GPL v3.0](LICENSE) (o, a tu elección, cualquier
versión posterior): podés usarlo, estudiarlo, compartirlo y modificarlo.

Gracias a [FFmpeg](https://ffmpeg.org/) (LGPL/GPL), [miniaudio](https://miniaud.io/)
(dominio público / MIT-0), [egui](https://github.com/emilk/egui) (MIT/Apache-2.0),
[mlua](https://github.com/mlua-rs/mlua) con Lua 5.4 (MIT) y
[Corrosion](https://github.com/corrosion-rs/corrosion) (MIT).

<div align="center">
<br>
<img src="assets/capyro_logo_128.png" width="56" alt="">
<br>
<b>Capyro</b> · <i>Just press play.</i>
</div>

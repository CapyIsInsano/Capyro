<div align="center">
<img src="assets/capyro_logo_128.png" width="64" alt="">

# Historial de versiones

*Just press play.*
</div>

---

## v0.3.0-alpha · El salto a Windows

- **Capyro corre en Windows 10 y 11**, además de Linux.
  - El núcleo en C usa hilos, reloj y archivos de Win32 (nombres con tildes y ñ incluidos), y el audio sale por WASAPI.
  - La GPU en Windows: D3D11VA, NVDEC o DXVA2, con la misma vuelta a la CPU si no se puede.
  - Fuente Segoe UI, ícono y versión adentro del `.exe`, y sin ventana de consola (salvo que le pases opciones).
  - Dibuja con OpenGL y, si la placa de video no lo soporta, con Direct3D 12 (wgpu). Si no puede abrir la ventana, lo dice en un cartel y lo anota en `%LOCALAPPDATA%\Capyro\capyro.log` (antes se quedaba sin abrir y sin avisar). `CAPYRO_RENDERER=wgpu` u `opengl` fuerza uno.
- **Cada sistema compila e instala solo lo suyo:**
  - CMake detecta el sistema (`CAPYRO_SYSTEM` = `Lin` o `Win`); en Linux no entra nada de Windows y al revés (dependencias de Rust, `.desktop` e íconos, DLL, recursos `.rc`...);
  - la config instalada lleva el sistema: `system = "Lin"` o `"Win"` (opción nueva en `capyro.lua`; `"auto"` lo detecta solo);
  - el actualizador instala solo lo de tu sistema: en Linux compila como siempre; en Windows baja la versión para Windows publicada en el repo y reemplaza los archivos, sin compilar nada.
- **CapyroInstaller** (repo aparte): el instalador para Windows, con la cara de Capyro.
  - Elegís dónde se instala y dónde va la config (por defecto `Documentos\Capyro\Config\Capyro.lua`).
  - Botón Instalar gris que crece y se pone amarillo con el mouse encima, y suena el clic de Capyro.
  - Barra de progreso verde; al terminar, la versión instalada y "¿Lanzar Capyro?" Sí/No.
  - Accesos directos, desinstalar desde Configuración → Aplicaciones, y no pide administrador.
- **Tutorial:**
  - antes de "¡Bienvenido a la manada!" aparece el **sistema detectado**: Linux (con tu distro y el kernel) o Windows 10/11 (con su versión);
  - al lado de "Potenciado por Claude" hay un destello que gira cuando le pasás el mouse, como pensando, y da una vuelta rápida con el clic.
  - más animado: chispitas que suben por el fondo, halos que salen del logo, cursor de máquina de escribir, la tecla Espacio dibujada que se hunde, puntitos de progreso que se estiran, carteles que entran con un saltito y una barrita con el avance del recorrido, esquinas tipo visor sobre lo iluminado, el capibara de "¿Estás listo?" mirando a los costados y, en la bienvenida, rayos de luz que giran y papelitos que dan vueltas.
- En Windows todo se guarda donde corresponde: la config donde la puso el instalador, las listas en `Documentos\Capyro`, la sesión y la caché en `%LOCALAPPDATA%\Capyro` y las capturas en tu carpeta de Imágenes.
- El menú ⓘ dice para qué sistema está compilado tu Capyro, y `capyro --version` también.
- CI: cada push a `main` compila Capyro para Windows y lo publica en la release `windows` (de ahí bajan el actualizador y el instalador).

## v0.2.9-alpha · Decodificación por GPU

- **Decodificación de video por GPU** en el núcleo en C:
  - VA-API (Intel y AMD), NVDEC (NVIDIA por CUDA) o VDPAU, lo primero que ande;
  - lo pesado (H.264, H.265, VP9, AV1, sobre todo en 4K) lo hace la placa de video y la CPU queda libre;
  - si la GPU no puede con un video, o falla a mitad de camino, sigue sola por CPU sin cortar.
- **Tutorial de bienvenida**: sale la primera vez que abrís Capyro (y en Ajustes → Ver el tutorial de nuevo).
  - Presentación con el logo, textos que se escriben solos (Espacio para seguir) y el perfil de GitHub del autor. Más abajo, chiquito, dice "Potenciado por Claude · Anthropic".
  - Recorrido por toda la interfaz, iluminando cada parte con su explicación.
  - "¿Estás listo?": No → otra vuelta; Sí → "¡Bienvenido a la manada!".
  - Tiene música de fondo, "Bienvenida a la manada" de Capy y los Capibaras, que suena en el propio reproductor. Así el área de video muestra la carátula, el visualizador, el formato y los milisegundos mientras se explican. Al terminar, la cola de antes vuelve a como estaba.
  - Esc lo salta.
- Nueva sección **Decodificación** en Ajustes: Automático, GPU o CPU.
  - El cambio se aplica al instante, aunque haya un video sonando.
  - Abajo de los botones dice qué se está usando y, si no es la GPU, por qué.
  - Con "GPU" elegido, si no se puede usar, te avisa con el sonido de error.
- Opción nueva en capyro.lua: `decoding = "auto"` (o `"gpu"`, `"cpu"`).
- Sonidos nuevos:
  - **clic** al tocar cualquier botón (se apaga con `click_sound = false`);
  - **foto** al capturar un fotograma;
  - **ready** al tocar **Instalar** en el aviso de actualización.
  - Si un botón tiene su propio sonido (logo, error, captura, instalar), suena ese y no el clic.
- Sonido **retraction** al cerrar o retraer algo (menús, la lista, el aviso de actualización, salir de pantalla completa), el compañero del clic de abrir. Usa el mismo `click_sound`.
- `CAPYRO_DEBUG_SFX=1 capyro` anota en la terminal cada sonido que suena.
- Debajo del formato, una burbuja con los **milisegundos** del segundo actual (000 a 999, vuelve a 0 en cada segundo). Se oculta con su propio interruptor en Ajustes (`show_millis`); sin el formato, sube deslizándose a su lugar.
- Arriba a la izquierda del video se ve el **formato** del archivo (por ejemplo `MP4 · H.264 · GPU`, o `MP3`). Se oculta en Ajustes → Mostrar el formato del archivo, o con `show_format = false`.

## v0.2.8-alpha · Playlists y atajos

- **Editar configuración** desde la app (en Ajustes, reemplaza a "Recargar configuración"):
  - cada función con sus teclas, y se pueden quitar o agregar;
  - "Agregar atajo" muestra las funciones que todavía no tienen tecla;
  - si la tecla ya la usaba otra función, se la pasa y te avisa;
  - se guarda solo en `~/.config/capyro/capyro.lua` (con copia `.bak`).
- **Playlists** en la Lista (hasta 3):
  - la Cola sigue siendo temporal, como siempre;
  - con **+** en una canción de la Cola la agregás a una playlist, o guardás toda la Cola de una;
  - cada playlist se reproduce entera o desde la canción que toques;
  - se renombran con doble clic y se borran con el tacho.
- Botón **Novedades** al lado del de información, con los cambios de esta versión (las anteriores, en el repo). Tiene un puntito mientras no lo abras.
- **Buscar actualizaciones** ahora primero te muestra qué commits se van a instalar (los más nuevos que el tuyo) y espera a que toques **Instalar**.
- La ventana se llama **Capyro** (con mayúscula) en la barra de tareas y el dock.
- El repo oficial ya viene puesto: si compilaste desde un .zip, igual se actualiza desde github.com/CapyIsInsano/Capyro.

## v0.2.7-alpha · Se actualiza solo

- **Buscar actualizaciones** en el menú ⓘ:
  - se fija si hay commits nuevos en el repo de GitHub;
  - si hay, baja el código, lo compila y lo instala solo (con `pkexec` si se instaló en el sistema);
  - suena el aviso de actualización y aparece **el parche**, con los commits nuevos;
  - un botón **Reiniciar** abre la versión nueva, con tu lista y tu posición.
- Sin conexión, repo equivocado o falta de herramientas: mensaje claro y sonido de error.
- La primera actualización compila todo. Las siguientes tardan segundos, porque reutiliza lo compilado.
- **Pantalla de carga** al abrir: logo, "Cargando…" y un ecualizador animado. Después la interfaz
  entra por partes y el logo vuela a su lugar. Al terminar de cargar suena el inicio.
- `capyro --version` muestra el commit y dónde está instalado.
- Opciones nuevas: `splash`, `startup_sound`, `update_repo` y `update_branch`.
- Los avisos ahora se ven por encima de los menús abiertos.
- El ícono de la barra de tareas ahora es el logo: se instala en todos los tamaños (16 a 512 px y SVG) y se refresca el caché de íconos al instalar.

## v0.2.6-alpha · Efectos de audio

- **Menú de efectos de audio** (botón ≡ al lado del volumen, <kbd>Ctrl</kbd>+<kbd>E</kbd>):
  - graves, medios y agudos (±12 dB);
  - tono ±12 semitonos sin cambiar la velocidad;
  - estéreo, de mono a 200 %;
  - modo noche (compresor).
- Presets: Normal, Bajos, Voces, Lo-fi y Noche.
- Interruptor general (<kbd>Shift</kbd>+<kbd>E</kbd>) y botón Restablecer.
- El ecualizador está escrito en C y se aplica en tiempo real, con un limitador suave para que no sature.
- Los efectos se recuerdan entre sesiones.
- El visualizador muestra el sonido ya ecualizado y ya no tapa al artista.
- El porcentaje de volumen ahora aparece arriba del ícono.
- README nuevo con el estilo de la app.

## v0.2.5-alpha · Compilación rápida y funciones nuevas

- Recompilar tras un cambio pasa de **~72 s a ~3 s** (sin LTO por defecto, scripts de compilación sin optimizar, miniaudio a `-O2`).
- `-DCAPYRO_DIST=ON` (LTO completo), `-DCAPYRO_NATIVE=ON` (`-march=native`) y soporte para `ccache`.
- El ejecutable baja de 11,9 MB a 9,8 MB.
- **Retoma la sesión**: lista, archivo, posición, volumen, velocidad, aleatorio y repetir.
- **Visualizador** de frecuencias con FFT propia.
- **Capturas** del video en PNG (<kbd>Shift</kbd>+<kbd>S</kbd>).
- Tiempo restante con un clic, y saltos al 0–90 % con <kbd>0</kbd>–<kbd>9</kbd>.

## v0.2.4-alpha · Formatos y sonidos

- La rueda sobre el volumen ahora mueve un paso por muesca (antes bajaba casi todo de golpe). Se ajusta con `wheel_step`.
- Nuevos formatos: **AVI** y **AAC**.
- En el menú ⓘ, cada formato muestra un resumen; solo uno abierto a la vez.
- Al tocar el logo suena un capibara, uno de tres al azar.
- Sonido de error al abrir un archivo no soportado o al tocar subtítulos.

## v0.2.3-alpha · WAV, OGG y MOV

- Nuevos formatos: **WAV**, **OGG** y **MOV**.
- Menú **ⓘ** con la versión, los formatos soportados y el sistema de audio en uso.
- Arreglo: al repetir un solo archivo, el audio quedaba mudo al volver a empezar.

## v0.2.2-alpha · Lista y animaciones

- **Aleatorio** (sin repetir hasta recorrer la lista) y **repetir** (lista o un archivo).
- Menú **⚙** con la velocidad (0.5× a 2×, sin cambiar el tono).
- Animaciones en todos los botones, menús y avisos.
- Arreglo: no se podía quitar un archivo de la lista.
- Capyro pasa a ser solo para Linux.

## v0.2.1-alpha · Agregar archivos

- Botón naranja **"+ Agregar archivo(s)"** con el selector de archivos del sistema, más <kbd>Ctrl</kbd>+<kbd>O</kbd>.
- Opción `backend` (X11/Wayland), porque arrastrar archivos no funciona en Wayland nativo.

## v0.2.0-alpha · ¡Reproduce!

- Reproducción real de **MP4** y **MP3** con FFmpeg y miniaudio.
- El video va sincronizado con el audio, que marca el tiempo.
- Búsqueda precisa, carátulas, metadatos y paso automático al siguiente.
- Pruebas de punta a punta con ThreadSanitizer y AddressSanitizer.

## v0.1.0-alpha · El esqueleto

- Ventana con el diseño original: logo, área de video, controles y lista.
- Núcleo en C, interfaz en Rust, compilación con CMake y configuración en Lua.

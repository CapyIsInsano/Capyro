<div align="center">
<img src="assets/capyro_logo_128.png" width="64" alt="">

# Historial de versiones

*Just press play.*
</div>

---

## v0.3.3-alpha · Capyro Studio

- **Capyro Studio**: un botón en la burbuja de funciones (vista de música) o abajo a la izquierda, al lado de ⓘ y Novedades (con un video), abre un **panel encima de la ventana** (no otra ventana). Arriba, la **portada** a la izquierda y el **nombre de la canción** en grande, con el artista y el resto de los datos al lado; abajo, la vista previa y el ecualizador. Se cierra con ✕, con Esc o tocando afuera. Cerrado, no queda nada cargado en memoria (ni las imágenes): Capyro no se vuelve un traga RAM.
- **Respeta el modo de rendimiento**: en **Bajo** aparece y se va al toque, sin sombras, brillos ni curva; en **Medio**, con su animación, la sombra de la portada y la curva del ecualizador; en **Alto**, además, la luz del color de la portada, el brillo de la curva y del tramo, y el panel que se descubre con una línea de luz.
- **Portada**: tocá la imagen y se abre el selector de archivos para elegir otra (jpg, png, webp, bmp). Se ve al toque, pero se guarda recién al tocar **Guardar cambios**. También se puede **quitar** o **volver a la original**. Un JPG o PNG de hasta 1600 px va tal cual; lo demás (WebP, BMP, fotos enormes) se guarda como JPEG de 1200 px. Anda en MP3, FLAC, M4A, OGG y Opus (en Ogg va como la etiqueta estándar `METADATA_BLOCK_PICTURE`); WAV no guarda portada.
- **Etiquetas**: editá el **nombre de la canción**, el **artista o intérprete**, el **álbum**, el **artista principal del álbum**, el **género**, el **año**, el **número de pista**, el **número de disco**, el **compositor** y los **comentarios**.
  - Al guardar, el archivo se reescribe **copiando el audio tal cual** (sin recodificar, sin pérdida de calidad). El original queda como `.bak`; si guardás de nuevo, la versión anterior va a `.bak2`, `.bak3`… y nunca se pisa un respaldo.
  - Si el archivo es el que está sonando, Capyro lo vuelve a abrir en el mismo punto y sigue sonando (o en pausa, como estaba), ya con la portada y los datos nuevos.
  - Si cerrás con cambios sin guardar, avisa: el segundo intento cierra y los descarta. **Ctrl+S** guarda.
  - En Windows, los archivos con acentos o ñ en el nombre ("Canción.mp3") se guardan bien.
  - Audio: MP3, FLAC, OGG, M4A, WAV, Opus. **En WAV** no se guardan el artista principal del álbum, el número de disco, el compositor ni la portada (el formato no tiene dónde ponerlos), y el panel lo avisa.
- **Capyro Studio también edita videos**: con un video se abre con su botón de abajo a la izquierda, al lado de ⓘ y Novedades (en la vista de música, con el de la burbuja). Los campos se adaptan ("Título del video", "Autor, canal o director", "Serie o colección"…) y sin portada se ve una película en vez de la nota.
  - Las etiquetas se guardan en todos los formatos de video. La **portada**, en **MP4/M4V** (como imagen de portada) y **MKV** (como adjunto `cover.jpg`/`cover.png`, que es lo que leen los reproductores y los exploradores de archivos). MOV y WebM no la guardan, y el panel lo avisa.
  - Al guardar se copian tal cual el video, el audio, **los subtítulos** (SRT en MKV, mov_text en MP4…) y **los capítulos**: no se recodifica nada ni se pierde calidad.
  - Guardar reescribe el archivo entero, así que ahora pasa **en otro hilo**: con un video de varios GB la ventana no se congela; el pie dice "Guardando… un video grande tarda unos segundos" y, mientras tanto, no se edita ni se cierra el panel. Ojo: el `.bak` es una copia entera del video (ocupa lo mismo).
- **Capyro Converter**: un panel nuevo (como Studio) para **pasar un archivo a otro formato**. Se abre con su botón ⇄, al lado del de Studio (en la burbuja de funciones, o abajo a la izquierda con un video). Arranca con lo que está sonando, o elegís otro archivo ("Cambiar archivo") o lo arrastrás a la ventana.
  - **7 perfiles**, que se arman según el archivo: **Alta calidad** (video: MP4 H.264 casi sin pérdida y AAC 256 kbps; audio: MP3 320 kbps), **Tamaño reducido** (MP4 H.265 a 720p, o H.264 si no hay con qué; audio: Opus 96 kbps), **Compatibilidad universal** (MP4 H.264 High 4.1 hasta 1080p y AAC estéreo; audio: MP3 192 kbps), **Música sin pérdida** (FLAC), **Video para celulares** (MP4 H.264 a 720p y 30 fps; de un audio, un MP4 con la portada), **Extraer audio** (el audio tal cual, sin recodificar: rápido y sin pérdida, en su formato: M4A, MP3, Opus, FLAC…) y **Crear video con portada** (de un audio, un MP4 a 1080p con la portada al medio y la misma desenfocada de fondo; sin portada, el logo de Capyro, o la imagen que elijas).
  - **Funciones avanzadas**: formato (MP4, MKV, WebM, MOV, MP3, M4A, FLAC, OGG, Opus, WAV), códec de video (H.264, H.265, VP9, AV1, copiar o sin video) y de audio (AAC, MP3, Opus, Vorbis, FLAC, ALAC, PCM, copiar o sin audio), **calidad** con un deslizador, **resolución**, **cuadros por segundo**, **velocidad** (rápida, normal, lenta), **compatibilidad máxima**, **kbps**, frecuencia, canales (estéreo, mono), **emparejar el volumen** (EBU R128), **recortar** con dos manijas y conservar etiquetas, capítulos y portada. Lo que no entra en un formato aparece en gris, y tocar algo avanzado pasa a "Personalizado".
  - **Usar la placa de video**: prueba NVENC (NVIDIA), Quick Sync (Intel) o AMF (AMD); si la PC no la tiene, sigue con el procesador sin avisar. En Windows, H.264 sale de **OpenH264** (el FFmpeg que trae Capyro es LGPL y no incluye x264).
  - Abajo dice **cuánto va a pesar** (aproximado) y dónde se guarda: la misma carpeta (o la que elijas) y **nunca pisa nada**: si ya existe, "nombre (2)"; si es el mismo archivo, "(convertido)".
  - Convierte **en otro hilo**: el panel se puede cerrar y seguir escuchando música; el botón ⇄ muestra un anillo con cuánto va y, al terminar, avisa con un sonido. En el panel: el porcentaje, cuánto falta, **Cancelar** (no deja archivos a medias) y al final una tarjeta con el tamaño, el tiempo, qué codificador se usó y **Reproducir**, **Agregar a la lista** y **Abrir carpeta**.
  - Videos girados (celulares) salen derechos, los subtítulos y capítulos se copian si el formato los acepta, y un video con H.265 en MP4 se marca como `hvc1` (lo abren Apple y los celulares).
  - El motor es nuevo, en C con las librerías de FFmpeg (`core/src/media/convert.c`), con sus pruebas.
- **Visualizador con tres lugares** (⚙ → Visualizador): **a los lados** de la portada (dos columnas de barras que salen de sus bordes, graves abajo y agudos arriba), **en medio** (sobre la portada, como en la versión anterior) o **apagado**. En `capyro.lua`: `visualizer = "lados"`, `"medio"` o `"apagado"` (el `true`/`false` de antes sigue andando).
- **Portada con los graves** (⚙): la portada se agranda y se achica con los bajos de la música. Se apaga con su interruptor (`cover_pulse` en `capyro.lua`).
  - En el modo **Bajo** no hay ni visualizador ni portada que late: los dos aparecen en gris ("no en Bajo") y vuelven al pasar a Medio o Alto.
- **Disco de vinilo** (⚙, con un audio): la portada va al medio de un **disco de vinilo que gira**, como etiqueta. Gira a 33⅓ vueltas por minuto **por la velocidad de reproducción** (a 2×, el doble de rápido; a 0,5×, la mitad), y arranca y frena despacio, como un tocadiscos. El disco tiene sus surcos, un brillo que no gira (la luz está quieta, como en uno de verdad) y, sin portada, una etiqueta del color de acento.
  - Con el disco, **el visualizador cambia sus opciones**: **Alrededor** (las barras rodean el disco, espejadas, con los graves arriba), **Abajo** o **Desactivado**. Sin el disco vuelven las de siempre (a los lados, en medio, apagado), con lo que tenías elegido.
  - También en la **vista clásica** (ventanas chicas) y en **Capyro Studio**: la portada se ve ya puesta en el disco (y la nueva, al elegirla), girando mientras suena.
  - Con "Portada con los graves", el disco late; en Alto, el aura gira alrededor del disco.
  - En el modo **Bajo** el disco se ve, pero quieto y sin visualizador. En `capyro.lua`: `vinyl = true` y `vinyl_visualizer = "alrededor"`, `"abajo"` o `"apagado"`.
- **⚙ tiene scroll** en ventanas bajitas (con la rueda), así entra todo aunque la ventana sea chica.
- **El proyecto está ordenado en carpetas**:
  - en Rust, `app.rs` (casi 4000 líneas) quedó partido: `app/views/` (vista clásica, de música y la ventana), `app/menus/` (ⓘ, ⚙, efectos, novedades, atajos, lista), `app/studio/` (portada, campos, vista previa, ecualizador, guardar), `app/converter/` (perfiles, entrada, opciones, salida) y piezas compartidas (`overlay.rs`: el panel que se pone encima; `widgets.rs`: botones, chips, campos y deslizadores). Lo demás, en `ui/` (tema, íconos, Alto, visualizador), `system/` (config, carpetas, sesión, playlists, caché, rendimiento…) y `media/` (formatos);
  - en C, `core/src/player/`, `audio/`, `media/` y `platform/`.
- **Sonido al cambiar de efecto**: tocar Bajos, Voces, Lo-fi, Noche… en el menú de efectos (o Rock, Pop, Jazz… en el ecualizador de Studio) suena un efecto corto. Se apaga junto con los demás sonidos de la interfaz (`click_sound = false` en `capyro.lua`).
- **Vista de música con más información**: el **visualizador** ya no ocupa un renglón propio; va **sobre la parte de abajo de la carátula**, con un degradé oscuro para que se vea bien. En ese lugar ahora entran dos líneas más debajo del título, el artista y el álbum: el **año y el género**, y los **datos técnicos** del archivo (por ejemplo "MP3 · 44,1 kHz · 320 kbps · Estéreo" o "FLAC · 96 kHz · 24 bits").
- **El modo Alto se nota "premium"**:
  - la carátula tiene un **aura** de dos colores (el acento y el color de la portada) que gira despacio y respira con los graves;
  - un **reflejo de vidrio** que sigue al mouse y un borde de luz;
  - la carátula se inclina apenas hacia el puntero (paralaje);
  - la barra de progreso tiene un **brillo que la recorre** mientras suena;
  - el chip "Alto" de ⚙ brilla en dorado con un destello, y su descripción arranca con "Premium".
- **Videos de hasta 8K** (7680×4320 y 8192 de lado): ya se abrían, pero en una PC que no llega a decodificarlos a tiempo (8K sin decodificar por GPU) **el sonido se cortaba** y todo iba en cámara lenta, porque audio y video se decodifican en el mismo hilo. Ahora:
  - el **audio va primero**: se llena su colchón antes de cada cuadro de video, así **nunca se corta** (medido con un 8K en una PC de 2 núcleos sin GPU: antes avanzaba a 0,37× con cortes; ahora a 1× y sin un solo corte);
  - si el video se atrasa, **se tiran cuadros** (sin convertirlos), el decodificador **saltea los cuadros B** que nadie necesita y el filtro de bloques; y si queda más de 1,5 s atrás, **salta al próximo cuadro clave**. La imagen se ve a saltos, pero sincronizada. Con decodificación por GPU, 8K va fluido;
  - achicar un cuadro enorme a la ventana usa otro filtro (**AREA**: promedia los píxeles, se ve igual) que cuesta la mitad: de 24 a 12 ms por cuadro con 8K;
  - **Capturar fotograma** guarda en tamaño completo hasta 8K (antes, hasta 3840 de lado);
  - un video 1080p sigue a 30 cuadros por segundo exactos: nada de esto se activa si la PC llega.
- **Ecualizador avanzado de 15 bandas** (de 25 Hz a 16 kHz): arrastrá cada banda para arriba o abajo (doble clic: 0 dB), con una curva que las une. Perfiles listos: **Plano**, **Graves**, **Voces**, **Brillante**, **Rock**, **Pop**, **Jazz**, **Clásica**, **Electrónica** y **Cine**. Se guarda con la sesión.
  - **Es uno u otro**: con el avanzado activado, el **ecualizador normal** (graves, medios y agudos) se apaga; en Efectos de audio sus barras quedan grises con un aviso y un botón "Usar el normal". Elegir un preset del menú de efectos también vuelve al normal.
- **Vista previa del tramo**: una barra con dos manijas para elegir el **inicio** y el **fin** (doble clic: todo el tema), o "Inicio aquí" / "Fin aquí" con la posición actual. El botón ▶ toca solo ese pedazo y se pausa al llegar al fin, o repite si activás "Repetir". El archivo no cambia.
- Con Capyro Studio o Capyro Converter abiertos, los atajos que abren otros menús o cambian de canción no hacen nada, y un archivo que arrastrás se agrega a la lista sin cambiar lo que suena (con el Converter abierto, es el que se convierte).
- **Mucho menos procesador en Wine** (la versión de Windows corriendo en Linux): sin hacer nada se comía casi un núcleo entero. Era un error de Wine: winit (lo que maneja la ventana) pide cada redibujo de una forma que el servidor de Wine cuenta pero nunca descuenta, así que el bucle de eventos quedaba girando en vacío miles de veces por segundo. Ahora, solo en Wine, Capyro pide esos redibujos de otra forma que Wine sí descuenta. Además, en Wine dibuja con **OpenGL primero** (va directo a la placa de video de Linux; Direct3D 12 necesita Vulkan, que las placas viejas como la de una ThinkPad X220 no tienen). Medido en Wine, en una PC de 2 núcleos que dibuja con el procesador:
  - sin nada abierto: de 86 % a **0 %**;
  - con música: de 122 % a **34 %**;
  - con un video 1080p: de 147 % a **96 %**.
- **Sin placa de video** (cuando la ventana se dibuja con el procesador: llvmpipe, "Basic Render Driver"...), lo que se mueve solo (los milisegundos, el visualizador, la barra) se redibuja a la mitad: 15 veces por segundo (10 en Bajo). Con música, de 60 % a **30 %** (19 % en Bajo). Y los cuadros de video se pintan con un filtro más barato, que se ve igual porque ya vienen del tamaño justo.
- Con un video, el contador de milisegundos ya no pide redibujos aparte (se actualiza con los cuadros del video).
- Al arrancar, Capyro dice en la terminal con qué dibuja ("capyro: dibujo con ..."): sirve para saber si está usando la placa de video.
- **Borrar caché** (ⓘ, entre las actualizaciones y "Desinstalar Capyro"): el primer toque calcula cuánto ocupa y el segundo la borra ("se liberaron 412 MB"). Se va todo lo que Capyro guarda para no repetirlo: el código y la compilación del actualizador (lo más pesado; la próxima actualización compila de cero), la canción del tutorial, los registros (`capyro.log`, `update.log`) y lo que dejan las pruebas y las actualizaciones en la carpeta temporal. Tu configuración, las playlists, la sesión y las capturas no se tocan. Mientras se actualiza Capyro, el botón espera.

## v0.3.2-alpha · Se adapta a lo que abrís

- **Interfaz adaptativa**: Capyro se da cuenta de si abriste un audio o un video y se acomoda.
- **Vista de música** (con un audio):
  - al medio, la **carátula grande** (un poco arriba) y abajo el **título**, el **artista** y el **álbum** (si los trae; el álbum es nuevo, el núcleo ahora lo lee);
  - más abajo, la barra de progreso, los **controles** (aleatorio, anterior, desde el principio, play, siguiente, repetir), el **volumen con una barra** para arrastrar (o la rueda) y los **efectos de audio**: el interruptor, los presets y "Efectos de audio ›" para el menú completo;
  - a un costado, la **lista** (la Cola y las playlists) siempre abierta, con "+ Agregar archivo(s)" abajo;
  - al otro, una **burbuja con todas las funciones**, en pestañas: **Ajustes**, **Info** (con los formatos ya abiertos, porque hay lugar) y **Novedades**. Con scroll si no entra;
  - arriba, el formato y los milisegundos en una fila, y el botón de pantalla completa;
  - se adapta a la ventana: las columnas se agrandan con ella y, si la ventana es chica (menos de unos 760 de ancho), queda la vista clásica. La ventana ahora arranca en 900×660 para que entre todo cómodo;
  - al cambiar de canción la carátula entra con un saltito y el título sube (en Medio y Alto); en Alto, la luz de la carátula ilumina la ventana y la carátula late un poquito con los graves.
- **Videos: ocultar la barra de abajo** con un clic. Al acercar el mouse abajo del video aparece un botoncito; con la barra escondida, el video ocupa todo y el botón para volverla vuelve a aparecer cuando movés el mouse (y se va solo). También con la tecla **B** (`toggle_bar`; si tu `capyro.lua` es de antes, la B se agrega sola si está libre).
- **Todo se desactiva en ⚙** (o en `capyro.lua`): "Vista de música" (`music_view`) y "Ocultar barra (videos)" (`hide_bar_button`). Y respeta los **modos de rendimiento**: en Bajo los cambios de vista y de canción son instantáneos y sin visualizador; en Medio, con sus transiciones; en Alto, además, con la luz y las partículas.
- **Desinstalar Capyro** (ⓘ, abajo de las actualizaciones). Un clic lo arma y el segundo confirma. En Windows abre el desinstalador de "Aplicaciones instaladas" (`uninstall.exe`, que antes solo se alcanzaba desde el Panel de control); en Linux corre `capyro-uninstall`, que la instalación con CMake deja junto al programa (si está en /usr o /usr/local, copia el comando `sudo sh …` para pegarlo en una terminal). Tu configuración (`~/.config/capyro` o Documentos\Capyro en Windows) no se borra.
- **Windows: "Abrir con…"** ahora muestra Capyro: el instalador también lo anota en la lista del Explorador por cada formato y en "App Paths", que es lo que Windows usa para encontrar capyro.exe. Si no aparece de una, cerrá sesión o reiniciá el Explorador. Y el instalador (1.2.1) no se adueña de ningún formato: lo elegís vos en "Abrir con…".
- Los textos de ⚙ ahora son más cortos (para que entren en la burbuja), y en los paneles angostos los chips achican la letra.
- **Optimización general** (medido en la misma PC que dibuja por software, ventana de 768×642, modo Medio):
  - un **video 1080p**: de 81 % a 71 % de procesador y unos **15 MB menos de memoria**;
  - **música** en la vista clásica: de 38 % a 30 %; en la vista de música (que muestra mucho más), 39 %;
  - **música sin milisegundos ni visualizador**: de 37 % a 18 %: ahora se redibuja solo cuando cambia algo que se ve (el reloj, una vez por segundo, o la barra cuando avanza un píxel), y no 30 veces por segundo;
  - el fondo de la ventana ya no se pinta debajo del video ni de los paneles (antes esa parte se pintaba dos veces), y las sombras son un anillo y no un rectángulo entero debajo;
  - los cuadros de video se pasan a la pantalla con una sola copia de memoria (antes, píxel por píxel);
  - la lista ya no se le pide entera al núcleo en cada cuadro (se guarda y se vuelve a pedir solo cuando cambia), solo se arman las filas que se ven (antes, todas: con miles de canciones se notaba) y no se revisa en cada cuadro si los archivos de las playlists siguen en el disco;
  - el título, el artista y el archivo actual se piden una vez, no varias veces por cuadro;
  - el hilo que decodifica duerme más cuando no tiene nada que hacer (antes se despertaba 250 veces por segundo, también en pausa);
  - en Linux, la memoria de los cuadros de video se devuelve al sistema al soltarla; los sonidos de la interfaz se guardan sin el silencio del final y sin espacio de sobra; las carátulas se guardan a 720 px como mucho (antes 1024).

## v0.3.1-alpha · Más liviano

- **Capyro usa mucho menos procesador**, sin perder animaciones (medido en una PC que dibuja por software, como pasa con placas de video viejas):
  - con música: de 156 % a 38 % de CPU;
  - con un video 1080p: de 159 % a 78 %;
  - en el tutorial: de 165 % a 65 %.
- Dibuja solo lo necesario:
  - antes se redibujaba sin parar mientras sonaba algo (60 veces por segundo, o más si la pantalla no tiene vsync);
  - ahora el video se dibuja al ritmo del video (24 o 30 cuadros);
  - lo que se mueve todo el tiempo (visualizador, milisegundos, chispitas del tutorial) va a 30 cuadros;
  - las animaciones (menús, botones, transiciones, el texto que se escribe) siguen a 60.
- Video más liviano:
  - cada cuadro se convierte al tamaño en que se ve en la ventana, no a 1080p: mucho menos para convertir y para subir a la placa de video;
  - con el núcleo bloqueado solo se copian los bytes (el audio no espera mientras se arma la imagen);
  - las capturas (<kbd>Shift</kbd>+<kbd>S</kbd>) siguen saliendo en el tamaño original del video.
- **"Abrir con…"**: Capyro aparece (y como recomendada) para mp4, mov, avi, mp3, wav, ogg y aac.
  - Linux: el lanzador ahora lista todos los nombres de esos formatos y tiene la ruta completa del programa; al instalar se actualizan los cachés (también el de KDE).
  - Windows: CapyroInstaller los registra (sin adueñarse de ninguno) y Capyro aparece en Aplicaciones predeterminadas. Al desinstalar se saca todo.
- `CAPYRO_DEBUG_FPS=1 capyro` muestra cuántos cuadros por segundo dibuja y cuánto cuesta cada uno.
- Extra: abajo a la izquierda del video, dos burbujas nuevas:
  - la **resolución** con su nombre, de 144p a 16K (480p SD, 720p HD, 1080p Full HD, 1440p 2K, 4K, 5K, 8K, 16K). Es solo el indicador: un video de 2786×1567 dice "1440p · 2K" y se ve tal cual es. Los videos verticales y los de cine (más anchos) caen en su escalón. Con el mouse encima, la resolución exacta;
  - los **fps** del video (30, 29.97, 60...). Si la PC no llega a mostrarlos todos, se pone amarillo y dice cuántos se están viendo.
  - Las dos se ocultan juntas en ⚙ → Mostrar resolución y fps (`show_video_info`).
- Extra: **modos de rendimiento** en ⚙ → Rendimiento (`performance` en capyro.lua):
  - **Bajo**: sin nada de más, pero sin sacar funciones. Se apagan todas las animaciones (menús, botones, avisos y el tutorial aparecen y se van al toque, el texto sale entero), la pantalla de carga, el visualizador, las chispitas, halos, papelitos y brillos del tutorial. Lo que se mueve solo se redibuja 20 veces por segundo en vez de 30. Con música y en el tutorial gasta alrededor de un tercio menos que en Medio;
  - **Medio** (el de siempre, viene por defecto): Capyro como hasta ahora;
  - **Alto**: suma la **iluminación cinematográfica**, como la de YouTube pero en toda la ventana: los colores del video (o de la carátula de la canción) se derraman por el header, los controles y las franjas negras del video, y cambian despacio siguiendo la imagen. Para no comerse el procesador, de cada cuadro se promedian unos pocos píxeles (una grilla de 16×9) y la luz es una malla con colores por vértice que pinta la placa de video; lo que queda tapado por el video ni se dibuja, y con la imagen quieta deja de recalcular.
  - Se cambia al instante, sin reiniciar.
- Extra: el modo **Alto** trae más cosas (en Bajo y Medio no cambia nada):
  - **fondo con degradé dinámico**: tres manchas de color se mueven despacio por detrás de toda la interfaz: el color de acento, el de lo que estás viendo (sacado del video o de la carátula, y cambia cuando cambia la imagen) y uno que los acompaña;
  - **resplandor interactivo**: una luz suave sigue al mouse por la ventana; los botones brillan con el color de acento al pasarles por encima y largan una onda al tocarlos; el botón de play tiene un halo que respira mientras suena algo; el botón "Agregar archivo(s)" y la punta de la barra de progreso también brillan;
  - **transiciones más elaboradas**: los menús (⚙, ⓘ, efectos, novedades, lista y atajos) se abren con un rebotecito y su contenido se descubre de arriba abajo con una línea de luz; al cambiar de archivo un destello cruza el área de video y, en las canciones, el título y el artista entran subiendo;
  - **partículas ambientales**: polvito de luz que sube flotando y titila, alrededor de la carátula y por el fondo (nunca encima del video); con música late con los graves;
  - **sin comerse el procesador**: no guarda nada por partícula (sale del tiempo), el degradé y la luz del video van en una sola malla, lo tapado por el video no se dibuja, el fondo se mueve a 15 cuadros por segundo (va despacio) y con la ventana desenfocada se queda quieto y no gasta nada. Con placa de video, cada cuadro le cuesta al procesador unos 3 ms.
- Extra: **6 formatos nuevos**: video **MKV** y **WebM**; audio **FLAC**, **Opus**, **M4A** y **AIFF** (también `.aif` y `.aifc`). Ya son 13.
  - En ⓘ ya no se ven todos de una: hay un botón **"Formatos soportados (abrir)"** que despliega la lista (en renglones, Video y Audio) con el resumen de cada uno al tocarlo, como antes.
  - Están en el selector de archivos, en arrastrar y soltar, en el tutorial y en "Abrir con…" (Linux: el lanzador; Windows: CapyroInstaller 1.2.0).
- Extra: **revisión de los 13 formatos**, con 42 archivos de prueba "de la vida real" (con y sin carátula, con tags, 24 bits, 96 kHz, 5.1, mono, 8 bits, VBR, ALAC, Theora, ProRes, HEVC 10 bits, VP8, VP9, AV1, videos de celular, anamórficos, sin duración, que no empiezan en 0...), revisados también con valgrind. Bugs arreglados:
  - **cuelgue al abrir algunas carátulas o videos verticales**: al convertir la imagen, FFmpeg escribía unos bytes más allá del final cuando el ancho no era múltiplo de 8 (una carátula de 300 px, un video vertical achicado) y pisaba memoria. Según lo que hubiera al lado, Capyro se cerraba al rato ("corrupted size vs. prev_size");
  - **videos de celular acostados**: los mp4/mov grabados con el celular parado se veían de costado. Ahora se giran como corresponde (90°, 180° o 270°), la burbuja de resolución los muestra parados (1080×1920) y las capturas salen derechas;
  - **duración mal o desconocida**: los mp3 VBR sin encabezado y los .aac sueltos mostraban una duración estimada (2,88 s en vez de 3), y los .mkv/.webm sin duración (como los grabados en el navegador) no se podían adelantar ni mostraban la barra. Ahora la duración se cuenta al abrir (leyendo el archivo sin decodificar, con tope de tiempo); si no se puede, se aprende al llegar al final;
  - **títulos falsos en MKV/WebM**: el nombre de la pista de audio ("Stereo", "Principal", "Track1") aparecía como título de la canción. Ahora los datos de las pistas solo se usan en Ogg/Opus, que es donde de verdad van; y si no hay artista se busca el "artista del álbum" o el autor;
  - **se cortaba la lista**: si el archivo siguiente estaba roto o lo habías borrado, la reproducción se paraba. Ahora se saltea y sigue con el próximo (lo mismo al abrir varios por línea de comandos);
  - un valor sin inicializar al armar el filtro de audio (lo marcó valgrind; en algunos sistemas podía colgarse);
  - "Abrir con…" en Linux no ofrecía Capyro para los .ogg con video (Theora);
  - al arrastrar archivos encima, la lista de formatos ya no entraba en una ventana chica: ahora va en dos renglones (video y audio). Y **se pueden soltar carpetas**: se agregan sus archivos compatibles, con las subcarpetas, en orden.

## v0.3.0-alpha · El salto a Windows

- **Capyro corre en Windows 10 y 11**, además de Linux.
  - El núcleo en C usa hilos, reloj y archivos de Win32 (nombres con tildes y ñ incluidos), y el audio sale por WASAPI.
  - La GPU en Windows: D3D11VA, NVDEC o DXVA2, con la misma vuelta a la CPU si no se puede.
  - Fuente Segoe UI, ícono y versión adentro del `.exe`, y sin ventana de consola (salvo que le pases opciones).
  - Un solo Capyro para cualquier PC: dibuja con **Direct3D 12** u **OpenGL**, el que ande. Prueba primero el que funcionó la vez anterior; si uno da error se pasa al otro, y si la ventana no llega a dibujarse en 10 s, se vuelve a abrir solo con el otro. Se fuerza con `renderer = "direct3d"` u `"opengl"` en capyro.lua (o `CAPYRO_RENDERER`). Si no puede abrir la ventana lo dice en un cartel y lo anota en `%LOCALAPPDATA%\Capyro\capyro.log`.
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

# Genera la media de prueba. Se corre desde ctest (no en el build), así que si
# el ffmpeg instalado no tiene algún encoder solo se saltean esas pruebas.
#   cmake -DFFMPEG=/usr/bin/ffmpeg -DOUT=<carpeta> -P gen_media.cmake

file(MAKE_DIRECTORY ${OUT})
set(FF ${FFMPEG} -hide_banner -loglevel error -y)

# FFmpeg no trae encoder de mp3 propio: depende de LAME o shine.
# (Para *reproducir* mp3 no hace falta ninguno; el decoder es interno.)
execute_process(COMMAND ${FFMPEG} -hide_banner -encoders OUTPUT_VARIABLE ENCODERS ERROR_QUIET)
set(MP3_ENC "")
foreach(enc libmp3lame libshine)
    if(ENCODERS MATCHES " ${enc} ")
        set(MP3_ENC ${enc})
        break()
    endif()
endforeach()

function(run name)
    execute_process(COMMAND ${FF} ${ARGN} RESULT_VARIABLE rc ERROR_VARIABLE err)
    if(NOT rc EQUAL 0)
        file(REMOVE ${OUT}/${name})
        message(WARNING "No pude generar ${name}: ${err}")
    endif()
endfunction()

if(MP3_ENC)
    run(tono.mp3 -f lavfi -i sine=frequency=440:duration=3 -c:a ${MP3_ENC}
        -metadata "title=Tono de prueba" -metadata artist=Capyro ${OUT}/tono.mp3)
    run(caratula.png -f lavfi -i color=c=orange:s=64x64 -frames:v 1 ${OUT}/caratula.png)
    run(caratula.mp3 -f lavfi -i sine=frequency=330:duration=2 -i ${OUT}/caratula.png
        -map 0:a -map 1:v -c:a ${MP3_ENC} -c:v mjpeg -disposition:v attached_pic
        -id3v2_version 3 ${OUT}/caratula.mp3)
else()
    message(WARNING "Tu ffmpeg no tiene encoder de mp3 (libmp3lame/libshine): salteo las pruebas de mp3")
    file(REMOVE ${OUT}/tono.mp3 ${OUT}/caratula.mp3)
endif()

# mpeg4, aac y pcm son encoders internos de FFmpeg: siempre están.
run(clip.mp4 -f lavfi -i testsrc=size=320x240:rate=25:duration=2
    -f lavfi -i sine=frequency=550:duration=2
    -c:v mpeg4 -q:v 5 -c:a aac -shortest ${OUT}/clip.mp4)
run(clip.mov -f lavfi -i testsrc=size=320x240:rate=25:duration=2
    -f lavfi -i sine=frequency=660:duration=2
    -c:v mpeg4 -q:v 5 -c:a aac -shortest ${OUT}/clip.mov)
run(tono.wav -f lavfi -i sine=frequency=440:duration=2 -c:a pcm_s16le ${OUT}/tono.wav)
run(tono.aac -f lavfi -i sine=frequency=440:duration=2 -c:a aac ${OUT}/tono.aac)
run(clip.avi -f lavfi -i testsrc=size=320x240:rate=25:duration=2
    -f lavfi -i sine=frequency=770:duration=2
    -c:v mpeg4 -q:v 5 -c:a pcm_s16le -shortest ${OUT}/clip.avi)

# ogg: Vorbis u Opus si están (lo normal); si no, FLAC dentro de ogg, que es interno.
set(OGG_ENC "")
foreach(enc libvorbis libopus)
    if(ENCODERS MATCHES " ${enc} ")
        set(OGG_ENC ${enc})
        break()
    endif()
endforeach()
if(NOT OGG_ENC)
    set(OGG_ENC flac)
endif()
run(tono.ogg -f lavfi -i sine=frequency=440:duration=2 -c:a ${OGG_ENC} ${OUT}/tono.ogg)

# --- formatos de la v0.3.1 (extra): mkv, webm, flac, opus, m4a y aiff ---
# Opus: libopus si está; si no, el encoder interno (experimental).
if(ENCODERS MATCHES " libopus ")
    set(OPUS_ENC -c:a libopus)
else()
    set(OPUS_ENC -c:a opus -strict -2 -ar 48000)
endif()
# mkv acepta lo que sea: mpeg4 + flac (los dos internos)
run(clip.mkv -f lavfi -i testsrc=size=320x240:rate=25:duration=2
    -f lavfi -i sine=frequency=500:duration=2
    -c:v mpeg4 -q:v 5 -c:a flac -shortest -metadata:s:a:0 title=Stereo ${OUT}/clip.mkv)
# webm exige VP8/VP9/AV1: depende de libvpx o de algún AV1
set(WEBM_V "")
foreach(enc libvpx-vp9 libvpx libaom-av1 libsvtav1)
    if(ENCODERS MATCHES " ${enc} ")
        set(WEBM_V ${enc})
        break()
    endif()
endforeach()
if(WEBM_V)
    run(clip.webm -f lavfi -i testsrc=size=320x240:rate=25:duration=2
        -f lavfi -i sine=frequency=600:duration=2
        -c:v ${WEBM_V} -b:v 200k -deadline realtime -cpu-used 8 ${OPUS_ENC} -shortest ${OUT}/clip.webm)
else()
    message(WARNING "Tu ffmpeg no tiene encoder de VP8/VP9/AV1: salteo el .webm con video")
    file(REMOVE ${OUT}/clip.webm)
endif()
# webm con solo audio (como los que baja yt-dlp): tiene que andar igual
run(solo-audio.webm -f lavfi -i sine=frequency=520:duration=2 ${OPUS_ENC}
    -metadata "title=Solo audio" ${OUT}/solo-audio.webm)
run(tono.flac -f lavfi -i sine=frequency=440:duration=2 -c:a flac
    -metadata "title=Tono FLAC" -metadata artist=Capyro ${OUT}/tono.flac)
run(tono.opus -f lavfi -i sine=frequency=440:duration=2 ${OPUS_ENC}
    -metadata "title=Tono Opus" -metadata artist=Capyro ${OUT}/tono.opus)
run(tono.m4a -f lavfi -i sine=frequency=440:duration=2 -c:a aac
    -metadata "title=Tono M4A" -metadata artist=Capyro ${OUT}/tono.m4a)
run(tono.aiff -f lavfi -i sine=frequency=440:duration=2 -c:a pcm_s16be ${OUT}/tono.aiff)

# --- casos que tenían bugs (revisión de formatos) ---
# video de celular: grabado "acostado" con la marca de girarlo 90°
run(rotado.mp4 -display_rotation 90 -i ${OUT}/clip.mp4 -c copy ${OUT}/rotado.mp4)
# carátula de 300 px (un ancho que no es múltiplo de 8: pisaba memoria)
run(tapa300.jpg -f lavfi -i color=c=0x3366ff:s=300x300 -frames:v 1 ${OUT}/tapa300.jpg)
run(tapa.flac -f lavfi -i sine=frequency=440:duration=2 -i ${OUT}/tapa300.jpg -map 0:a -map 1:v
    -c:a flac -c:v copy -disposition:v attached_pic ${OUT}/tapa.flac)
# video vertical de ancho impar al achicarlo
run(vertical.mp4 -f lavfi -i testsrc=size=180x320:rate=25:duration=2 -c:v mpeg4 -q:v 5 ${OUT}/vertical.mp4)

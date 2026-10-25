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

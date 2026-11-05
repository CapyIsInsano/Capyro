#!/bin/bash
# ---------------------------------------------------------------------------
# El FFmpeg que va adentro de Capyro.app (macOS). Lo usa el CI
# (.github/workflows/macos.yml) y sirve igual en tu Mac:
#
#   packaging/macos/build-ffmpeg.sh ffmpeg-mac
#   cmake -S . -B build -DCAPYRO_FFMPEG_DIR=$PWD/ffmpeg-mac ...
#
# - Para macOS 11 (Big Sur) o más nuevo, en la arquitectura de esta Mac
#   (arm64 en Apple Silicon, x86_64 en Intel).
# - Librerías compartidas de FFmpeg (libavcodec.XX.dylib...) que se buscan en
#   "@rpath": CMake las copia a Capyro.app/Contents/Frameworks.
# - Lo de afuera (x264, LAME, Opus, libvpx, dav1d) va adentro de esas
#   librerías (estático): Capyro.app no depende de Homebrew.
# - VideoToolbox y AudioToolbox: decodificar con el chip de video de la Mac
#   y codificar H.264/H.265 (Capyro Converter).
# - GPL (por x264), como Capyro.
#
# Hace falta: Xcode (o sus Command Line Tools), y de Homebrew nasm, meson,
# ninja y pkgconf.
# ---------------------------------------------------------------------------
set -euo pipefail

PREFIX=${1:?uso: build-ffmpeg.sh <carpeta donde instalarlo>}
mkdir -p "$PREFIX"
PREFIX=$(cd "$PREFIX" && pwd)

FFMPEG_VERSION=8.0.1
LAME_VERSION=3.100
OPUS_VERSION=1.5.2
VPX_VERSION=v1.15.0
DAV1D_VERSION=1.5.1
X264_BRANCH=stable

MIN=${MACOSX_DEPLOYMENT_TARGET:-11.0}
export MACOSX_DEPLOYMENT_TARGET=$MIN
ARCH=$(uname -m) # arm64 o x86_64
JOBS=$(sysctl -n hw.ncpu)
SRC=${CAPYRO_FFMPEG_SRC:-$PREFIX-src}
mkdir -p "$SRC"

FLAGS="-mmacosx-version-min=$MIN -O2"
export CFLAGS="$FLAGS" CXXFLAGS="$FLAGS" LDFLAGS="-mmacosx-version-min=$MIN"
# solo lo nuestro: nada de los .pc de Homebrew
export PKG_CONFIG_LIBDIR="$PREFIX/lib/pkgconfig"
export PKG_CONFIG_PATH="$PREFIX/lib/pkgconfig"
export PATH="$PREFIX/bin:$PATH"

echo "== FFmpeg $FFMPEG_VERSION para macOS $MIN ($ARCH) en $PREFIX"

# Los servidores de los proyectos a veces le cortan la conexión al CI
# ("curl: (35) Recv failure: Connection reset by peer", que el --retry de curl
# no reintenta). Cada dirección se prueba 3 veces y, si la oficial no anda,
# se usa el espejo (casi siempre el oficial en GitHub).
fetch() { # archivo url [espejo...]
    local out=$1 url n
    shift
    [ -s "$out" ] && return 0
    for url in "$@"; do
        for n in 1 2 3; do
            # .part: si se corta a la mitad no queda un archivo roto
            if curl -fL --connect-timeout 30 --speed-time 60 --speed-limit 1024 \
                -o "$out.part" "$url"; then
                mv "$out.part" "$out"
                return 0
            fi
            echo "!! no pude bajar $url (intento $n de 3)" >&2
            [ "$n" = 3 ] || sleep $((n * 10))
        done
    done
    rm -f "$out.part"
    echo "!! no pude bajar $out de ningún lado" >&2
    return 1
}

clone() { # rama carpeta url [espejo...]
    local branch=$1 dir=$2 url n
    shift 2
    [ -d "$dir/.git" ] && return 0
    for url in "$@"; do
        for n in 1 2 3; do
            rm -rf "$dir"
            git clone --depth 1 --branch "$branch" "$url" "$dir" && return 0
            echo "!! no pude clonar $url (intento $n de 3)" >&2
            [ "$n" = 3 ] || sleep $((n * 10))
        done
    done
    echo "!! no pude bajar $dir de ningún lado" >&2
    return 1
}

cd "$SRC"

# --- LAME (MP3) -------------------------------------------------------------
if [ ! -f "$PREFIX/lib/libmp3lame.a" ]; then
    # espejo: el mismo .tar.gz que guarda Debian
    fetch lame.tar.gz \
        "https://downloads.sourceforge.net/project/lame/lame/$LAME_VERSION/lame-$LAME_VERSION.tar.gz" \
        "https://deb.debian.org/debian/pool/main/l/lame/lame_$LAME_VERSION.orig.tar.gz"
    rm -rf "lame-$LAME_VERSION" && tar xf lame.tar.gz
    (cd "lame-$LAME_VERSION" &&
        ./configure --prefix="$PREFIX" --disable-shared --enable-static --disable-frontend \
            --disable-dependency-tracking --disable-debug &&
        make -j"$JOBS" && make install)
fi

# --- Opus -------------------------------------------------------------------
if [ ! -f "$PREFIX/lib/libopus.a" ]; then
    fetch opus.tar.gz \
        "https://downloads.xiph.org/releases/opus/opus-$OPUS_VERSION.tar.gz" \
        "https://github.com/xiph/opus/releases/download/v$OPUS_VERSION/opus-$OPUS_VERSION.tar.gz"
    rm -rf "opus-$OPUS_VERSION" && tar xf opus.tar.gz
    (cd "opus-$OPUS_VERSION" &&
        ./configure --prefix="$PREFIX" --disable-shared --enable-static --disable-doc \
            --disable-extra-programs --disable-dependency-tracking &&
        make -j"$JOBS" && make install)
fi

# --- libvpx (VP9 para WebM) ---------------------------------------------------
if [ ! -f "$PREFIX/lib/libvpx.a" ]; then
    clone "$VPX_VERSION" libvpx \
        https://chromium.googlesource.com/webm/libvpx https://github.com/webmproject/libvpx.git
    # darwin20 = macOS 11: el mismo mínimo que Capyro
    VPX_TARGET=$([ "$ARCH" = arm64 ] && echo arm64-darwin20-gcc || echo x86_64-darwin20-gcc)
    (cd libvpx && make distclean >/dev/null 2>&1 || true)
    (cd libvpx &&
        ./configure --prefix="$PREFIX" --target="$VPX_TARGET" --disable-shared --enable-static \
            --enable-pic --enable-vp9-highbitdepth --disable-examples --disable-tools \
            --disable-docs --disable-unit-tests &&
        make -j"$JOBS" && make install)
fi

# --- dav1d (AV1) --------------------------------------------------------------
if [ ! -f "$PREFIX/lib/libdav1d.a" ]; then
    clone "$DAV1D_VERSION" dav1d \
        https://code.videolan.org/videolan/dav1d.git https://github.com/videolan/dav1d.git
    (cd dav1d && rm -rf build &&
        meson setup build --prefix="$PREFIX" --libdir=lib --buildtype=release \
            --default-library=static -Denable_tools=false -Denable_tests=false \
            -Dc_args="$FLAGS" -Dc_link_args="$LDFLAGS" &&
        ninja -C build install)
fi

# --- x264 (H.264) -------------------------------------------------------------
if [ ! -f "$PREFIX/lib/libx264.a" ]; then
    clone "$X264_BRANCH" x264 \
        https://code.videolan.org/videolan/x264.git https://github.com/mirror/x264.git
    (cd x264 &&
        ./configure --prefix="$PREFIX" --enable-static --disable-cli --enable-pic \
            --extra-cflags="$FLAGS" --extra-ldflags="$LDFLAGS" &&
        make -j"$JOBS" && make install)
fi

# --- FFmpeg -------------------------------------------------------------------
rm -rf "ffmpeg-$FFMPEG_VERSION"
if fetch ffmpeg.tar.xz "https://ffmpeg.org/releases/ffmpeg-$FFMPEG_VERSION.tar.xz"; then
    tar xf ffmpeg.tar.xz
else
    # el espejo oficial de FFmpeg en GitHub: la etiqueta n8.0.1 es la misma versión
    echo "!! ffmpeg.org no responde: uso el espejo de GitHub" >&2
    clone "n$FFMPEG_VERSION" "ffmpeg-$FFMPEG_VERSION" https://github.com/FFmpeg/FFmpeg.git
fi
cd "ffmpeg-$FFMPEG_VERSION"
# --disable-autodetect: nada que se cuele de Homebrew (SDL, X11...); lo que
# se usa va pedido a mano.
./configure \
    --prefix="$PREFIX" \
    --install-name-dir='@rpath' \
    --enable-shared --disable-static \
    --disable-programs --disable-doc --disable-debug \
    --disable-autodetect --disable-avdevice \
    --enable-gpl \
    --enable-libx264 --enable-libmp3lame --enable-libopus --enable-libvpx --enable-libdav1d \
    --enable-videotoolbox --enable-audiotoolbox \
    --enable-zlib --enable-bzlib --enable-iconv \
    --pkg-config-flags=--static \
    --extra-cflags="$FLAGS -I$PREFIX/include" \
    --extra-ldflags="$LDFLAGS -L$PREFIX/lib" \
    --extra-libs="-liconv"
# -liconv: en la Mac iconv no está en la libc, está en /usr/lib/libiconv
# (viene con macOS). El configure de FFmpeg lo da por bueno sin él y después
# libavcodec no linkea ("_iconv_open" undefined).
make -j"$JOBS"
make install

# --- revisión: que no dependa de nada que no venga con macOS -----------------
bad=0
for lib in "$PREFIX"/lib/lib{avutil,avcodec,avformat,avfilter,swscale,swresample}.dylib; do
    echo "-- $(basename "$lib")"
    otool -L "$lib" | tail -n +2
    if otool -L "$lib" | tail -n +2 | grep -Ev '^\s+(@rpath/|/usr/lib/|/System/Library/)' >/dev/null; then
        echo "   ^ depende de algo de afuera" >&2
        bad=1
    fi
done
[ "$bad" = 0 ] || { echo "El FFmpeg depende de librerías que no vienen con macOS" >&2; exit 1; }
echo "== listo: $PREFIX"

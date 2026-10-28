#!/bin/sh
set -eu
autorun="$1"
src="$2"
build="$3"
shift 3
horizon_wine="$autorun/horizon-wine"
revision=395b132f346b1a45def246d10c52245edba1ef02
version=FEX-2609
export PATH="${WINE_NX_LLVM_MINGW:-/opt/llvm-mingw}/bin:$PATH"

if [ ! -d "$src/.git" ]; then
    mkdir -p "$src"
    git -C "$src" init -q
    git -C "$src" remote add origin https://github.com/FEX-Emu/FEX.git
fi
if [ "$(git -C "$src" rev-parse -q --verify HEAD || true)" != "$revision" ]; then
    git -C "$src" fetch --depth=1 origin "$revision"
    git -C "$src" checkout -q --force --detach FETCH_HEAD
    rm -f "$src/.git/carafe-applied"
fi
git -C "$src" submodule update --init --depth=1 \
    External/fmt External/xxhash External/range-v3 External/unordered_dense \
    External/rpmalloc Source/Common/cpp-optparse

wanted="$(cat "$horizon_wine/fex/horizon.patch" "$@" | sha256sum | cut -d' ' -f1)"
if [ "$(cat "$src/.git/carafe-applied" 2>/dev/null || true)" != "$wanted" ]; then
    git -C "$src" checkout -q --force HEAD -- .
    git -C "$src" clean -fdq
    git -C "$src" apply "$horizon_wine/fex/horizon.patch"
    for patch in "$@"; do
        git -C "$src" apply "$patch"
        echo "FEX: applied $(basename "$patch")"
    done
    echo "$wanted" > "$src/.git/carafe-applied"
fi

cmake -S "$src" -B "$build" -G Ninja \
    -DCMAKE_TOOLCHAIN_FILE="$src/Data/CMake/toolchain_mingw.cmake" \
    -DMINGW_TRIPLE=aarch64-w64-mingw32 -DCMAKE_BUILD_TYPE=Release \
    -DOVERRIDE_VERSION="$version" -DOVERRIDE_HASH="$revision" \
    -DENABLE_LTO=OFF -DBUILD_TESTING=OFF -DBUILD_FEXCONFIG=OFF \
    -DENABLE_JEMALLOC_GLIBC_ALLOC=OFF -DENABLE_OFFLINE_TELEMETRY=OFF -DTUNE_CPU=none \
    -DFEX_HORIZON=ON -DFEX_HORIZON_ABI_DIR="$horizon_wine/fex" >/dev/null
cmake --build "$build" --target wow64fex -j "${JOBS:-2}"

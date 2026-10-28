#!/bin/sh
set -eu
autorun="$1"
src="$2"
build="$3"
shift 3
revision=b1a1c99ab52b687cf950d62c88bc2fa316b41663
toolchain="${WINE_NX_LLVM_MINGW:-/opt/llvm-mingw}/bin"

if [ ! -d "$src/.git" ]; then
    mkdir -p "$src"
    git -C "$src" init -q
    git -C "$src" config core.autocrlf false
    git -C "$src" remote add origin https://github.com/doitsujin/dxvk.git
fi
if [ "$(git -C "$src" rev-parse -q --verify HEAD || true)" != "$revision" ]; then
    git -C "$src" fetch --depth=1 origin "$revision"
    git -C "$src" checkout -q --force --detach FETCH_HEAD
    rm -f "$src/.git/carafe-applied"
fi
git -C "$src" submodule update --init --recursive --depth=1

wanted="$(cat "$@" | sha256sum | cut -d' ' -f1)"
if [ "$(cat "$src/.git/carafe-applied" 2>/dev/null || true)" != "$wanted" ]; then
    git -C "$src" checkout -q --force HEAD -- src
    git -C "$src" clean -fdq -- src
    for patch in "$@"; do
        git -C "$src" apply "$patch"
        echo "DXVK: applied $(basename "$patch")"
    done
    echo "$wanted" > "$src/.git/carafe-applied"
fi

configure()
{
    dir="$build/$1"
    mkdir -p "$dir"
    {
        echo '[binaries]'
        for pair in c:clang cpp:clang++ ar:ar strip:strip windres:windres; do
            echo "${pair%%:*} = '$toolchain/$2-w64-mingw32-${pair#*:}'"
        done
        printf "\n[properties]\nneeds_exe_wrapper = true\n\n[host_machine]\n"
        printf "system = 'windows'\ncpu_family = '%s'\ncpu = '%s'\nendian = 'little'\n" "$3" "$3"
    } > "$dir/cross.txt"
    shift 3
    if [ ! -f "$dir/build.ninja" ]; then
        meson setup "$dir" "$src" --cross-file "$dir/cross.txt" --buildtype=release --strip \
            --wrap-mode=nodownload "$@" >/dev/null
    fi
}

configure i386 i686 x86 -Denable_d3d9=true -Denable_d3d8=false -Denable_d3d10=false -Denable_d3d11=false \
    -Denable_dxgi=false
configure amd64 x86_64 x86_64 -Denable_dxgi=true -Denable_d3d8=true -Denable_d3d9=true -Denable_d3d10=true \
    -Denable_d3d11=true
ninja -C "$build/i386" -j "${JOBS:-2}" src/d3d9/d3d9.dll
ninja -C "$build/amd64" -j "${JOBS:-2}"

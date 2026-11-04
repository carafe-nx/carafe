#!/bin/sh
set -eu

autorun=$1
pe=$2
out=$3
shift 3

dlls="dlls/wow64/aarch64-windows/wow64.dll dlls/ntdll/aarch64-windows/ntdll.dll dlls/ntdll/i386-windows/ntdll.dll"
src=$out/src
obj=$out/obj

commands() {
    make -s -C "$pe" -n "$@" | sed -e ':a' -e '/\\$/{N;s/\\\n//;ba' -e '}'
}

strip_tool() {
    case $1 in
    i386-windows) echo i686-w64-mingw32-strip ;;
    *) echo aarch64-w64-mingw32-strip ;;
    esac
}

rm -rf "$out"
mkdir -p "$src" "$obj"
make -C "$pe" -j"${JOBS:-2}" $dlls

for patch in "$@"; do
    file=$(sed -n 's#^+++ b/\([^\t ]*\).*#\1#p' "$patch")
    if [ ! -f "$src/$file" ]; then
        mkdir -p "$src/$(dirname "$file")"
        cp "$autorun/$file" "$src/$file"
    fi
    patch --batch --forward -p1 -d "$src" -i "$patch"
done

for dll in $dlls; do
    module=${dll%/*/*}
    arch=$(basename "$(dirname "$dll")")
    name=$(basename "$dll")
    rm -f "$pe/$dll"
    link=" $(commands "$dll" | grep -e "winegcc") "
    objects=$(printf '%s' "$link" | tr ' ' '\n' | grep "^$module/[a-z0-9_]*-windows/[a-z0-9_]*\.o$" || true)
    for source in $(find "$src/$module" -maxdepth 1 -name '*.c' 2>/dev/null); do
        file=$module/$(basename "$source")
        for object in $(printf '%s\n' $objects | grep "/$(basename "$source" .c)\.o$" || true); do
            command=$(commands -W "$autorun/$file" "$object" | grep -e "-c -o $object ")
            command=$(printf '%s' "$command" | sed "s#-c -o $object $autorun/$file#-c -o $obj/$object $source#")
            mkdir -p "$obj/$(dirname "$object")"
            (cd "$pe" && eval "$command")
            link=$(printf '%s' "$link" | sed "s# $object # $obj/$object #g")
        done
    done
    mkdir -p "$out/$arch"
    link=$(printf '%s' "$link" | sed "s#-o $dll #-o $out/$arch/$name #")
    (cd "$pe" && eval "$link")
    $(strip_tool "$arch") --strip-debug "$out/$arch/$name"
    make -C "$pe" "$dll"
    test -s "$out/$arch/$name"
done

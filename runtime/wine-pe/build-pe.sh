#!/bin/sh
set -eu

autorun=$1
pe=$2
out=$3
shift 3

module=dlls/wow64
arch=aarch64-windows
dll=$module/$arch/wow64.dll
src=$out/src
obj=$out/$arch

commands() {
    make -s -C "$pe" -n "$@" | sed -e ':a' -e '/\\$/{N;s/\\\n//;ba' -e '}'
}

rm -rf "$src" "$obj"
mkdir -p "$src" "$obj"
make -C "$pe" -j"${JOBS:-2}" "$dll"

for patch in "$@"; do
    file=$(sed -n 's#^+++ b/\([^\t ]*\).*#\1#p' "$patch")
    if [ ! -f "$src/$file" ]; then
        mkdir -p "$src/$(dirname "$file")"
        cp "$autorun/$file" "$src/$file"
    fi
    patch --batch --forward -p1 -d "$src" -i "$patch"
done

objects=$(awk -v rule="$dll:" 'index($0, rule) == 1 { found = 1 } found { print } found && /^\t/ { exit }' \
    "$pe/Makefile" | tr ' \\\t' '\n\n\n' | grep "^$module/$arch/.*\.o$")

for object in $objects; do
    name=$(basename "$object" .o)
    source=$autorun/$module/$name.c
    if [ -f "$src/$module/$name.c" ]; then source=$src/$module/$name.c; fi
    command=$(commands -W "$autorun/$module/$name.c" "$object" | grep -e "-c -o $object ")
    command=$(printf '%s' "$command" | sed "s#-c -o $object $autorun/$module/$name.c#-c -o $obj/$name.o $source#")
    (cd "$pe" && eval "$command")
done

rm -f "$pe/$dll"
link=$(commands "$dll" | grep -e "winegcc")
link=$(printf '%s' "$link" | sed -e "s#$module/$arch/\([a-z0-9_]*\)\.o#$obj/\1.o#g" -e "s#-o $dll #-o $out/wow64.dll #")
(cd "$pe" && eval "$link")
aarch64-w64-mingw32-strip --strip-debug "$out/wow64.dll"
make -C "$pe" "$dll"
test -s "$out/wow64.dll"

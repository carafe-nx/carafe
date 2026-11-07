import copy
import hashlib
import json
import re
import shutil
import struct
import subprocess
import sys
import zlib
from pathlib import Path

romfs, build, source = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
wine = romfs / 'switch/wine'
manifest_path = wine / 'horizon-dlls/manifest.json'
manifest = json.loads(manifest_path.read_text(encoding='utf-8'))
build_manifest_path = wine / 'build-manifest.json'
DLLS = ('d3d8.dll', 'd3d9.dll', 'd3d10core.dll', 'd3d11.dll', 'dxgi.dll')
SYSTEM = {'i386': 'drive_c/windows/syswow64', 'amd64': 'drive_c/windows/system32'}
ARCH = {'i386': 'i386', 'amd64': 'x86_64'}
FOLDERS = ('drive_c/dxvk', 'drive_c/dxvk64')


def say_directx9(path):
    data = bytearray(path.read_bytes())
    version = (5, 3, 1, 904)
    ms, ls = (version[0] << 16) | version[1], (version[2] << 16) | version[3]
    fixed = b'\xbd\x04\xef\xfe'
    at = data.find(fixed)
    found = 0
    while at >= 0:
        struct.pack_into('<IIII', data, at + 8, ms, ls, ms, ls)
        found += 1
        at = data.find(fixed, at + 4)
    if not found:
        sys.exit(f'{path} has no version resource')
    text = '%d.%d.%d.%d' % version
    for old in ('10.0.17763.1 (WinBuild.160101.0800)', '10.0.17763.1'):
        new = text + ' ' * (len(old) - len(text))
        data = bytearray(bytes(data).replace(old.encode('utf-16-le'), new.encode('utf-16-le')))
    path.write_bytes(data)


def entry_for(folder, name):
    entries = [f for f in manifest['files'] if f['name'] == name and f['path'] == folder]
    if len(entries) > 1:
        sys.exit(f'{folder}/{name}: {len(entries)} entries in {manifest_path}')
    if entries:
        return entries[0]
    template = next(f for f in manifest['files'] if f['name'] == 'd3d9.dll' and f['path'] == folder)
    entry = copy.deepcopy(template)
    entry['name'] = name
    entry['url'] = entry['url'][:-len('d3d9.dll')] + name
    entry['compressed']['url'] = entry['compressed']['url'][:-len('d3d9.dll.z')] + name + '.z'
    manifest['files'].append(entry)
    return entry


def install(arch, name):
    folder = SYSTEM[arch]
    component = 'd3d10' if name == 'd3d10core.dll' else name[:-4]
    target = wine / folder / name
    shutil.copyfile(build / arch / 'src' / component / name, target)
    if arch == 'i386' and name == 'd3d9.dll':
        say_directx9(target)
    data = target.read_bytes()
    entry = entry_for(folder, name)
    entry['arch'] = ARCH[arch]
    entry['size'] = len(data)
    entry['sha256'] = hashlib.sha256(data).hexdigest()
    packed = zlib.compress(data, 9)
    entry['compressed'].update(size=len(packed), sha256=hashlib.sha256(packed).hexdigest())


for arch in SYSTEM:
    for name in DLLS:
        install(arch, name)

for folder in FOLDERS:
    shutil.rmtree(wine / folder, ignore_errors=True)
manifest['files'] = [f for f in manifest['files'] if f['path'] not in FOLDERS]
manifest_path.write_text(json.dumps(manifest, indent=1, ensure_ascii=False) + '\n', encoding='utf-8')

version = re.search(r"project\('dxvk'.*?version\s*:\s*'([^']+)'", (source / 'meson.build').read_text(encoding='utf-8'))
if not version:
    sys.exit(f'{source / "meson.build"} has no version')
revision = subprocess.run(['git', '-C', str(source), 'rev-parse', 'HEAD'], stdout=subprocess.PIPE, text=True,
                          check=True).stdout.strip()
build_manifest = json.loads(build_manifest_path.read_text(encoding='utf-8'))
build_manifest['system-dxvk'] = {'version': version.group(1), 'revision': revision}
build_manifest_path.write_text(json.dumps(build_manifest, indent=2) + '\n', encoding='utf-8')
print(f'DXVK {version.group(1)}: Carafe build of {len(DLLS)} modules in {SYSTEM["i386"]} and {SYSTEM["amd64"]}')

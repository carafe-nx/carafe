import copy
import hashlib
import json
import shutil
import struct
import sys
import zlib
from pathlib import Path

romfs, build = Path(sys.argv[1]), Path(sys.argv[2])
wine = romfs / 'switch/wine'
manifest_path = wine / 'horizon-dlls/manifest.json'
manifest = json.loads(manifest_path.read_text(encoding='utf-8'))
DLLS = ('d3d8.dll', 'd3d9.dll', 'd3d10core.dll', 'd3d11.dll', 'dxgi.dll')


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


def record(folder, name):
    data = (wine / folder / name).read_bytes()
    entries = [f for f in manifest['files'] if f['name'] == name and f['path'] == folder]
    if len(entries) != 1:
        sys.exit(f'{folder}/{name}: {len(entries)} entries in {manifest_path}')
    entries[0]['size'] = len(data)
    entries[0]['sha256'] = hashlib.sha256(data).hexdigest()


target = wine / 'drive_c/dxvk/d3d9.dll'
shutil.copyfile(build / 'i386/src/d3d9/d3d9.dll', target)
say_directx9(target)
record('drive_c/dxvk', 'd3d9.dll')

dxvk64 = wine / 'drive_c/dxvk64'
dxvk_manifest = json.loads((dxvk64 / 'dxvk-manifest.json').read_text(encoding='utf-8'))
for name in DLLS:
    component = 'd3d10' if name == 'd3d10core.dll' else name[:-4]
    shutil.copyfile(build / 'amd64/src' / component / name, dxvk64 / name)
    dxvk_manifest['files'][name] = hashlib.sha256((dxvk64 / name).read_bytes()).hexdigest()
    record('drive_c/dxvk64', name)
(dxvk64 / 'dxvk-manifest.json').write_text(json.dumps(dxvk_manifest, indent=2) + '\n', encoding='utf-8')
record('drive_c/dxvk64', 'dxvk-manifest.json')

dxvk32_manifest = dict(dxvk_manifest, architecture='i386', files={'d3d9.dll': hashlib.sha256(target.read_bytes()).hexdigest()})
(target.parent / 'dxvk-manifest.json').write_text(json.dumps(dxvk32_manifest, indent=2) + '\n', encoding='utf-8')
if not any(f['name'] == 'dxvk-manifest.json' and f['path'] == 'drive_c/dxvk' for f in manifest['files']):
    entry = copy.deepcopy(next(f for f in manifest['files'] if f['name'] == 'dxvk-manifest.json' and f['path'] == 'drive_c/dxvk64'))
    entry.update(path='drive_c/dxvk', arch='i386', url=entry['url'].replace('/dxvk64/', '/dxvk/'))
    entry['source']['extra'] = 'dxvk32'
    entry['compressed']['url'] = entry['compressed']['url'].replace('/dxvk64/', '/dxvk/')
    manifest['files'].append(entry)
record('drive_c/dxvk', 'dxvk-manifest.json')
packed = zlib.compress((target.parent / 'dxvk-manifest.json').read_bytes(), 9)
for f in manifest['files']:
    if f['name'] == 'dxvk-manifest.json' and f['path'] == 'drive_c/dxvk':
        f['compressed'].update(size=len(packed), sha256=hashlib.sha256(packed).hexdigest())

manifest_path.write_text(json.dumps(manifest, indent=1, ensure_ascii=False) + '\n', encoding='utf-8')
print(f'DXVK: Carafe build of d3d9 (32-bit, version {dxvk32_manifest["version"]}) and {len(DLLS)} 64-bit modules')

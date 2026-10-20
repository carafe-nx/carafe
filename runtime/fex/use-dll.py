import hashlib
import json
import shutil
import sys
from pathlib import Path

romfs, folder, name, built = Path(sys.argv[1]), sys.argv[2], sys.argv[3], Path(sys.argv[4])
manifest_path = romfs / 'switch/wine/horizon-dlls/manifest.json'
target = romfs / 'switch/wine' / folder / name
manifest = json.loads(manifest_path.read_text(encoding='utf-8'))
entries = [f for f in manifest['files'] if f['name'] == name and f['path'] == folder]
if len(entries) != 1:
    sys.exit(f'{folder}/{name}: {len(entries)} entries in {manifest_path}')
shutil.copyfile(built, target)
data = target.read_bytes()
entries[0]['size'] = len(data)
entries[0]['sha256'] = hashlib.sha256(data).hexdigest()
manifest_path.write_text(json.dumps(manifest, indent=1, ensure_ascii=False) + '\n', encoding='utf-8')
print(f'{folder}/{name}: Carafe build, {len(data)} bytes')

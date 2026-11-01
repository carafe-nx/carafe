import shutil
import sys
from pathlib import Path

out, archive, wine = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
sys.path.insert(0, str(wine / 'tools'))
from wine_mono_payload import INSTALL_PATH, stage_wine_mono

if out.exists():
    shutil.rmtree(out)
result = stage_wine_mono(archive, out / INSTALL_PATH)
license_path = out / 'switch/wine/licenses/WineMono.txt'
license_path.parent.mkdir(parents=True, exist_ok=True)
shutil.copyfile(wine / 'licenses/WineMono.txt', license_path)
size = sum(path.stat().st_size for path in out.rglob('*') if path.is_file())
print(f"Wine Mono {result['version']}: {result['files']} files, {size / 1048576:.1f} MiB for games that use .NET")

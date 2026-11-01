import re
import sys
from pathlib import Path

romfs, autorun = Path(sys.argv[1]), Path(sys.argv[2])
wine = romfs / 'switch/wine'
system32 = wine / 'drive_c/windows/system32'
classes_reg = wine / 'horizon-dlls/classes.reg'

namespace_line = re.compile(r'^\s*namespace\s+([\w.]+)\s*\{')
class_line = re.compile(r'^\s*runtimeclass\s+(\w+)\s*;')

lines = []
count = 0
for idl in sorted((autorun / 'dlls').glob('*/classes.idl')):
    dll = idl.parent.name + '.dll'
    if not (system32 / dll).is_file():
        continue
    namespace = None
    names = []
    for text in idl.read_text(encoding='utf-8', errors='replace').splitlines():
        if (match := namespace_line.match(text)):
            namespace = match.group(1)
        elif (match := class_line.match(text)) and namespace:
            names.append(f'{namespace}.{match.group(1)}')
    if not names:
        continue
    lines.append(f';; WinRT: {idl.parent.name}')
    for name in names:
        lines.append(f'[Software\\\\Microsoft\\\\WindowsRuntime\\\\ActivatableClassId\\\\{name}]')
        lines.append(f'"DllPath"="C:\\\\windows\\\\system32\\\\{dll}"')
        lines.append('')
        count += 1

text = classes_reg.read_text(encoding='utf-8')
if not text.endswith('\n'):
    text += '\n'
classes_reg.write_text(text + '\n' + '\n'.join(lines) + '\n', encoding='utf-8')
print(f'WinRT: {count} activatable classes registered in {classes_reg.name}')

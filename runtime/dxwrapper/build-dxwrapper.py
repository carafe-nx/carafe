import concurrent.futures
import hashlib
import os
import shutil
import subprocess
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

NS = {'m': 'http://schemas.microsoft.com/developer/msbuild/2003'}
CONFIGURATION = "'$(Configuration)|$(Platform)'=='Release|Win32'"
DEFAULT_LIBS = ['kernel32.lib', 'user32.lib', 'gdi32.lib', 'winspool.lib', 'comdlg32.lib', 'advapi32.lib',
                'shell32.lib', 'ole32.lib', 'oleaut32.lib', 'uuid.lib', 'odbc32.lib', 'odbccp32.lib']
DETOURS_SOURCES = ['detours', 'modules', 'disasm', 'image', 'creatwth', 'disolx86', 'disolx64', 'disolia64',
                   'disolarm', 'disolarm64']
DETOURS_FLAGS = ['/nologo', '/W4', '/we4777', '/we4800', '/MT', '/Gy', '/Gm-', '/Zl', '/Od', '/DDETOUR_DEBUG=0',
                 '/DWIN32_LEAN_AND_MEAN', '/D_WIN32_WINNT=0x501']

src = Path(sys.argv[1]).resolve()
out = Path(sys.argv[2]).resolve()
jobs = int(os.environ.get('JOBS', '2'))


def run(command, cwd):
    result = subprocess.run(command, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
                            errors='replace')
    if result.returncode != 0:
        sys.exit(f'dxwrapper: {" ".join(command)} failed in {cwd}:\n{result.stdout}')


def relative(path, cwd):
    return os.path.relpath(path, cwd)


def applies(element):
    condition = element.get('Condition')
    return condition is None or condition.replace(' ', '') == CONFIGURATION


def values(text, macros):
    for value in (text or '').split(';'):
        value = value.strip()
        if not value or value.startswith('%('):
            continue
        for macro, replacement in macros.items():
            value = value.replace(macro, replacement)
        yield value.replace('\\', '/').rstrip('/') or '.'


def compile_all(units, cwd):
    with concurrent.futures.ThreadPoolExecutor(jobs) as pool:
        for future in [pool.submit(run, command, cwd) for command in units]:
            future.result()


def project(path, macros, libs, objects_dir):
    root = ET.parse(path).getroot()
    cwd = path.parent
    settings = {}
    for group in root.findall('m:ItemDefinitionGroup', NS):
        if applies(group):
            for tool in group:
                settings.setdefault(tool.tag.split('}')[1], {}).update(
                    {child.tag.split('}')[1]: child.text for child in tool})
    whole_program = any(applies(group) and group.findtext('m:WholeProgramOptimization', '', NS) == 'true'
                        for group in root.findall('m:PropertyGroup', NS))
    compile_settings = settings.get('ClCompile', {})
    flags = ['/nologo', '/c', '/O2', '/EHsc', '/Zc:wchar_t', '/Zc:forScope', '/Zc:inline', '/fp:precise', '/GS']
    flags += [f'/D{d}' for d in values(compile_settings.get('PreprocessorDefinitions'), macros)]
    flags += [f'/I{i}' for i in values(compile_settings.get('AdditionalIncludeDirectories'), macros)]
    flags += {'MultiThreaded': ['/MT'], 'MultiThreadedDLL': ['/MD']}[compile_settings.get('RuntimeLibrary',
                                                                                          'MultiThreadedDLL')]
    flags += {'Level4': ['/W4'], 'Level3': ['/W3']}.get(compile_settings.get('WarningLevel'), [])
    flags += {'Speed': ['/Ot'], 'Size': ['/Os']}.get(compile_settings.get('FavorSizeOrSpeed'), [])
    flags += {'AnySuitable': ['/Ob2']}.get(compile_settings.get('InlineFunctionExpansion'), [])
    flags += ['/GR-'] if compile_settings.get('RuntimeTypeInfo') == 'false' else []
    flags += ['/GL'] if whole_program else []
    standard = {'stdcpp17': '/std:c++17', 'stdcpp20': '/std:c++20'}.get(compile_settings.get('LanguageStandard'))

    units, objects, resources = [], [], []
    for group in root.findall('m:ItemGroup', NS):
        for item in group.findall('m:ClCompile', NS):
            excluded = any(applies(child) and child.text == 'true'
                           for child in item.findall('m:ExcludedFromBuild', NS))
            source = item.get('Include').replace('\\', '/')
            if excluded or Path(source).suffix.lower() not in ('.c', '.cpp', '.cc', '.cxx'):
                continue
            obj = objects_dir / (os.path.normpath(source).replace('..', '_') + '.obj')
            obj.parent.mkdir(parents=True, exist_ok=True)
            language = [standard] if standard and not source.endswith('.c') else []
            units.append(['cl'] + flags + language + [f'/Fo{relative(obj, cwd)}', source])
            objects.append(obj)
        for item in group.findall('m:ResourceCompile', NS):
            source = item.get('Include').replace('\\', '/')
            res = objects_dir / (Path(source).stem + '.res')
            resources.append((['rc', '/nologo', f'/fo{relative(res, cwd)}', source], res))
    compile_all(units, cwd)
    for command, res in resources:
        run(command, cwd)

    link_settings = settings.get('Link', {})
    dependencies = list(values(link_settings.get('AdditionalDependencies'), macros)) + DEFAULT_LIBS
    lib_dirs = [d for d in values(link_settings.get('AdditionalLibraryDirectories'), macros)
                if 'detours' not in d.lower()]
    target = objects_dir / f'{path.stem}.dll'
    command = ['link', '/nologo', '/DLL', '/MACHINE:X86', '/SUBSYSTEM:WINDOWS', f'/OUT:{relative(target, cwd)}']
    command += ['/LTCG'] if whole_program else []
    command += ['/OPT:REF'] if link_settings.get('OptimizeReferences') == 'true' else []
    command += ['/OPT:ICF'] if link_settings.get('EnableCOMDATFolding') == 'true' else []
    command += ['/SAFESEH:NO'] if link_settings.get('ImageHasSafeExceptionHandlers') == 'false' else []
    if link_settings.get('ModuleDefinitionFile'):
        command.append('/DEF:' + next(values(link_settings['ModuleDefinitionFile'], macros)))
    command += [f'/LIBPATH:{relative(d, cwd)}' for d in libs] + [f'/LIBPATH:{d}' for d in lib_dirs]
    command += [relative(o, cwd) for o in objects] + [relative(r, cwd) for _, r in resources] + dependencies
    run(command, cwd)
    return target


def detours(objects_dir):
    cwd = src / 'External/detours/src'
    objects_dir.mkdir(parents=True, exist_ok=True)
    compile_all([['cl'] + DETOURS_FLAGS + ['/c', f'/Fo{relative(objects_dir / (name + ".obj"), cwd)}',
                                           f'{name}.cpp'] for name in DETOURS_SOURCES], cwd)
    library = objects_dir / 'detours.lib'
    run(['lib', '/nologo', f'/OUT:{relative(library, cwd)}']
        + [relative(objects_dir / f'{name}.obj', cwd) for name in DETOURS_SOURCES], cwd)
    return objects_dir


def dx7_settings(source, target):
    lines = []
    for line in source.read_bytes().splitlines(keepends=True):
        ending = line[len(line.rstrip(b'\r\n')):]
        lines.append(b'Dd7to9                     = 1' + ending if line.replace(b' ', b'').startswith(b'Dd7to9') else line)
    target.write_bytes(b''.join(lines))


revision = subprocess.run(['git', '-C', str(src), 'rev-parse', 'HEAD'], stdout=subprocess.PIPE, text=True,
                          check=True).stdout.strip()
patches = sorted((Path(__file__).resolve().parent.parent / 'patches/dxwrapper').glob('*.patch'))
stamp = revision + hashlib.sha256(Path(__file__).read_bytes() + b''.join(p.read_bytes() for p in patches)).hexdigest()
products = [out / name for name in ('ddraw.dll', 'dxwrapper.dll', 'dxwrapper.ini', 'License.txt')]
if (out / 'stamp').exists() and (out / 'stamp').read_text() == stamp and all(p.exists() for p in products):
    print(f'dxwrapper: {revision[:12]} already built')
    sys.exit(0)

shutil.rmtree(out, ignore_errors=True)
work = out.parent / 'dxwrapper-src'
shutil.rmtree(work, ignore_errors=True)
shutil.copytree(src, work, ignore=lambda folder, names: [
    name for name in names if name == '.git' or (Path(folder) == src and name == 'DirectX')])
for patch in patches:
    subprocess.run(['git', 'apply', str(patch)], cwd=work, check=True,
                   env=dict(os.environ, GIT_CEILING_DIRECTORIES=str(work.parent)))
    print(f'dxwrapper: applied {patch.name}')
src = work
subprocess.run(['wineserver', '-p'], check=True)
subprocess.run(['wine', 'wineboot'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=True)
try:
    detours_dir = detours(out / 'obj/detours')
    wrapper = project(src / 'dxwrapper.vcxproj', {'$(ProjectDir)': '.', '$(SolutionDir)': '.'}, [detours_dir],
                      out / 'obj/dxwrapper')
    stub = project(src / 'Stub/stub.vcxproj', {'$(ProjectDir)': '.', '$(SolutionDir)': '..'}, [],
                   out / 'obj/stub')
finally:
    subprocess.run(['wineserver', '-k'])
shutil.copyfile(wrapper, out / 'dxwrapper.dll')
shutil.copyfile(stub, out / 'ddraw.dll')
dx7_settings(src / 'Settings/Settings.ini', out / 'dxwrapper.ini')
shutil.copyfile(src / 'License.txt', out / 'License.txt')
(out / 'stamp').write_text(stamp)
print(f'dxwrapper: {revision[:12]} built, ddraw.dll {(out / "ddraw.dll").stat().st_size} bytes, '
      f'dxwrapper.dll {(out / "dxwrapper.dll").stat().st_size} bytes')

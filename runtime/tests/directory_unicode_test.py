from pathlib import Path
import hashlib
import re
import subprocess
import sys

root, source_root, output = map(Path, sys.argv[1:])
output.mkdir(parents=True, exist_ok=True)


def function(source, name):
    source = re.sub(r"/[*].*?[*]/", "", source, flags=re.S)
    match = re.search(r'^(?:static )?(?:inline )?[^;\n]*\b' + name + r'\([^;]+?\)\n\{', source, re.M)
    if not match:
        raise RuntimeError('missing real C function: ' + name)
    start = source.index('{', match.start())
    end, depth = start + 1, 1
    while depth:
        depth += (source[end] == '{') - (source[end] == '}')
        end += 1
    return source[match.start():end]


horizon = (source_root / 'dlls/ntdll/unix/horizon.c').read_text()
file = (source_root / 'dlls/ntdll/unix/file.c').read_text()
autorun = root / 'external/autorun'
locale = (autorun / 'dlls/ntdll/locale_private.h').read_text()
env = (autorun / 'dlls/ntdll/unix/env.c').read_text()
private = (autorun / 'dlls/ntdll/unix/unix_private.h').read_text()
access = (autorun / 'dlls/ntdll/unix/horizon_file_access.h').read_text()
parts = []
parts += [function(locale, name) for name in ['get_utf16', 'decode_utf8_char', 'utf8_mbstowcs_size',
          'utf8_mbstowcs', 'utf8_wcstombs_size', 'utf8_wcstombs']]
parts += [function(env, name) for name in ['RtlUTF8ToUnicodeN', 'RtlUnicodeToUTF8N']]
parts += [function(private, name) for name in ['ntdll_towupper', 'ntdll_towlower', 'ntdll_wcsupr']]
parts += [function(file, name) for name in ['is_invalid_dos_char', 'is_wildcard', 'hash_short_file_name',
          'match_filename_part', 'match_filename', 'is_legal_8dot3_name', 'horizon_directory_short_name',
          'horizon_directory_name_matches']]
parts += [function(horizon, name) for name in ['horizon_server_utf16_name_len', 'horizon_server_write_utf16_name',
          'horizon_server_dir_mask_from_utf16', 'horizon_server_handle_query_directory_file',
          'horizon_server_handle_get_mapping_info']]
parts += [function(access, 'horizon_dir_scan_end_status')]
assert 'static const BOOL is_case_sensitive = FALSE;' in file
end_status = parts.pop()
(output / 'directory_unicode_impl.h').write_text(end_status + '\n\n' + '\n\n'.join(parts) + '\n')
source_tests = (autorun / 'dlls/ntdll/tests/directory.c').read_text()
entries = source_tests[source_tests.index('} testfiles[] = {'):source_tests.index('static const int test_dir_count')]
names = re.findall(r'\{ 0, FILE_ATTRIBUTE_\w+,\s*(\{[^}]+\})', entries)
vectors = source_tests[source_tests.index('    mask_tests[] ='):source_tests.index('    UNICODE_STRING ntdirname, mask;')]
vectors = re.findall(r'\{L"((?:\\.|[^"\\])*)",\s*\{([01, ]+)\}\}', vectors)
assert len(names) == 18 and len(vectors) > 70
(output / 'directory_unicode_vectors.h').write_text(
    'static const WCHAR vector_names[][32] = {\n' + ',\n'.join(names) + '\n};\n'
    + 'static const struct { const WCHAR *mask; int found[18]; } vectors[] = {\n'
    + ',\n'.join('{u"' + mask + '", {' + found + '}}' for mask, found in vectors) + '\n};\n')
for path in [source_root/'dlls/ntdll/unix/horizon.c', source_root/'dlls/ntdll/unix/file.c',
             autorun/'dlls/ntdll/locale_private.h', autorun/'nls/l_intl.nls']:
    print(hashlib.sha256(path.read_bytes()).hexdigest(), path, flush=True)
exe = output/'directory-unicode-test'
subprocess.run(['cc', '-std=c11', '-D_GNU_SOURCE', '-D__SWITCH__', '-O2', '-g',
                '-Wall', '-Wextra', '-Werror', '-Wno-unused-function', '-Wno-implicit-fallthrough',
                '-I'+str(output), '-o', str(exe), str(Path(__file__).with_suffix('.c')), '-pthread'], check=True)
subprocess.run([str(exe), str(autorun/'nls/l_intl.nls')], check=True)

client_functions = [function(locale, name) for name in ['get_utf16', 'decode_utf8_char', 'utf8_wcstombs_size', 'utf8_wcstombs']]
client_functions += [function(private, name) for name in ['ntdll_towupper', 'ntdll_towlower', 'ntdll_wcsupr']]
client_functions += [function(file, name) for name in ['is_invalid_dos_char', 'hash_short_file_name', 'is_legal_8dot3_name',
    'horizon_directory_short_name', 'dir_info_size', 'dir_info_align', 'get_dir_data_entry']]
client_functions += [function(access, 'horizon_dir_entry_path')]
client_functions += [function(file, name) for name in ['horizon_dir_entry_unix_name', 'horizon_query_directory_file']]
(output / 'client_impl.h').write_text('\n\n'.join(client_functions))
client_exe = output/'directory-client-test'
subprocess.run(['cc', '-std=c11', '-D_GNU_SOURCE', '-D__SWITCH__', '-ffixed-x18', '-O2', '-g',
    '-Wall', '-Wextra', '-Werror', '-Wno-unused-function', '-Wno-implicit-fallthrough',
    '-I'+str(autorun/'include'), '-I'+str(output), '-o', str(client_exe),
    str(Path(__file__).with_name('directory_client_test.c'))], check=True)
subprocess.run([str(client_exe), str(autorun/'nls/l_intl.nls')], check=True)

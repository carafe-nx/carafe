from pathlib import Path
import re
import subprocess
import sys

source_path, output_path = map(Path, sys.argv[1:])
source = source_path.read_text()
match = re.search(r"^static int map_anonymous_backings\([^;]+?\)\n\{", source, re.M)
if match is None:
    raise RuntimeError("anonymous mapping implementation not found")
start = source.index("{", match.start())
end, depth = start + 1, 1
while depth:
    depth += (source[end] == "{") - (source[end] == "}")
    end += 1
definitions = re.findall(r"^#define HORIZON_ANONYMOUS_BACKING_(?:MIN|UNIT) .*", source, re.M)
implementation = "\n".join(definitions) + "\n" + source[match.start():end] + "\n"
(output_path / "anonymous_mapping_impl.h").write_text(implementation)
test_path = Path(__file__).with_suffix(".c")
executable = output_path / "anonymous-mapping"
subprocess.run([
    "cc", "-std=c11", "-D_GNU_SOURCE", "-O2", "-Wall", "-Wextra", "-Werror",
    "-Wno-unused-function", "-pthread", "-I" + str(output_path),
    "-I" + str(output_path / "dlls/ntdll/unix"), "-o", str(executable), str(test_path)
], check=True)
subprocess.run([str(executable)], check=True)

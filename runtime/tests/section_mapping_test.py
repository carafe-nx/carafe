from pathlib import Path
import re
import subprocess
import sys

source_path, output_path = map(Path, sys.argv[1:])
source = source_path.read_text()
functions = [
    "find_anchor_run_locked", "horizon_section_anchor", "horizon_section_unanchor",
    "horizon_section_alias", "horizon_section_unalias", "alloc_section_range",
    "free_section_range", "horizon_mmap_section",
]
implementations = []
for name in functions:
    match = re.search(r"^static [^;\n]*\b" + name + r"\([^;]+?\)\n\{", source, re.M)
    if match is None:
        raise RuntimeError(f"section implementation not found: {name}")
    start = source.index("{", match.start())
    end, depth = start + 1, 1
    while depth:
        depth += (source[end] == "{") - (source[end] == "}")
        end += 1
    implementations.append(source[match.start():end])
(output_path / "section_mapping_impl.h").write_text("\n\n".join(implementations) + "\n")
executable = output_path / "section-mapping"
subprocess.run([
    "cc", "-std=c11", "-D_GNU_SOURCE", "-O2", "-Wall", "-Wextra", "-Werror",
    "-Wno-unused-function", "-pthread", "-I" + str(output_path),
    "-I" + str(output_path / "dlls/ntdll/unix"), "-o", str(executable),
    str(Path(__file__).with_suffix(".c")),
], check=True)
subprocess.run([str(executable)], check=True)

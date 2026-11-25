from pathlib import Path
import os
import shutil
import subprocess
import sys
import tempfile

TARGET = "horizon-wine/source/runtime.c"
TEST_BUDGET = 1024 * 1024


def apply_patches(source, patch_dir, work):
    target = work / TARGET
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, target)
    patch = shutil.which("patch")
    if not patch:
        raise SystemExit("patch not found")
    for item in sorted(Path(patch_dir).glob("*.patch")):
        text = item.read_text()
        if "+++ b/" + TARGET not in text:
            continue
        subprocess.run([patch, "--batch", "--forward", "-p1", "-i", str(item)], cwd=work, check=True)
    return target.read_text()


def extract_macros(text):
    start = text.index("#ifndef CARAFE_LOG_BUDGET_BYTES")
    end = text.index("#define CARAFE_LOG_FINAL_TAIL", start)
    end = text.index("\n", end) + 1
    return text[start:end]


def extract_function(text, name):
    start = text.index("static int " + name + "(")
    brace = text.index("{", start)
    depth = 1
    end = brace + 1
    while depth:
        depth += (text[end] == "{") - (text[end] == "}")
        end += 1
    return text[start:end]


def check_source(text):
    assert "#define CARAFE_LOG_BUDGET_BYTES (512ull * 1024 * 1024)" in text
    assert "log_write_budgeted( log_file, &log_file_bytes" in text
    assert "log_write_budgeted( game_log_file, &game_log_bytes" in text
    assert text.count("log_write_budgeted(") >= 5
    assert 'fprintf( log_file, "[CLEAN]' not in text
    region = text[text.index("static FILE *log_file;"):text.index("static void runtime_tick_std_streams")]
    for forbidden in ("fseek(", "ftell(", "fstat(", "stat("):
        assert forbidden not in region, forbidden
    assert "if (!log_flusher_running || urgent) fflush( log_file );" in text
    assert "if (!log_flusher_running || urgent) fflush( game_log_file );" in text
    alias = text.index('if (log_is_runtime_path( path )) return;')
    assert alias < text.index('fopen( path, "w" )', alias)


def harness(text):
    macros = extract_macros(text)
    helper = extract_function(text, "log_write_budgeted")
    final = extract_function(text, "log_line_is_final")
    alias = extract_function(text, "log_is_runtime_path")
    return f'''
#include <assert.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#include <strings.h>

{macros}
{final}
{alias}
{helper}

#define log_write_budgeted(file, used, noted, priority, data, len) \\
    (log_write_budgeted)(file, used, noted, (file) == left ? &seen_left : &seen_right, priority, data, len)

static unsigned long long file_bytes(FILE *file)
{{
    long at;

    fflush(file);
    at = ftell(file);
    assert(at >= 0);
    return (unsigned long long)at;
}}

int main(void)
{{
    char chunk[4096];
    FILE *left = tmpfile(), *right = tmpfile();
    unsigned long long used_left = 0, used_right = 0;
    unsigned long long normal = CARAFE_LOG_SINK_BYTES;
    unsigned long long i;
    int noted_left = 0, noted_right = 0;
    unsigned int seen_left = 0, seen_right = 0;

    assert(left && right);
    assert(log_is_runtime_path("sdmc:/logs/AUTORUN_Runtime.LOG"));
    assert(log_is_runtime_path("autorun_runtime.log"));
    assert(!log_is_runtime_path("sdmc:/logs/autorun_runtime_verbose.log"));
    normal -= CARAFE_LOG_FINAL_TAIL + CARAFE_LOG_URGENT_TAIL;
    memset(chunk, 'n', sizeof(chunk));

    for (i = 0; i < normal / sizeof(chunk); i++)
        assert(log_write_budgeted(left, &used_left, &noted_left, 0, chunk, sizeof(chunk)));
    assert(used_left == normal);
    assert(file_bytes(left) == used_left);

    assert(!log_write_budgeted(left, &used_left, &noted_left, 0, chunk, sizeof(chunk)));
    assert(noted_left == 1);
    assert(!log_write_budgeted(left, &used_left, &noted_left, 0, chunk, sizeof(chunk)));
    assert(used_left <= CARAFE_LOG_SINK_BYTES);
    assert(file_bytes(left) == used_left);

    memset(chunk, 'u', sizeof(chunk));
    while (log_write_budgeted(left, &used_left, &noted_left, 1, chunk, sizeof(chunk))) {{ }}
    assert(used_left <= CARAFE_LOG_SINK_BYTES);
    assert(used_left + sizeof(chunk) > CARAFE_LOG_SINK_BYTES - CARAFE_LOG_FINAL_TAIL);
    assert(file_bytes(left) == used_left);

    for (i = 0; i < used_left / sizeof(chunk); i++)
        assert(log_write_budgeted(right, &used_right, &noted_right, 1, chunk, sizeof(chunk)));
    if (used_left % sizeof(chunk))
        assert(log_write_budgeted(right, &used_right, &noted_right, 1, chunk, used_left % sizeof(chunk)));
    assert(used_right == used_left);
    assert(noted_right == 0);
    assert(file_bytes(right) == used_right);

    assert(!log_write_budgeted(left, &used_left, &noted_left, 0, chunk, sizeof(chunk)));
    assert(!log_write_budgeted(right, &used_right, &noted_right, 0, chunk, sizeof(chunk)));
    assert(noted_right == 1);
    assert(!log_write_budgeted(left, &used_left, &noted_left, 1, chunk, sizeof(chunk)));
    assert(!log_write_budgeted(right, &used_right, &noted_right, 1, chunk, sizeof(chunk)));
    assert(log_line_is_final("[EXIT] final exit"));
    assert(log_line_is_final("[LIFECYCLE] final status"));
    assert(!log_line_is_final("[EXC] earlier urgent"));
    assert(log_write_budgeted(left, &used_left, &noted_left, 2, "[EXIT] final exit\\n", 18));
    assert(log_write_budgeted(right, &used_right, &noted_right, 2, "[EXIT] final exit\\n", 18));
    for (i = 0; i < 10000; i++) {{
        assert(!log_write_budgeted(left, &used_left, &noted_left, 2, "[EXIT] repeated\\n", 16));
        assert(!log_write_budgeted(right, &used_right, &noted_right, 2, "[EXIT] repeated\\n", 16));
    }}
    assert(log_line_is_final("[LIFECYCLE] final status") == 3);
    assert(log_line_is_final("[LIFECYCLE] verdict ok") == 4);
    assert(log_write_budgeted(left, &used_left, &noted_left, 3, "[LIFECYCLE] final status\\n", 25));
    assert(log_write_budgeted(right, &used_right, &noted_right, 3, "[LIFECYCLE] final status\\n", 25));
    assert(log_write_budgeted(left, &used_left, &noted_left, 4, "[LIFECYCLE] verdict ok\\n", 23));
    assert(log_write_budgeted(right, &used_right, &noted_right, 4, "[LIFECYCLE] verdict ok\\n", 23));
    {{
        char result[67] = {{0}};
        assert(!fflush(left) && !fflush(right));
        assert(!fseek(left, -66, SEEK_END) && !fseek(right, -66, SEEK_END));
        assert(fread(result, 1, 66, left) == 66);
        assert(!strcmp(result, "[EXIT] final exit\\n[LIFECYCLE] final status\\n[LIFECYCLE] verdict ok\\n"));
        assert(fread(result, 1, 66, right) == 66);
        assert(!strcmp(result, "[EXIT] final exit\\n[LIFECYCLE] final status\\n[LIFECYCLE] verdict ok\\n"));
    }}
    assert(used_left <= CARAFE_LOG_SINK_BYTES);
    assert(used_right <= CARAFE_LOG_SINK_BYTES);
    assert(used_left + used_right <= CARAFE_LOG_BUDGET_BYTES);

    printf("log budget ok: sink=%llu combined=%llu used=%llu+%llu\\n",
           (unsigned long long)CARAFE_LOG_SINK_BYTES,
           (unsigned long long)CARAFE_LOG_BUDGET_BYTES, used_left, used_right);
    fclose(left);
    fclose(right);
    return 0;
}}
'''


def main():
    source = Path(sys.argv[1])
    patch_dir = Path(sys.argv[2])
    work = Path(tempfile.mkdtemp(prefix="carafe-log-budget-"))
    try:
        text = apply_patches(source, patch_dir, work)
        check_source(text)
        harness_path = work / "log_budget_test.c"
        harness_path.write_text(harness(text))
        cc = os.environ.get("CC", "cc")
        binary = work / "log_budget_test"
        subprocess.run([cc, "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror",
                        f"-DCARAFE_LOG_BUDGET_BYTES={TEST_BUDGET}",
                        "-o", str(binary), str(harness_path)], check=True)
        subprocess.run([str(binary)], check=True)
    finally:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    main()

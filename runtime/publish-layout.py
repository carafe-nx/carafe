import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
from uuid import uuid4


def require_local_path(path):
    try:
        metadata = path.lstat()
    except FileNotFoundError:
        return
    if stat.S_ISLNK(metadata.st_mode) or getattr(metadata, "st_file_attributes", 0) & 0x400:
        raise ValueError(f"refusing to use redirected path: {path}")


def remove_tree(path):
    require_local_path(path)
    if path.exists():
        shutil.rmtree(path)


def acquire(lock):
    if os.name == "nt":
        import msvcrt

        msvcrt.locking(lock.fileno(), msvcrt.LK_LOCK, 1)
        return {}
    import fcntl

    fcntl.flock(lock, fcntl.LOCK_EX)
    return {"pass_fds": (lock.fileno(),)}


def publish(out, lock_path, command):
    state = out.with_name(f".{out.name}.layout")
    pending = state / "pending"
    previous = state / "previous"
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    require_local_path(lock_path)
    with lock_path.open("ab") as lock:
        inheritance = acquire(lock)
        owner = state / "owner"
        for path in (out, state, owner, pending, previous):
            require_local_path(path)
        try:
            state.mkdir(parents=True)
        except FileExistsError:
            if not owner.is_file() or owner.read_bytes() != b"carafe-runtime-layout-v1\n":
                raise ValueError(f"refusing to use an unowned staging directory: {state}")
        else:
            owner.write_bytes(b"carafe-runtime-layout-v1\n")
        if previous.exists() and not out.exists():
            previous.rename(out)
        remove_tree(previous)
        remove_tree(pending)
        pending.mkdir()
        try:
            subprocess.run(
                [*command, f"OUT={pending}"],
                check=True,
                **inheritance,
            )
            for relative in ("exefs/main", "exefs/main.npdm", "romfs/switch/wine/build-manifest.json"):
                if not (pending / relative).is_file() or not (pending / relative).stat().st_size:
                    raise ValueError(f"missing or empty runtime layout file: {relative}")
            (pending / ".layout-generation").write_text(uuid4().hex, encoding="ascii")
            if out.exists():
                out.rename(previous)
            try:
                pending.rename(out)
            except OSError:
                if previous.exists() and not out.exists():
                    previous.rename(out)
                raise
        finally:
            remove_tree(pending)
        remove_tree(previous)


if __name__ == "__main__":
    publish(Path(os.path.abspath(sys.argv[1])), Path(sys.argv[2]), sys.argv[3:])

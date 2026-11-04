"""POSIX metadata-only probe; not the production Semble collector.

Does not select source files or read source bodies. Ignore policy and Cursor
permission orchestration must be implemented before using an inventory to read
content. Run with Python 3.9+ and an absolute physical directory argument.
"""

import json
import os
import stat
import sys
import time


RULE_NAMES = frozenset((".gitignore", ".ignore", ".sembleignore", ".cursorignore"))
SKIP_DIRS = frozenset((
    ".git", ".hg", ".svn", "node_modules", "target", ".venv", "venv",
    ".tox", "__pycache__", ".next", "dist", "build", ".cache", ".semble",
))
MAX_ENTRIES = 20000
MAX_DEPTH = 128
MAX_RULE_BYTES = 32768
MAX_TOTAL_RULE_BYTES = 262144
MAX_OUTPUT_BYTES = 4 * 1024 * 1024
MAX_SECONDS = 10


class InventoryError(Exception):
    pass


def stamp(info):
    return dict(device=info.st_dev, inode=info.st_ino, size=info.st_size,
                modified_ns=info.st_mtime_ns, changed_ns=info.st_ctime_ns)


def open_directory(name, parent=None):
    return os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
                   dir_fd=parent)


def physical_root(root):
    if not root.startswith("/") or "\x00" in root:
        raise InventoryError("root must be an absolute POSIX path")
    parts = root.split("/")[1:]
    if any(part in (".", "..") for part in parts):
        raise InventoryError("root must not contain dot components")
    descriptor = open_directory("/")
    try:
        for part in filter(None, parts):
            child = open_directory(part, descriptor)
            os.close(descriptor)
            descriptor = child
        return descriptor
    except BaseException:
        os.close(descriptor)
        raise


def collect(root):
    if os.name != "posix" or not hasattr(os, "O_NOFOLLOW"):
        raise InventoryError("this experiment requires POSIX O_NOFOLLOW")
    root = root.rstrip("/") or "/"
    deadline = time.monotonic() + MAX_SECONDS
    files, rules, skipped_links = [], [], []
    entries_seen = total_rule_bytes = 0

    def check_time():
        if time.monotonic() >= deadline:
            raise InventoryError("inventory deadline exceeded")

    def visit(descriptor, relative, depth):
        nonlocal entries_seen, total_rule_bytes
        check_time()
        if depth > MAX_DEPTH:
            raise InventoryError("directory depth limit exceeded")
        before = os.fstat(descriptor)
        # Enumerate through the held descriptor, never through a resolved path.
        with os.scandir(descriptor) as entries:
            for entry in entries:
                check_time()
                entries_seen += 1
                if entries_seen > MAX_ENTRIES:
                    raise InventoryError("entry limit exceeded")
                name = entry.name
                name.encode("utf-8", errors="strict")
                path = relative + name
                info = entry.stat(follow_symlinks=False)
                if name in RULE_NAMES and not stat.S_ISREG(info.st_mode):
                    raise InventoryError("ignore file is not a regular physical file")
                if stat.S_ISLNK(info.st_mode):
                    skipped_links.append(path)
                elif stat.S_ISDIR(info.st_mode) and name not in SKIP_DIRS:
                    child = open_directory(name, descriptor)
                    try:
                        if stamp(os.fstat(child)) != stamp(info):
                            raise InventoryError("directory changed during collection")
                        visit(child, path + "/", depth + 1)
                    finally:
                        os.close(child)
                elif stat.S_ISREG(info.st_mode):
                    if name in RULE_NAMES:
                        if info.st_size > MAX_RULE_BYTES:
                            raise InventoryError("ignore file exceeds size limit")
                        rule_fd = os.open(name, os.O_RDONLY | os.O_NOFOLLOW
                                          | os.O_NONBLOCK, dir_fd=descriptor)
                        try:
                            opened = os.fstat(rule_fd)
                            if not stat.S_ISREG(opened.st_mode) or stamp(opened) != stamp(info):
                                raise InventoryError("ignore file changed before read")
                            with os.fdopen(rule_fd, "rb", closefd=False) as reader:
                                body = reader.read(MAX_RULE_BYTES + 1)
                            if len(body) != info.st_size or stamp(os.fstat(rule_fd)) != stamp(info):
                                raise InventoryError("ignore file changed during read")
                        finally:
                            os.close(rule_fd)
                        total_rule_bytes += len(body)
                        if total_rule_bytes > MAX_TOTAL_RULE_BYTES:
                            raise InventoryError("total ignore size limit exceeded")
                        rules.append(dict(path=path, text=body.decode("utf-8", errors="strict"),
                                          stamp=stamp(info)))
                    else:
                        files.append(dict(path=path, stamp=stamp(info)))
                # FIFOs, sockets and device nodes are never opened.
        if stamp(os.fstat(descriptor)) != stamp(before):
            raise InventoryError("directory changed during collection")

    descriptor = physical_root(root)
    try:
        identity = stamp(os.fstat(descriptor))
        visit(descriptor, "", 0)
    finally:
        os.close(descriptor)
    result = dict(schema=1, ok=True, root=root, root_stamp=identity,
                  policy_complete=False,
                  files=sorted(files, key=lambda value: value["path"]),
                  rules=sorted(rules, key=lambda value: value["path"]),
                  skipped_links=sorted(skipped_links), entries_seen=entries_seen)
    encoded = json.dumps(result, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    if len(encoded) > MAX_OUTPUT_BYTES:
        raise InventoryError("inventory output limit exceeded")
    check_time()
    return result


def main():
    try:
        if sys.version_info < (3, 9):
            raise InventoryError("this experiment requires Python 3.9 or newer")
        if len(sys.argv) != 2:
            raise InventoryError("expected exactly one root argument")
        result = collect(sys.argv[1])
        code = 0
    except (InventoryError, OSError, UnicodeError) as error:
        # Never emit a partial inventory as a successful snapshot.
        result = dict(schema=1, ok=False, error=str(error))
        code = 2
    print(json.dumps(result, ensure_ascii=False, separators=(",", ":")))
    return code


if __name__ == "__main__":
    sys.exit(main())

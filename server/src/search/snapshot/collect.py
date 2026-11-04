"""Read-only workspace collection for Cursor Exec. Python 3.9+."""

import base64
import hashlib
import json
import os
import stat
import subprocess
import sys
import time

RULES = (".ignore", ".sembleignore", ".cursorignore")
SKIP = frozenset((".git", ".hg", ".svn", "node_modules", "target", ".venv",
                  "venv", ".tox", "__pycache__", ".next", "dist", "build",
                  ".cache", ".semble"))
MAX_ENTRIES = 20000
MAX_RULE_BYTES = 32768
MAX_RULE_TOTAL = 262144
MAX_MANIFEST_BYTES = 4 * 1024 * 1024
MAX_FILE_BYTES = 1000000
MAX_BATCH_BYTES = 2 * 1024 * 1024
PAGE_BYTES = 256 * 1024
MAX_RESPONSE_BYTES = 5 * 1024 * 1024
DEADLINE = time.monotonic() + 20


def check_time():
    if time.monotonic() >= DEADLINE:
        raise ValueError("workspace collection timed out")


def encode(value):
    return json.dumps(value, ensure_ascii=True, sort_keys=True,
                      separators=(",", ":")).encode("utf-8")


def stamp(info):
    return (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def relative(path):
    return (isinstance(path, str) and len(path.encode("utf-8")) <= 4096
            and "\\" not in path and "\0" not in path
            and all(p not in ("", ".", "..") for p in path.split("/")))



def is_link(info):
    return stat.S_ISLNK(info.st_mode) or bool(getattr(info, "st_file_attributes", 0) & 0x400)


def collect(root):
    check_time()
    rules, entries, checks, owners = [], [], [], {}
    counters = {"entries": 0, "rules": 0}

    def rules_at(fd, absolute):
        for name in RULES:
            check_time()
            info = info_at(fd, name)
            if info is None:
                checks.append((absolute, name, None))
                continue
            if is_link(info) or not stat.S_ISREG(info.st_mode) or info.st_size > MAX_RULE_BYTES:
                raise ValueError("ignore rules must be bounded physical files")
            body = read_regular(fd, name, info, MAX_RULE_BYTES)
            counters["rules"] += len(body)
            if counters["rules"] > MAX_RULE_TOTAL:
                raise ValueError("workspace ignore rules exceed size limit")
            rules.append(dict(directory=absolute, name=name, contents=body.decode("utf-8")))
            checks.append((absolute, name, stamp(info)))

    def repo_at(fd, absolute, inherited):
        marker = info_at(fd, ".git")
        if marker is None:
            return inherited
        if is_link(marker) or not (stat.S_ISREG(marker.st_mode) or stat.S_ISDIR(marker.st_mode)):
            raise ValueError("Git metadata marker is not a physical file or directory")
        checks.append((absolute, ".git", stamp(marker)))
        return absolute

    inherited = None
    parents = []
    parent = os.path.dirname(root)
    while parent != root:
        parents.append(parent)
        next_parent = os.path.dirname(parent)
        if next_parent == parent:
            break
        parent = next_parent
    for parent in reversed(parents):
        fd = open_root(parent)
        try:
            rules_at(fd, parent)
            inherited = repo_at(fd, parent, inherited)
        finally:
            close_directory(fd)

    def visit(fd, prefix, repo, depth):
        check_time()
        if depth > 128:
            raise ValueError("workspace exceeds directory depth limit")
        before = directory_info(fd)
        absolute = root if not prefix else root.rstrip("/") + "/" + prefix.rstrip("/")
        rules_at(fd, absolute)
        repo = repo_at(fd, absolute, repo)
        with scan_directory(fd) as iterator:
            for entry in iterator:
                check_time()
                counters["entries"] += 1
                if counters["entries"] > MAX_ENTRIES:
                    raise ValueError("workspace exceeds inventory entry limit")
                path = prefix + entry.name
                if not relative(path):
                    raise ValueError("workspace has an unsupported file name")
                info = entry.stat(follow_symlinks=False)
                if is_link(info):
                    continue
                is_dir = stat.S_ISDIR(info.st_mode)
                if entry.name == ".git" or (is_dir and entry.name in SKIP):
                    continue
                if not (is_dir or stat.S_ISREG(info.st_mode)):
                    continue
                entries.append(dict(path=path, is_dir=is_dir, size=info.st_size, git_match="none"))
                checks.append((path, stamp(info)))
                if repo is not None:
                    owners.setdefault(repo, []).append(entries[-1])
                if is_dir:
                    child = directory(entry.name, fd)
                    try:
                        if stamp(directory_info(child)) != stamp(info):
                            raise ValueError("workspace directory changed during inventory")
                        visit(child, path + "/", repo, depth + 1)
                    finally:
                        close_directory(child)
        if stamp(directory_info(fd)) != stamp(before):
            raise ValueError("workspace directory changed during inventory")

    root_fd = open_root(root)
    try:
        root_stamp = stamp(directory_info(root_fd))
        visit(root_fd, "", inherited, 0)
    finally:
        close_directory(root_fd)

    env = dict(os.environ)
    # check-ignore accepts pathnames, not literal/glob pathspec magic.
    for key in ("GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_LITERAL_PATHSPECS",
                "GIT_GLOB_PATHSPECS", "GIT_NOGLOB_PATHSPECS", "GIT_ICASE_PATHSPECS"):
        env.pop(key, None)
    for repo, grouped in owners.items():
        check_time()
        paths = {root.rstrip("/") + "/" + entry["path"]: entry for entry in grouped}
        result = subprocess.run(
            ["git", "-c", "core.fsmonitor=false", "-C", repo, "check-ignore",
             "--no-index", "--verbose", "--non-matching", "-z", "--stdin"],
            input=b"".join(path.encode("utf-8") + b"\0" for path in paths),
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env,
            timeout=max(0.1, min(5, DEADLINE - time.monotonic())),
        )
        if result.returncode not in (0, 1):
            raise ValueError("Git exclusion evaluation failed: " + result.stderr[:2048].decode("utf-8", "replace"))
        fields = result.stdout.split(b"\0")
        if fields[-1:] != [b""] or len(fields[:-1]) != len(paths) * 4:
            raise ValueError("Git returned an incomplete exclusion result")
        seen = set()
        for pos in range(0, len(fields) - 1, 4):
            pattern, path = fields[pos + 2], fields[pos + 3].decode("utf-8")
            if path not in paths or path in seen:
                raise ValueError("Git returned an unexpected path")
            seen.add(path)
            paths[path]["git_match"] = "none" if not pattern else "allow" if pattern.startswith(b"!") else "ignore"

    entries.sort(key=lambda entry: entry["path"])
    rules.sort(key=lambda rule: (rule["directory"], rule["name"]))
    inventory = dict(root=root, entries=entries, rules=rules)
    check_time()
    data = encode(dict(inventory=inventory, root_stamp=root_stamp, checks=sorted(checks)))
    if len(data) > MAX_MANIFEST_BYTES:
        raise ValueError("workspace inventory exceeds size limit")
    return inventory, hashlib.sha256(data).hexdigest()


def read_batch(root, paths, fingerprint):
    if not isinstance(paths, list) or len(paths) > 128 or len(set(paths)) != len(paths):
        raise ValueError("invalid selected file batch")
    inventory, current = collect(root)
    if current != fingerprint:
        raise ValueError("workspace changed; start a new search")
    known = {entry["path"]: entry for entry in inventory["entries"] if not entry["is_dir"]}
    output, total = [], 0
    root_fd = open_root(root)
    try:
        for path in paths:
            check_time()
            if not relative(path) or path not in known or known[path]["size"] > MAX_FILE_BYTES:
                raise ValueError("selected file is outside the inventory or size limit")
            fd = duplicate_directory(root_fd)
            try:
                parts = path.split("/")
                for part in parts[:-1]:
                    child = directory(part, fd)
                    close_directory(fd)
                    fd = child
                info = info_at(fd, parts[-1])
                if info is None or not stat.S_ISREG(info.st_mode):
                    raise ValueError("selected file is no longer a physical file")
                body = read_regular(fd, parts[-1], info, MAX_FILE_BYTES)
                total += len(body)
                if total > MAX_BATCH_BYTES:
                    raise ValueError("selected batch exceeds size limit")
                output.append(dict(path=path, data=base64.b64encode(body).decode("ascii")))
            finally:
                close_directory(fd)
    finally:
        close_directory(root_fd)
    _, after = collect(root)
    if after != fingerprint:
        raise ValueError("workspace changed during collection; start a new search")
    return output


def response_page(result, request):
    payload = encode(result)
    digest = hashlib.sha256(payload).hexdigest()
    offset = request["offset"]
    expected = request["response_sha256"]
    if (type(offset) is not int or not 0 <= offset < len(payload)
            or offset % PAGE_BYTES != 0 or len(payload) > MAX_RESPONSE_BYTES):
        raise ValueError("invalid workspace response offset or size")
    if (offset == 0 and expected is not None) or (offset > 0 and expected != digest):
        raise ValueError("workspace response changed between pages; start a new search")
    return dict(offset=offset, total=len(payload), sha256=digest,
                data=base64.b64encode(payload[offset:offset + PAGE_BYTES]).decode("ascii"))


def main():
    try:
        if sys.version_info < (3, 9):
            raise ValueError("Semble workspace collection requires Python 3.9+")
        if len(sys.argv) != 2:
            raise ValueError("expected one encoded collection request")
        request = json.loads(base64.b64decode(sys.argv[1], validate=True))
        root = request["root"]
        if request["operation"] == "inventory":
            inventory, fingerprint = collect(root)
            result = dict(ok=True, inventory=inventory, fingerprint=fingerprint)
        elif request["operation"] == "read":
            bodies = read_batch(root, request["paths"], request["fingerprint"])
            result = dict(ok=True, files=bodies, fingerprint=request["fingerprint"])
        else:
            raise ValueError("unknown collection operation")
        sys.stdout.buffer.write(encode(response_page(result, request)) + b"\n")
        return 0
    except (ValueError, OSError, KeyError, TypeError, UnicodeError, subprocess.SubprocessError) as error:
        sys.stdout.buffer.write(encode(dict(error=str(error))) + b"\n")
        sys.stderr.write(str(error) + "\n")
        return 2


if __name__ == "__main__":
    sys.exit(main())

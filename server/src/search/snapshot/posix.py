"""Descriptor-relative POSIX access; never follows symlinks."""
import os
import stat

def directory(name, parent=None):
    return os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent)


def open_root(root):
    if not isinstance(root, str) or not root.startswith("/"):
        raise ValueError("Semble requires an absolute POSIX workspace path")
    if root != "/" and not relative(root[1:]):
        raise ValueError("invalid workspace path")
    fd = directory("/")
    try:
        for part in filter(None, root.split("/")):
            child = directory(part, fd)
            os.close(fd)
            fd = child
        return fd
    except BaseException:
        os.close(fd)
        raise


def info_at(fd, name):
    try:
        return os.stat(name, dir_fd=fd, follow_symlinks=False)
    except FileNotFoundError:
        return None


def read_regular(fd, name, expected, limit):
    opened = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=fd)
    try:
        actual = os.fstat(opened)
        if not stat.S_ISREG(actual.st_mode) or stamp(actual) != stamp(expected):
            raise ValueError("source changed before reading")
        with os.fdopen(opened, "rb", closefd=False) as reader:
            body = reader.read(limit + 1)
        if len(body) > limit or len(body) != expected.st_size or stamp(os.fstat(opened)) != stamp(expected):
            raise ValueError("source changed or exceeded the read limit")
        return body
    finally:
        os.close(opened)



def close_directory(fd):
    os.close(fd)


def directory_info(fd):
    return os.fstat(fd)


def duplicate_directory(fd):
    return os.dup(fd)


def scan_directory(fd):
    return os.scandir(fd)

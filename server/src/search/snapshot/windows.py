"""Handle-validated Windows access. No reparse traversal or source writes."""
import ctypes
from ctypes import wintypes as w
import os
import stat
from types import SimpleNamespace

kernel = ctypes.WinDLL("kernel32", use_last_error=True)


class FileInfo(ctypes.Structure):
    _fields_ = [("attributes", w.DWORD), ("creation", w.FILETIME),
                ("access", w.FILETIME), ("write", w.FILETIME),
                ("volume", w.DWORD), ("size_high", w.DWORD), ("size_low", w.DWORD),
                ("links", w.DWORD), ("index_high", w.DWORD), ("index_low", w.DWORD)]


class BasicInfo(ctypes.Structure):
    _fields_ = [("creation", ctypes.c_longlong), ("access", ctypes.c_longlong),
                ("write", ctypes.c_longlong), ("change", ctypes.c_longlong),
                ("attributes", w.DWORD)]


kernel.CreateFileW.argtypes = [w.LPCWSTR, w.DWORD, w.DWORD, w.LPVOID, w.DWORD, w.DWORD, w.HANDLE]
kernel.CreateFileW.restype = w.HANDLE
kernel.CloseHandle.argtypes = [w.HANDLE]
kernel.CloseHandle.restype = w.BOOL
kernel.GetFileInformationByHandle.argtypes = [w.HANDLE, ctypes.POINTER(FileInfo)]
kernel.GetFileInformationByHandle.restype = w.BOOL
kernel.GetFileInformationByHandleEx.argtypes = [w.HANDLE, ctypes.c_int, w.LPVOID, w.DWORD]
kernel.GetFileInformationByHandleEx.restype = w.BOOL
kernel.GetFinalPathNameByHandleW.argtypes = [w.HANDLE, w.LPWSTR, w.DWORD, w.DWORD]
kernel.GetFinalPathNameByHandleW.restype = w.DWORD
kernel.ReadFile.argtypes = [w.HANDLE, w.LPVOID, w.DWORD, ctypes.POINTER(w.DWORD), w.LPVOID]
kernel.ReadFile.restype = w.BOOL


def handle_info(handle):
    info, basic = FileInfo(), BasicInfo()
    if not kernel.GetFileInformationByHandle(handle, ctypes.byref(info)):
        raise ctypes.WinError(ctypes.get_last_error())
    if not kernel.GetFileInformationByHandleEx(handle, 0, ctypes.byref(basic), ctypes.sizeof(basic)):
        raise ctypes.WinError(ctypes.get_last_error())
    mode = stat.S_IFLNK if info.attributes & 0x400 else stat.S_IFDIR if info.attributes & 0x10 else stat.S_IFREG
    return SimpleNamespace(st_dev=info.volume, st_ino=(info.index_high << 32) | info.index_low,
                           st_size=(info.size_high << 32) | info.size_low,
                           st_mtime_ns=basic.write * 100, st_ctime_ns=basic.change * 100,
                           st_mode=mode, st_file_attributes=info.attributes)


def normalized(path):
    if path.startswith("\\\\?\\UNC\\"):
        path = "\\\\" + path[8:]
    elif path.startswith("\\\\?\\"):
        path = path[4:]
    return os.path.normcase(os.path.normpath(path))


def open_handle(path, read=False):
    absolute = os.path.normpath(path)
    extended = "\\\\?\\UNC\\" + absolute[2:] if absolute.startswith("\\\\") else "\\\\?\\" + absolute
    handle = kernel.CreateFileW(extended, 0x80000000 if read else 0x80, 7, None,
                                3, 0x02000000 | 0x00200000, None)
    if handle == w.HANDLE(-1).value:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        # Expose a final-component link as metadata so callers can skip it.
        # Directory and body readers reject this type before accessing content.
        if handle_info(handle).st_file_attributes & 0x400:
            return handle
        buffer = ctypes.create_unicode_buffer(32768)
        size = kernel.GetFinalPathNameByHandleW(handle, buffer, len(buffer), 0)
        if not size or size >= len(buffer):
            raise ctypes.WinError(ctypes.get_last_error())
        # The final name must equal the requested physical path. In particular,
        # a junction in any ancestor cannot redirect a read to another tree.
        if normalized(buffer.value) != normalized(absolute):
            raise ValueError("workspace path traverses a reparse point")
        return handle
    except BaseException:
        kernel.CloseHandle(handle)
        raise


class Directory:
    def __init__(self, path):
        self.path = path
        self.handle = open_handle(path)
        try:
            if not stat.S_ISDIR(handle_info(self.handle).st_mode):
                raise ValueError("workspace directory is a link or not a directory")
        except BaseException:
            kernel.CloseHandle(self.handle)
            raise


def open_root(root):
    drive, tail = os.path.splitdrive(root)
    if not drive or (not tail and not drive.startswith(("//", "\\\\"))) or (tail and (not tail.startswith(("/", "\\")) or (tail not in ("/", "\\") and not relative(tail[1:])))):
        raise ValueError("Semble requires an absolute Windows workspace path")
    return Directory(root)


def directory(name, parent):
    return Directory(os.path.join(parent.path, name))


def close_directory(directory):
    kernel.CloseHandle(directory.handle)


def directory_info(directory):
    return handle_info(directory.handle)


def duplicate_directory(directory):
    copy = Directory(directory.path)
    if stamp(directory_info(copy)) != stamp(directory_info(directory)):
        close_directory(copy)
        raise ValueError("workspace directory changed")
    return copy


def info_at(directory, name):
    try:
        handle = open_handle(os.path.join(directory.path, name))
    except FileNotFoundError:
        return None
    try:
        return handle_info(handle)
    finally:
        kernel.CloseHandle(handle)


def scan_directory(directory):
    # DirEntry.stat is replaced with handle metadata, so all platforms use the
    # same identity/change checks before a source body is opened.
    class Entry:
        def __init__(self, name):
            self.name = name

        def stat(self, follow_symlinks=False):
            info = info_at(directory, self.name)
            if info is None:
                raise ValueError("workspace entry disappeared during collection")
            return info

    class Scan:
        def __enter__(self):
            self.iterator = os.scandir(directory.path)
            return (Entry(entry.name) for entry in self.iterator)

        def __exit__(self, *args):
            self.iterator.close()

    return Scan()


def read_regular(directory, name, expected, limit):
    handle = open_handle(os.path.join(directory.path, name), read=True)
    try:
        actual = handle_info(handle)
        if not stat.S_ISREG(actual.st_mode) or stamp(actual) != stamp(expected):
            raise ValueError("source changed before reading")
        buffer, count = ctypes.create_string_buffer(limit + 1), w.DWORD()
        if not kernel.ReadFile(handle, buffer, len(buffer), ctypes.byref(count), None):
            raise ctypes.WinError(ctypes.get_last_error())
        if count.value != expected.st_size or count.value > limit or stamp(handle_info(handle)) != stamp(expected):
            raise ValueError("source changed or exceeded the read limit")
        return buffer.raw[:count.value]
    finally:
        kernel.CloseHandle(handle)

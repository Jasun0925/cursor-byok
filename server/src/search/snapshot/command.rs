//! Transport the same collector through the workspace host's native shell.

use std::io::Write;

use base64::{engine::general_purpose::STANDARD, Engine};
use flate2::{write::ZlibEncoder, Compression};
use serde_json::Value;

pub(super) fn build(root: &str, request: &Value) -> Result<String, String> {
    let windows = windows_root(root);
    let access = if windows {
        include_str!("windows.py")
    } else {
        include_str!("posix.py")
    };
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(access.as_bytes())
        .map_err(|e| e.to_string())?;
    encoder.write_all(b"\n").map_err(|e| e.to_string())?;
    encoder
        .write_all(include_bytes!("collect.py"))
        .map_err(|e| e.to_string())?;
    let script = STANDARD.encode(encoder.finish().map_err(|e| e.to_string())?);
    let request = STANDARD.encode(serde_json::to_vec(request).map_err(|e| e.to_string())?);
    let program = "import base64,sys,zlib;source=zlib.decompress(base64.b64decode(sys.argv.pop(1)));exec(compile(source,'semble-collect','exec'))";
    // Collection runs inside the source tree, whose Python modules must never
    // become collector dependencies. Both interpreter startup and collection
    // use only the standard library, without PYTHON* paths or site hooks.
    let command = if windows {
        // EncodedCommand avoids the configured outer shell's quote rules.
        // Payload and JSON output are ASCII, including non-ASCII path names.
        let powershell = format!(
            "$ErrorActionPreference='Stop'; \
             foreach ($name in @('python3','python')) {{ \
             $exe=Get-Command $name -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1; \
             if ($null -ne $exe -and $exe.Source -notlike '*\\WindowsApps\\*') {{ \
             & $exe.Source -I -S -c 'import sys;sys.exit(sys.version_info < (3,9))'; \
             if ($LASTEXITCODE -eq 0) {{ & $exe.Source -I -S -c \"{program}\" '{script}' '{request}'; exit $LASTEXITCODE }} }} }}; \
             [Console]::Error.WriteLine('Semble requires Python 3.9+ on the Cursor workspace host'); exit 127"
        );
        let encoded = STANDARD.encode(
            powershell
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>(),
        );
        format!("powershell.exe -NoProfile -NonInteractive -EncodedCommand {encoded}")
    } else {
        format!(
            "for semble_python in python3 python3.13 python3.12 python3.11 python3.10 python3.9; do \
             if command -v \"$semble_python\" >/dev/null 2>&1 && \
             \"$semble_python\" -I -S -c 'import sys;sys.exit(sys.version_info < (3,9))' >/dev/null 2>&1; then \
             exec \"$semble_python\" -I -S -c \"{program}\" '{script}' '{request}'; fi; done; \
             printf '%s\\n' 'Semble requires Python 3.9+ on the Cursor workspace host' >&2; exit 127"
        )
    };
    if command.len() > if windows { 32_000 } else { 60_000 } {
        return Err("Semble collection command exceeds the transport size limit".into());
    }
    Ok(command)
}

pub(crate) fn windows_root(root: &str) -> bool {
    root.starts_with("//")
        || root.starts_with("\\\\")
        || (root.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
            && root.as_bytes().get(1) == Some(&b':'))
}

pub(crate) fn normalize_root(root: &str) -> String {
    let root = if windows_root(root) {
        root.replace('\\', "/")
    } else {
        root.into()
    };
    let trimmed = root.trim_end_matches('/');
    if trimmed.is_empty() && root == "/" {
        root
    } else if trimmed.len() == 2 && trimmed.ends_with(':') && root.ends_with('/') {
        format!("{trimmed}/")
    } else {
        trimmed.into()
    }
}

pub(crate) fn valid_root(root: &str) -> bool {
    let tail = if let Some(tail) = root.strip_prefix("//") {
        if tail.split('/').count() < 2 {
            return false;
        }
        tail
    } else if windows_root(root) {
        let Some(tail) = root
            .get(3..)
            .filter(|_| root.as_bytes().get(2) == Some(&b'/'))
        else {
            return false;
        };
        if tail.is_empty() {
            return true;
        }
        tail
    } else if let Some(tail) = root.strip_prefix('/') {
        if tail.is_empty() {
            return true;
        }
        tail
    } else {
        return false;
    };
    root.len() <= 4096
        && !tail.contains(['\\', '\0'])
        && tail.split('/').all(|p| !matches!(p, "" | "." | ".."))
}

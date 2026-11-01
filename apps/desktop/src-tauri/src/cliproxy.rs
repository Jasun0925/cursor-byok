use std::{
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

const PROXY_PORT: u16 = 8317;
const MAX_LOG_LINES: usize = 400;
const MAX_LOG_CHARS: usize = 4_000;
const LOG_EVENT: &str = "cliproxy-log";
const STATUS_EVENT: &str = "cliproxy-status";

#[derive(Clone, Serialize)]
pub struct CliproxyStatus {
    owned: bool,
    listening: bool,
    logs: Vec<String>,
    seq: u64,
    notice: Option<String>,
    notice_detail: Option<String>,
}

#[derive(Clone, Serialize)]
struct LogEvent {
    seq: u64,
    line: String,
}

#[derive(Default)]
struct LogBuffer {
    seq: u64,
    lines: Vec<String>,
}

impl LogBuffer {
    fn push(&mut self, line: String) -> Option<LogEvent> {
        let line = truncate_line(line);
        if line.is_empty() {
            return None;
        }
        self.seq += 1;
        self.lines.push(line.clone());
        if self.lines.len() > MAX_LOG_LINES {
            let overflow = self.lines.len() - MAX_LOG_LINES;
            self.lines.drain(0..overflow);
        }
        Some(LogEvent {
            seq: self.seq,
            line,
        })
    }
}

struct Inner {
    child: Mutex<Option<Child>>,
    logs: Mutex<LogBuffer>,
    generation: AtomicU64,
}

pub struct CliproxySupervisor {
    inner: Arc<Inner>,
}

impl CliproxySupervisor {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                child: Mutex::new(None),
                logs: Mutex::new(LogBuffer::default()),
                generation: AtomicU64::new(0),
            }),
        }
    }

    pub fn snapshot(&self) -> CliproxyStatus {
        let owned = self.running_child();
        let listening = owned || proxy_port_open();
        let logs = self.inner.logs.lock().expect("cliproxy logs");
        CliproxyStatus {
            owned,
            listening,
            logs: logs.lines.clone(),
            seq: logs.seq,
            notice: (!owned && listening).then(|| "external".to_string()),
            notice_detail: None,
        }
    }

    pub fn start(&self, app: &AppHandle, directory: &str) -> CliproxyStatus {
        if self.running_child() {
            return self.snapshot();
        }
        let directory = match validate_launch_directory(directory) {
            Ok(path) => path,
            Err(notice) => return self.with_notice(notice, None),
        };
        if proxy_port_open() {
            return self.snapshot();
        }

        let binary = directory.join(binary_file_name());
        let (child, output) = match spawn_proxy(&binary, &directory) {
            Ok(spawned) => spawned,
            Err(error) => return self.with_notice("spawn-failed", Some(error.to_string())),
        };
        let generation = {
            let mut slot = self.inner.child.lock().expect("cliproxy child");
            if let Some(mut existing) = slot.take() {
                if running(&mut existing) {
                    *slot = Some(existing);
                    drop(slot);
                    terminate(child);
                    return self.snapshot();
                }
                wait_for(existing);
            }
            *slot = Some(child);
            self.inner.generation.fetch_add(1, Ordering::AcqRel) + 1
        };

        self.push(
            app,
            format!("已在 {} 启动 cli-proxy-api", directory.display()),
        );
        self.spawn_output(app, output);
        self.spawn_watcher(app.clone(), generation);
        self.snapshot()
    }

    pub fn stop_owned(&self, app: &AppHandle) -> bool {
        let child = {
            let mut slot = self.inner.child.lock().expect("cliproxy child");
            self.inner.generation.fetch_add(1, Ordering::AcqRel);
            slot.take()
        };
        let Some(child) = child else {
            return false;
        };
        terminate(child);
        self.push(app, "已停止本应用启动的 CLIProxyAPI".to_string());
        let _ = app.emit(STATUS_EVENT, ());
        true
    }

    pub fn stop(&self, app: &AppHandle) -> CliproxyStatus {
        self.stop_owned(app);
        if !proxy_port_open() {
            return self.snapshot();
        }
        self.stop_external(app)
    }

    fn stop_external(&self, app: &AppHandle) -> CliproxyStatus {
        let listeners = match list_listeners() {
            Ok(listeners) => listeners,
            Err(error) => return self.with_notice("stop-failed", Some(error.to_string())),
        };
        let targets: Vec<Listener> = listeners
            .into_iter()
            .filter(|listener| is_cli_proxy(&listener.command))
            .collect();
        if targets.is_empty() {
            if proxy_port_open() {
                return self.with_notice("not-cliproxy", None);
            }
            return self.snapshot();
        }
        for listener in &targets {
            let _ = signal_pid(listener.pid, terminate_signal());
        }
        for _ in 0..20 {
            if !proxy_port_open() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        if proxy_port_open() {
            for listener in &targets {
                let _ = signal_pid(listener.pid, kill_signal());
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        if proxy_port_open() {
            return self.with_notice(
                "stop-failed",
                Some("进程仍在监听 127.0.0.1:8317".to_string()),
            );
        }
        self.push(
            app,
            "已停止正在监听 127.0.0.1:8317 的 CLIProxyAPI".to_string(),
        );
        let _ = app.emit(STATUS_EVENT, ());
        self.snapshot()
    }

    fn with_notice(&self, notice: &str, detail: Option<String>) -> CliproxyStatus {
        let mut status = self.snapshot();
        status.notice = Some(notice.to_string());
        status.notice_detail = detail;
        status
    }

    fn running_child(&self) -> bool {
        let mut slot = self.inner.child.lock().expect("cliproxy child");
        slot.as_mut().is_some_and(running)
    }

    fn push(&self, app: &AppHandle, line: String) {
        push_line(&self.inner, app, line);
    }

    fn spawn_output(&self, app: &AppHandle, output: ProxyOutput) {
        match output {
            ProxyOutput::Pty { master } => {
                spawn_pump(app.clone(), Arc::clone(&self.inner), master, false);
            }
            ProxyOutput::Pipes { stdout, stderr } => {
                spawn_pump(app.clone(), Arc::clone(&self.inner), stdout, false);
                spawn_pump(app.clone(), Arc::clone(&self.inner), stderr, true);
            }
        }
    }

    fn spawn_watcher(&self, app: AppHandle, generation: u64) {
        let inner = Arc::clone(&self.inner);
        let _ = std::thread::Builder::new()
            .name("cliproxy-watch".to_string())
            .spawn(move || watch_child(inner, app, generation));
    }
}

fn watch_child(inner: Arc<Inner>, app: AppHandle, generation: u64) {
    loop {
        std::thread::sleep(Duration::from_millis(400));
        let mut slot = inner.child.lock().expect("cliproxy child");
        if inner.generation.load(Ordering::Acquire) != generation {
            break;
        }
        let Some(child) = slot.as_mut() else {
            break;
        };
        match child.try_wait() {
            Ok(None) => {}
            Ok(Some(status)) => {
                *slot = None;
                drop(slot);
                if inner.generation.load(Ordering::Acquire) == generation {
                    push_line(&inner, &app, format!("CLIProxyAPI 已退出（{status}）"));
                    let _ = app.emit(STATUS_EVENT, ());
                }
                break;
            }
            Err(error) => {
                *slot = None;
                drop(slot);
                if inner.generation.load(Ordering::Acquire) == generation {
                    push_line(
                        &inner,
                        &app,
                        format!("无法读取 CLIProxyAPI 进程状态：{error}"),
                    );
                    let _ = app.emit(STATUS_EVENT, ());
                }
                break;
            }
        }
    }
}

fn spawn_pump(app: AppHandle, inner: Arc<Inner>, reader: impl Read + Send + 'static, stderr: bool) {
    let _ = std::thread::Builder::new()
        .name(
            if stderr {
                "cliproxy-stderr"
            } else {
                "cliproxy-stdout"
            }
            .to_string(),
        )
        .spawn(move || pump(reader, inner, app, stderr));
}

fn pump(reader: impl Read, inner: Arc<Inner>, app: AppHandle, stderr: bool) {
    let mut reader = BufReader::new(reader);
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        match reader.read_until(b'\n', &mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                if buffer.last() == Some(&b'\n') {
                    buffer.pop();
                }
                if buffer.last() == Some(&b'\r') {
                    buffer.pop();
                }
                let mut line = String::from_utf8_lossy(&buffer).into_owned();
                if stderr {
                    line = format!("stderr: {line}");
                }
                push_line(&inner, &app, line);
            }
        }
    }
}

fn push_line(inner: &Inner, app: &AppHandle, line: String) {
    let event = {
        let mut logs = inner.logs.lock().expect("cliproxy logs");
        logs.push(line)
    };
    if let Some(event) = event {
        let _ = app.emit(LOG_EVENT, event);
    }
}

fn running(child: &mut Child) -> bool {
    matches!(child.try_wait(), Ok(None))
}

fn wait_for(mut child: Child) {
    let _ = child.wait();
}

fn terminate(mut child: Child) {
    #[cfg(unix)]
    {
        let pid = child.id() as libc::pid_t;
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn proxy_port_open() -> bool {
    let address = std::net::SocketAddr::from(([127, 0, 0, 1], PROXY_PORT));
    std::net::TcpStream::connect_timeout(&address, Duration::from_millis(200)).is_ok()
}

struct Listener {
    pid: u32,
    command: String,
}

fn is_cli_proxy(command: &str) -> bool {
    let name = command.rsplit(['/', '\\']).next().unwrap_or(command);
    name == "cli-proxy-api" || name == "cli-proxy-api.exe"
}

fn parse_lsof_listeners(output: &str) -> Vec<Listener> {
    let mut listeners: Vec<Listener> = Vec::new();
    let mut pid = None;
    for line in output.lines() {
        let Some(id) = line.chars().next() else {
            continue;
        };
        let value = &line[id.len_utf8()..];
        match id {
            'p' => pid = value.parse().ok(),
            'c' => {
                if let Some(pid) = pid.take() {
                    if !listeners.iter().any(|listener| listener.pid == pid) {
                        listeners.push(Listener {
                            pid,
                            command: value.to_string(),
                        });
                    }
                }
            }
            _ => {}
        }
    }
    listeners
}

fn list_listeners() -> std::io::Result<Vec<Listener>> {
    #[cfg(unix)]
    {
        let output = Command::new("lsof")
            .args(["-nP", "-iTCP:8317", "-sTCP:LISTEN", "-Fpc"])
            .output()?;
        if output.stdout.is_empty() {
            Ok(Vec::new())
        } else {
            Ok(parse_lsof_listeners(&String::from_utf8_lossy(
                &output.stdout,
            )))
        }
    }
    #[cfg(not(unix))]
    {
        Ok(Vec::new())
    }
}

fn terminate_signal() -> i32 {
    #[cfg(unix)]
    {
        libc::SIGTERM
    }
    #[cfg(not(unix))]
    {
        15
    }
}

fn kill_signal() -> i32 {
    #[cfg(unix)]
    {
        libc::SIGKILL
    }
    #[cfg(not(unix))]
    {
        9
    }
}

fn signal_pid(pid: u32, signal: i32) -> std::io::Result<()> {
    if pid <= 1 {
        return Err(std::io::Error::other("refusing to signal a system process"));
    }
    signal_process(pid, signal)
}

#[cfg(unix)]
fn signal_process(pid: u32, signal: i32) -> std::io::Result<()> {
    let result = unsafe { libc::kill(pid as libc::pid_t, signal) };
    if result == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(error)
    }
}

#[cfg(not(unix))]
fn signal_process(pid: u32, signal: i32) -> std::io::Result<()> {
    let _ = (pid, signal);
    Err(std::io::Error::other(
        "stopping a process that this app did not start is only supported on macOS and Linux",
    ))
}

fn binary_file_name() -> &'static str {
    if cfg!(windows) {
        "cli-proxy-api.exe"
    } else {
        "cli-proxy-api"
    }
}

fn validate_launch_directory(directory: &str) -> Result<PathBuf, &'static str> {
    let trimmed = directory.trim();
    if trimmed.is_empty() {
        return Err("empty-directory");
    }
    let path = Path::new(trimmed)
        .canonicalize()
        .map_err(|_| "missing-directory")?;
    if !path.is_dir() {
        return Err("missing-directory");
    }
    if !path.join(binary_file_name()).is_file() {
        return Err("missing-binary");
    }
    Ok(path)
}

fn truncate_line(line: String) -> String {
    if line.chars().count() <= MAX_LOG_CHARS {
        return line;
    }
    let mut shortened: String = line.chars().take(MAX_LOG_CHARS).collect();
    shortened.push('…');
    shortened
}

enum ProxyOutput {
    Pty {
        master: std::fs::File,
    },
    Pipes {
        stdout: std::process::ChildStdout,
        stderr: std::process::ChildStderr,
    },
}

fn spawn_proxy(binary: &Path, directory: &Path) -> std::io::Result<(Child, ProxyOutput)> {
    #[cfg(unix)]
    if let Ok(pty) = open_pty() {
        return spawn_proxy_on_pty(binary, directory, pty);
    }
    spawn_proxy_pipes(binary, directory)
}

fn spawn_proxy_pipes(binary: &Path, directory: &Path) -> std::io::Result<(Child, ProxyOutput)> {
    let mut command = Command::new(binary);
    command
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    assign_process_group(&mut command);
    let mut child = command.spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| std::io::Error::other("cli-proxy-api did not provide stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| std::io::Error::other("cli-proxy-api did not provide stderr"))?;
    Ok((child, ProxyOutput::Pipes { stdout, stderr }))
}

#[cfg(unix)]
fn spawn_proxy_on_pty(
    binary: &Path,
    directory: &Path,
    pty: OpenPty,
) -> std::io::Result<(Child, ProxyOutput)> {
    use std::os::fd::FromRawFd;

    let mut command = Command::new(binary);
    command
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(unsafe { Stdio::from_raw_fd(pty.slave) })
        .stderr(unsafe { Stdio::from_raw_fd(pty.stderr) });
    assign_process_group(&mut command);
    let child = command.spawn()?;
    Ok((child, ProxyOutput::Pty { master: pty.master }))
}

#[cfg(unix)]
struct OpenPty {
    master: std::fs::File,
    slave: std::os::fd::RawFd,
    stderr: std::os::fd::RawFd,
}

#[cfg(unix)]
fn open_pty() -> std::io::Result<OpenPty> {
    use std::os::fd::FromRawFd;

    let mut master = 0;
    let mut slave = 0;
    let mut size = libc::winsize {
        ws_row: 50,
        ws_col: 200,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let opened = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut size,
        )
    };
    if opened != 0 {
        return Err(std::io::Error::last_os_error());
    }
    unsafe {
        libc::fcntl(master, libc::F_SETFD, libc::FD_CLOEXEC);
    }
    let master = unsafe { std::fs::File::from_raw_fd(master) };
    let stderr = unsafe { libc::dup(slave) };
    if stderr < 0 {
        unsafe { libc::close(slave) };
        return Err(std::io::Error::last_os_error());
    }
    Ok(OpenPty {
        master,
        slave,
        stderr,
    })
}

fn assign_process_group(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            command.pre_exec(|| {
                if libc::setpgid(0, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    #[cfg(not(unix))]
    {
        let _ = command;
    }
}

#[tauri::command]
pub fn cliproxy_status(supervisor: State<'_, CliproxySupervisor>) -> CliproxyStatus {
    supervisor.snapshot()
}

#[tauri::command]
pub fn cliproxy_start(
    app: AppHandle,
    supervisor: State<'_, CliproxySupervisor>,
    directory: String,
) -> CliproxyStatus {
    supervisor.start(&app, &directory)
}

#[tauri::command]
pub fn cliproxy_stop(app: AppHandle, supervisor: State<'_, CliproxySupervisor>) -> CliproxyStatus {
    supervisor.stop(&app)
}

#[tauri::command]
pub fn cliproxy_pick_directory(current: Option<String>) -> Option<String> {
    let mut dialog = rfd::FileDialog::new().set_title("选择 CLIProxyAPI 目录");
    if let Some(current) = current
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        dialog = dialog.set_directory(current);
    }
    dialog
        .pick_folder()
        .map(|path| path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_directory_is_rejected() {
        assert_eq!(validate_launch_directory("  "), Err("empty-directory"));
    }

    #[test]
    fn missing_directory_is_rejected() {
        assert_eq!(
            validate_launch_directory("/this/path/does/not/exist/cliproxy-byok"),
            Err("missing-directory")
        );
    }

    #[test]
    fn directory_without_binary_is_rejected() {
        let directory = tempfile::tempdir().expect("temp dir");
        assert_eq!(
            validate_launch_directory(directory.path().to_str().expect("utf-8 path")),
            Err("missing-binary")
        );
    }

    #[test]
    fn directory_with_binary_is_accepted() {
        let directory = tempfile::tempdir().expect("temp dir");
        std::fs::write(directory.path().join(binary_file_name()), b"").expect("binary");
        let found = validate_launch_directory(directory.path().to_str().expect("utf-8 path"))
            .expect("directory");
        assert_eq!(found, directory.path().canonicalize().expect("canonical"));
    }

    #[test]
    fn log_buffer_keeps_the_latest_lines() {
        let mut logs = LogBuffer::default();
        for index in 0..(MAX_LOG_LINES + 5) {
            assert!(logs.push(format!("line {index}")).is_some());
        }
        assert_eq!(logs.lines.len(), MAX_LOG_LINES);
        assert_eq!(logs.lines[0], "line 5");
        assert_eq!(logs.seq, (MAX_LOG_LINES + 5) as u64);
        assert!(logs.push("   ".repeat(MAX_LOG_CHARS + 8)).is_some());
        assert_eq!(
            logs.lines.last().expect("line").chars().count(),
            MAX_LOG_CHARS + 1
        );
    }

    #[cfg(unix)]
    #[test]
    fn spawned_process_output_is_readable() {
        use std::os::fd::AsRawFd;
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().expect("temp dir");
        let binary = directory.path().join(binary_file_name());
        std::fs::write(&binary, "#!/bin/sh\nprintf 'hello-from-proxy\\n'\n").expect("script");
        let mut permissions = std::fs::metadata(&binary).expect("metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&binary, permissions).expect("permissions");

        let (child, output) = spawn_proxy(&binary, directory.path()).expect("spawn");
        let ProxyOutput::Pty { mut master } = output else {
            terminate(child);
            panic!("expected a pty");
        };
        let fd = master.as_raw_fd();
        unsafe {
            let flags = libc::fcntl(fd, libc::F_GETFL);
            libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
        }
        let mut collected = Vec::new();
        let started = std::time::Instant::now();
        while started.elapsed() < Duration::from_secs(2) {
            let mut buffer = [0u8; 256];
            match master.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => collected.extend_from_slice(&buffer[..count]),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(_) => break,
            }
            if String::from_utf8_lossy(&collected).contains("hello-from-proxy") {
                break;
            }
        }
        terminate(child);
        let text = String::from_utf8_lossy(&collected);
        assert!(text.contains("hello-from-proxy"), "pty output was {text:?}");
    }

    #[test]
    fn lsof_output_selects_only_cli_proxy() {
        let listeners = parse_lsof_listeners("p42\nccli-proxy-api\np7\ncnginx\n");
        let targets: Vec<u32> = listeners
            .iter()
            .filter(|listener| is_cli_proxy(&listener.command))
            .map(|listener| listener.pid)
            .collect();
        assert_eq!(targets, vec![42]);
        assert!(is_cli_proxy("/tmp/cli-proxy-api"));
        assert!(is_cli_proxy("cli-proxy-api.exe"));
        assert!(!is_cli_proxy("nginx"));
    }

    #[test]
    fn binary_name_matches_the_platform() {
        if cfg!(windows) {
            assert_eq!(binary_file_name(), "cli-proxy-api.exe");
        } else {
            assert_eq!(binary_file_name(), "cli-proxy-api");
        }
    }
}

use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

use cli4all::platform::Platform;

#[cfg(unix)]
use std::os::fd::{AsRawFd, FromRawFd};
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PtyOutputEvent {
    pub session_id: u64,
    pub data: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PtyExitEvent {
    pub session_id: u64,
}

#[derive(Clone)]
pub struct PtySession {
    writer: Arc<Mutex<File>>,
    child_pid: libc::pid_t,
    alive: Arc<AtomicBool>,
}

impl PtySession {
    #[cfg(unix)]
    pub fn start(
        app: AppHandle,
        session_id: u64,
        platform: Platform,
        cols: u16,
        rows: u16,
    ) -> Result<Self> {
        let shell_path = native_shell_path(platform);
        let shell_cstr = std::ffi::CString::new(shell_path.as_os_str().as_bytes())
            .context("shell path contains an interior null byte")?;
        let interactive_flag =
            std::ffi::CString::new("-i").expect("static interactive shell flag is valid");
        let term_key = std::ffi::CString::new("TERM").expect("static TERM key is valid");
        let term_value =
            std::ffi::CString::new("xterm-256color").expect("static TERM value is valid");

        let mut master_fd = -1;
        let mut winsize = libc::winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        let pid = unsafe {
            libc::forkpty(
                &mut master_fd,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut winsize,
            )
        };

        if pid < 0 {
            return Err(anyhow!("failed to create PTY session"));
        }

        if pid == 0 {
            unsafe {
                libc::setenv(term_key.as_ptr(), term_value.as_ptr(), 1);
                let argv = [
                    shell_cstr.as_ptr(),
                    interactive_flag.as_ptr(),
                    std::ptr::null(),
                ];
                libc::execvp(shell_cstr.as_ptr(), argv.as_ptr());
                libc::_exit(127);
            }
        }

        let writer = unsafe { File::from_raw_fd(master_fd) };
        let reader = writer
            .try_clone()
            .context("failed to clone PTY file descriptor")?;
        let alive = Arc::new(AtomicBool::new(true));

        spawn_reader_thread(app.clone(), session_id, reader, alive.clone());
        spawn_wait_thread(app, session_id, pid, alive.clone());

        Ok(Self {
            writer: Arc::new(Mutex::new(writer)),
            child_pid: pid,
            alive,
        })
    }

    #[cfg(not(unix))]
    pub fn start(
        _app: AppHandle,
        _session_id: u64,
        _platform: Platform,
        _cols: u16,
        _rows: u16,
    ) -> Result<Self> {
        bail!("PTY sessions are not implemented on this platform")
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    pub fn write_text(&self, text: &str) -> Result<()> {
        self.write_bytes(text.as_bytes())
    }

    pub fn write_bytes(&self, bytes: &[u8]) -> Result<()> {
        self.ensure_alive()?;
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| anyhow!("PTY writer lock is poisoned"))?;
        writer
            .write_all(bytes)
            .context("failed to write to PTY session")?;
        writer.flush().context("failed to flush PTY session")?;
        Ok(())
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        self.ensure_alive()?;
        #[cfg(unix)]
        {
        let writer = self
            .writer
            .lock()
            .map_err(|_| anyhow!("PTY writer lock is poisoned"))?;
        let winsize = libc::winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        let result =
            unsafe { libc::ioctl(writer.as_raw_fd(), libc::TIOCSWINSZ, &winsize as *const _) };
        if result != 0 {
            return Err(anyhow!("failed to resize PTY session"));
        }
        Ok(())
        }
        #[cfg(not(unix))]
        {
            let _ = (cols, rows);
            bail!("PTY resizing is not implemented on this platform")
        }
    }

    pub fn stop(&self) -> Result<()> {
        if !self.alive.swap(false, Ordering::SeqCst) {
            return Ok(());
        }

        #[cfg(unix)]
        {
        let result = unsafe { libc::kill(self.child_pid, libc::SIGHUP) };
        if result != 0 && std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH) {
            return Err(anyhow!("failed to stop PTY session"));
        }
        Ok(())
        }
        #[cfg(not(unix))]
        {
            Ok(())
        }
    }

    fn ensure_alive(&self) -> Result<()> {
        if self.is_alive() {
            Ok(())
        } else {
            bail!("PTY session is not running")
        }
    }
}

fn native_shell_path(platform: Platform) -> PathBuf {
    let env_shell = std::env::var_os("SHELL")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);

    match platform {
        Platform::Macos => env_shell.unwrap_or_else(|| PathBuf::from("/bin/zsh")),
        Platform::Ubuntu => env_shell.unwrap_or_else(|| PathBuf::from("/bin/bash")),
        Platform::Windows => PathBuf::from("powershell.exe"),
    }
}

fn spawn_reader_thread(
    app: AppHandle,
    session_id: u64,
    mut reader: File,
    alive: Arc<AtomicBool>,
) {
    std::thread::spawn(move || {
        let mut buffer = [0_u8; 4096];

        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => {
                    let payload = PtyOutputEvent {
                        session_id,
                        data: String::from_utf8_lossy(&buffer[..read]).into_owned(),
                    };
                    let _ = app.emit("pty-output", payload);
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }

        alive.store(false, Ordering::SeqCst);
    });
}

#[cfg(unix)]
fn spawn_wait_thread(
    app: AppHandle,
    session_id: u64,
    child_pid: libc::pid_t,
    alive: Arc<AtomicBool>,
) {
    std::thread::spawn(move || {
        let mut status = 0;
        let _ = unsafe { libc::waitpid(child_pid, &mut status, 0) };
        alive.store(false, Ordering::SeqCst);
        let _ = app.emit("pty-exit", PtyExitEvent { session_id });
    });
}

#[cfg(not(unix))]
fn spawn_wait_thread(
    _app: AppHandle,
    _session_id: u64,
    _child_pid: libc::pid_t,
    _alive: Arc<AtomicBool>,
) {
}

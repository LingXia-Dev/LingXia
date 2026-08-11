//! App/process lifecycle: list (`proc_listallpids`), kill (`kill(2)`), launch
//! (direct spawn or `open` for `.app` bundles), and graceful app quit
//! (`NSRunningApplication.terminate`, else `SIGTERM`).

use crate::error::{Error, Result};
use crate::model::{Ack, LaunchResult, ProcessInfo, QuitTarget, WindowQuery};
use objc2_app_kit::NSRunningApplication;

pub fn process_list(filter: Option<&str>) -> Result<Vec<ProcessInfo>> {
    // First call with a null buffer to size the pid array, then fetch.
    let cap = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
    if cap <= 0 {
        return Err(Error::Failed("proc_listallpids failed".into()));
    }
    // Over-allocate: processes can appear between the two calls.
    let mut pids = vec![0i32; cap as usize + 32];
    let bytes = (pids.len() * std::mem::size_of::<i32>()) as libc::c_int;
    let n = unsafe { libc::proc_listallpids(pids.as_mut_ptr() as *mut libc::c_void, bytes) };
    if n <= 0 {
        return Err(Error::Failed("proc_listallpids returned no pids".into()));
    }
    pids.truncate(n as usize);

    let mut out = Vec::new();
    for &pid in &pids {
        if pid <= 0 {
            continue;
        }
        let name = proc_name(pid);
        let keep = filter.is_none_or(|f| name.to_lowercase().contains(&f.to_lowercase()));
        if keep {
            out.push(ProcessInfo {
                pid: pid as u32,
                name,
            });
        }
    }
    Ok(out)
}

fn proc_name(pid: i32) -> String {
    let mut buf = [0u8; 256];
    let n =
        unsafe { libc::proc_name(pid, buf.as_mut_ptr() as *mut libc::c_void, buf.len() as u32) };
    if n <= 0 {
        return String::new();
    }
    String::from_utf8_lossy(&buf[..n as usize]).into_owned()
}

pub fn process_kill(pid: u32, force: bool) -> Result<Ack> {
    let sig = if force { libc::SIGKILL } else { libc::SIGTERM };
    let rc = unsafe { libc::kill(pid as libc::pid_t, sig) };
    if rc != 0 {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::ESRCH) {
            return Err(Error::NotFound(format!("no process {pid}")));
        }
        if err.raw_os_error() == Some(libc::EPERM) {
            return Err(Error::Permission(format!("not permitted to signal {pid}")));
        }
        return Err(Error::Failed(format!("kill {pid} failed: {err}")));
    }
    Ok(Ack::new("process.kill"))
}

pub fn app_launch(
    app: &str,
    args: &[String],
    wait_window: Option<&str>,
    timeout_ms: u64,
) -> Result<LaunchResult> {
    // `.app` bundles must go through `open`; a bundle's Info.plist executable is
    // not something we can exec directly. Everything else (CLI binaries, scripts,
    // absolute paths) spawns directly so we keep the real child pid.
    let child = if app.ends_with(".app") || app.ends_with(".app/") {
        let mut cmd = std::process::Command::new("/usr/bin/open");
        cmd.arg("-n").arg(app);
        if !args.is_empty() {
            cmd.arg("--args").args(args);
        }
        cmd.spawn()
    } else {
        std::process::Command::new(app).args(args).spawn()
    };
    let child = child.map_err(|e| Error::Failed(format!("could not launch '{app}': {e}")))?;
    let launcher_pid = child.id();

    // When asked to wait for a window, a timeout is a real failure — propagate it
    // (its exit code) rather than swallowing it into a bare pid.
    let window = match wait_window {
        Some(query) => {
            let q = WindowQuery::parse(query);
            Some(super::wait_window(&q, Some(true), timeout_ms)?)
        }
        None => None,
    };
    // The matched window's owning pid is the durable target: with `open`, the
    // launcher pid is already gone.
    let pid = window.as_ref().map(|w| w.pid).unwrap_or(launcher_pid);
    Ok(LaunchResult {
        pid,
        launcher_pid,
        window,
    })
}

fn quit_pid(target: &QuitTarget) -> Result<u32> {
    match target {
        QuitTarget::Pid(p) => Ok(*p),
        QuitTarget::Window(id) => {
            // Map the window id back to its owning process.
            let wid = super::parse_window_id(id)?;
            super::window_record(wid)
                .map(|w| w.pid)
                .ok_or_else(|| Error::Stale(format!("window {id} is not available")))
        }
        QuitTarget::Match(q) => {
            let wins = super::windows(q)?;
            match wins.len() {
                0 => Err(Error::NotFound("no window matched".into())),
                1 => Ok(wins[0].pid),
                n => Err(Error::Ambiguous(format!(
                    "{n} windows matched; use --pid or a narrower --match"
                ))),
            }
        }
    }
}

pub fn app_quit(target: QuitTarget, force: bool) -> Result<Ack> {
    let pid = quit_pid(&target)?;
    if force {
        return process_kill(pid, true).map(|_| Ack::new("app.quit"));
    }
    // Graceful: ask the GUI app to terminate; fall back to SIGTERM for a
    // non-GUI process.
    let running = NSRunningApplication::runningApplicationWithProcessIdentifier(pid as libc::pid_t);
    if let Some(app) = running
        && app.terminate()
    {
        return Ok(Ack::new("app.quit"));
    }
    process_kill(pid, false).map(|_| Ack::new("app.quit"))
}

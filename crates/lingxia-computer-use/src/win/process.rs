//! App/process lifecycle: launch (CreateProcess), list (ToolHelp), kill
//! (TerminateProcess), and graceful app quit (WM_CLOSE to a process's windows).

use super::parse_hwnd;
use crate::error::{Error, Result};
use crate::model::{Ack, LaunchResult, ProcessInfo, QuitTarget, WindowQuery};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    CreateProcessW, OpenProcess, PROCESS_CREATION_FLAGS, PROCESS_INFORMATION, PROCESS_TERMINATE,
    STARTUPINFOW, TerminateProcess,
};
use windows::Win32::UI::WindowsAndMessaging::{GetWindowThreadProcessId, PostMessageW, WM_CLOSE};
use windows::core::{PCWSTR, PWSTR};

pub fn process_list(filter: Option<&str>) -> Result<Vec<ProcessInfo>> {
    let mut out = Vec::new();
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
            .map_err(|e| Error::Failed(format!("process snapshot failed: {e}")))?;
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snap, &mut entry).is_ok() {
            loop {
                let name = {
                    let raw = &entry.szExeFile;
                    let len = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
                    String::from_utf16_lossy(&raw[..len])
                };
                let keep = filter.is_none_or(|f| name.to_lowercase().contains(&f.to_lowercase()));
                if keep {
                    out.push(ProcessInfo {
                        pid: entry.th32ProcessID,
                        name,
                    });
                }
                if Process32NextW(snap, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
    }
    Ok(out)
}

pub fn process_kill(pid: u32, _force: bool) -> Result<Ack> {
    unsafe {
        let handle: HANDLE = OpenProcess(PROCESS_TERMINATE, false, pid)
            .map_err(|_| Error::NotFound(format!("cannot open process {pid}")))?;
        let r = TerminateProcess(handle, 1);
        let _ = CloseHandle(handle);
        r.map_err(|e| Error::Failed(format!("terminate {pid} failed: {e}")))?;
    }
    Ok(Ack::new("process.kill"))
}

fn to_wide_nul(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn app_launch(
    app: &str,
    args: &[String],
    wait_window: Option<&str>,
    timeout_ms: u64,
) -> Result<LaunchResult> {
    super::ensure_dpi_aware();
    // Build one command line: quoted program, then args. Leaving the program
    // name in the command line lets CreateProcess resolve it via PATH.
    let mut command = if app.contains(' ') && !app.starts_with('"') {
        format!("\"{app}\"")
    } else {
        app.to_string()
    };
    for a in args {
        command.push(' ');
        if a.contains(' ') {
            command.push('"');
            command.push_str(a);
            command.push('"');
        } else {
            command.push_str(a);
        }
    }
    let mut cmd_buf = to_wide_nul(&command);

    let startup = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut info = PROCESS_INFORMATION::default();
    unsafe {
        CreateProcessW(
            PCWSTR::null(),
            Some(PWSTR(cmd_buf.as_mut_ptr())),
            None,
            None,
            false,
            PROCESS_CREATION_FLAGS(0),
            None,
            PCWSTR::null(),
            &startup,
            &mut info,
        )
        .map_err(|e| Error::Failed(format!("could not launch '{app}': {e}")))?;
        let _ = CloseHandle(info.hThread);
        let _ = CloseHandle(info.hProcess);
    }
    let launcher_pid = info.dwProcessId;

    // When the caller asked to wait for a window, a timeout is a real failure
    // (exit code follows the error) — don't swallow it into a bare pid.
    let window = match wait_window {
        Some(query) => {
            let q = WindowQuery::parse(query);
            Some(super::wait_window(&q, Some(true), timeout_ms)?)
        }
        None => None,
    };
    // Report the matched window's owning process as the durable pid: a
    // relauncher stub's CreateProcess pid may already be dead by now.
    let pid = window.as_ref().map(|w| w.pid).unwrap_or(launcher_pid);
    Ok(LaunchResult {
        pid,
        launcher_pid,
        window,
    })
}

/// Resolve a quit target to a pid.
fn quit_pid(target: &QuitTarget) -> Result<u32> {
    match target {
        QuitTarget::Pid(p) => Ok(*p),
        QuitTarget::Window(id) => {
            let hwnd = parse_hwnd(id)?;
            let mut pid = 0u32;
            unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
            if pid == 0 {
                Err(Error::Stale(format!("window {id} is not available")))
            } else {
                Ok(pid)
            }
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
    // Graceful: ask each of the process's top-level windows to close.
    let mut closed = 0;
    for win in super::windows(&WindowQuery::by_pid(pid))? {
        if let Ok(hwnd) = parse_hwnd(&win.id) {
            unsafe {
                let _ = PostMessageW(
                    Some(hwnd),
                    WM_CLOSE,
                    windows::Win32::Foundation::WPARAM(0),
                    windows::Win32::Foundation::LPARAM(0),
                );
            }
            closed += 1;
        }
    }
    if closed == 0 {
        return Err(Error::NotFound(format!(
            "process {pid} has no windows to close (use --force to terminate)"
        )));
    }
    Ok(Ack::new("app.quit"))
}

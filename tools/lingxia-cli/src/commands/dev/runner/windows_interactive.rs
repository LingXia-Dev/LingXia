use super::{process_executable_matches, quote_windows_arg};
use anyhow::{Context, Result, anyhow};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use windows::Win32::Foundation::{HANDLE, HWND, LPARAM, RECT, TRUE};
use windows::Win32::System::Threading::{
    AttachThreadInput, GetCurrentThreadId, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, BringWindowToTop, EnumWindows, GWL_EXSTYLE, GetForegroundWindow,
    GetWindowLongW, GetWindowRect, GetWindowThreadProcessId, HWND_NOTOPMOST, HWND_TOPMOST,
    IsIconic, IsWindowVisible, SW_RESTORE, SW_SHOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_SHOWWINDOW, SetForegroundWindow, SetWindowPos, ShowWindow, WS_EX_TOOLWINDOW,
};
use windows::core::BOOL;

const RUNNER_MARKER_ENV: &str = "LINGXIA_RUNNER";
const INTERACTIVE_START_TIMEOUT: Duration = Duration::from_secs(15);
const FOCUS_WINDOW_TIMEOUT: Duration = Duration::from_secs(15);

pub(super) fn is_ssh_session() -> bool {
    ssh_environment_present(
        std::env::var_os("SSH_CONNECTION").as_deref(),
        std::env::var_os("SSH_CLIENT").as_deref(),
        std::env::var_os("SSH_TTY").as_deref(),
    )
}

pub(super) fn focus_windows_process(pid: u32) -> Result<()> {
    let deadline = Instant::now() + FOCUS_WINDOW_TIMEOUT;
    loop {
        if let Some(hwnd) = find_process_main_window(pid)
            && activate_window(hwnd)
        {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(anyhow!(
                "Could not activate a visible window for Windows Runner process {pid}"
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

struct ProcessWindowSearch {
    pid: u32,
    best: HWND,
    best_area: i64,
}

fn find_process_main_window(pid: u32) -> Option<HWND> {
    let mut search = ProcessWindowSearch {
        pid,
        best: HWND::default(),
        best_area: 0,
    };
    unsafe {
        let _ = EnumWindows(
            Some(enum_process_window),
            LPARAM(&mut search as *mut _ as isize),
        );
    }
    (!search.best.0.is_null()).then_some(search.best)
}

unsafe extern "system" fn enum_process_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        let search = &mut *(lparam.0 as *mut ProcessWindowSearch);
        if !IsWindowVisible(hwnd).as_bool() {
            return TRUE;
        }
        let mut owner_pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut owner_pid));
        if owner_pid != search.pid
            || GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0 != 0
        {
            return TRUE;
        }
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return TRUE;
        }
        let area =
            i64::from((rect.right - rect.left).max(0)) * i64::from((rect.bottom - rect.top).max(0));
        if area > search.best_area {
            search.best = hwnd;
            search.best_area = area;
        }
        TRUE
    }
}

fn activate_window(hwnd: HWND) -> bool {
    unsafe {
        if GetForegroundWindow() == hwnd {
            return true;
        }

        let _ = ShowWindow(
            hwnd,
            if IsIconic(hwnd).as_bool() {
                SW_RESTORE
            } else {
                SW_SHOW
            },
        );

        let current_thread = GetCurrentThreadId();
        let foreground = GetForegroundWindow();
        let foreground_thread = if foreground.0.is_null() {
            0
        } else {
            GetWindowThreadProcessId(foreground, None)
        };
        let target_thread = GetWindowThreadProcessId(hwnd, None);
        let attach_foreground = foreground_thread != 0 && foreground_thread != current_thread;
        let attach_target = target_thread != 0
            && target_thread != current_thread
            && target_thread != foreground_thread;

        if attach_foreground {
            let _ = AttachThreadInput(current_thread, foreground_thread, true);
        }
        if attach_target {
            let _ = AttachThreadInput(current_thread, target_thread, true);
        }

        // Pulse through the topmost band before returning to normal Z-order.
        // This uncovers Runner even when the previous foreground window had
        // already placed another normal window above it.
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
        );
        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_NOTOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );

        if attach_target {
            let _ = AttachThreadInput(current_thread, target_thread, false);
        }
        if attach_foreground {
            let _ = AttachThreadInput(current_thread, foreground_thread, false);
        }

        GetForegroundWindow() == hwnd
    }
}

fn ssh_environment_present(
    connection: Option<&OsStr>,
    client: Option<&OsStr>,
    tty: Option<&OsStr>,
) -> bool {
    [connection, client, tty]
        .into_iter()
        .flatten()
        .any(|value| !value.is_empty())
}

pub(super) struct InteractiveRunnerLaunch {
    pub(super) handle: HANDLE,
    pub(super) cleanup: InteractiveRunnerCleanup,
}

pub(super) struct InteractiveRunnerCleanup {
    _bootstrap: BootstrapCleanup,
}

pub(super) fn launch_runner(
    exe_path: &Path,
    working_dir: &Path,
    launch_args: &[String],
    state_dir: &Path,
) -> Result<InteractiveRunnerLaunch> {
    std::fs::create_dir_all(state_dir)
        .with_context(|| format!("Failed to create {}", state_dir.display()))?;

    let launch_id = uuid::Uuid::new_v4().simple().to_string();
    let task_name = format!(r"\LingXia-Runner-{launch_id}");
    let script_path = state_dir.join(format!("launch-{launch_id}.vbs"));
    let task_xml_path = state_dir.join(format!("launch-{launch_id}.xml"));
    let mut cleanup = BootstrapCleanup::new(task_name.clone(), task_xml_path, script_path);
    let mut args = launch_args.to_vec();
    args.push("--launch-id".to_string());
    args.push(launch_id.clone());

    let cli_exe_path = std::env::current_exe().context("Failed to resolve the LingXia CLI path")?;
    std::fs::write(
        cleanup.launch_script_path(),
        render_launch_script(exe_path, &args, &cli_exe_path),
    )
    .with_context(|| format!("Failed to write {}", cleanup.launch_script_path().display()))?;

    let user_sid = current_user_sid()?;
    // WScript is a GUI-subsystem host, so the interactive bootstrap does not
    // flash a console window before Runner appears.
    let wscript_path = windows_wscript_path();
    let wscript_args = format!(
        "//B //Nologo {}",
        quote_windows_arg(&cleanup.launch_script_path().display().to_string())
    );
    let task_xml = render_task_xml(&user_sid, &wscript_path, &wscript_args, working_dir);
    write_utf16_xml(cleanup.task_xml_path(), &task_xml)?;

    execute_schtasks(
        &[
            OsString::from("/Create"),
            OsString::from("/TN"),
            OsString::from(&task_name),
            OsString::from("/XML"),
            cleanup.task_xml_path().as_os_str().to_os_string(),
            OsString::from("/F"),
        ],
        "register interactive Runner task",
    )?;
    cleanup.registered = true;
    cleanup.remove_task_xml();

    execute_schtasks(
        &[
            OsString::from("/Run"),
            OsString::from("/TN"),
            OsString::from(&task_name),
        ],
        "start interactive Runner task",
    )?;

    let Some(pid) = wait_for_runner_pid(exe_path, &launch_id, INTERACTIVE_START_TIMEOUT) else {
        let diagnostic = task_diagnostic(&task_name);
        return Err(anyhow!(
            "Windows Runner did not start in the interactive desktop within {} seconds.\n\
             The SSH account ({user_sid}) must also have an active local or RDP desktop sign-in.\
             {diagnostic}",
            INTERACTIVE_START_TIMEOUT.as_secs()
        ));
    };

    let handle = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE | PROCESS_TERMINATE,
            false,
            pid,
        )
    }
    .with_context(|| format!("Failed to open interactive Windows Runner process {pid}"))?;
    unsafe {
        let _ = AllowSetForegroundWindow(pid);
    }

    Ok(InteractiveRunnerLaunch {
        handle,
        cleanup: InteractiveRunnerCleanup {
            _bootstrap: cleanup,
        },
    })
}

fn windows_wscript_path() -> PathBuf {
    std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
        .join("System32")
        .join("wscript.exe")
}

fn current_user_sid() -> Result<String> {
    let output = Command::new("whoami.exe")
        .args(["/user", "/fo", "csv", "/nh"])
        .stdin(Stdio::null())
        .output()
        .context("Failed to query the Windows SSH account SID")?;
    if !output.status.success() {
        return Err(anyhow!(
            "Failed to query the Windows SSH account SID: {}",
            command_output(&output)
        ));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    parse_user_sid(&text).ok_or_else(|| anyhow!("whoami returned no Windows user SID: {text}"))
}

fn parse_user_sid(output: &str) -> Option<String> {
    output
        .split(|ch: char| ch == ',' || ch == '"' || ch.is_whitespace())
        .find(|field| {
            field.starts_with("S-1-")
                && field[4..]
                    .chars()
                    .all(|ch| ch.is_ascii_digit() || ch == '-')
        })
        .map(ToOwned::to_owned)
}

fn render_launch_script(exe_path: &Path, launch_args: &[String], cli_exe_path: &Path) -> String {
    let command = std::iter::once(exe_path.display().to_string())
        .chain(launch_args.iter().cloned())
        .map(|arg| quote_windows_arg(&arg))
        .collect::<Vec<_>>()
        .join(" ");
    let focus_command = format!(
        "{} dev-focus-window ",
        quote_windows_arg(&cli_exe_path.display().to_string())
    );
    format!(
        "Option Explicit\r\n\
         Dim shell, environment, runner, focusExitCode, exitCode, fileSystem\r\n\
         Set shell = CreateObject(\"WScript.Shell\")\r\n\
         Set environment = shell.Environment(\"PROCESS\")\r\n\
         environment(\"{RUNNER_MARKER_ENV}\") = \"1\"\r\n\
         Set runner = shell.Exec({})\r\n\
         focusExitCode = shell.Run({} & CStr(runner.ProcessID), 0, True)\r\n\
         If focusExitCode <> 0 Then shell.AppActivate(runner.ProcessID)\r\n\
         Do While runner.Status = 0\r\n\
             WScript.Sleep 250\r\n\
         Loop\r\n\
         exitCode = runner.ExitCode\r\n\
         Set fileSystem = CreateObject(\"Scripting.FileSystemObject\")\r\n\
         On Error Resume Next\r\n\
         fileSystem.DeleteFile WScript.ScriptFullName, True\r\n\
         On Error GoTo 0\r\n\
         WScript.Quit exitCode\r\n",
        vbscript_literal(&command),
        vbscript_literal(&focus_command)
    )
}

fn vbscript_literal(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn render_task_xml(user_sid: &str, command: &Path, arguments: &str, working_dir: &Path) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.4" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Author>{}</Author>
    <Description>Launch LingXia Runner in the signed-in Windows desktop.</Description>
  </RegistrationInfo>
  <Principals>
    <Principal id="Author">
      <UserId>{}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>Parallel</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>false</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>true</Hidden>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{}</Command>
      <Arguments>{}</Arguments>
      <WorkingDirectory>{}</WorkingDirectory>
    </Exec>
  </Actions>
</Task>
"#,
        xml_escape(user_sid),
        xml_escape(user_sid),
        xml_escape(&command.display().to_string()),
        xml_escape(arguments),
        xml_escape(&working_dir.display().to_string()),
    )
}

fn write_utf16_xml(path: &Path, xml: &str) -> Result<()> {
    let mut bytes = Vec::with_capacity(2 + xml.len() * 2);
    bytes.extend_from_slice(&[0xff, 0xfe]);
    for code_unit in xml.encode_utf16() {
        bytes.extend_from_slice(&code_unit.to_le_bytes());
    }
    std::fs::write(path, bytes).with_context(|| format!("Failed to write {}", path.display()))
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn execute_schtasks(args: &[OsString], action: &str) -> Result<Output> {
    let output = Command::new("schtasks.exe")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("Failed to {action}"))?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(anyhow!("Failed to {action}: {}", command_output(&output)))
    }
}

fn command_output(output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    [stdout.trim(), stderr.trim()]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn wait_for_runner_pid(exe_path: &Path, launch_id: &str, timeout: Duration) -> Option<u32> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(pid) = find_runner_pid(exe_path, launch_id) {
            return Some(pid);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
}

fn find_runner_pid(exe_path: &Path, launch_id: &str) -> Option<u32> {
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .with_exe(UpdateKind::Always)
            .with_cmd(UpdateKind::Always),
    );
    system.processes().values().find_map(|process| {
        let process_exe = process.exe()?;
        (process_executable_matches(process_exe, exe_path)
            && process
                .cmd()
                .iter()
                .any(|arg| arg.to_string_lossy() == launch_id))
        .then(|| process.pid().as_u32())
    })
}

fn task_diagnostic(task_name: &str) -> String {
    let args = [
        OsString::from("/Query"),
        OsString::from("/TN"),
        OsString::from(task_name),
        OsString::from("/V"),
        OsString::from("/FO"),
        OsString::from("LIST"),
    ];
    match execute_schtasks(&args, "query interactive Runner task") {
        Ok(output) => {
            let detail = command_output(&output);
            if detail.is_empty() {
                String::new()
            } else {
                format!("\nTask Scheduler status:\n{detail}")
            }
        }
        Err(err) => format!("\nTask Scheduler status unavailable: {err}"),
    }
}

struct BootstrapCleanup {
    task_name: String,
    task_xml_path: Option<PathBuf>,
    launch_script_path: Option<PathBuf>,
    registered: bool,
}

impl BootstrapCleanup {
    fn new(task_name: String, task_xml_path: PathBuf, launch_script_path: PathBuf) -> Self {
        Self {
            task_name,
            task_xml_path: Some(task_xml_path),
            launch_script_path: Some(launch_script_path),
            registered: false,
        }
    }

    fn task_xml_path(&self) -> &Path {
        self.task_xml_path
            .as_deref()
            .expect("task XML exists until registration")
    }

    fn launch_script_path(&self) -> &Path {
        self.launch_script_path
            .as_deref()
            .expect("launch script exists until Runner exits")
    }

    fn remove_task_xml(&mut self) {
        if let Some(path) = self.task_xml_path.take() {
            let _ = std::fs::remove_file(path);
        }
    }

    fn delete_registered_task(&mut self) {
        if !self.registered {
            return;
        }
        let args = [
            OsString::from("/Delete"),
            OsString::from("/TN"),
            OsString::from(&self.task_name),
            OsString::from("/F"),
        ];
        if execute_schtasks(&args, "delete interactive Runner task").is_ok() {
            self.registered = false;
        }
    }
}

impl Drop for BootstrapCleanup {
    fn drop(&mut self) {
        self.delete_registered_task();
        self.remove_task_xml();
        if let Some(path) = self.launch_script_path.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_environment_detection_accepts_standard_openssh_variables() {
        assert!(ssh_environment_present(
            Some(OsStr::new("192.168.1.2 50000 192.168.1.3 22")),
            None,
            None
        ));
        assert!(!ssh_environment_present(None, None, None));
        assert!(!ssh_environment_present(Some(OsStr::new("")), None, None));
    }

    #[test]
    fn user_sid_parser_ignores_localized_account_name() {
        assert_eq!(
            parse_user_sid("\"desktop\\admin\",\"S-1-5-21-1-2-3-1001\"\r\n"),
            Some("S-1-5-21-1-2-3-1001".to_string())
        );
    }

    #[test]
    fn launch_script_preserves_arguments_and_sets_runner_marker() {
        let script = render_launch_script(
            Path::new(r"C:\Program Files\LingXia\runner.exe"),
            &["--dev-ws-url".to_string(), "ws://h/?token=a&b".to_string()],
            Path::new(r"C:\Program Files\LingXia\lingxia.exe"),
        );

        assert!(script.contains("environment(\"LINGXIA_RUNNER\") = \"1\""));
        assert!(script.contains(r#"""C:\Program Files\LingXia\runner.exe"""#));
        assert!(script.contains("ws://h/?token=a&b"));
        assert!(script.contains("Set runner = shell.Exec("));
        assert!(script.contains("dev-focus-window"));
        assert!(script.contains(", 0, True)"));
        assert!(script.contains("fileSystem.DeleteFile WScript.ScriptFullName"));
    }

    #[test]
    fn task_xml_targets_interactive_token_and_escapes_values() {
        let xml = render_task_xml(
            "S-1-5-21-1",
            Path::new(r"C:\Windows\wscript.exe"),
            "//B \"D:\\A&B\\launch.vbs\"",
            Path::new(r"D:\A&B"),
        );

        assert!(xml.contains("<LogonType>InteractiveToken</LogonType>"));
        assert!(xml.contains("D:\\A&amp;B"));
        assert!(xml.contains("&quot;D:\\A&amp;B\\launch.vbs&quot;"));
    }

    #[test]
    fn task_xml_declares_utf16_for_task_scheduler_import() {
        let xml = render_task_xml(
            "S-1-5-21-1",
            Path::new(r"C:\Windows\wscript.exe"),
            "//B launch.vbs",
            Path::new(r"D:\apps"),
        );

        assert!(xml.starts_with(r#"<?xml version="1.0" encoding="UTF-16"?>"#));
    }
}

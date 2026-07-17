use super::{process_executable_matches, quote_windows_arg};
use anyhow::{Context, Result, anyhow};
use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use windows::Win32::Foundation::{
    CloseHandle, HANDLE, HWND, LPARAM, RECT, TRUE, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows::Win32::System::Threading::{
    AttachThreadInput, GetCurrentThreadId, GetExitCodeProcess, OpenProcess,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE, TerminateProcess,
    WaitForSingleObject,
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

pub(in crate::commands::dev) fn is_ssh_session() -> bool {
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
                "Could not activate a visible window for Windows process {pid}"
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

pub(super) fn focus_windows_launch(exe_path: &Path, excluded_pids: &str) -> Result<()> {
    let excluded = parse_excluded_pids(excluded_pids)?;
    let pid =
        wait_for_launched_pid(exe_path, &excluded, INTERACTIVE_START_TIMEOUT).ok_or_else(|| {
            anyhow!(
                "Could not find newly launched Windows process {}",
                exe_path.display()
            )
        })?;
    focus_windows_process(pid)
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

pub(in crate::commands::dev) struct InteractiveLaunch {
    handle: HANDLE,
    _cleanup: Option<InteractiveCleanup>,
    output_log: Option<PathBuf>,
}

struct InteractiveCleanup {
    _bootstrap: BootstrapCleanup,
}

impl InteractiveLaunch {
    pub(in crate::commands::dev) fn from_handle(handle: HANDLE) -> Self {
        Self {
            handle,
            _cleanup: None,
            output_log: None,
        }
    }

    pub(in crate::commands::dev) fn exit_code(&self) -> Result<Option<u32>> {
        let wait = unsafe { WaitForSingleObject(self.handle, 0) };
        if wait == WAIT_TIMEOUT {
            return Ok(None);
        }
        if wait != WAIT_OBJECT_0 {
            return Err(anyhow!(
                "Failed to wait for interactive Windows process: {wait:?}"
            ));
        }

        let mut code = 0u32;
        unsafe { GetExitCodeProcess(self.handle, &mut code) }
            .context("Failed to read interactive Windows process exit code")?;
        Ok(Some(code))
    }

    pub(in crate::commands::dev) fn terminate(&mut self, label: &str) -> Result<()> {
        unsafe { TerminateProcess(self.handle, 1) }
            .with_context(|| format!("Failed to terminate {label}"))?;
        Ok(())
    }

    pub(in crate::commands::dev) fn output_log(&self) -> Option<&Path> {
        self.output_log.as_deref()
    }
}

impl Drop for InteractiveLaunch {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

pub(super) fn launch_runner(
    exe_path: &Path,
    working_dir: &Path,
    launch_args: &[String],
    state_dir: &Path,
) -> Result<InteractiveLaunch> {
    launch_interactive(
        exe_path,
        working_dir,
        launch_args,
        &[(RUNNER_MARKER_ENV.to_string(), "1".to_string())],
        state_dir,
        "Runner",
    )
}

pub(in crate::commands::dev) fn launch_app(
    exe_path: &Path,
    working_dir: &Path,
    environment: &[(String, String)],
    state_dir: &Path,
) -> Result<InteractiveLaunch> {
    launch_interactive(exe_path, working_dir, &[], environment, state_dir, "App")
}

fn launch_interactive(
    exe_path: &Path,
    working_dir: &Path,
    launch_args: &[String],
    environment: &[(String, String)],
    state_dir: &Path,
    label: &str,
) -> Result<InteractiveLaunch> {
    std::fs::create_dir_all(state_dir)
        .with_context(|| format!("Failed to create {}", state_dir.display()))?;

    let launch_id = uuid::Uuid::new_v4().simple().to_string();
    let task_name = format!(r"\LingXia-{label}-{launch_id}");
    let script_path = state_dir.join(format!("launch-{launch_id}.vbs"));
    let task_xml_path = state_dir.join(format!("launch-{launch_id}.xml"));
    let output_log = state_dir.join("interactive.log");
    let mut cleanup = BootstrapCleanup::new(task_name.clone(), task_xml_path, script_path);
    let excluded_pids = process_ids_for_exe(exe_path);
    let excluded_pids_arg = format_excluded_pids(&excluded_pids);

    let cli_exe_path = std::env::current_exe().context("Failed to resolve the LingXia CLI path")?;
    std::fs::write(
        cleanup.launch_script_path(),
        render_launch_script(
            exe_path,
            launch_args,
            &excluded_pids_arg,
            &cli_exe_path,
            environment,
            &output_log,
        ),
    )
    .with_context(|| format!("Failed to write {}", cleanup.launch_script_path().display()))?;

    let user_sid = current_user_sid()?;
    // WScript is a GUI-subsystem host, so the interactive bootstrap does not
    // flash a console window before the app appears.
    let wscript_path = windows_wscript_path();
    let wscript_args = format!(
        "//B //Nologo {}",
        quote_windows_arg(&cleanup.launch_script_path().display().to_string())
    );
    let task_xml = render_task_xml(&user_sid, &wscript_path, &wscript_args, working_dir, label);
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
        &format!("register interactive {label} task"),
    )?;
    cleanup.registered = true;
    cleanup.remove_task_xml();

    execute_schtasks(
        &[
            OsString::from("/Run"),
            OsString::from("/TN"),
            OsString::from(&task_name),
        ],
        &format!("start interactive {label} task"),
    )?;

    let Some(pid) = wait_for_launched_pid(exe_path, &excluded_pids, INTERACTIVE_START_TIMEOUT)
    else {
        let diagnostic = task_diagnostic(&task_name);
        return Err(anyhow!(
            "Windows {label} did not start in the interactive desktop within {} seconds.\n\
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
    .with_context(|| format!("Failed to open interactive Windows {label} process {pid}"))?;
    unsafe {
        let _ = AllowSetForegroundWindow(pid);
    }

    Ok(InteractiveLaunch {
        handle,
        _cleanup: Some(InteractiveCleanup {
            _bootstrap: cleanup,
        }),
        output_log: Some(output_log),
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
    let output = hidden_console_command("whoami.exe")
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

fn render_launch_script(
    exe_path: &Path,
    launch_args: &[String],
    excluded_pids: &str,
    cli_exe_path: &Path,
    environment: &[(String, String)],
    output_log: &Path,
) -> String {
    let command = std::iter::once(exe_path.display().to_string())
        .chain(launch_args.iter().cloned())
        .map(|arg| quote_windows_arg(&arg))
        .collect::<Vec<_>>()
        .join(" ");
    let redirected_command = format!(
        "cmd.exe /D /S /C \"{command} > {} 2>&1\"",
        quote_windows_arg(&output_log.display().to_string())
    );
    // Keep the redirecting cmd wrapper hidden; dev-focus-window below restores
    // and activates the GUI app itself after its window exists.
    let environment_assignments = environment
        .iter()
        .map(|(key, value)| {
            format!(
                "environment({}) = {}\r\n",
                vbscript_literal(key),
                vbscript_literal(value)
            )
        })
        .collect::<String>();
    let focus_command = format!(
        "{} dev-focus-window {} {}",
        quote_windows_arg(&cli_exe_path.display().to_string()),
        quote_windows_arg(&exe_path.display().to_string()),
        quote_windows_arg(excluded_pids)
    );
    format!(
        "Option Explicit\r\n\
         Dim shell, environment, focusExitCode, fileSystem\r\n\
         Set shell = CreateObject(\"WScript.Shell\")\r\n\
         Set environment = shell.Environment(\"PROCESS\")\r\n\
         {environment_assignments}\
         shell.Run {}, 0, False\r\n\
         focusExitCode = shell.Run({}, 0, True)\r\n\
         Set fileSystem = CreateObject(\"Scripting.FileSystemObject\")\r\n\
         On Error Resume Next\r\n\
         fileSystem.DeleteFile WScript.ScriptFullName, True\r\n\
         On Error GoTo 0\r\n\
         WScript.Quit focusExitCode\r\n",
        vbscript_literal(&redirected_command),
        vbscript_literal(&focus_command)
    )
}

fn vbscript_literal(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn render_task_xml(
    user_sid: &str,
    command: &Path,
    arguments: &str,
    working_dir: &Path,
    label: &str,
) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.4" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Author>{}</Author>
    <Description>Launch LingXia {} in the signed-in Windows desktop.</Description>
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
        xml_escape(label),
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
    let output = hidden_console_command("schtasks.exe")
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

fn hidden_console_command(program: impl AsRef<OsStr>) -> Command {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut command = Command::new(program);
    command.creation_flags(CREATE_NO_WINDOW);
    command
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

fn wait_for_launched_pid(
    exe_path: &Path,
    excluded_pids: &HashSet<u32>,
    timeout: Duration,
) -> Option<u32> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(pid) = find_launched_pid(exe_path, excluded_pids) {
            return Some(pid);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
}

fn find_launched_pid(exe_path: &Path, excluded_pids: &HashSet<u32>) -> Option<u32> {
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_exe(UpdateKind::Always),
    );
    system
        .processes()
        .values()
        .filter(|process| {
            let pid = process.pid().as_u32();
            !excluded_pids.contains(&pid)
                && process
                    .exe()
                    .is_some_and(|process_exe| process_executable_matches(process_exe, exe_path))
        })
        .max_by_key(|process| process.start_time())
        .map(|process| process.pid().as_u32())
}

fn process_ids_for_exe(exe_path: &Path) -> HashSet<u32> {
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_exe(UpdateKind::Always),
    );
    system
        .processes()
        .values()
        .filter(|process| {
            process
                .exe()
                .is_some_and(|process_exe| process_executable_matches(process_exe, exe_path))
        })
        .map(|process| process.pid().as_u32())
        .collect()
}

fn format_excluded_pids(pids: &HashSet<u32>) -> String {
    let mut pids = pids.iter().copied().collect::<Vec<_>>();
    pids.sort_unstable();
    pids.into_iter()
        .map(|pid| pid.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

fn parse_excluded_pids(raw: &str) -> Result<HashSet<u32>> {
    raw.split(',')
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<u32>()
                .with_context(|| format!("Invalid excluded process id: {part}"))
        })
        .collect()
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
    match execute_schtasks(&args, "query interactive launch task") {
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
            .expect("launch script exists until the process exits")
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
        if execute_schtasks(&args, "delete interactive launch task").is_ok() {
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
            "123,456",
            Path::new(r"C:\Program Files\LingXia\lingxia.exe"),
            &[(RUNNER_MARKER_ENV.to_string(), "1".to_string())],
            Path::new(r"D:\state\interactive.log"),
        );

        assert!(script.contains("environment(\"LINGXIA_RUNNER\") = \"1\""));
        assert!(script.contains(r#"""C:\Program Files\LingXia\runner.exe"""#));
        assert!(script.contains("ws://h/?token=a&b"));
        assert!(script.contains("shell.Run"));
        assert!(!script.contains("shell.Exec"));
        assert!(!script.contains("--launch-id"));
        assert!(script.contains("cmd.exe /D /S /C"));
        assert!(script.contains(r"D:\state\interactive.log"));
        assert!(script.contains("dev-focus-window"));
        assert!(script.contains("123,456"));
        assert!(script.contains(", 0, False"));
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
            "Runner",
        );

        assert!(xml.contains("<LogonType>InteractiveToken</LogonType>"));
        assert!(xml.contains("Launch LingXia Runner"));
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
            "App",
        );

        assert!(xml.starts_with(r#"<?xml version="1.0" encoding="UTF-16"?>"#));
        assert!(xml.contains("Launch LingXia App"));
    }

    #[test]
    fn launch_script_applies_host_app_environment() {
        let script = render_launch_script(
            Path::new(r"D:\Demo\demo.exe"),
            &[],
            "42",
            Path::new(r"C:\Tools\lingxia.exe"),
            &[
                (
                    "LINGXIA_DEV_WS_URL".to_string(),
                    "ws://127.0.0.1:39000".to_string(),
                ),
                (
                    "LINGXIA_APP_ICON_PATH".to_string(),
                    r"D:\Demo\icon.png".to_string(),
                ),
            ],
            Path::new(r"D:\state\interactive.log"),
        );

        assert!(script.contains("environment(\"LINGXIA_DEV_WS_URL\") = \"ws://127.0.0.1:39000\""));
        assert!(script.contains(r#"environment("LINGXIA_APP_ICON_PATH") = "D:\Demo\icon.png""#));
        assert!(!script.contains("LINGXIA_RUNNER"));
    }

    #[test]
    fn excluded_process_ids_round_trip_stably() {
        let pids = HashSet::from([456, 123]);
        let encoded = format_excluded_pids(&pids);

        assert_eq!(encoded, "123,456");
        assert_eq!(parse_excluded_pids(&encoded).unwrap(), pids);
        assert!(parse_excluded_pids("12,invalid").is_err());
    }
}

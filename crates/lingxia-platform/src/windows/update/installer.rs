//! Installer update handoff; independent of Win32 so smoke tests use the same code.
use std::path::Path;
use std::process::Command;

pub(super) fn command(script: &Path) -> Command {
    let mut command = Command::new("powershell.exe");
    command
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(script)
        // An inherited app directory would keep Windows from swapping or
        // cleaning the payload after the host exits.
        .current_dir(std::env::temp_dir());
    command
}

pub(super) enum Installer<'a> {
    Nsis {
        setup: &'a Path,
        install_root: &'a Path,
        executable: &'a Path,
    },
    Portable {
        source: &'a Path,
        launcher: &'a Path,
        launcher_pid: u32,
    },
}

fn quote(value: &Path) -> String {
    format!("'{}'", value.to_string_lossy().replace('\'', "''"))
}

pub(super) fn render(pid: u32, installer: Installer<'_>, cleanup: &Path, script: &Path) -> String {
    let mut body = format!(
        r#"$ErrorActionPreference = 'Stop'
$self = {script}
$cleanup = {cleanup}
function WaitForExit([int] $id) {{
  $process = Get-Process -Id $id -ErrorAction SilentlyContinue
  if ($process -and -not $process.WaitForExit(120000)) {{ throw "Timed out waiting for process $id" }}
}}
try {{
  WaitForExit {pid}
"#,
        script = quote(script),
        cleanup = quote(cleanup)
    );
    match installer {
        Installer::Nsis {
            setup,
            install_root,
            executable,
        } => body.push_str(&format!(
            r#"  $setup = {setup}
  $installRoot = {root}
  $exe = {exe}
  # NSIS requires /D to be last and unquoted, including paths with spaces.
  $process = Start-Process -FilePath $setup -ArgumentList ('/S /D=' + $installRoot) -Wait -PassThru
  if ($process.ExitCode -ne 0) {{ throw "Installer exited with $($process.ExitCode)" }}
  Start-Process -FilePath $exe -WorkingDirectory (Split-Path -Parent $exe)
"#,
            setup = quote(setup),
            root = quote(install_root),
            exe = quote(executable)
        )),
        Installer::Portable {
            source,
            launcher,
            launcher_pid,
        } => body.push_str(&format!(
            r#"  WaitForExit {launcher_pid}
  $source = {source}
  $launcher = {launcher}
  $next = $launcher + '.lx-next-' + [Guid]::NewGuid().ToString('N')
  $backup = $launcher + '.lx-backup-' + [Guid]::NewGuid().ToString('N')
  Copy-Item -LiteralPath $source -Destination $next
  try {{
    # Same-volume replacement preserves the previous launcher on failure.
    [System.IO.File]::Replace($next, $launcher, $backup)
    try {{
      Start-Process -FilePath $launcher -WorkingDirectory (Split-Path -Parent $launcher)
    }} catch {{
      [System.IO.File]::Replace($backup, $launcher, $null)
      throw
    }}
    Remove-Item -LiteralPath $backup -Force -ErrorAction SilentlyContinue
  }} finally {{
    Remove-Item -LiteralPath $next -Force -ErrorAction SilentlyContinue
  }}
"#,
            source = quote(source),
            launcher = quote(launcher)
        )),
    }
    body.push_str(
        r#"  Remove-Item -LiteralPath $cleanup -Recurse -Force -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $self -Force -ErrorAction SilentlyContinue
} catch {
  # Retain staging and the helper/log on failure for diagnosis and retry.
  Write-Error $_ -ErrorAction Continue
  exit 1
}
"#,
    );
    format!("\u{feff}{body}")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nsis_uses_installer_not_directory_mirror() {
        assert_eq!(
            command(Path::new("C:/helper.ps1")).get_current_dir(),
            Some(std::env::temp_dir().as_path())
        );
        let script = render(
            12,
            Installer::Nsis {
                setup: Path::new("C:/stage/setup.exe"),
                install_root: Path::new("C:/O'Brien App"),
                executable: Path::new("C:/O'Brien App/app/demo.exe"),
            },
            Path::new("C:/stage"),
            Path::new("C:/helper.ps1"),
        );
        assert!(script.contains("'C:/O''Brien App'"));
        assert!(script.contains("('/S /D=' + $installRoot)"));
        assert!(script.contains("$process.ExitCode -ne 0"));
        assert!(!script.contains("robocopy"));
    }
    #[test]
    fn portable_waits_for_wrapper_and_keeps_rollback() {
        let script = render(
            12,
            Installer::Portable {
                source: Path::new("C:/stage/portable.exe"),
                launcher: Path::new("C:/USB/Demo.exe"),
                launcher_pid: 11,
            },
            Path::new("C:/stage"),
            Path::new("C:/helper.ps1"),
        );
        assert!(script.find("WaitForExit 12").unwrap() < script.find("WaitForExit 11").unwrap());
        assert!(script.contains("[System.IO.File]::Replace($next, $launcher, $backup)"));
        assert!(script.contains("[System.IO.File]::Replace($backup, $launcher, $null)"));
    }
}

#[cfg(target_os = "windows")]
pub(crate) struct ConsoleGuard {
    previous_output_code_page: Option<u32>,
}

#[cfg(not(target_os = "windows"))]
pub(crate) struct ConsoleGuard;

#[cfg(target_os = "windows")]
impl Drop for ConsoleGuard {
    fn drop(&mut self) {
        if let Some(code_page) = self.previous_output_code_page {
            unsafe {
                let _ = windows::Win32::System::Console::SetConsoleOutputCP(code_page);
            }
        }
    }
}

/// Borrow the parent process's console for a GUI-subsystem product command.
#[cfg(target_os = "windows")]
pub(crate) fn attach_parent() -> ConsoleGuard {
    use windows::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE};
    use windows::Win32::Globalization::CP_UTF8;
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows::Win32::System::Console::{
        ATTACH_PARENT_PROCESS, AttachConsole, GetConsoleOutputCP, GetConsoleWindow,
        STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, SetConsoleOutputCP, SetStdHandle,
    };
    use windows::core::w;

    unsafe {
        if !GetConsoleWindow().is_invalid() || AttachConsole(ATTACH_PARENT_PROCESS).is_err() {
            return ConsoleGuard {
                previous_output_code_page: None,
            };
        }
        let reopen = |name, access, id| {
            if let Ok(handle) = CreateFileW(
                name,
                access,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            ) && !handle.is_invalid()
            {
                let _ = SetStdHandle(id, handle);
            }
        };
        reopen(w!("CONOUT$"), GENERIC_WRITE.0, STD_OUTPUT_HANDLE);
        reopen(w!("CONOUT$"), GENERIC_WRITE.0, STD_ERROR_HANDLE);
        reopen(w!("CONIN$"), GENERIC_READ.0, STD_INPUT_HANDLE);

        let previous_output_code_page = GetConsoleOutputCP();
        let _ = SetConsoleOutputCP(CP_UTF8);
        ConsoleGuard {
            previous_output_code_page: Some(previous_output_code_page),
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn attach_parent() -> ConsoleGuard {
    ConsoleGuard
}

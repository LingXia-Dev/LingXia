//! Named pipe, restricted to the user who launched the app.
//!
//! The name is derived from the app id so two products never collide, and the
//! DACL names the calling user's SID explicitly rather than relying on a
//! default that inherits whatever the process token happens to carry.

use std::io::{Read, Write};
use std::path::Path;

use windows::Win32::Foundation::{CloseHandle, DUPLICATE_SAME_ACCESS, DuplicateHandle, HANDLE};
use windows::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::{
    GetTokenInformation, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
    TokenUser,
};
use windows::Win32::Storage::FileSystem::{
    FlushFileBuffers, PIPE_ACCESS_DUPLEX, ReadFile, WriteFile,
};
use windows::Win32::System::Memory::LocalFree;
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE,
    PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};
use windows::Win32::System::Threading::{GetCurrentProcess, GetCurrentProcessId, OpenProcessToken};
use windows::core::{HSTRING, PCWSTR};

const BUFFER_BYTES: u32 = 64 * 1024;

/// Where the endpoint lives, so a client can find it without being told.
///
/// Takes the state dir for signature parity with the Unix half; a pipe lives
/// in the kernel namespace, not the filesystem, so only the app id shapes it.
pub fn endpoint_name(_state_dir: &Path) -> String {
    let app_id = lingxia_app_context::app_config()
        .map(|config| {
            config
                .lingxia_id
                .clone()
                .unwrap_or_else(|| config.product_name.clone())
        })
        .unwrap_or_default();
    let app_id = sanitize(&app_id);
    if app_id.is_empty() {
        // No app id yet — fall back to something unique to this process rather
        // than a shared name two products could both claim.
        format!(r"\\.\pipe\lingxia-{}", unsafe { GetCurrentProcessId() })
    } else {
        format!(r"\\.\pipe\lingxia-{app_id}")
    }
}

/// Pipe names may not contain a backslash and are otherwise free-form; keeping
/// it to the id alphabet avoids surprises either way.
fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub(super) struct Stream {
    handle: HANDLE,
    /// Only the accepting handle owns the connection; a duplicate used purely
    /// for writing must not tear it down when it drops.
    owns_connection: bool,
}

// The handle is moved to a serving thread and never shared without a duplicate.
unsafe impl Send for Stream {}

impl Read for Stream {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let mut read = 0u32;
        unsafe { ReadFile(self.handle, Some(buffer), Some(&mut read), None) }
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        Ok(read as usize)
    }
}

impl Write for Stream {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let mut written = 0u32;
        unsafe { WriteFile(self.handle, Some(buffer), Some(&mut written), None) }
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        Ok(written as usize)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        unsafe { FlushFileBuffers(self.handle) }
            .map_err(|error| std::io::Error::other(error.to_string()))
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        unsafe {
            if self.owns_connection {
                let _ = DisconnectNamedPipe(self.handle);
            }
            let _ = CloseHandle(self.handle);
        }
    }
}

pub(super) fn split_writer(stream: &Stream) -> std::io::Result<Stream> {
    let mut duplicate = HANDLE::default();
    unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            stream.handle,
            GetCurrentProcess(),
            &mut duplicate,
            0,
            false,
            DUPLICATE_SAME_ACCESS,
        )
    }
    .map_err(|error| std::io::Error::other(error.to_string()))?;
    Ok(Stream {
        handle: duplicate,
        owns_connection: false,
    })
}

/// Unblock a pending `ConnectNamedPipe` by opening the pipe once.
pub(super) fn poke(endpoint: &str) {
    let _ = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(endpoint);
}

pub(super) struct Listener {
    name: String,
    descriptor: SecurityDescriptor,
}

impl Listener {
    pub(super) fn bind(state_dir: &Path) -> std::io::Result<Self> {
        let name = endpoint_name(state_dir);
        let descriptor = SecurityDescriptor::for_current_user()?;
        // Create one instance up front so a client that connects immediately
        // after startup finds the pipe rather than a name that does not exist
        // yet — and so a failure surfaces here instead of on the thread.
        let probe = create_instance(&name, &descriptor)?;
        unsafe {
            let _ = CloseHandle(probe);
        }
        Ok(Self { name, descriptor })
    }

    pub(super) fn name(&self) -> String {
        self.name.clone()
    }

    pub(super) fn accept(&self) -> std::io::Result<Stream> {
        let handle = create_instance(&self.name, &self.descriptor)?;
        // ERROR_PIPE_CONNECTED means the client won the race and is already
        // attached; that is a successful accept, not a failure.
        const ERROR_PIPE_CONNECTED: i32 = 535;
        if let Err(error) = unsafe { ConnectNamedPipe(handle, None) } {
            if error.code().0 & 0xffff != ERROR_PIPE_CONNECTED {
                unsafe {
                    let _ = CloseHandle(handle);
                }
                return Err(std::io::Error::other(error.to_string()));
            }
        }
        Ok(Stream {
            handle,
            owns_connection: true,
        })
    }
}

fn create_instance(name: &str, descriptor: &SecurityDescriptor) -> std::io::Result<HANDLE> {
    let mut attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0.0,
        bInheritHandle: false.into(),
    };
    let wide = HSTRING::from(name);
    let handle = unsafe {
        CreateNamedPipeW(
            PCWSTR(wide.as_ptr()),
            PIPE_ACCESS_DUPLEX,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            PIPE_UNLIMITED_INSTANCES,
            BUFFER_BYTES,
            BUFFER_BYTES,
            0,
            Some(&mut attributes),
        )
    };
    if handle.is_invalid() {
        return Err(std::io::Error::last_os_error());
    }
    Ok(handle)
}

/// A DACL that grants the launching user full access and nobody else any —
/// `P` blocks inheritance, so no parent ACE can widen this after the fact.
struct SecurityDescriptor(PSECURITY_DESCRIPTOR);

// Read-only after construction and freed once, on drop.
unsafe impl Send for SecurityDescriptor {}
unsafe impl Sync for SecurityDescriptor {}

impl SecurityDescriptor {
    fn for_current_user() -> std::io::Result<Self> {
        let sid = current_user_sid()?;
        let sddl = HSTRING::from(format!("D:P(A;;GA;;;{sid})"));
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(sddl.as_ptr()),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )
        }
        .map_err(|error| std::io::Error::other(error.to_string()))?;
        Ok(Self(descriptor))
    }
}

impl Drop for SecurityDescriptor {
    fn drop(&mut self) {
        unsafe {
            let _ = LocalFree(Some(windows::Win32::Foundation::HLOCAL(self.0.0)));
        }
    }
}

fn current_user_sid() -> std::io::Result<String> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        let token = OwnedHandle(token);

        let mut needed = 0u32;
        // First call fails with ERROR_INSUFFICIENT_BUFFER and reports the size.
        let _ = GetTokenInformation(token.0, TokenUser, None, 0, &mut needed);
        if needed == 0 {
            return Err(std::io::Error::last_os_error());
        }
        let mut buffer = vec![0u8; needed as usize];
        GetTokenInformation(
            token.0,
            TokenUser,
            Some(buffer.as_mut_ptr().cast()),
            needed,
            &mut needed,
        )
        .map_err(|error| std::io::Error::other(error.to_string()))?;

        let user = &*(buffer.as_ptr() as *const TOKEN_USER);
        let mut text = windows::core::PWSTR::null();
        ConvertSidToStringSidW(user.User.Sid, &mut text)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        let sid = text.to_string().unwrap_or_default();
        let _ = LocalFree(Some(windows::Win32::Foundation::HLOCAL(text.0.cast())));
        if sid.is_empty() {
            return Err(std::io::Error::other("empty user SID"));
        }
        Ok(sid)
    }
}

struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

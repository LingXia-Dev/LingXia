//! Best-effort current-directory lookup for LingXia-owned child shells.
//!
//! Win32 has no public process-cwd API. The host and its ConPTY children
//! share an architecture, so read the PEB's native process-parameter prefix.
//! Every read is bounded and failure simply falls back to the shell name.

use std::ffi::{OsString, c_void};
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;

const PROCESS_QUERY_INFORMATION: u32 = 0x0400;
const PROCESS_VM_READ: u32 = 0x0010;
const MAX_DIRECTORY_BYTES: usize = 32 * 1024;

pub(super) fn process_cwd(pid: u32) -> Option<PathBuf> {
    let process = unsafe { OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, pid) };
    if process.is_null() {
        return None;
    }
    let process = ProcessHandle(process);
    let mut basic = ProcessBasicInformation::default();
    let status = unsafe {
        NtQueryInformationProcess(
            process.0,
            0,
            (&mut basic as *mut ProcessBasicInformation).cast(),
            u32::try_from(std::mem::size_of_val(&basic)).ok()?,
            std::ptr::null_mut(),
        )
    };
    if status < 0 || basic.peb_base_address.is_null() {
        return None;
    }
    let peb = read_process_value(process.0, basic.peb_base_address)?;
    if peb.process_parameters.is_null() {
        return None;
    }
    let parameters = read_process_value(process.0, peb.process_parameters)?;
    read_unicode_path(process.0, parameters.current_directory.path)
}

fn read_unicode_path(process: *mut c_void, path: UnicodeString) -> Option<PathBuf> {
    let byte_len = usize::from(path.length);
    if path.buffer.is_null()
        || byte_len == 0
        || !byte_len.is_multiple_of(2)
        || byte_len > usize::from(path.maximum_length)
        || byte_len > MAX_DIRECTORY_BYTES
    {
        return None;
    }
    let mut wide = vec![0_u16; byte_len / 2];
    let mut bytes_read = 0;
    let ok = unsafe {
        ReadProcessMemory(
            process,
            path.buffer.cast(),
            wide.as_mut_ptr().cast(),
            byte_len,
            &mut bytes_read,
        )
    };
    (ok != 0 && bytes_read == byte_len).then(|| OsString::from_wide(&wide).into())
}

fn read_process_value<T: Copy>(process: *mut c_void, address: *const T) -> Option<T> {
    let mut value = std::mem::MaybeUninit::<T>::uninit();
    let mut bytes_read = 0;
    let size = std::mem::size_of::<T>();
    let ok = unsafe {
        ReadProcessMemory(
            process,
            address.cast(),
            value.as_mut_ptr().cast(),
            size,
            &mut bytes_read,
        )
    };
    (ok != 0 && bytes_read == size).then(|| unsafe { value.assume_init() })
}

struct ProcessHandle(*mut c_void);

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

#[derive(Clone, Copy, Default)]
#[repr(C)]
struct ProcessBasicInformation {
    exit_status: i32,
    peb_base_address: *const Peb,
    affinity_mask: usize,
    base_priority: i32,
    unique_process_id: usize,
    inherited_from_unique_process_id: usize,
}

#[derive(Clone, Copy)]
#[repr(C)]
struct Peb {
    reserved1: [u8; 2],
    being_debugged: u8,
    reserved2: [u8; 1],
    reserved3: [*mut c_void; 2],
    loader_data: *mut c_void,
    process_parameters: *const ProcessParameters,
}

#[derive(Clone, Copy)]
#[repr(C)]
struct ProcessParameters {
    maximum_length: u32,
    length: u32,
    flags: u32,
    debug_flags: u32,
    console_handle: *mut c_void,
    console_flags: u32,
    standard_input: *mut c_void,
    standard_output: *mut c_void,
    standard_error: *mut c_void,
    current_directory: CurrentDirectory,
}

#[derive(Clone, Copy)]
#[repr(C)]
struct CurrentDirectory {
    path: UnicodeString,
    handle: *mut c_void,
}

#[derive(Clone, Copy)]
#[repr(C)]
struct UnicodeString {
    length: u16,
    maximum_length: u16,
    buffer: *const u16,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn OpenProcess(desired_access: u32, inherit_handle: i32, process_id: u32) -> *mut c_void;
    fn ReadProcessMemory(
        process: *mut c_void,
        base_address: *const c_void,
        buffer: *mut c_void,
        size: usize,
        bytes_read: *mut usize,
    ) -> i32;
    fn CloseHandle(handle: *mut c_void) -> i32;
}

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtQueryInformationProcess(
        process: *mut c_void,
        information_class: i32,
        information: *mut c_void,
        information_length: u32,
        return_length: *mut u32,
    ) -> i32;
}

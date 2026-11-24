//! Minimal LoadLibraryW + CreateRemoteThread injector for the rustshade
//! overlay DLL.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

use windows::core::{Error, HSTRING, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{
    VirtualAllocEx, VirtualFreeEx, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE,
};
use windows::Win32::System::Threading::{
    CreateRemoteThread, GetExitCodeThread, OpenProcess, WaitForSingleObject, PROCESS_CREATE_THREAD,
    PROCESS_QUERY_INFORMATION, PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE,
};

pub struct Injector {
    pub process: HANDLE,
}

impl Drop for Injector {
    fn drop(&mut self) {
        unsafe {
            if !self.process.is_invalid() {
                let _ = CloseHandle(self.process);
            }
        }
    }
}

fn pid_by_name(name: &str) -> Option<u32> {
    unsafe {
        let snapshot =
            CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).expect("CreateToolhelp32Snapshot");
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let exe = String::from_utf16_lossy(
                    &entry.szExeFile[..entry
                        .szExeFile
                        .iter()
                        .position(|c| *c == 0)
                        .unwrap_or(entry.szExeFile.len())],
                );
                if exe.eq_ignore_ascii_case(name) {
                    return Some(entry.th32ProcessID);
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        None
    }
}

fn wide(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

impl Injector {
    pub fn attach_pid(pid: u32) -> Result<Self, Error> {
        unsafe {
            let process = OpenProcess(
                PROCESS_CREATE_THREAD
                    | PROCESS_QUERY_INFORMATION
                    | PROCESS_VM_OPERATION
                    | PROCESS_VM_WRITE
                    | PROCESS_VM_READ,
                false,
                pid,
            )?;
            Ok(Self { process })
        }
    }

    pub fn attach_process(name: &str) -> Result<Self, Error> {
        let pid = pid_by_name(name).ok_or_else(|| {
            Error::from_hresult(windows::Win32::Foundation::ERROR_FILE_NOT_FOUND.into())
        })?;
        Self::attach_pid(pid)
    }

    /// Writes the DLL path into the target process and calls LoadLibraryW.
    pub fn inject(&self, dll_path: &str) -> Result<(), Error> {
        unsafe {
            let wpath = wide(dll_path);
            let path_bytes = wpath.len() * 2;
            let remote = VirtualAllocEx(
                self.process,
                None,
                path_bytes,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            );
            if remote.is_null() {
                return Err(Error::from_win32());
            }

            let mut written = 0usize;
            windows::Win32::System::Diagnostics::Debug::WriteProcessMemory(
                self.process,
                remote,
                wpath.as_ptr().cast(),
                path_bytes,
                Some(&mut written),
            )?;

            let kernel32 = GetModuleHandleW(PCWSTR::from(&HSTRING::from("kernel32.dll")))?;
            let load_library = windows::Win32::System::LibraryLoader::GetProcAddress(
                kernel32,
                windows::core::PCSTR::from_raw(b"LoadLibraryW\0".as_ptr()),
            );
            let Some(load_library) = load_library else {
                VirtualFreeEx(self.process, remote, 0, MEM_RELEASE);
                return Err(Error::from_hresult(
                    windows::Win32::Foundation::E_FAIL.to_hresult(),
                ));
            };

            let thread = CreateRemoteThread(
                self.process,
                None,
                0,
                Some(std::mem::transmute(load_library)),
                Some(remote),
                0,
                None,
            )?;
            let _ = WaitForSingleObject(thread, 10_000);
            let mut code = 0u32;
            let _ = GetExitCodeThread(thread, &mut code);
            let _ = CloseHandle(thread);
            VirtualFreeEx(self.process, remote, 0, MEM_RELEASE);

            if code == 0 {
                return Err(Error::from_hresult(
                    windows::Win32::Foundation::E_FAIL.to_hresult(),
                ));
            }
            Ok(())
        }
    }
}

// Windows-only hook implementation lives behind cfg; the crate also builds
// as a stub on Linux so the workspace stays green on all platforms.

#[cfg(windows)]
pub mod d3d11;

#[cfg(windows)]
pub mod d3d9;

#[cfg(windows)]
pub mod injector;

#[cfg(windows)]
pub(crate) fn dlog(msg: &str) {
    use windows::core::HSTRING;
    use windows::Win32::System::Diagnostics::Debug::OutputDebugStringW;
    let line = HSTRING::from(format!("rustshade: {msg}"));
    unsafe { OutputDebugStringW(&line) };
}

/// Configures the pass chain preset for every hooked graphics API.
#[cfg(windows)]
pub fn configure(preset: rustshade_core::Preset) {
    d3d11::configure(preset.clone());
    d3d9::configure(preset);
}

#[cfg(windows)]
const DLL_PROCESS_ATTACH: u32 = 1;
#[cfg(windows)]
const DLL_PROCESS_DETACH: u32 = 0;

#[cfg(windows)]
static OWN_MODULE: std::sync::atomic::AtomicPtr<core::ffi::c_void> =
    std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());

#[cfg(windows)]
#[no_mangle]
extern "system" fn DllMain(hinstance: *mut u8, reason: u32, _reserved: *mut u8) -> i32 {
    match reason {
        DLL_PROCESS_ATTACH => {
            // Heavy init (dummy devices, module probing) must not run under the
            // loader lock — defer to a worker thread instead.
            std::sync::atomic::AtomicPtr::<core::ffi::c_void>::store(
                &OWN_MODULE,
                hinstance as *mut core::ffi::c_void,
                std::sync::atomic::Ordering::Release,
            );
            let _ = std::thread::Builder::new()
                .stack_size(4 * 1024 * 1024)
                .spawn(|| unsafe { init_worker() });
            1
        }
        DLL_PROCESS_DETACH => {
            unsafe {
                d3d11::uninstall();
                d3d9::uninstall();
            }
            1
        }
        _ => 1,
    }
}

#[cfg(windows)]
unsafe fn init_worker() {
    let hinstance = OWN_MODULE.load(std::sync::atomic::Ordering::Acquire);
    if let Some(preset) = load_preset(hinstance) {
        dlog("preset loaded");
        configure(preset);
    } else {
        dlog("no rustshade.toml found next to the game or in the working directory");
    }
    if d3d11::install().is_ok() {
        dlog("d3d11 hooks active");
    }
    if d3d9::install().is_ok() {
        dlog("d3d9 hooks active");
    }
}

/// Loads `rustshade.toml` from the directory of the injected DLL, falling back
/// to the game's working directory.
#[cfg(windows)]
unsafe fn load_preset(hinstance: *mut core::ffi::c_void) -> Option<rustshade_core::Preset> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::path::PathBuf;
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::System::LibraryLoader::GetModuleFileNameW;

    let mut candidates: Vec<PathBuf> = vec![PathBuf::from("rustshade.toml")];
    if !hinstance.is_null() {
        let mut buf = [0u16; 1024];
        let len = GetModuleFileNameW(HMODULE(hinstance), &mut buf) as usize;
        let wide = &buf[..len.min(buf.len())];
        if let Some(nul) = wide.iter().position(|c| *c == 0) {
            let dll_path = PathBuf::from(OsString::from_wide(&wide[..nul]));
            if let Some(dir) = dll_path.parent() {
                candidates.insert(0, dir.join("rustshade.toml"));
            }
        }
    }
    for candidate in candidates {
        if let Ok(text) = std::fs::read_to_string(&candidate) {
            match rustshade_core::Preset::parse(&text) {
                Ok(preset) => return Some(preset),
                Err(err) => dlog(&format!(
                    "preset parse error in {}: {err}",
                    candidate.display()
                )),
            }
        }
    }
    None
}

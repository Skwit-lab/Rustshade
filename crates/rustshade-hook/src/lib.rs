// Windows-only hook implementation lives behind cfg; the crate also builds
// as a stub on Linux so the workspace stays green on all platforms.

#[cfg(windows)]
pub mod d3d11;

#[cfg(windows)]
pub mod injector;

#[cfg(windows)]
#[no_mangle]
extern "system" fn DllMain(hinstance: *mut u8, reason: u32, _reserved: *mut u8) -> i32 {
    unsafe { self::d3d11::dll_main_impl(hinstance, reason) }
}

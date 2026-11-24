//! Experimental D3D11 present hook (Windows only).
//!
//! Strategy: inject a dummy swap chain to obtain `IDXGISwapChain::Present`
//! via its vtable (slot 8), then patch the shared COM vtable so every
//! swap chain routes through `hooked_present`. No nightly-only detour
//! library is required; everything runs on the stable channel.

#![allow(non_snake_case)]

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::OnceLock;

use windows::core::{HRESULT, PCSTR, PCWSTR};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::System::Memory::{VirtualProtect, PAGE_PROTECTION_FLAGS, PAGE_READWRITE};

const PRESENT_VTABLE_SLOT: usize = 8;
const DLL_PROCESS_ATTACH: u32 = 1;
const DLL_PROCESS_DETACH: u32 = 0;
const D3D_DRIVER_TYPE_HARDWARE: u32 = 1;
const D3D11_SDK_VERSION: u32 = 7;

static INSTALLED: AtomicBool = AtomicBool::new(false);
static ORIGINAL_PRESENT: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static PRESENT_SLOT_ADDR: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static PRESET: OnceLock<rustshade_core::Preset> = OnceLock::new();

/// Stored at injection time so the pass chain can read it per frame.
pub fn configure(preset: rustshade_core::Preset) {
    let _ = PRESET.set(preset);
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SwapChainDesc {
    // DXGI_MODE_DESC.BufferDesc
    buffer_width: u32,
    buffer_height: u32,
    refresh_numerator: u32,
    refresh_denominator: u32,
    format: u32,
    scanline_ordering: u32,
    scaling: u32,
    // DXGI_SAMPLE_DESC
    sample_count: u32,
    sample_quality: u32,
    buffer_usage: u32,
    buffer_count: u32,
    output_window: usize,
    windowed: i32,
    swap_effect: u32,
    flags: u32,
}

type D3D11CreateDeviceAndSwapChainFn = unsafe extern "system" fn(
    adapter: *const c_void,
    driver_type: u32,
    software: *const c_void,
    flags: u32,
    feature_levels: *const u32,
    feature_level_count: u32,
    sdk_version: u32,
    swap_chain_desc: *const SwapChainDesc,
    out_swap_chain: *mut *mut c_void,
    out_device: *mut *mut c_void,
    out_feature_level: *mut u32,
    out_context: *mut *mut c_void,
) -> HRESULT;

type PresentFn =
    unsafe extern "system" fn(swap: *mut c_void, sync_interval: u32, flags: u32) -> HRESULT;

extern "system" fn hooked_present(swap: *mut c_void, sync_interval: u32, flags: u32) -> HRESULT {
    // Post-processing pass chain against the backbuffer will be executed
    // here once the native D3D11 renderer is wired up. Forwarding for now.
    let original = ORIGINAL_PRESENT.load(Ordering::Relaxed);
    let original: PresentFn = unsafe { std::mem::transmute(original) };
    unsafe { original(swap, sync_interval, flags) }
}

unsafe fn wide(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Creates a dummy D3D11 device + swap chain and returns the address of the
/// `Present` vtable slot.
unsafe fn load_present_slot() -> Result<*mut *mut c_void, HRESULT> {
    if GetModuleHandleW(PCWSTR::from_raw(wide("d3d11.dll").as_ptr())).is_err() {
        // d3d11.dll not loaded yet in the game process.
        return Err(HRESULT(-1i32));
    }
    let module = GetModuleHandleW(PCWSTR::from_raw(wide("d3d11.dll").as_ptr()))
        .expect("module checked above");

    let Some(create_ptr) = GetProcAddress(
        module,
        PCSTR::from_raw(b"D3D11CreateDeviceAndSwapChain\0".as_ptr()),
    ) else {
        return Err(HRESULT(-2i32));
    };
    let create: D3D11CreateDeviceAndSwapChainFn = std::mem::transmute(create_ptr);

    let desc = SwapChainDesc {
        buffer_width: 1,
        buffer_height: 1,
        refresh_numerator: 60,
        refresh_denominator: 1,
        format: 0,
        scanline_ordering: 0,
        scaling: 0,
        sample_count: 1,
        sample_quality: 0,
        buffer_usage: 0,
        buffer_count: 2,
        output_window: 0,
        windowed: 1,
        swap_effect: 0,
        flags: 0,
    };

    let mut swap: *mut c_void = std::ptr::null_mut();
    let mut device: *mut c_void = std::ptr::null_mut();
    let mut feature_level: u32 = 0;
    let mut context: *mut c_void = std::ptr::null_mut();

    let hr = create(
        std::ptr::null(),
        D3D_DRIVER_TYPE_HARDWARE,
        std::ptr::null(),
        0,
        std::ptr::null(),
        0,
        D3D11_SDK_VERSION,
        &desc,
        &mut swap,
        &mut device,
        &mut feature_level,
        &mut context,
    );
    if hr.is_err() || swap.is_null() {
        return Err(hr);
    }

    let vtable = *(swap as *mut *mut c_void);
    Ok(vtable.add(PRESENT_VTABLE_SLOT).cast::<*mut c_void>())
}

/// Patches the shared swap chain vtable; call from `DllMain`.
pub unsafe fn install() -> Result<(), HRESULT> {
    if INSTALLED.swap(true, Ordering::AcqRel) {
        return Ok(());
    }
    let slot = load_present_slot()?;
    let original = *slot;
    if original.is_null() {
        return Err(HRESULT(-3i32));
    }
    ORIGINAL_PRESENT.store(original, Ordering::Release);
    PRESENT_SLOT_ADDR.store(slot.cast(), Ordering::Release);

    let mut old_protect = PAGE_PROTECTION_FLAGS(0);
    let slot_bytes = std::mem::size_of::<usize>();
    VirtualProtect(
        slot.cast::<c_void>(),
        slot_bytes,
        PAGE_READWRITE,
        &mut old_protect,
    )
    .map_err(|_| HRESULT(-4i32))?;

    *slot = hooked_present as *mut c_void;

    VirtualProtect(
        slot.cast::<c_void>(),
        slot_bytes,
        old_protect,
        &mut old_protect,
    )
    .map_err(|_| HRESULT(-5i32))?;
    Ok(())
}

pub unsafe fn uninstall() {
    if !INSTALLED.swap(false, Ordering::AcqRel) {
        return;
    }
    let slot = PRESENT_SLOT_ADDR
        .load(Ordering::Acquire)
        .cast::<*mut c_void>();
    let original = ORIGINAL_PRESENT.swap(std::ptr::null_mut(), Ordering::AcqRel);
    if !slot.is_null() && !original.is_null() {
        let mut old_protect = PAGE_PROTECTION_FLAGS(0);
        let slot_bytes = std::mem::size_of::<usize>();
        if VirtualProtect(
            slot.cast::<c_void>(),
            slot_bytes,
            PAGE_READWRITE,
            &mut old_protect,
        )
        .is_ok()
        {
            *slot = original;
            let _ = VirtualProtect(
                slot.cast::<c_void>(),
                slot_bytes,
                old_protect,
                &mut old_protect,
            );
        }
    }
}

/// Real `DllMain` body, called by the exported stub in lib.rs.
pub unsafe fn dll_main_impl(_hinstance: *mut u8, reason: u32) -> i32 {
    match reason {
        DLL_PROCESS_ATTACH => {
            if install().is_err() {
                0
            } else {
                1
            }
        }
        DLL_PROCESS_DETACH => {
            uninstall();
            1
        }
        _ => 1,
    }
}

//! D3D9 present hook (Windows only) — the graphics API used by Source engine
//! titles such as Portal 2.
//!
//! Same stable-safe strategy as `d3d11.rs`: create a throwaway device through
//! the process' own `d3d9.dll`, read the address of `IDirect3DDevice9` vtable
//! slots from it, and patch those slots in place. The D3D9 runtime shares one
//! static vtable across all devices, so the patch fires for the game's device
//! as well (the same trick ReShade uses).
//!
//! Depth-buffer access mirrors ReShade's classic approach: the
//! `CreateDepthStencilSurface` hook (vtable slot 29) upgrades the ubiquitous
//! `D3DFMT_D24S8` format to `D3DFMT_INTZ`, a vendor fourcc that pixel shaders
//! can sample as a plain float texture.

#![allow(non_snake_case)]

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, AtomicU64, Ordering};
use std::sync::OnceLock;

use windows::core::{s, w, HRESULT};
use windows::Win32::Foundation::{BOOL, FALSE, HANDLE, TRUE};
use windows::Win32::Graphics::Direct3D9::{
    D3DADAPTER_DEFAULT, D3DCREATE_FPU_PRESERVE, D3DCREATE_HARDWARE_VERTEXPROCESSING,
    D3DCREATE_NOWINDOWCHANGES, D3DCREATE_SOFTWARE_VERTEXPROCESSING, D3DDEVTYPE_HAL, D3DFMT_D24S8,
    D3DFMT_UNKNOWN, D3DFORMAT, D3DPRESENT_PARAMETERS, D3DSWAPEFFECT_DISCARD, D3D_SDK_VERSION,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::System::Memory::{VirtualProtect, PAGE_PROTECTION_FLAGS, PAGE_READWRITE};
use windows::Win32::UI::WindowsAndMessaging::GetDesktopWindow;

const D3D9_PRESENT_SLOT: usize = 17;
const D3D9_RESET_SLOT: usize = 16;
const D3D9_CREATE_DEPTH_STENCIL_SLOT: usize = 29;
const D3D9_SET_DEPTH_STENCIL_SLOT: usize = 39;
const D3D9_CREATE_DEVICE_SLOT: usize = 16;

const D3DFMT_INTZ: D3DFORMAT = D3DFORMAT(0x5A544E49);

static INSTALLED: AtomicBool = AtomicBool::new(false);
static ORIGINAL_PRESENT: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static ORIGINAL_RESET: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static ORIGINAL_CREATE_DS: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static ORIGINAL_SET_DS: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static SLOT_PRESENT: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static SLOT_RESET: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static SLOT_CREATE_DS: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static SLOT_SET_DS: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static TRACKED_DEPTH_STENCIL: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static FRAME_COUNT: AtomicU64 = AtomicU64::new(0);
static DEPTH_SWAPS: AtomicU32 = AtomicU32::new(0);

static PRESET: OnceLock<rustshade_core::Preset> = OnceLock::new();

pub fn configure(preset: rustshade_core::Preset) {
    let _ = PRESET.set(preset);
}

pub fn preset() -> Option<&'static rustshade_core::Preset> {
    PRESET.get()
}

pub fn frame_count() -> u64 {
    FRAME_COUNT.load(Ordering::Relaxed)
}

pub fn depth_attached() -> bool {
    !TRACKED_DEPTH_STENCIL.load(Ordering::Relaxed).is_null()
}

type Direct3DCreate9Fn = unsafe extern "system" fn(sdk_version: u32) -> *mut c_void;

type CreateDeviceFn = unsafe extern "system" fn(
    d3d: *mut c_void,
    adapter: u32,
    device_type: u32,
    focus_window: *mut c_void,
    behavior_flags: u32,
    params: *mut D3DPRESENT_PARAMETERS,
    out_device: *mut *mut c_void,
) -> HRESULT;

type PresentFn = unsafe extern "system" fn(
    device: *mut c_void,
    source_rect: *const c_void,
    dest_rect: *const c_void,
    dest_window: *mut c_void,
    dirty_region: *const c_void,
    flags: u32,
) -> HRESULT;

type ResetFn =
    unsafe extern "system" fn(device: *mut c_void, params: *mut D3DPRESENT_PARAMETERS) -> HRESULT;

type CreateDepthStencilSurfaceFn = unsafe extern "system" fn(
    device: *mut c_void,
    width: u32,
    height: u32,
    format: D3DFORMAT,
    multisample_type: D3DMULTISAMPLE_TYPE,
    quality: u32,
    discard: BOOL,
    out_surface: *mut *mut c_void,
    shared: *mut HANDLE,
) -> HRESULT;

use windows::Win32::Graphics::Direct3D9::D3DMULTISAMPLE_TYPE;

type SetDepthStencilSurfaceFn =
    unsafe extern "system" fn(device: *mut c_void, surface: *mut c_void) -> HRESULT;

extern "system" fn hooked_present(
    device: *mut c_void,
    source_rect: *const c_void,
    dest_rect: *const c_void,
    dest_window: *mut c_void,
    dirty_region: *const c_void,
    flags: u32,
) -> HRESULT {
    FRAME_COUNT.fetch_add(1, Ordering::Relaxed);
    // The D3D9 post-processing pass chain (backbuffer -> effects -> backbuffer,
    // sampling the INTZ depth texture captured below) will be driven from here.
    // Until the native D3D9 renderer lands, forward untouched.
    let original = ORIGINAL_PRESENT.load(Ordering::Relaxed);
    let original: PresentFn = unsafe { std::mem::transmute(original) };
    unsafe {
        original(
            device,
            source_rect,
            dest_rect,
            dest_window,
            dirty_region,
            flags,
        )
    }
}

extern "system" fn hooked_reset(
    device: *mut c_void,
    params: *mut D3DPRESENT_PARAMETERS,
) -> HRESULT {
    TRACKED_DEPTH_STENCIL.store(std::ptr::null_mut(), Ordering::Release);
    let original = ORIGINAL_RESET.load(Ordering::Relaxed);
    let original: ResetFn = unsafe { std::mem::transmute(original) };
    unsafe { original(device, params) }
}

extern "system" fn hooked_create_depth_stencil_surface(
    device: *mut c_void,
    width: u32,
    height: u32,
    format: D3DFORMAT,
    multisample_type: D3DMULTISAMPLE_TYPE,
    quality: u32,
    discard: BOOL,
    out_surface: *mut *mut c_void,
    shared: *mut HANDLE,
) -> HRESULT {
    let mut format = format;
    if format == D3DFMT_D24S8 {
        format = D3DFMT_INTZ;
        DEPTH_SWAPS.fetch_add(1, Ordering::Relaxed);
    }
    let original = ORIGINAL_CREATE_DS.load(Ordering::Relaxed);
    let original: CreateDepthStencilSurfaceFn = unsafe { std::mem::transmute(original) };
    unsafe {
        original(
            device,
            width,
            height,
            format,
            multisample_type,
            quality,
            discard,
            out_surface,
            shared,
        )
    }
}

extern "system" fn hooked_set_depth_stencil_surface(
    device: *mut c_void,
    surface: *mut c_void,
) -> HRESULT {
    TRACKED_DEPTH_STENCIL.store(surface, Ordering::Release);
    let original = ORIGINAL_SET_DS.load(Ordering::Relaxed);
    let original: SetDepthStencilSurfaceFn = unsafe { std::mem::transmute(original) };
    unsafe { original(device, surface) }
}

unsafe fn patch_slot(
    vtable: *mut *mut c_void,
    idx: usize,
    hook: *mut c_void,
) -> Option<*mut c_void> {
    let slot = vtable.add(idx);
    let original = *slot;
    if original.is_null() || hook.is_null() || original == hook {
        return None;
    }
    let mut old_protect = PAGE_PROTECTION_FLAGS(0);
    VirtualProtect(
        slot.cast::<c_void>(),
        std::mem::size_of::<usize>(),
        PAGE_READWRITE,
        &mut old_protect,
    )
    .ok()?;
    *slot = hook;
    let _ = VirtualProtect(
        slot.cast::<c_void>(),
        std::mem::size_of::<usize>(),
        old_protect,
        &mut old_protect,
    );
    Some(original)
}

unsafe fn restore_slot(slot_addr: *mut c_void, original: *mut c_void, hook: *mut c_void) -> bool {
    if slot_addr.is_null() || original.is_null() {
        return false;
    }
    let slot = slot_addr as *mut *mut c_void;
    if *slot != hook {
        return false;
    }
    let mut old_protect = PAGE_PROTECTION_FLAGS(0);
    if VirtualProtect(
        slot.cast::<c_void>(),
        std::mem::size_of::<usize>(),
        PAGE_READWRITE,
        &mut old_protect,
    )
    .is_err()
    {
        return false;
    }
    *slot = original;
    let _ = VirtualProtect(
        slot.cast::<c_void>(),
        std::mem::size_of::<usize>(),
        old_protect,
        &mut old_protect,
    );
    true
}

type ReleaseFn = unsafe extern "system" fn(*mut c_void) -> u32;

unsafe fn com_release(obj: *mut c_void) {
    if obj.is_null() {
        return;
    }
    let vtable = *(obj as *mut *mut c_void) as *mut *mut c_void;
    let release: ReleaseFn = std::mem::transmute(*vtable.add(2));
    release(obj);
}

/// Creates a throwaway device through the game's own `d3d9.dll`, then patches
/// the four relevant `IDirect3DDevice9` vtable slots.
///
/// # Safety
///
/// Must only run inside the target process while `d3d9.dll` is loaded, and
/// must not race with another thread calling `install`/`uninstall`.
pub unsafe fn install() -> Result<(), HRESULT> {
    if INSTALLED.swap(true, Ordering::AcqRel) {
        return Ok(());
    }
    if GetModuleHandleW(w!("d3d9.dll")).is_err() {
        INSTALLED.store(false, Ordering::Release);
        return Err(HRESULT(-1i32));
    }
    let module = GetModuleHandleW(w!("d3d9.dll")).expect("d3d9.dll checked above");
    let Some(create_d3d_ptr) = GetProcAddress(module, s!("Direct3DCreate9")) else {
        INSTALLED.store(false, Ordering::Release);
        return Err(HRESULT(-2i32));
    };

    let create_d3d: Direct3DCreate9Fn = std::mem::transmute(create_d3d_ptr);
    let d3d_obj = create_d3d(D3D_SDK_VERSION);
    if d3d_obj.is_null() {
        INSTALLED.store(false, Ordering::Release);
        return Err(HRESULT(-3i32));
    }

    let d3d_vtable = *(d3d_obj as *mut *mut c_void) as *mut *mut c_void;
    let create_device_ptr = *d3d_vtable.add(D3D9_CREATE_DEVICE_SLOT);
    if create_device_ptr.is_null() {
        com_release(d3d_obj);
        INSTALLED.store(false, Ordering::Release);
        return Err(HRESULT(-4i32));
    }
    let create_device: CreateDeviceFn = std::mem::transmute(create_device_ptr);

    let mut params = D3DPRESENT_PARAMETERS {
        BackBufferWidth: 1,
        BackBufferHeight: 1,
        BackBufferFormat: D3DFMT_UNKNOWN,
        Windowed: TRUE,
        SwapEffect: D3DSWAPEFFECT_DISCARD,
        EnableAutoDepthStencil: FALSE,
        ..Default::default()
    };
    let flags = (D3DCREATE_HARDWARE_VERTEXPROCESSING
        | D3DCREATE_FPU_PRESERVE
        | D3DCREATE_NOWINDOWCHANGES) as u32;
    let mut device: *mut c_void = std::ptr::null_mut();
    let mut hr = create_device(
        d3d_obj,
        D3DADAPTER_DEFAULT,
        D3DDEVTYPE_HAL.0 as u32,
        GetDesktopWindow().0,
        flags,
        &mut params,
        &mut device,
    );
    if hr.is_err() || device.is_null() {
        device = std::ptr::null_mut();
        let software_flags = (D3DCREATE_SOFTWARE_VERTEXPROCESSING
            | D3DCREATE_FPU_PRESERVE
            | D3DCREATE_NOWINDOWCHANGES) as u32;
        hr = create_device(
            d3d_obj,
            D3DADAPTER_DEFAULT,
            D3DDEVTYPE_HAL.0 as u32,
            GetDesktopWindow().0,
            software_flags,
            &mut params,
            &mut device,
        );
        if hr.is_err() || device.is_null() {
            com_release(d3d_obj);
            INSTALLED.store(false, Ordering::Release);
            return Err(hr);
        }
    }

    let vtable = *(device as *mut *mut c_void) as *mut *mut c_void;

    let present_orig = match patch_slot(vtable, D3D9_PRESENT_SLOT, hooked_present as *mut c_void) {
        Some(ptr) => ptr,
        None => {
            com_release(device);
            com_release(d3d_obj);
            INSTALLED.store(false, Ordering::Release);
            return Err(HRESULT(-5i32));
        }
    };
    let reset_orig = match patch_slot(vtable, D3D9_RESET_SLOT, hooked_reset as *mut c_void) {
        Some(ptr) => ptr,
        None => {
            restore_slot(
                vtable.add(D3D9_PRESENT_SLOT).cast::<c_void>(),
                present_orig,
                hooked_present as *mut c_void,
            );
            com_release(device);
            com_release(d3d_obj);
            INSTALLED.store(false, Ordering::Release);
            return Err(HRESULT(-6i32));
        }
    };
    let create_ds_orig = patch_slot(
        vtable,
        D3D9_CREATE_DEPTH_STENCIL_SLOT,
        hooked_create_depth_stencil_surface as *mut c_void,
    );
    let set_ds_orig = patch_slot(
        vtable,
        D3D9_SET_DEPTH_STENCIL_SLOT,
        hooked_set_depth_stencil_surface as *mut c_void,
    );

    ORIGINAL_PRESENT.store(present_orig, Ordering::Release);
    ORIGINAL_RESET.store(reset_orig, Ordering::Release);
    SLOT_PRESENT.store(
        vtable.add(D3D9_PRESENT_SLOT).cast::<c_void>(),
        Ordering::Release,
    );
    SLOT_RESET.store(
        vtable.add(D3D9_RESET_SLOT).cast::<c_void>(),
        Ordering::Release,
    );
    if let Some(ptr) = create_ds_orig {
        ORIGINAL_CREATE_DS.store(ptr, Ordering::Release);
        SLOT_CREATE_DS.store(
            vtable.add(D3D9_CREATE_DEPTH_STENCIL_SLOT).cast::<c_void>(),
            Ordering::Release,
        );
    }
    if let Some(ptr) = set_ds_orig {
        ORIGINAL_SET_DS.store(ptr, Ordering::Release);
        SLOT_SET_DS.store(
            vtable.add(D3D9_SET_DEPTH_STENCIL_SLOT).cast::<c_void>(),
            Ordering::Release,
        );
    }

    com_release(device);
    com_release(d3d_obj);
    super::dlog("d3d9 hooks installed (present, reset, depth-stencil trace)");
    Ok(())
}

/// Restores the original vtable entries patched by [`install`].
///
/// # Safety
///
/// Must only run in the process where `install` succeeded, and must not race
/// with a frame inside one of the hooked functions.
pub unsafe fn uninstall() {
    if !INSTALLED.swap(false, Ordering::AcqRel) {
        return;
    }
    let slots = [
        (
            SLOT_PRESENT.load(Ordering::Acquire),
            ORIGINAL_PRESENT.load(Ordering::Acquire),
            hooked_present as *mut c_void,
        ),
        (
            SLOT_RESET.load(Ordering::Acquire),
            ORIGINAL_RESET.load(Ordering::Acquire),
            hooked_reset as *mut c_void,
        ),
        (
            SLOT_CREATE_DS.load(Ordering::Acquire),
            ORIGINAL_CREATE_DS.load(Ordering::Acquire),
            hooked_create_depth_stencil_surface as *mut c_void,
        ),
        (
            SLOT_SET_DS.load(Ordering::Acquire),
            ORIGINAL_SET_DS.load(Ordering::Acquire),
            hooked_set_depth_stencil_surface as *mut c_void,
        ),
    ];
    for (slot, original, hook) in slots {
        restore_slot(slot, original, hook);
    }
    TRACKED_DEPTH_STENCIL.store(std::ptr::null_mut(), Ordering::Release);
}

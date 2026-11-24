//! Experimental D3D11 present hook.
//!
//! instalar: injected DLL creates a dummy swap chain to locate
//! `IDXGISwapChain::Present` (vtable slot 8), detours it, and pushes the
//! backbuffer through the rustshade effect stack. This module is developed
//! against a real Windows machine, not verified by CI here.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use windows::Win32::Foundation::HRESULT;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDeviceAndSwapChain, ID3D11Device, ID3D11DeviceContext, D3D11_SDK_VERSION,
};
use windows::Win32::Graphics::Dxgi::{IDXGISwapChain, DXGI_SWAP_CHAIN_DESC};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;

use retour::static_detour;

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static EFFECTS: OnceLock<rustshade_core::Preset> = OnceLock::new();

type PresentFn = unsafe extern "system" fn(*mut IDXGISwapChain, u32, u32) -> HRESULT;
type ResizeBuffersFn =
    unsafe extern "system" fn(*mut IDXGISwapChain, u32, u32, u32, *const c_void) -> HRESULT;

// IDXGISwapChain vtable: Present = 8, ResizeBuffers = 13.
const PRESENT_VTABLE_SLOT: usize = 8;

static_detour! {
    static HookPresent: unsafe extern "system" fn(*mut IDXGISwapChain, u32, u32) -> HRESULT;
}

unsafe fn load_present() -> Result<PresentFn, HRESULT> {
    let mut device: Option<ID3D11Device> = None;
    let mut context: Option<ID3D11DeviceContext> = None;
    let mut swap: Option<IDXGISwapChain> = None;
    let mut desc = DXGI_SWAP_CHAIN_DESC::default();

    let hr = D3D11CreateDeviceAndSwapChain(
        None,
        windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE,
        None,
        windows::Win32::Graphics::Direct3D::D3D11_CREATE_DEVICE_FLAG(0),
        None,
        D3D11_SDK_VERSION,
        Some(&mut desc),
        Some(&mut swap),
        Some(&mut device),
        None,
        Some(&mut context),
        None,
    );
    hr.ok()?;
    let swap = swap.ok_or(hr)?;
    let vtable = *(swap.as_raw() as *mut *mut c_void);
    Ok(std::mem::transmute::<*mut c_void, PresentFn>(
        *vtable.add(PRESENT_VTABLE_SLOT),
    ))
}

/// Installs the present detour. Called once by `DllMain`.
pub unsafe fn install() -> Result<(), HRESULT> {
    if INITIALIZED.swap(true, Ordering::AcqRel) {
        return Ok(());
    }
    let present = load_present()?;
    HookPresent
        .initialize(present, |swap, sync_interval, flags| {
            // The actual post-processing pass chain is executed here once the
            // D3D11 renderer (rustshade-gpu backend ported to ID3D11Device
            // + immediate context) is wired up. For now the original
            // present is transparently forwarded.
            HookPresent.call(swap, sync_interval, flags)
        })
        .ok();
    HookPresent.enable().map_err(|_| HRESULT(-1i32))?;
    Ok(())
}

pub unsafe fn uninstall() {
    if INITIALIZED.swap(false, Ordering::AcqRel) {
        let _ = HookPresent.disable();
    }
}

/// Entry point of the injected DLL. Build with
/// `cargo build -p rustshade-hook --target x86_64-pc-windows-msvc --release`
/// and a `cdylib` wrapper that calls `install()` from `DllMain`.
pub fn configure(preset: rustshade_core::Preset) {
    let _ = EFFECTS.set(preset);
}

#[allow(non_snake_case)]
pub unsafe fn dll_main(hinst: *mut u8, reason: u32) -> i32 {
    const DLL_PROCESS_ATTACH: u32 = 1;
    const DLL_PROCESS_DETACH: u32 = 0;
    let _ = GetModuleHandleW;
    match reason {
        DLL_PROCESS_ATTACH => {
            if install().is_err() {
                return 0;
            }
            std::mem::forget(std::boxed::Box::new(hinst as usize)); // keep alive
            1
        }
        DLL_PROCESS_DETACH => {
            uninstall();
            1
        }
        _ => 1,
    }
}

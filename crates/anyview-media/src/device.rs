//! A graphics device on no window, for a session that plays with no picture to show.

use crate::error::MediaError;
use std::sync::{Mutex, PoisonError};

/// The headless device this process opened, kept until it exits. Tearing a device down while
/// another thread makes one crashes the loader (`loader_icd_scan`, `loader_get_icd_and_device`)
/// and has hung the NVIDIA driver, so it is opened once and shared, as quire's harness shares its
/// own. A wgpu device is safe to use from many threads.
static HEADLESS: Mutex<Option<(wgpu::Device, wgpu::Queue)>> = Mutex::new(None);

/// A device and queue made without a window or a surface. The player needs one to build its
/// pipeline even when its slot is empty, so a background session takes this one; a windowed
/// session uses its window's. The first call opens it, asking for low power since nothing is
/// drawn and taking a software adapter when no hardware one opens; later calls share it.
pub fn headless_device() -> Result<(wgpu::Device, wgpu::Queue), MediaError> {
    let mut shared = HEADLESS.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(pair) = shared.as_ref() {
        return Ok(pair.clone());
    }
    let pair = open_headless()?;
    *shared = Some(pair.clone());
    Ok(pair)
}

/// A new headless device and queue, opened under the desktop's one opening lock
/// (`blitz_kit::adapter::opening`): the Vulkan loader crashes when two threads make an instance at
/// once, and quire's harness and every other opener in the process take the same lock.
fn open_headless() -> Result<(wgpu::Device, wgpu::Queue), MediaError> {
    let _one_at_a_time = blitz_kit::adapter::opening();
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    for fallback in [false, true] {
        let request = wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: fallback,
        };
        let Ok(adapter) = pollster::block_on(instance.request_adapter(&request)) else {
            continue;
        };
        let descriptor = wgpu::DeviceDescriptor {
            label: Some("anyview-media"),
            ..wgpu::DeviceDescriptor::default()
        };
        if let Ok(pair) = pollster::block_on(adapter.request_device(&descriptor)) {
            return Ok(pair);
        }
    }
    Err(MediaError::NoDevice)
}

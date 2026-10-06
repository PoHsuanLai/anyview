//! A graphics device on no window, for a session that plays with no picture to show.

use crate::error::MediaError;
use std::sync::{Mutex, PoisonError};

/// Held while a graphics instance and device are opened. The Vulkan loader crashes (in
/// `vkEnumerateInstanceExtensionProperties`) when two threads make an instance at once, so every
/// opener in this process takes it: [`headless_device`] here, and the tests that open their own.
/// quire's harness serialises its openings the same way.
pub static GPU_OPENING: Mutex<()> = Mutex::new(());

/// A device and queue made without a window or a surface. The player needs one to build its
/// pipeline even when its slot is empty, so a background session makes this one; a windowed
/// session uses its window's. Low power is asked for, since nothing is drawn; a software
/// adapter is taken when no hardware one opens.
pub fn headless_device() -> Result<(wgpu::Device, wgpu::Queue), MediaError> {
    let _one_at_a_time = GPU_OPENING.lock().unwrap_or_else(PoisonError::into_inner);
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

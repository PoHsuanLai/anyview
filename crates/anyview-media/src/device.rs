//! A graphics device on no window, for a session that plays with no picture to show.

use crate::error::MediaError;

/// A device and queue made without a window or a surface. The player needs one to build its
/// pipeline even when its slot is empty, so a background session makes this one; a windowed
/// session uses its window's. Low power is asked for, since nothing is drawn; a software
/// adapter is taken when no hardware one opens.
pub fn headless_device() -> Result<(wgpu::Device, wgpu::Queue), MediaError> {
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

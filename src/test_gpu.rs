//! Headless GPU helpers shared by the render/text unit tests.
//!
//! These tests need a real `wgpu::Device` but no window and no surface. On a
//! machine with a GPU (or a software adapter such as lavapipe) they run for
//! real; on a headless CI runner with neither, `headless_device` returns
//! `None` and the caller skips instead of failing.

/// Try to acquire a device without a surface. Prefers a real adapter, then
/// falls back to a software adapter.
pub fn headless_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = wgpu::Backends::all();
    let instance = wgpu::Instance::new(desc);

    let request = |fallback: bool| {
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: fallback,
        }))
    };

    let adapter = match request(false) {
        Ok(a) => a,
        Err(_) => request(true).ok()?,
    };

    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("foundry-headless-test"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::downlevel_defaults(),
        memory_hints: wgpu::MemoryHints::Performance,
        trace: wgpu::Trace::Off,
        experimental_features: wgpu::ExperimentalFeatures::default(),
    }))
    .ok()
}

/// Run `body` with a headless device, or print a skip notice and do nothing.
pub fn with_headless_device(test_name: &str, body: impl FnOnce(&wgpu::Device, &wgpu::Queue)) {
    match headless_device() {
        Some((device, queue)) => body(&device, &queue),
        None => eprintln!("{test_name}: skipped -- no wgpu adapter available on this machine"),
    }
}

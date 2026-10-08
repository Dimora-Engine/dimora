use std::fmt;

/// Failures while setting up or driving the GPU.
#[derive(Debug)]
pub enum RenderError {
    NoAdapter(String),
    Device(String),
    Surface(String),
    Readback(String),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderError::NoAdapter(e) => write!(f, "no suitable GPU adapter: {e}"),
            RenderError::Device(e) => write!(f, "could not create GPU device: {e}"),
            RenderError::Surface(e) => write!(f, "window surface error: {e}"),
            RenderError::Readback(e) => write!(f, "could not read back frame: {e}"),
        }
    }
}
impl std::error::Error for RenderError {}

/// The wgpu objects every renderer needs.
pub struct GpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl GpuContext {
    /// A context with no window, for offscreen rendering and tests.
    pub fn headless() -> Result<Self, RenderError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        pollster::block_on(Self::from_instance(instance, None))
    }

    /// A context able to present to `surface` (which must have been created from `instance`).
    pub async fn from_instance(
        instance: wgpu::Instance,
        compatible_surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<Self, RenderError> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface,
                ..Default::default()
            })
            .await
            .map_err(|e| RenderError::NoAdapter(e.to_string()))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor { label: Some("engine device"), ..Default::default() })
            .await
            .map_err(|e| RenderError::Device(e.to_string()))?;
        Ok(Self { instance, adapter, device, queue })
    }
}

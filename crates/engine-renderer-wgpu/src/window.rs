use engine_assets::AssetManager;
use engine_renderer::RenderScene;

use crate::{GpuContext, RenderError, SceneRenderer};

/// A [`SceneRenderer`] presenting to a window surface.
pub struct WindowRenderer {
    pub gpu: GpuContext,
    pub scene: SceneRenderer,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
}

impl WindowRenderer {
    /// `window` is anything wgpu can create a surface from (e.g. `Arc<winit::window::Window>`).
    /// `display` should be the windowing system's display handle where available (required on
    /// Wayland).
    pub fn new(
        window: impl Into<wgpu::SurfaceTarget<'static>>,
        display: Option<Box<dyn wgpu::wgt::WgpuHasDisplayHandle>>,
        size: (u32, u32),
    ) -> Result<Self, RenderError> {
        let descriptor = match display {
            Some(d) => wgpu::InstanceDescriptor::new_with_display_handle_from_env(d),
            None => wgpu::InstanceDescriptor::new_without_display_handle_from_env(),
        };
        let instance = wgpu::Instance::new(descriptor);
        let surface = instance.create_surface(window).map_err(|e| RenderError::Surface(e.to_string()))?;
        let gpu = pollster::block_on(GpuContext::from_instance(instance, Some(&surface)))?;

        let (w, h) = (size.0.max(1), size.1.max(1));
        let mut config = surface
            .get_default_config(&gpu.adapter, w, h)
            .ok_or_else(|| RenderError::Surface("surface is not supported by the adapter".into()))?;
        // Shaders output linear color; an sRGB target format makes the hardware do the conversion.
        let caps = surface.get_capabilities(&gpu.adapter);
        if let Some(srgb) = caps.formats.iter().copied().find(|f| f.is_srgb()) {
            config.format = srgb;
        }
        surface.configure(&gpu.device, &config);
        let scene = SceneRenderer::new(&gpu, config.format);
        Ok(Self { gpu, scene, surface, config })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.gpu.device, &self.config);
    }

    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    /// Draws and presents one frame. Transient surface problems (resize race, occlusion) skip the
    /// frame instead of failing.
    pub fn render(&mut self, scene: &RenderScene, assets: &AssetManager) -> Result<(), RenderError> {
        use wgpu::CurrentSurfaceTexture as Cst;
        let frame = match self.surface.get_current_texture() {
            Cst::Success(f) | Cst::Suboptimal(f) => f,
            Cst::Outdated | Cst::Lost => {
                self.surface.configure(&self.gpu.device, &self.config);
                return Ok(());
            }
            Cst::Timeout | Cst::Occluded => return Ok(()),
            Cst::Validation => return Err(RenderError::Surface("validation error acquiring frame".into())),
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.scene.render(&self.gpu, scene, assets, &view, (self.config.width, self.config.height));
        self.gpu.queue.present(frame);
        Ok(())
    }
}

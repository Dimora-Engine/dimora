use engine_assets::AssetManager;
use engine_renderer::RenderScene;

use crate::present::{choose_surface_present, surface_presents_from_wgpu, wgpu_present_for, SurfacePresent};
use crate::{GpuContext, RenderError, SceneRenderer};

/// A [`SceneRenderer`] presenting to a window surface.
pub struct WindowRenderer {
    pub gpu: GpuContext,
    pub scene: SceneRenderer,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    /// Set after a lost or outdated surface is logged, cleared after a frame is acquired.
    surface_loss_noted: bool,
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
        Ok(Self { gpu, scene, surface, config, surface_loss_noted: false })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if self.config.width == width && self.config.height == height {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.gpu.device, &self.config);
    }

    /// Selects a present mode for `vsync` from this surface's capabilities and configures the
    /// surface when the mode changes. Returns the mode that is now configured.
    ///
    /// A request that resolves to the mode already in use does not reconfigure the surface.
    pub fn apply_vsync(&mut self, vsync: bool) -> SurfacePresent {
        let listed = self.surface.get_capabilities(&self.gpu.adapter).present_modes;
        let choice = choose_surface_present(vsync, &surface_presents_from_wgpu(&listed));
        let mode = wgpu_present_for(choice, &listed);
        if self.config.present_mode != mode {
            self.config.present_mode = mode;
            if self.config.width > 0 && self.config.height > 0 {
                self.surface.configure(&self.gpu.device, &self.config);
            }
        }
        choice
    }

    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    /// Draws and presents one frame. Transient surface problems (resize race, occlusion) skip the
    /// frame instead of failing.
    pub fn render(&mut self, scene: &RenderScene, assets: &AssetManager) -> Result<(), RenderError> {
        use wgpu::CurrentSurfaceTexture as Cst;
        let frame = match self.surface.get_current_texture() {
            Cst::Success(f) | Cst::Suboptimal(f) => {
                self.surface_loss_noted = false;
                f
            }
            Cst::Outdated | Cst::Lost => {
                note_surface_loss(&mut self.surface_loss_noted);
                self.surface.configure(&self.gpu.device, &self.config);
                return Ok(());
            }
            // Timeout and occlusion can repeat every frame. They skip the present and are not logged.
            Cst::Timeout | Cst::Occluded => return Ok(()),
            Cst::Validation => return Err(RenderError::Surface("validation error acquiring frame".into())),
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.scene.render(&self.gpu, scene, assets, &view, (self.config.width, self.config.height));
        self.gpu.queue.present(frame);
        Ok(())
    }
}

/// Logs the first loss in a run of lost or outdated acquires. A later successful acquire clears
/// `noted`, so the next loss logs again.
fn note_surface_loss(noted: &mut bool) {
    if *noted {
        return;
    }
    *noted = true;
    log::warn!("window surface lost; reconfiguring");
}

#[cfg(test)]
mod tests {
    use super::*;

    mod capture {
        include!("../../log_capture.rs");
    }

    #[test]
    fn a_surface_loss_warns_once_until_the_next_acquire() {
        capture::begin();
        let mut noted = false;
        note_surface_loss(&mut noted);
        note_surface_loss(&mut noted);
        assert!(noted);
        assert_eq!(capture::messages().len(), 1);
        assert!(capture::messages()[0].contains("window surface lost"));
        assert_eq!(capture::records()[0].level, log::Level::Warn);

        noted = false;
        note_surface_loss(&mut noted);
        assert_eq!(capture::messages().len(), 2, "a later loss is a new occurrence");
    }
}

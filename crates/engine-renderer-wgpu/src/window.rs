use engine_assets::AssetManager;
use engine_renderer::{MsaaSamples, RenderScene};

use crate::present::{choose_surface_present, surface_presents_from_wgpu, wgpu_present_for, SurfacePresent};
use crate::{GpuContext, RenderError, SceneRenderer};

/// Usage the window swapchain is configured with.
///
/// A 4× `CameraClear::Load` samples a private frame image, so the native surface only needs to
/// be a color attachment. `TEXTURE_BINDING` is not requested: several backends, including Metal,
/// omit it from [`wgpu::SurfaceCapabilities::usages`], and configuring it is a validation error.
pub(crate) fn window_surface_usage(available: wgpu::TextureUsages) -> Result<wgpu::TextureUsages, String> {
    let requested = wgpu::TextureUsages::RENDER_ATTACHMENT;
    if available.contains(requested) {
        Ok(requested)
    } else {
        Err(format!("window surface does not support RENDER_ATTACHMENT; supported usages are {available:?}"))
    }
}

/// A [`SceneRenderer`] presenting to a window surface.
pub struct WindowRenderer {
    pub gpu: GpuContext,
    pub scene: SceneRenderer,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    /// Usages reported by the surface before configuration. The configured usage is a subset.
    supported_usages: wgpu::TextureUsages,
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
        config.usage = window_surface_usage(caps.usages).map_err(RenderError::Surface)?;
        configure_surface(&surface, &gpu.device, &config)?;
        let scene = SceneRenderer::new(&gpu, config.format);
        Ok(Self { gpu, scene, surface, config, supported_usages: caps.usages, surface_loss_noted: false })
    }

    /// Format configured on the window surface.
    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    /// Usage the surface was configured with. This is [`wgpu::TextureUsages::RENDER_ATTACHMENT`].
    pub fn surface_usage(&self) -> wgpu::TextureUsages {
        self.config.usage
    }

    /// Usages the surface advertised before configuration.
    pub fn supported_surface_usages(&self) -> wgpu::TextureUsages {
        self.supported_usages
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
        if let Err(error) = configure_surface(&self.surface, &self.gpu.device, &self.config) {
            log::error!("window surface resize was rejected: {error}");
        }
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
                if let Err(error) = configure_surface(&self.surface, &self.gpu.device, &self.config) {
                    log::error!("window surface present-mode change was rejected: {error}");
                }
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
                if let Err(error) = configure_surface(&self.surface, &self.gpu.device, &self.config) {
                    log::error!("window surface reconfigure was rejected: {error}");
                }
                return Ok(());
            }
            // Timeout and occlusion can repeat every frame. They skip the present and are not logged.
            Cst::Timeout | Cst::Occluded => return Ok(()),
            Cst::Validation => return Err(RenderError::Surface("validation error acquiring frame".into())),
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let size = (self.config.width, self.config.height);
        // One sample draws into the swapchain. Four samples resolve into a private image, because
        // this surface is not shader-readable, then copy that image here.
        let presented = if self.scene.msaa() == MsaaSamples::Four {
            self.scene.present_to_surface(&self.gpu, scene, assets, &view, size)
        } else {
            self.scene.drop_surface_frame();
            self.scene.render(&self.gpu, scene, assets, &view, size);
            Ok(())
        };
        match presented {
            Ok(()) => {
                self.gpu.queue.present(frame);
                Ok(())
            }
            Err(error) => Err(error),
        }
    }
}

/// Configures `surface` and returns a validation error instead of panicking.
fn configure_surface(
    surface: &wgpu::Surface<'_>,
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
) -> Result<(), RenderError> {
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    surface.configure(device, config);
    match pollster::block_on(scope.pop()) {
        Some(error) => Err(RenderError::Surface(format!("surface configuration was rejected: {error}"))),
        None => Ok(()),
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

    #[test]
    fn window_usage_is_the_color_attachment_when_that_usage_is_advertised() {
        let available = wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::RENDER_ATTACHMENT;
        assert_eq!(window_surface_usage(available).unwrap(), wgpu::TextureUsages::RENDER_ATTACHMENT);
    }

    #[test]
    fn window_usage_fails_when_the_surface_cannot_be_a_color_attachment() {
        let error = window_surface_usage(wgpu::TextureUsages::COPY_SRC).unwrap_err();
        assert!(error.contains("RENDER_ATTACHMENT"));
        assert!(error.contains("COPY_SRC"));
    }
}

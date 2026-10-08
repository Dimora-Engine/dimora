use std::time::Duration;

use engine_animation::animate_system;
use engine_assets::AssetManager;
use engine_core::{FixedTimestep, Input, InputEvent, Time};
use engine_math::Vec2;
use engine_platform::WindowConfig;
use engine_renderer::{extract_scene, AmbientLight, ExtractStats, RenderScene};
use engine_renderer_wgpu::{FrameStats, GpuContext, RenderError, SceneRenderer};
use engine_world::{propagate_transforms, Schedule, World};

/// Startup options.
#[derive(Clone, Debug)]
pub struct GameConfig {
    pub window: WindowConfig,
    /// Rate of the `fixed_update` schedule.
    pub fixed_hz: u32,
    /// If set, assets are read from this directory; otherwise the asset manager is in-memory and
    /// everything is inserted from code.
    pub asset_root: Option<std::path::PathBuf>,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self { window: WindowConfig::default(), fixed_hz: 60, asset_root: None }
    }
}

impl GameConfig {
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.window.title = title.into();
        self
    }
    pub fn with_size(mut self, width: u32, height: u32) -> Self {
        self.window.width = width;
        self.window.height = height;
        self
    }
}

/// Size of the render target in physical pixels. Updated by the runner on resize.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport(pub Vec2);

/// Lets systems talk to the window without depending on it.
#[derive(Clone, Debug, Default)]
pub struct WindowControl {
    /// Set to `true` to end the run loop after this frame.
    pub exit: bool,
    /// If `Some`, the window title is changed and this is reset to `None`.
    pub title: Option<String>,
}

/// Statistics from the most recent rendered frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct RenderStats {
    pub extract: ExtractStats,
    pub frame: FrameStats,
}

/// A world plus the frame pipeline that drives it.
pub struct Game {
    pub world: World,
    /// Runs every frame with variable `Time::delta`.
    pub update: Schedule,
    /// Runs zero or more times per frame at a fixed rate (`GameConfig::fixed_hz`).
    pub fixed_update: Schedule,
    pub(crate) config: GameConfig,
    fixed: FixedTimestep,
    headless: Option<(GpuContext, SceneRenderer)>,
}

impl Game {
    pub fn new(config: GameConfig) -> Self {
        let mut world = World::new();
        let assets = match &config.asset_root {
            Some(root) => AssetManager::filesystem(root),
            None => AssetManager::in_memory(),
        };
        world.insert_resource(Time::default());
        world.insert_resource(Input::default());
        world.insert_resource(assets);
        world.insert_resource(AmbientLight::default());
        world.insert_resource(WindowControl::default());
        world.insert_resource(RenderStats::default());
        world.insert_resource(Viewport(Vec2::new(config.window.width as f32, config.window.height as f32)));
        let fixed = FixedTimestep::new(config.fixed_hz);
        Self { world, update: Schedule::new(), fixed_update: Schedule::new(), config, fixed, headless: None }
    }

    pub fn assets(&self) -> std::cell::RefMut<'_, AssetManager> {
        self.world.resource_mut()
    }

    pub fn add_system(&mut self, name: &'static str, system: impl FnMut(&mut World) + 'static) -> &mut Self {
        self.update.add_system(name, system);
        self
    }

    pub fn add_fixed_system(&mut self, name: &'static str, system: impl FnMut(&mut World) + 'static) -> &mut Self {
        self.fixed_update.add_system(name, system);
        self
    }

    /// Feeds a device event into the `Input` resource.
    pub fn send_input(&mut self, event: InputEvent) {
        self.world.resource_mut::<Input>().apply(event);
    }

    /// Advances the simulation by `dt`. The windowed loop calls this once per frame; tests and
    /// tools can call it directly with any `dt` for fully deterministic stepping.
    pub fn step(&mut self, dt: Duration) {
        self.world.resource_mut::<Time>().advance(dt);
        self.world.resource_mut::<AssetManager>().process_loads();

        for _ in 0..self.fixed.accumulate(dt) {
            self.fixed_update.run(&mut self.world);
        }
        self.update.run(&mut self.world);
        animate_system(&mut self.world);
        propagate_transforms(&mut self.world);
        self.world.resource_mut::<Input>().end_frame();
    }

    /// Snapshots the world for the active camera at the current `Viewport`.
    pub fn extract(&self) -> Option<RenderScene> {
        let viewport = self.world.resource::<Viewport>().0;
        extract_scene(&self.world, &self.world.resource::<AssetManager>(), viewport)
    }

    /// Renders the current state to an RGBA8 image without opening a window (screenshots, tests).
    /// Creates a GPU context on first use; needs a Vulkan/Metal/DX12 adapter (software is fine).
    pub fn render_to_rgba(&mut self, width: u32, height: u32) -> Result<Vec<u8>, RenderError> {
        if self.headless.is_none() {
            let gpu = GpuContext::headless()?;
            let renderer = SceneRenderer::new(&gpu, wgpu_rgba8_srgb());
            self.headless = Some((gpu, renderer));
        }
        self.world.insert_resource(Viewport(Vec2::new(width as f32, height as f32)));
        let scene = self.extract().ok_or_else(|| RenderError::Device("no active camera in the world".into()))?;
        let (gpu, renderer) = self.headless.as_mut().expect("created above");
        let assets = self.world.resource::<AssetManager>();
        let pixels = renderer.render_to_rgba(gpu, &scene, &assets, width, height)?;
        *self.world.resource_mut::<RenderStats>() = RenderStats { extract: scene.stats, frame: renderer.stats() };
        Ok(pixels)
    }

    /// Opens a window and runs until it is closed or [`WindowControl::exit`] is set.
    pub fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        crate::runner::run(self)
    }
}

fn wgpu_rgba8_srgb() -> engine_renderer_wgpu::wgpu::TextureFormat {
    engine_renderer_wgpu::wgpu::TextureFormat::Rgba8UnormSrgb
}

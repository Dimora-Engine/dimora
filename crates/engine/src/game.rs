use std::time::Duration;

use engine_animation::animate_system;
use engine_assets::AssetManager;
use engine_core::{FixedTimestep, Input, InputEvent, Time};
use engine_math::Vec2;
use engine_platform::WindowConfig;
use engine_renderer::{extract_scene, AmbientLight, ExtractStats, RenderScene};
use engine_renderer_wgpu::{CacheStats, FrameStats, GpuContext, RenderError, SceneRenderer};
use engine_world::{propagate_transforms, Schedule, ScheduleError, World};

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
    /// GPU copies of textures and meshes held by the renderer after the frame.
    pub cache: CacheStats,
}

/// One world stage in [`Game::step`], in execution order.
///
/// Extract and render are not stages. The runner calls them after `step`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Time,
    Loads,
    FixedUpdate,
    Update,
    Animate,
    PostUpdate,
    InputClear,
}

impl Stage {
    pub const ADVANCE_TIME: &'static str = "advance_time";
    pub const PROCESS_LOADS: &'static str = "process_loads";
    pub const ANIMATE: &'static str = "animate";
    pub const PROPAGATE_TRANSFORMS: &'static str = "propagate_transforms";
    pub const END_INPUT_FRAME: &'static str = "end_input_frame";

    pub const ALL: [Stage; 7] = [
        Stage::Time,
        Stage::Loads,
        Stage::FixedUpdate,
        Stage::Update,
        Stage::Animate,
        Stage::PostUpdate,
        Stage::InputClear,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Stage::Time => "time",
            Stage::Loads => "loads",
            Stage::FixedUpdate => "fixed_update",
            Stage::Update => "update",
            Stage::Animate => "animate",
            Stage::PostUpdate => "post_update",
            Stage::InputClear => "input_clear",
        }
    }

    /// Built-in system for this stage, and whether unconstrained systems run after it (`First`)
    /// or before it (`Last`). `fixed_update` and `update` have no built-in.
    fn builtin(self) -> Option<(&'static str, BuiltinPlace)> {
        match self {
            Stage::Time => Some((Self::ADVANCE_TIME, BuiltinPlace::First)),
            Stage::Loads => Some((Self::PROCESS_LOADS, BuiltinPlace::First)),
            Stage::FixedUpdate | Stage::Update => None,
            Stage::Animate => Some((Self::ANIMATE, BuiltinPlace::First)),
            Stage::PostUpdate => Some((Self::PROPAGATE_TRANSFORMS, BuiltinPlace::Last)),
            Stage::InputClear => Some((Self::END_INPUT_FRAME, BuiltinPlace::Last)),
        }
    }
}

#[derive(Clone, Copy)]
enum BuiltinPlace {
    /// Unconstrained systems run after the built-in.
    First,
    /// Unconstrained systems run before the built-in.
    Last,
}

#[derive(Clone, Copy)]
enum ExplicitOrder {
    Before(&'static str),
    After(&'static str),
}

/// The `dt` argument of the current [`Game::step`], and how many fixed iterations it produced.
#[derive(Clone, Copy, Debug, Default)]
struct StepClock {
    dt: Duration,
    fixed_steps: u32,
}

/// A world plus the frame pipeline that drives it.
pub struct Game {
    pub world: World,
    /// Variable-step systems. [`Game::add_system`] appends here. Runs once per step, after
    /// `fixed_update`, and sees the frame delta.
    pub update: Schedule,
    /// Fixed-step systems. [`Game::add_fixed_system`] appends here. Each iteration sees the
    /// fixed step as `Time::delta`, then the frame delta is restored.
    pub fixed_update: Schedule,
    time: Schedule,
    loads: Schedule,
    animate: Schedule,
    post_update: Schedule,
    input_clear: Schedule,
    pub(crate) config: GameConfig,
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
        world.insert_resource(FixedTimestep::new(config.fixed_hz));
        world.insert_resource(StepClock::default());

        let mut time = Schedule::new();
        time.add_system(Stage::ADVANCE_TIME, advance_time).expect("advance_time");
        let mut loads = Schedule::new();
        loads.add_system(Stage::PROCESS_LOADS, process_loads).expect("process_loads");
        let mut animate = Schedule::new();
        animate.add_system(Stage::ANIMATE, animate_system).expect("animate");
        let mut post_update = Schedule::new();
        post_update.add_system(Stage::PROPAGATE_TRANSFORMS, propagate_transforms).expect("propagate_transforms");
        let mut input_clear = Schedule::new();
        input_clear.add_system(Stage::END_INPUT_FRAME, end_input_frame).expect("end_input_frame");

        Self {
            world,
            update: Schedule::new(),
            fixed_update: Schedule::new(),
            time,
            loads,
            animate,
            post_update,
            input_clear,
            config,
            headless: None,
        }
    }

    pub fn assets(&self) -> std::cell::RefMut<'_, AssetManager> {
        self.world.resource_mut()
    }

    /// Adds a system to the `update` stage, in registration order.
    ///
    /// Panics if `name` is already in that stage. Use [`Game::add_to_stage`] to place a system in
    /// another stage or to receive a [`ScheduleError`] instead of panicking.
    pub fn add_system(&mut self, name: &'static str, system: impl FnMut(&mut World) + 'static) -> &mut Self {
        self.add_to_stage(Stage::Update, name, system).unwrap_or_else(|e| panic!("{e}"));
        self
    }

    /// Adds a system to the `fixed_update` stage, in registration order.
    ///
    /// Panics if `name` is already in that stage.
    pub fn add_fixed_system(&mut self, name: &'static str, system: impl FnMut(&mut World) + 'static) -> &mut Self {
        self.add_to_stage(Stage::FixedUpdate, name, system).unwrap_or_else(|e| panic!("{e}"));
        self
    }

    /// Adds a system to `stage` with no explicit constraint.
    ///
    /// In `time`, `loads` and `animate`, that puts it after the built-in system. In `post_update`
    /// and `input_clear`, that puts it before the built-in. `fixed_update` and `update` have no
    /// built-in, so this is registration order. The target of a constraint has to be registered
    /// already; [`Game::add_to_stage_before`] and [`Game::add_to_stage_after`] are the explicit forms.
    pub fn add_to_stage(
        &mut self,
        stage: Stage,
        name: &'static str,
        system: impl FnMut(&mut World) + 'static,
    ) -> Result<&mut Self, ScheduleError> {
        self.place(stage, name, system, None)?;
        Ok(self)
    }

    /// Adds a system that runs before `before` in `stage`. `before` must already be in that stage.
    pub fn add_to_stage_before(
        &mut self,
        stage: Stage,
        name: &'static str,
        before: &'static str,
        system: impl FnMut(&mut World) + 'static,
    ) -> Result<&mut Self, ScheduleError> {
        self.place(stage, name, system, Some(ExplicitOrder::Before(before)))?;
        Ok(self)
    }

    /// Adds a system that runs after `after` in `stage`. `after` must already be in that stage.
    pub fn add_to_stage_after(
        &mut self,
        stage: Stage,
        name: &'static str,
        after: &'static str,
        system: impl FnMut(&mut World) + 'static,
    ) -> Result<&mut Self, ScheduleError> {
        self.place(stage, name, system, Some(ExplicitOrder::After(after)))?;
        Ok(self)
    }

    /// System names in `stage`, in the order [`Game::step`] runs them.
    pub fn system_order(&self, stage: Stage) -> Vec<&'static str> {
        self.schedule(stage).execution_order()
    }

    /// Feeds a device event into the `Input` resource.
    pub fn send_input(&mut self, event: InputEvent) {
        self.world.resource_mut::<Input>().apply(event);
    }

    /// Advances the simulation by `dt`. The windowed loop calls this once per frame; tests and
    /// tools can call it directly with any `dt` for fully deterministic stepping.
    ///
    /// Stages run in order: `time`, `loads`, `fixed_update` (zero to eight times), `update`,
    /// `animate`, `post_update`, `input_clear`. During each fixed iteration `Time::delta` is the
    /// fixed step; it is restored to `dt` before `update`. `Time::elapsed_secs` and `Time::frame`
    /// advance once per call. Input edges stay visible until `input_clear`. Extract and render
    /// are not part of this call.
    pub fn step(&mut self, dt: Duration) {
        self.world.resource_mut::<StepClock>().dt = dt;
        self.time.run(&mut self.world);
        self.loads.run(&mut self.world);

        let (steps, fixed_dt, frame_delta) = {
            let steps = self.world.resource::<StepClock>().fixed_steps;
            let fixed_dt = self.world.resource::<FixedTimestep>().step();
            let frame_delta = self.world.resource::<Time>().delta();
            (steps, fixed_dt, frame_delta)
        };
        for _ in 0..steps {
            self.world.resource_mut::<Time>().set_delta(fixed_dt);
            self.fixed_update.run(&mut self.world);
        }
        self.world.resource_mut::<Time>().set_delta(frame_delta);

        self.update.run(&mut self.world);
        self.animate.run(&mut self.world);
        self.post_update.run(&mut self.world);
        self.input_clear.run(&mut self.world);
    }

    fn schedule(&self, stage: Stage) -> &Schedule {
        match stage {
            Stage::Time => &self.time,
            Stage::Loads => &self.loads,
            Stage::FixedUpdate => &self.fixed_update,
            Stage::Update => &self.update,
            Stage::Animate => &self.animate,
            Stage::PostUpdate => &self.post_update,
            Stage::InputClear => &self.input_clear,
        }
    }

    fn schedule_mut(&mut self, stage: Stage) -> &mut Schedule {
        match stage {
            Stage::Time => &mut self.time,
            Stage::Loads => &mut self.loads,
            Stage::FixedUpdate => &mut self.fixed_update,
            Stage::Update => &mut self.update,
            Stage::Animate => &mut self.animate,
            Stage::PostUpdate => &mut self.post_update,
            Stage::InputClear => &mut self.input_clear,
        }
    }

    fn place(
        &mut self,
        stage: Stage,
        name: &'static str,
        system: impl FnMut(&mut World) + 'static,
        explicit: Option<ExplicitOrder>,
    ) -> Result<(), ScheduleError> {
        let schedule = self.schedule_mut(stage);
        match explicit {
            Some(ExplicitOrder::Before(before)) => {
                schedule.add_system_before(name, before, system)?;
            }
            Some(ExplicitOrder::After(after)) => {
                schedule.add_system_after(name, after, system)?;
            }
            None => match stage.builtin() {
                Some((builtin, BuiltinPlace::First)) => {
                    schedule.add_system_after(name, builtin, system)?;
                }
                Some((builtin, BuiltinPlace::Last)) => {
                    schedule.add_system_before(name, builtin, system)?;
                }
                None => {
                    schedule.add_system(name, system)?;
                }
            },
        }
        Ok(())
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
        *self.world.resource_mut::<RenderStats>() =
            RenderStats { extract: scene.stats, frame: renderer.stats(), cache: renderer.cache_stats() };
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

fn advance_time(world: &mut World) {
    let dt = world.resource::<StepClock>().dt;
    world.resource_mut::<Time>().advance(dt);
    let steps = world.resource_mut::<FixedTimestep>().accumulate(dt);
    world.resource_mut::<StepClock>().fixed_steps = steps;
}

fn process_loads(world: &mut World) {
    world.resource_mut::<AssetManager>().process_loads();
}

fn end_input_frame(world: &mut World) {
    world.resource_mut::<Input>().end_frame();
}

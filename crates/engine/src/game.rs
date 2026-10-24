use std::time::{Duration, Instant};

use engine_animation::animate_system;
use engine_assets::AssetManager;
use engine_core::{Actions, FixedTimestep, Input, InputEvent, Time};
use engine_platform::WindowConfig;
use engine_renderer::{
    extract_scene, AmbientLight, DebugDraw, ExtractStats, MissingAssetWarnings, RenderScene, RenderTargetStore,
};
use engine_renderer_wgpu::{CacheStats, FrameStats, GpuContext, RenderError, SceneRenderer};
use engine_world::{propagate_transforms, snapshot_interpolation, Schedule, ScheduleError, World};

use crate::diagnostics::{Diagnostics, StageDurations};
use crate::viewport::Viewport;

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
    pub fn with_fullscreen(mut self, fullscreen: bool) -> Self {
        self.window.fullscreen = fullscreen;
        self
    }
    pub fn with_vsync(mut self, vsync: bool) -> Self {
        self.window.vsync = vsync;
        self
    }
    /// `None` or `Some(0)` disables the frame cap.
    pub fn with_max_frame_rate(mut self, frames_per_second: Option<u32>) -> Self {
        self.window.max_frame_rate = frames_per_second;
        self
    }
}

/// Requests a system makes of the window. The runner applies them after `step`.
///
/// [`crate::WindowState`] is what was actually applied. A grab that the window rejects is cleared
/// here and recorded as failed on that state. Focus loss clears `cursor_grabbed` and does not
/// set it again when focus returns.
#[derive(Clone, Debug)]
pub struct WindowControl {
    /// Set to `true` to end the run loop after this frame.
    pub exit: bool,
    /// If `Some`, the window title is changed and this is reset to `None`.
    pub title: Option<String>,
    /// Borderless fullscreen on the current monitor. Exclusive fullscreen is not available.
    pub fullscreen: bool,
    /// Requested vertical sync. The effective mode is [`crate::WindowState::present_mode`].
    pub vsync: bool,
    /// Target frames per second for the event loop. `None` or `Some(0)` disables the cap.
    ///
    /// The cap does not change [`Game::step`]. It is a scheduling target, not a promise that the
    /// display presents at that rate.
    pub max_frame_rate: Option<u32>,
    /// `false` hides the cursor. The runner applies this to the window.
    pub cursor_visible: bool,
    /// Ask to grab the cursor. The runner tries locked, then confined, and clears this if both fail.
    pub cursor_grabbed: bool,
}

impl Default for WindowControl {
    fn default() -> Self {
        Self::from_config(&WindowConfig::default())
    }
}

impl WindowControl {
    pub fn from_config(config: &WindowConfig) -> Self {
        Self {
            exit: false,
            title: None,
            fullscreen: config.fullscreen,
            vsync: config.vsync,
            max_frame_rate: config.max_frame_rate.filter(|hz| *hz > 0),
            cursor_visible: true,
            cursor_grabbed: false,
        }
    }

    /// Focus loss drops the grab request. Focus gain does not set it again.
    pub(crate) fn release_grab_request(&mut self) {
        self.cursor_grabbed = false;
    }
}

/// Statistics from the most recent rendered frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
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

    /// Where this stage's built-in sits. `fixed_update` and `update` have none.
    ///
    /// `SealedFirst` and `SealedLast` are lifecycle ends: every system in the stage stays on the
    /// built-in's open side, including one with an explicit `before` / `after`. `FlexibleFirst`
    /// is the default place only; an explicit constraint may run on either side.
    fn anchor(self) -> Option<Anchor> {
        match self {
            Stage::Time => Some(Anchor::SealedFirst(Self::ADVANCE_TIME)),
            Stage::Loads => Some(Anchor::SealedFirst(Self::PROCESS_LOADS)),
            Stage::FixedUpdate | Stage::Update => None,
            Stage::Animate => Some(Anchor::FlexibleFirst(Self::ANIMATE)),
            Stage::PostUpdate => Some(Anchor::SealedLast(Self::PROPAGATE_TRANSFORMS)),
            Stage::InputClear => Some(Anchor::SealedLast(Self::END_INPUT_FRAME)),
        }
    }
}

#[derive(Clone, Copy)]
enum Anchor {
    /// Every system runs after this built-in.
    SealedFirst(&'static str),
    /// Unconstrained systems run after this built-in. An explicit constraint may run on either side.
    FlexibleFirst(&'static str),
    /// Every system runs before this built-in.
    SealedLast(&'static str),
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
        crate::registry::register_builtin_components(&world).expect("built-in components have distinct names");
        let assets = match &config.asset_root {
            Some(root) => AssetManager::filesystem(root),
            None => AssetManager::in_memory(),
        };
        world.insert_resource(Time::default());
        world.insert_resource(Input::default());
        world.insert_resource(Actions::default());
        world.insert_resource(assets);
        world.insert_resource(AmbientLight::default());
        world.insert_resource(WindowControl::from_config(&config.window));
        world.insert_resource(crate::window::WindowState {
            fullscreen: config.window.fullscreen,
            vsync_requested: config.window.vsync,
            max_frame_rate: config.window.max_frame_rate.filter(|hz| *hz > 0),
            ..crate::window::WindowState::default()
        });
        world.insert_resource(RenderStats::default());
        world.insert_resource(Diagnostics::default());
        world.insert_resource(MissingAssetWarnings::default());
        world.insert_resource(RenderTargetStore::new());
        world.insert_resource(Viewport::from_logical_pixels(config.window.width, config.window.height));
        world.insert_resource(FixedTimestep::new(config.fixed_hz));
        world.insert_resource(StepClock::default());
        world.insert_resource(DebugDraw::default());

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
    ///
    /// `advance_time` and `process_loads` stay first in their stages. `propagate_transforms` and
    /// `end_input_frame` stay last. A constraint on the closed side returns
    /// [`ScheduleError::MustRunAfter`] or [`ScheduleError::MustRunBefore`] and leaves the schedule
    /// unchanged. `animate` is the exception: an explicit constraint may place a system before or
    /// after it. Restoring the frame delta after `fixed_update` is done by [`Game::step`], so
    /// system order cannot move it.
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

    /// Feeds a device event into `Input`, then updates `Actions` from the new held state.
    pub fn send_input(&mut self, event: InputEvent) {
        self.world.resource_mut::<Input>().apply(event);
        let input = self.world.resource::<Input>().clone();
        self.world.resource_mut::<Actions>().sync(&input);
    }

    /// Advances the simulation by `dt`. The windowed loop calls this once per frame; tests and
    /// tools can call it directly with any `dt` for fully deterministic stepping.
    ///
    /// Stages run in order: `time`, `loads`, `fixed_update` (zero to eight times), `update`,
    /// `animate`, `post_update`, `input_clear`. During each fixed iteration `Time::delta` is the
    /// fixed step; it is restored to `dt` before `update`. At the start of each fixed iteration,
    /// before fixed-update systems, a marked entity's current local is copied into
    /// [`engine_world::Interpolate::previous`]. That copy is not a system. `Time::elapsed_secs` and `Time::frame`
    /// advance once per call. Frame input edges stay visible until `input_clear`. Fixed-step edges
    /// are published at the start of this call and concealed after the first fixed iteration's
    /// systems and command flush. Later iterations do not see them. A step with no fixed iteration
    /// leaves them published. An edge stored after conceal waits for the next call. Extract and
    /// render are not part of this call.
    ///
    /// Deferred commands flush after every stage run, and after each fixed iteration. Events sent
    /// in a stage become visible to later stages once that stage's run has finished. `fixed_update`
    /// is one stage: its events stay invisible through the remaining iterations and become visible
    /// to `update`. Events still pending after `input_clear` are dropped. [`World::discarded_events`]
    /// counts them until the next call. Command failures stay readable until the next call, each
    /// tagged with the stage that flushed it. Each of those failures is logged once, at the end of
    /// this call. The next call clears the list before any system runs, so an old failure is not
    /// logged again. A deferred scene operation that fails is logged once when its
    /// [`engine_world::SceneOpStatus`] becomes `Failed`. That log is not a command failure, and
    /// this call does not clear the scene-operation record.
    ///
    /// [`Diagnostics`] records `dt` as [`Diagnostics::frame`] and the CPU time of each stage
    /// separately. `dt` is simulation time. It is not how long the call took. The fixed-update
    /// measurement is the CPU time of the whole fixed loop. [`Diagnostics::render`] is left as the
    /// most recent successful render; this call does not extract or render.
    ///
    /// [`DebugDraw`] is cleared before any stage runs. A segment submitted from `fixed_update`,
    /// `update`, `animate`, `post_update`, or `input_clear` is still present when this call
    /// returns, so the extract after this step can see it. A second extract before the next call
    /// sees the same segments. The next call drops them. A removed `DebugDraw` resource is
    /// inserted empty and then cleared, so a system can submit during this call.
    pub fn step(&mut self, dt: Duration) {
        self.world.clear_command_failures();
        self.world.clear_discarded_events();
        if self.world.try_resource::<DebugDraw>().is_none() {
            self.world.insert_resource(DebugDraw::default());
        }
        self.world.resource_mut::<DebugDraw>().clear();
        self.world.resource_mut::<Input>().publish_fixed_edges();
        self.world.resource_mut::<Actions>().publish_fixed_edges();
        self.world.resource_mut::<StepClock>().dt = dt;

        let started = Instant::now();
        self.time.run(&mut self.world);
        self.after_stage(Stage::Time);
        let time = started.elapsed();

        let started = Instant::now();
        self.loads.run(&mut self.world);
        self.after_stage(Stage::Loads);
        let loads = started.elapsed();

        let (steps, fixed_dt, frame_delta) = {
            let steps = self.world.resource::<StepClock>().fixed_steps;
            let fixed_dt = self.world.resource::<FixedTimestep>().step();
            let frame_delta = self.world.resource::<Time>().delta();
            (steps, fixed_dt, frame_delta)
        };
        let started = Instant::now();
        for index in 0..steps {
            self.world.resource_mut::<Time>().set_delta(fixed_dt);
            snapshot_interpolation(&mut self.world);
            self.fixed_update.run(&mut self.world);
            self.world.flush_commands_in(Stage::FixedUpdate.name());
            if index == 0 {
                self.world.resource_mut::<Input>().conceal_fixed_edges();
                self.world.resource_mut::<Actions>().conceal_fixed_edges();
            }
        }
        self.world.promote_events();
        let fixed_update = started.elapsed();
        self.world.resource_mut::<Time>().set_delta(frame_delta);

        let started = Instant::now();
        self.update.run(&mut self.world);
        self.after_stage(Stage::Update);
        let update = started.elapsed();

        let started = Instant::now();
        self.animate.run(&mut self.world);
        self.after_stage(Stage::Animate);
        let animate = started.elapsed();

        let started = Instant::now();
        self.post_update.run(&mut self.world);
        self.after_stage(Stage::PostUpdate);
        let post_update = started.elapsed();

        let started = Instant::now();
        self.input_clear.run(&mut self.world);
        self.world.flush_commands_in(Stage::InputClear.name());
        let input_clear = started.elapsed();
        self.world.clear_events();

        let failures = self.world.command_failures();
        for failure in &failures {
            log::error!("command failed: {failure}");
        }
        let entities = self.world.entities.len();
        let discarded_events = self.world.discarded_events();
        let fixed_steps = self.world.resource::<StepClock>().fixed_steps;
        let mut diagnostics = self.world.resource_mut::<Diagnostics>();
        diagnostics.frame = dt;
        diagnostics.fixed_steps = fixed_steps;
        diagnostics.stages = StageDurations { time, loads, fixed_update, update, animate, post_update, input_clear };
        diagnostics.entities = entities;
        diagnostics.command_failures = failures.len() as u32;
        diagnostics.discarded_events = discarded_events;
    }

    fn after_stage(&mut self, stage: Stage) {
        self.world.flush_commands_in(stage.name());
        self.world.promote_events();
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
        let anchor = stage.anchor();
        if let Some(error) = explicit.and_then(|order| sealed_violation(name, anchor, order)) {
            return Err(error);
        }
        let mut before = Vec::new();
        let mut after = Vec::new();
        match anchor {
            Some(Anchor::SealedFirst(builtin)) => after.push(builtin),
            Some(Anchor::SealedLast(builtin)) => before.push(builtin),
            Some(Anchor::FlexibleFirst(builtin)) if explicit.is_none() => after.push(builtin),
            Some(Anchor::FlexibleFirst(_)) | None => {}
        }
        match explicit {
            Some(ExplicitOrder::Before(target)) => before.push(target),
            Some(ExplicitOrder::After(target)) => after.push(target),
            None => {}
        }
        self.schedule_mut(stage).add_ordered(name, &before, &after, system)?;
        Ok(())
    }

    /// Snapshots the world for the active camera at the current `Viewport`.
    pub fn extract(&self) -> Option<RenderScene> {
        let viewport = self.world.resource::<Viewport>().physical_size_pixels();
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
        self.world.insert_resource(Viewport::from_logical_pixels(width, height));
        let scene = self.extract().ok_or_else(|| RenderError::Device("no active camera in the world".into()))?;
        let (gpu, renderer) = self.headless.as_mut().expect("created above");
        let assets = self.world.resource::<AssetManager>();
        let pixels = renderer.render_to_rgba(gpu, &scene, &assets, width, height)?;
        let stats = RenderStats { extract: scene.stats, frame: renderer.stats(), cache: renderer.cache_stats() };
        *self.world.resource_mut::<RenderStats>() = stats;
        self.world.resource_mut::<Diagnostics>().render = stats;
        Ok(pixels)
    }

    /// Opens a window and runs until it is closed or [`WindowControl::exit`] is set.
    ///
    /// Installs the default stderr logger when the application has not installed one.
    pub fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        crate::logging::install_stderr_logger();
        crate::runner::run(self)
    }
}

fn wgpu_rgba8_srgb() -> engine_renderer_wgpu::wgpu::TextureFormat {
    engine_renderer_wgpu::wgpu::TextureFormat::Rgba8UnormSrgb
}

fn sealed_violation(name: &'static str, anchor: Option<Anchor>, order: ExplicitOrder) -> Option<ScheduleError> {
    match (anchor, order) {
        (Some(Anchor::SealedFirst(anchor)), ExplicitOrder::Before(target)) if target == anchor => {
            Some(ScheduleError::MustRunAfter { system: name, anchor })
        }
        (Some(Anchor::SealedLast(anchor)), ExplicitOrder::After(target)) if target == anchor => {
            Some(ScheduleError::MustRunBefore { system: name, anchor })
        }
        _ => None,
    }
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
    world.resource_mut::<Actions>().end_frame();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_loss_clears_a_grab_request_and_focus_gain_does_not_restore_it() {
        let mut game = Game::new(GameConfig::default());
        {
            let mut control = game.world.resource_mut::<WindowControl>();
            control.cursor_grabbed = true;
            control.cursor_visible = false;
            control.release_grab_request();
        }
        assert!(!game.world.resource::<WindowControl>().cursor_grabbed);
        assert!(!game.world.resource::<WindowControl>().cursor_visible);
        game.send_input(InputEvent::FocusGained);
        assert!(!game.world.resource::<WindowControl>().cursor_grabbed);
        assert_eq!(game.world.resource::<crate::WindowState>().cursor_grab, crate::CursorGrab::Free);
    }
}

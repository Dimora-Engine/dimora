//! Native-window smoke test for surface usage and optional 4× presentation.
//!
//! This process is the main thread. It presents frames and records validation errors. It does not
//! read swapchain pixels. Headless tests cover those bytes.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use engine::assets::{AssetManager, ColorSpace, SpriteAsset, Texture};
use engine::math::{Transform, Vec2};
use engine::platform::winit::event_loop::OwnedDisplayHandle;
use engine::platform::winit::window::Window;
use engine::platform::{run, App, Flow, WindowConfig};
use engine::renderer::{extract_scene, Camera, CameraClear, MsaaSamples, ViewportRect};
use engine::renderer_wgpu::WindowRenderer;
use engine::world::{propagate_transforms, World};

struct Probe {
    window: Option<Arc<Window>>,
    display: Option<OwnedDisplayHandle>,
    renderer: Option<WindowRenderer>,
    errors: Arc<Mutex<Vec<String>>>,
    world: World,
    assets: AssetManager,
    phase: Phase,
    frames_in_phase: u32,
    four_creates_after_steady: u64,
    failure: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    One,
    Four,
    OneAgain,
    FourAgain,
}

fn main() {
    let mut assets = AssetManager::in_memory();
    let texture =
        assets.insert("white", Texture::solid(1, 1, [80, 140, 220, 255]).with_color_space(ColorSpace::Linear)).unwrap();
    assets.insert("sprite", SpriteAsset::whole_texture(texture, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    let mut world = World::new();
    world.spawn((
        Camera::orthographic(4.0).with_clear_color(engine::core::Color::rgb(0.7, 0.1, 0.1)),
        Transform::from_xyz(0.0, 0.0, -4.0),
    ));
    world.spawn((
        Camera {
            order: 1,
            ..Camera::orthographic(4.0)
                .with_clear(CameraClear::Load)
                .with_viewport(ViewportRect::new(0.25, 0.2, 0.5, 0.6))
        },
        Transform::from_xyz(0.4, 0.2, -6.0),
    ));
    world.spawn((
        Transform::IDENTITY,
        engine::renderer::Sprite::new("sprite").with_size(Vec2::splat(0.8)).with_anchor(Vec2::splat(0.5)).unlit(),
    ));

    let mut probe = Probe {
        window: None,
        display: None,
        renderer: None,
        errors: Arc::new(Mutex::new(Vec::new())),
        world,
        assets,
        phase: Phase::One,
        frames_in_phase: 0,
        four_creates_after_steady: 0,
        failure: None,
    };
    if let Err(error) = run(
        WindowConfig {
            title: "Dimora window MSAA".into(),
            width: 480,
            height: 270,
            fullscreen: false,
            vsync: true,
            max_frame_rate: Some(60),
        },
        &mut probe,
    ) {
        eprintln!("platform error: {error}");
        std::process::exit(1);
    }
    if let Some(failure) = probe.failure {
        eprintln!("window msaa: {failure}");
        std::process::exit(1);
    }
    println!("window_msaa: presented 1×, 4× before the second renderer drew, resize, vsync, and 1× again");
}

impl Probe {
    fn fail(&mut self, message: impl Into<String>) {
        let message = message.into();
        eprintln!("{message}");
        self.failure = Some(message);
    }

    fn open_renderer(&mut self, four_before_draw: bool) -> bool {
        let (Some(window), Some(display)) = (self.window.clone(), self.display.clone()) else {
            self.fail("window was not retained");
            return false;
        };
        let size = window.inner_size();
        self.renderer = None;
        let mut renderer = match WindowRenderer::new(window, Some(Box::new(display)), (size.width, size.height)) {
            Ok(renderer) => renderer,
            Err(error) => {
                self.fail(format!("WindowRenderer::new failed: {error}"));
                return false;
            }
        };
        let errors = Arc::clone(&self.errors);
        renderer.gpu.device.on_uncaptured_error(Arc::new(move |error| {
            errors.lock().expect("error lock").push(error.to_string());
        }));
        println!(
            "surface format={:?} configured={:?} advertised={:?}",
            renderer.surface_format(),
            renderer.surface_usage(),
            renderer.supported_surface_usages()
        );
        if renderer.surface_usage() != engine::renderer_wgpu::wgpu::TextureUsages::RENDER_ATTACHMENT {
            self.fail(format!("configured usage is {:?}, expected RENDER_ATTACHMENT", renderer.surface_usage()));
            return false;
        }
        if !renderer.supported_surface_usages().contains(renderer.surface_usage()) {
            self.fail("configured usage is not in the advertised set");
            return false;
        }
        if four_before_draw {
            if let Err(error) = renderer.scene.set_msaa(&renderer.gpu, MsaaSamples::Four) {
                self.fail(format!("set_msaa before the first frame failed: {error}"));
                return false;
            }
            if renderer.scene.msaa() != MsaaSamples::Four {
                self.fail("4× requested before the first frame was not active");
                return false;
            }
            if renderer.scene.surface_frame().is_some() {
                self.fail("4× before the first frame allocated the private image during set_msaa");
                return false;
            }
        }
        self.renderer = Some(renderer);
        self.frames_in_phase = 0;
        true
    }

    fn draw(&mut self) -> bool {
        let Some(renderer) = self.renderer.as_mut() else {
            self.fail("no renderer");
            return false;
        };
        let (width, height) = renderer.size();
        propagate_transforms(&mut self.world);
        let Some(scene) = extract_scene(&self.world, &self.assets, Vec2::new(width as f32, height as f32)) else {
            self.fail("extract returned no cameras");
            return false;
        };
        if let Err(error) = renderer.render(&scene, &self.assets) {
            self.fail(format!("render failed: {error}"));
            return false;
        }
        let validation = {
            let errors = self.errors.lock().expect("error lock");
            if errors.is_empty() {
                None
            } else {
                Some(errors.join(" | "))
            }
        };
        if let Some(validation) = validation {
            self.fail(format!("gpu validation: {validation}"));
            return false;
        }
        true
    }
}

impl App for Probe {
    fn init(&mut self, window: Arc<Window>, display: OwnedDisplayHandle) {
        window.set_visible(true);
        window.focus_window();
        self.window = Some(window);
        self.display = Some(display);
        if !self.open_renderer(false) {
            return;
        }
        let renderer = self.renderer.as_ref().unwrap();
        if renderer.scene.msaa() != MsaaSamples::One {
            self.fail("a new window renderer was not one sample");
        }
    }

    fn input(&mut self, _event: engine::core::InputEvent) {}

    fn resize(&mut self, width: u32, height: u32) {
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.resize(width, height);
        }
    }

    fn scale_factor(&mut self, _scale: f64) {}

    fn frame(&mut self, _dt: Duration) -> Flow {
        if self.failure.is_some() {
            return Flow::Exit;
        }
        if !self.draw() {
            return Flow::Exit;
        }
        self.frames_in_phase += 1;
        let mut problem = None;
        match self.phase {
            Phase::One => {
                problem = {
                    let renderer = self.renderer.as_ref().unwrap();
                    if renderer.scene.surface_frame().is_some() || renderer.scene.stats().frame_present_draws != 0 {
                        Some("one-sample window allocated a private frame or encoded the 4× copy".into())
                    } else if renderer.scene.msaa_images().image_count != 0 {
                        Some("one-sample window allocated 4× images".into())
                    } else {
                        None
                    }
                };
                if problem.is_none() && self.frames_in_phase == 2 {
                    let renderer = self.renderer.as_mut().unwrap();
                    let (width, height) = renderer.size();
                    renderer.resize(width + 40, height + 24);
                    let _ = renderer.apply_vsync(false);
                    let _ = renderer.apply_vsync(true);
                }
                if problem.is_none() && self.frames_in_phase >= 4 {
                    self.phase = Phase::Four;
                    if !self.open_renderer(true) {
                        return Flow::Exit;
                    }
                }
            }
            Phase::Four => {
                let snapshot = {
                    let renderer = self.renderer.as_ref().unwrap();
                    let frame = renderer.scene.surface_frame();
                    let (width, height) = renderer.size();
                    (renderer.scene.stats(), frame, width, height, renderer.scene.resource_stats())
                };
                let (stats, frame, width, height, resources) = snapshot;
                if let Some(frame) = frame {
                    if frame.width != width || frame.height != height {
                        problem = Some(format!(
                            "private frame is {}x{}, window is {width}x{height}",
                            frame.width, frame.height
                        ));
                    } else if stats.frame_present_draws != 1 {
                        problem = Some(format!("4× frame present draws = {}", stats.frame_present_draws));
                    } else if stats.expand_draws == 0 || stats.resolve_draws == 0 {
                        problem = Some("4× overlapping Load did not expand and resolve".into());
                    }
                } else {
                    problem = Some("4× window did not retain a private frame image".into());
                }
                if problem.is_none() && self.frames_in_phase == 2 {
                    self.four_creates_after_steady = resources.surface_frame_creates;
                    let renderer = self.renderer.as_mut().unwrap();
                    let (width, height) = renderer.size();
                    renderer.resize(width + 30, height + 16);
                }
                if problem.is_none() && self.frames_in_phase == 3 {
                    let renderer = self.renderer.as_ref().unwrap();
                    let creates = renderer.scene.resource_stats().surface_frame_creates;
                    let reallocs = renderer.scene.resource_stats().surface_frame_reallocs;
                    if creates != self.four_creates_after_steady {
                        problem =
                            Some("resize created a second private frame instead of replacing the retained one".into());
                    } else if reallocs == 0 {
                        problem = Some("resize did not reallocate the private frame".into());
                    }
                }
                if problem.is_none() && self.frames_in_phase >= 4 {
                    let renderer = self.renderer.as_mut().unwrap();
                    let pipelines = renderer.scene.resource_stats().msaa_pipeline_creates;
                    renderer.scene.set_msaa(&renderer.gpu, MsaaSamples::One).unwrap();
                    if renderer.scene.surface_frame().is_some() || renderer.scene.msaa_images().image_count != 0 {
                        problem = Some("returning to 1× kept the private frame or a 4× image".into());
                    } else if renderer.scene.resource_stats().msaa_pipeline_creates != pipelines {
                        problem = Some("returning to 1× rebuilt 4× pipelines".into());
                    } else {
                        self.phase = Phase::OneAgain;
                        self.frames_in_phase = 0;
                    }
                }
            }
            Phase::OneAgain => {
                problem = {
                    let renderer = self.renderer.as_ref().unwrap();
                    if renderer.scene.stats().frame_present_draws != 0 || renderer.scene.surface_frame().is_some() {
                        Some("the restored 1× frame encoded the 4× copy".into())
                    } else {
                        None
                    }
                };
                if problem.is_none() && self.frames_in_phase >= 2 {
                    let renderer = self.renderer.as_mut().unwrap();
                    let pipelines = renderer.scene.resource_stats().msaa_pipeline_creates;
                    renderer.scene.set_msaa(&renderer.gpu, MsaaSamples::Four).unwrap();
                    if renderer.scene.resource_stats().msaa_pipeline_creates != pipelines {
                        problem = Some("switching back to 4× rebuilt pipelines".into());
                    } else {
                        self.phase = Phase::FourAgain;
                        self.frames_in_phase = 0;
                    }
                }
            }
            Phase::FourAgain => {
                problem = {
                    let renderer = self.renderer.as_ref().unwrap();
                    if renderer.scene.surface_frame().is_none() || renderer.scene.stats().frame_present_draws != 1 {
                        Some("the second 4× run did not present the private frame".into())
                    } else {
                        None
                    }
                };
                if problem.is_none() && self.frames_in_phase >= 2 {
                    return Flow::Exit;
                }
            }
        }
        if let Some(problem) = problem {
            self.fail(problem);
            return Flow::Exit;
        }
        Flow::Continue
    }
}

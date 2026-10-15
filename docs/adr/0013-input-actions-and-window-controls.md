# ADR-0013: Input actions and window controls

* **Status:** Settled
* **Date:** 2026-10-15
* **Backlog id:** D-49
* **Decided by:** project owner (approved the M3b plan with the refinements in this record)
* **Affects:** `Input`, the future `Actions` resource, `Viewport`, `WindowControl`, the winit loop, the wgpu surface

## Context and problem

M3a left device input as a resource. The platform applies `InputEvent`s before `Game::step`. `key_pressed` and `key_released` stay set until `end_input_frame`, so every fixed iteration of that step sees the same edge (`input_edges_survive_fixed_and_update_and_clear_at_the_end`). A frame with no fixed step still shows the edge to `update`, then clears it. A fixed system that treats the edge as one simulation tick therefore fires once per iteration, and a tap that lands on a frame with no tick is gone before the next tick.

The key enum has 46 variants. F-keys, punctuation, and the numeric keypad are dropped. Mouse buttons have a just-pressed set and no just-released set. There is no action map. `Viewport` is one physical size. The window loop does not forward DPI, fullscreen, vsync, a frame cap, or cursor grab. winit in the lockfile is 0.30.13, which does dispatch `WindowEvent::ScaleFactorChanged`.

D-39 (UI capture, focus navigation, text, and the broader scaling model) stays open for M10. This record decides the M3b slice only.

## Requirements and constraints

* The M3a stage list, command flush, and event lifetime stay as ADR-0007 and ADR-0008 left them.
* Existing `Input::key_pressed`, `key_released`, `mouse_pressed`, and `key_down` keep their current timing. Variable-step code keeps reading them.
* A one-shot used by fixed-step code fires once per delivered transition, including when several fixed iterations share a frame and when a transition arrives on a frame with no fixed iteration.
* Several systems in the same iteration observe that transition. Reading it does not clear it.
* Raw input and mapped actions use the same two timings. Actions do not grow a second clock.
* No gamepad, touch, text input, action-map stack, rebinding UI, or input-event log. A persisted binding format waits for a versioned file (ADR-0011).
* New public types do not name `winit` or `wgpu` (ADR-0010). The existing re-exports stay.
* GPU projection and the surface stay in physical pixels.

## Options considered

### A. Leave fixed systems on the frame edge
* Advantages: no new state.
* Disadvantages: a jump in `fixed_update` repeats for every iteration that frame, and a tap on a zero-tick frame never reaches a later tick.

### B. A fixed-step edge latch beside the frame edges
* Advantages: frame edges stay. The latch is two bits per device (press, release), cleared once after the first fixed iteration that runs. Every system in that iteration sees the same bits. A zero-tick frame keeps the latch for the next tick. A press and a release before that tick both stay set.
* Disadvantages: transitions that pile up before a tick collapse. Two press-release cycles become one press bit and one release bit, not four events.

### C. Replay every device event into successive fixed iterations
* Advantages: each transition can be delivered on its own tick.
* Disadvantages: an event queue, ordering rules, and a second clock. More mechanism than M3b has a consumer for.

## Decision and rationale

B. Frame edges stay the variable-step and raw `Input` API. Phase 2 adds a latch for fixed-step one-shots and makes `Actions` follow both timings. Phase 1 does not add the latch. It widens `Key` and adds the mouse release edge on the existing frame clock.

### Frame edges, unchanged

`press` / `release` set the frame edge only on a real down or up transition. A repeat `press` while the key is down does not. `end_frame` clears frame edges, mouse delta, and scroll. Held state remains. This includes `mouse_released`, added in Phase 1 on that same rule. A press and release before `end_frame` leave both frame edges set and the device up.

These edges are visible to every stage of the step, including every fixed iteration. Readers do not consume them.

### Fixed latch, Phase 2

`Input` gains a second press bit and release bit per key and per mouse button.

* The same transition sets the frame edge and the latch bit.
* `end_frame` does not clear the latch.
* When `step` runs at least one fixed iteration, the latch is visible for that first iteration, to every system in it, and is cleared after that iteration's systems and command flush. Later iterations in the step see the bits clear. `key_down` and `mouse_down` stay live on every iteration.
* A step with zero fixed iterations leaves the latch set. The next step that runs a fixed iteration delivers it once.
* Bits OR together until delivery. The latch is not a queue. Two taps before the next fixed iteration produce one press bit and one release bit, plus whatever `down` is at delivery.
* Variable-step systems keep using frame edges. Reading the latch does not clear it. Only the end of the delivering iteration clears it.

The readable bits and the stored bits are not the same set after delivery. `Game::step` calls `publish_fixed_edges` at the start of every step and `conceal_fixed_edges` after the first fixed iteration's systems and command flush. Queries (`fixed_key_pressed`, `fixed_key_released`, `fixed_mouse_pressed`, `fixed_mouse_released`) always read the published set. They do not change meaning based on which stage called them.

* Before the first conceal, and for a whole step that runs no fixed iteration, the published set is the latch. `update` on a zero-step frame still sees it.
* After conceal, queries return false for the rest of that step: later fixed iterations and `update`. Held state is unchanged.
* A transition after conceal is stored and is not published until the next step. A rebind in a later iteration, or in `update`, therefore waits for the next step's first fixed iteration. It does not also fire on a later iteration of the same step.

`Actions` uses that same publish and conceal pair. It does not keep a second clock. It stores aggregate edges of its own, because an action edge is not the OR of the physical edges. `Game::send_input` applies the device event and then `Actions::sync`. Rebinding compares the aggregate against the held keys and buttons saved by the last sync.

* `down` is the live aggregate: any binding held.
* `pressed` / `released` are the frame-edge aggregate, cleared with `end_frame`.
* `fixed_pressed` / `fixed_released` are the latch aggregate, cleared with the latch.
* Aggregate press: a binding transitions down while no other binding of that action is held.
* Aggregate release: the last held binding transitions up.
* A second binding pressed while one is held is not a press. Releasing one binding while another stays held is not a release.
* A press and release of the last binding before `end_frame` leaves both action edges set.
* `axis` is `+1`, `-1`, or `0` from held bindings. Several bindings on one side count once. Both sides or neither side are `0`. `axis2` is two axes.
* Rebinding replaces the binding list, then compares the aggregate held state before and after using current `down`. Up to down records a press on the frame edge and the latch. Down to up records a release on both. No change in aggregate records no edge. A physical edge already stored is left set.
* Names are `&'static str`. One resource. `bind_button` adds a binding. `set_button` replaces the list. `bind_axis` adds one negative and one positive source. `set_axis` replaces both sides. Axes have no press or release.

Device events stay on `Game::send_input`. They are not typed `World` events. Routing them through ADR-0008 would delay them by a stage.

### Usage

A button, a second binding, and a rebind:

```rust
actions.bind_button("jump", ButtonSource::Key(Key::Space));
actions.bind_button("jump", ButtonSource::Mouse(MouseButton::Left));
if actions.pressed("jump") {
    // variable-step one-shot
}
actions.set_button("jump", &[ButtonSource::Key(Key::J)]);
```

A 2D axis from two 1D axes. Opposing keys on one axis read as `0`.

```rust
actions.bind_axis("move_x", ButtonSource::Key(Key::A), ButtonSource::Key(Key::D));
actions.bind_axis("move_y", ButtonSource::Key(Key::S), ButtonSource::Key(Key::W));
let move_dir = actions.axis2(&input, "move_x", "move_y");
```

A one-shot inside `fixed_update`. The same call is false in later iterations of that step.

```rust
if actions.fixed_pressed("jump") {
    // one simulation tick
}
```

### Keys and mouse, Phase 1

`Key` gains F1–F12, the US punctuation positions (`Backquote`, `Minus`, `Equal`, brackets, `Backslash`, `Semicolon`, `Quote`, `Comma`, `Period`, `Slash`), Backspace, Delete, Insert, Home, End, PageUp, PageDown, and the numeric keypad including a distinct `NumpadEnter`. Left and right Shift, Control, and Alt stay one variant each. Mapping stays on physical `KeyCode`. F13 and above, media keys, and the horizontal wheel stay unmapped. OS key repeat stays dropped in the platform loop.

Before this correction, each physical release of a collapsed modifier emitted a logical release. Holding both Shift keys and releasing the left one cleared `Key::Shift` while the right one was still down. The platform now counts physical downs for Shift, Control, and Alt. The logical key goes down on the first side and up on the last side. A side that changes nothing does not emit an `InputEvent`. Other keys are unchanged: one physical event still emits one logical event. `Input::press` and `Input::release` still see only the logical `Key`.

### Viewport and pointer coordinates, Phase 3

`Viewport` stores the physical framebuffer size (`u32` width and height) and a display scale (`f64`). The scale must be finite and greater than zero; `Viewport::new` returns `ViewportError::InvalidScale` otherwise. Logical size is always `physical / scale`, as an `f32` `Vec2`. Changing the scale does not multiply the physical size. A later resize supplies the new framebuffer, so the scale is not applied twice.

`WindowConfig` width and height are logical pixels and only size the window at creation. The real window is the authority after that. `WindowedGame::init` reads `window.scale_factor()` and creates the surface from `window.inner_size()`. The platform then calls `resize` with that physical size. `Viewport::from_logical_pixels` is the headless and pre-window case: scale 1, so the physical size equals the requested size. `render_to_rgba` uses it, so a headless screenshot is a scale-1 framebuffer of the requested pixel size.

winit 0.30.13 defines `WindowEvent::ScaleFactorChanged { scale_factor, inner_size_writer }`. The handler forwards `scale_factor` and does not call `inner_size_writer.request_inner_size`. On macOS, Wayland, and X11 that backend then emits `Resized` with the physical size the OS chose, and that event is what stores the new framebuffer. Between the two events the logical size is the previous framebuffer divided by the new scale. A game frame runs after both events on those backends. Windows `WM_DPICHANGED` and the web and iOS backends also emit `ScaleFactorChanged`; this record does not claim a windowed check on those platforms.

`WindowEvent::Resized` stores the physical size when both sides are non-zero. A zero width or height, as from a minimized window, leaves the last positive framebuffer in place. `WindowRenderer::resize` skips a zero size the same way, so the surface and the viewport stay together. A repeated scale event with the same factor is a no-op and is not logged. A non-finite or non-positive scale is logged and ignored. On this Mac, showing the window also emitted `Resized` and then `ScaleFactorChanged` with the scale already read at init; the second scale event did not change the framebuffer.

GPU surfaces and camera extraction use `physical_size_pixels()`. The same physical size keeps the same camera projection at any scale. Sprite and mesh transforms are world-space and do not read `Viewport`.

Both pointer spaces are top-left, +X right, +Y down. `Input::mouse_position` stays in physical pixels. `physical_to_logical` and `logical_to_physical` use that same origin and axis direction. They do not clamp, so a pointer outside the window converts too. World space stays Y-up and is a different system. Future UI hit-testing converts the physical pointer with `physical_to_logical`, or works in physical pixels and converts layout rectangles the other way. World picking, camera raycasting, and reference-resolution fit modes are not part of this phase (D-39).

`Viewport` and `ViewportError` are on `engine` and `engine::prelude`. No new `winit` or `wgpu` type is added to that API.

Phase 4, still without `winit` or `wgpu` in the new signatures:

* Borderless fullscreen. Exclusive fullscreen is not in this decision.
* `vsync: bool`. The renderer picks `Fifo` when vsync is on. When it is off, it picks `Immediate` if the surface capabilities list it, otherwise `Mailbox`, otherwise it stays on `Fifo`. The fallback is recorded, not assumed.
* An optional frame cap waits with the event loop (`ControlFlow::WaitUntil`), not a blocking sleep. `Game::step` itself stays uncapped.
* Cursor visibility and grab. Grab tries `Locked`, then `Confined`. A failure is logged and the request is left unset. It does not panic.
* While grabbed, `mouse_delta` comes from `DeviceEvent::MouseMotion` only. While free, it comes from cursor-position deltas only. The two sources are not added.
* Focus loss releases every held key and button, so the frame edges and the latch both see those releases, and it clears the grab.

`basic_2_5d` keeps reading raw `Input`. `spatial_scene` moves to `Actions` in Phase 2. Scroll stays on `Input`.

## Consequences and follow-up

* Phase 1 is the wider `Key` map, `mouse_released`, and the collapsed-modifier count, checkpointed locally as `57dbc4a`. Phase 2, the latch and `Actions`, is checkpointed locally as `83286a9`. Phase 3, `Viewport` and DPI, is implemented in the working tree and is not committed. Window controls are Phase 4.
* D-39 stays open for UI capture, focus navigation, text, and any scale policy beyond physical, logical, and the device factor.
* Gamepad, touch, IME, a rebinding UI, action-map stacks, exclusive fullscreen, and an input log are not part of M3b.
* The collapsed multi-tap is the bound of the latch. A replay queue would be a new decision.

## Evidence and revisit conditions

winit 0.30.13 `event.rs` documents `ScaleFactorChanged`, and the macOS `window_delegate` emits `Resized` after it. The frame-edge test is `input_edges_survive_fixed_and_update_and_clear_at_the_end`. Phase 1 tests cover the mouse release edge, a same-frame tap, ignored key repeat, the new physical codes, and the collapsed-modifier count. Phase 2 tests cover the latch (`the_fixed_latch_survives_end_frame_and_a_read_does_not_clear_it`, `fixed_edges_are_visible_once_and_a_zero_step_frame_keeps_them`, `zero_steps_keep_the_latch_and_rebinding_waits_until_publish`) and aggregate actions (`the_first_binding_presses_once_and_the_second_does_not`, `rebinding_follows_the_aggregate`, `action_edges_follow_the_fixed_latch_and_aggregate_bindings`). Phase 3 tests cover construction, logical size, invalid scales, a zero resize, a non-integer round trip including points outside the window (`a_non_integer_scale_round_trips_including_points_outside_the_window`), scale-then-resize without applying the scale twice (`resize_and_scale_events_do_not_apply_the_scale_twice`), camera extraction staying on the physical size (`extraction_uses_the_physical_size_at_every_scale`), and `mouse_position` staying physical (`mouse_position_stays_physical_and_logical_conversion_does_not_change_input`).

Reopen the latch if a fixed-step game needs each collapsed tap delivered on its own iteration. Reopen presentation fallback if a real surface has none of `Fifo`, `Mailbox`, and `Immediate`.

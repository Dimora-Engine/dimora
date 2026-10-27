//! Collision scheduling. No GPU.

use std::time::Duration;

use engine::core::FixedTimestep;
use engine::prelude::*;
use engine::world::ScheduleError;

fn fixed_step() -> Duration {
    FixedTimestep::new(60).step()
}

#[test]
fn sync_collision_runs_after_fixed_systems_and_a_zero_step_does_not_rebuild() {
    let mut game = Game::new(GameConfig::default());
    let entity = game.world.spawn((Transform::IDENTITY, Collider::sphere(1.0)));
    game.add_fixed_system("slide_origin", move |world| {
        world.component_mut::<Transform>(entity).unwrap().position.x = 4.0;
    });
    assert_eq!(game.system_order(Stage::FixedUpdate), vec!["slide_origin", Stage::SYNC_COLLISION]);
    let rejected = game.add_to_stage_after(Stage::FixedUpdate, "too_late", Stage::SYNC_COLLISION, |_world| {});
    assert_eq!(
        rejected.err(),
        Some(ScheduleError::MustRunBefore { system: "too_late", anchor: Stage::SYNC_COLLISION })
    );

    game.step(Duration::ZERO);
    assert_eq!(game.world.resource::<CollisionWorld>().generation(), 0);
    assert!(game.world.resource::<CollisionWorld>().center(entity).is_none());

    game.step(fixed_step());
    let center = game.world.resource::<CollisionWorld>().center(entity).unwrap();
    assert!(center.approx_eq(Vec3::new(4.0, 0.0, 0.0), 1e-4));
    assert_eq!(game.world.resource::<CollisionWorld>().generation(), 1);

    game.world.component_mut::<Transform>(entity).unwrap().position.x = 9.0;
    game.step(Duration::ZERO);
    let stale = game.world.resource::<CollisionWorld>().center(entity).unwrap();
    assert!(stale.approx_eq(Vec3::new(4.0, 0.0, 0.0), 1e-4));
}

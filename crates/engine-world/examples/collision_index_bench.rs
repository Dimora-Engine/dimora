//! Release timings for the linear collision index.
//!
//! `cargo run --release -p engine-world --example collision_index_bench`
//!
//! Each sample is one call. The median is the middle sample after warmup. This is a CPU scan of a
//! sorted list. A later pairwise or trigger pass over the same list would be quadratic in the
//! collider count. These numbers are not a broadphase budget.

use std::time::Instant;

use engine_math::{Transform, Vec3};
use engine_world::{sync_collision, Collider, ColliderShape, CollisionQuery, CollisionWorld, World};

const WARMUP: usize = 10;
const SAMPLES: usize = 31;

fn main() {
    println!("collision index bench");
    time_rebuild(10, false);
    time_rebuild(100, false);
    time_rebuild(100, true);
    time_queries(100);
}

fn time_rebuild(count: usize, disabled: bool) {
    let mut world = grid(count, disabled);
    let label = if disabled { "disabled" } else { "indexed" };
    let median = median_micros(|| sync_collision(&mut world));
    let indexed = world.resource::<CollisionWorld>().len();
    println!("sync_collision {count} {label}: median {median:.1} us, indexed {indexed}");
}

fn time_queries(count: usize) {
    let mut world = grid(count, false);
    sync_collision(&mut world);
    let shape = ColliderShape::Sphere { radius: 0.5 };
    let pose = Transform::IDENTITY;
    let query = CollisionQuery::all();
    let overlap = median_micros(|| {
        let hits = world.resource::<CollisionWorld>().overlap(shape, &pose, query);
        std::hint::black_box(hits);
    });
    let ray = median_micros(|| {
        let hit = world.resource::<CollisionWorld>().ray(Vec3::new(-2.0, 0.0, 0.0), Vec3::X, 400.0, query);
        std::hint::black_box(hit);
    });
    println!("overlap query against {count}: median {overlap:.1} us");
    println!("ray query against {count}: median {ray:.1} us");
}

fn grid(count: usize, disabled: bool) -> World {
    let mut world = World::new();
    for index in 0..count {
        let mut collider = Collider::sphere(0.5);
        if disabled {
            collider = collider.disabled();
        }
        world.spawn((Transform::from_xyz(index as f32 * 3.0, 0.0, 0.0), collider));
    }
    world
}

fn median_micros(mut body: impl FnMut()) -> f64 {
    for _ in 0..WARMUP {
        body();
    }
    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let started = Instant::now();
        body();
        samples.push(started.elapsed().as_secs_f64() * 1.0e6);
    }
    samples.sort_by(|left, right| left.partial_cmp(right).unwrap());
    samples[SAMPLES / 2]
}

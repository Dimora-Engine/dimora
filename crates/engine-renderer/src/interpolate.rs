use std::collections::HashMap;

use engine_core::FixedTimestep;
use engine_math::{Mat4, Transform};
use engine_world::{Children, Entity, GlobalTransform, Interpolate, Parent, World};

/// Render world matrices for one extraction. `None` means no entity has a stored previous local,
/// so callers keep using [`GlobalTransform`].
pub(crate) struct RenderPoses {
    matrices: Option<HashMap<Entity, Mat4>>,
}

impl RenderPoses {
    pub(crate) fn matrix(&self, entity: Entity, global: &GlobalTransform) -> Mat4 {
        self.matrices.as_ref().and_then(|matrices| matrices.get(&entity).copied()).unwrap_or(global.matrix)
    }
}

/// Builds render world matrices when any [`Interpolate`] has a stored previous local.
///
/// The walk matches [`engine_world::propagate_transforms`]: a transform-less node is identity, and
/// a child's matrix is the parent matrix times its render-local. The render-local is
/// [`Transform::lerp`] of the stored previous and the current local, or the current local when
/// there is no history. A non-finite previous, current, factor, or result uses the current local.
/// Nothing is written back to the world.
pub(crate) fn render_poses(world: &World) -> RenderPoses {
    let blending = world.query::<&Interpolate>().iter().any(|(_, mark)| mark.previous.is_some());
    if !blending {
        return RenderPoses { matrices: None };
    }
    let alpha = world.try_resource::<FixedTimestep>().map(|time| time.alpha()).unwrap_or(0.0);
    RenderPoses { matrices: Some(compose(world, alpha)) }
}

fn compose(world: &World, alpha: f32) -> HashMap<Entity, Mat4> {
    let mut matrices = HashMap::new();
    let mut stack = Vec::new();

    let roots: Vec<(Entity, Transform)> =
        world.query::<&Transform>().without::<&Parent>().iter().map(|(entity, local)| (entity, *local)).collect();
    for (entity, local) in roots {
        let matrix = render_local(world, entity, local, alpha).to_matrix();
        matrices.insert(entity, matrix);
        stack.push((entity, matrix));
    }

    let groups: Vec<Entity> = world
        .query::<&Children>()
        .without::<&Transform>()
        .without::<&Parent>()
        .iter()
        .map(|(entity, _)| entity)
        .collect();
    stack.extend(groups.into_iter().map(|entity| (entity, Mat4::IDENTITY)));

    while let Some((entity, parent_matrix)) = stack.pop() {
        let children = match world.component::<Children>(entity) {
            Ok(children) => children.0.clone(),
            Err(_) => continue,
        };
        for child in children {
            let current = match world.component::<Transform>(child) {
                Ok(local) => *local,
                Err(_) => {
                    stack.push((child, parent_matrix));
                    continue;
                }
            };
            let matrix = parent_matrix * render_local(world, child, current, alpha).to_matrix();
            matrices.insert(child, matrix);
            stack.push((child, matrix));
        }
    }
    matrices
}

fn render_local(world: &World, entity: Entity, current: Transform, alpha: f32) -> Transform {
    let previous = match world.component::<Interpolate>(entity) {
        Ok(mark) => mark.previous,
        Err(_) => None,
    };
    let Some(previous) = previous else { return current };
    if !alpha.is_finite() || !transform_finite(&previous) || !transform_finite(&current) {
        return current;
    }
    let blended = previous.lerp(&current, alpha);
    if transform_finite(&blended) {
        blended
    } else {
        current
    }
}

fn transform_finite(transform: &Transform) -> bool {
    let (p, s, r) = (transform.position, transform.scale, transform.rotation);
    p.x.is_finite()
        && p.y.is_finite()
        && p.z.is_finite()
        && s.x.is_finite()
        && s.y.is_finite()
        && s.z.is_finite()
        && r.x.is_finite()
        && r.y.is_finite()
        && r.z.is_finite()
        && r.w.is_finite()
}

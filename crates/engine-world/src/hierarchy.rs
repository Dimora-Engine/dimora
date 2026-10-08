use engine_math::{Mat4, Transform, Vec3};
use hecs::Entity;

use crate::World;

/// Points at an entity's parent. Maintain it through [`World::set_parent`] / `spawn_child`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Parent(pub Entity);

/// Ordered list of an entity's children, kept in sync with [`Parent`] by the world.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Children(pub Vec<Entity>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HierarchyError {
    Cycle,
    NoSuchEntity,
}

impl core::fmt::Display for HierarchyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            HierarchyError::Cycle => f.write_str("reparenting would create a cycle"),
            HierarchyError::NoSuchEntity => f.write_str("entity does not exist"),
        }
    }
}
impl std::error::Error for HierarchyError {}

/// World-space transform, computed from the local [`Transform`] chain by
/// [`propagate_transforms`]. Read-only for gameplay: edit `Transform` instead.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlobalTransform {
    pub matrix: Mat4,
}

impl Default for GlobalTransform {
    fn default() -> Self {
        Self { matrix: Mat4::IDENTITY }
    }
}

impl GlobalTransform {
    pub fn translation(&self) -> Vec3 {
        self.matrix.translation()
    }
    /// World-space direction of the entity's local +Z axis (unnormalized if scaled).
    pub fn forward(&self) -> Vec3 {
        self.matrix.transform_vector3(Vec3::Z)
    }
    pub fn to_transform(&self) -> Transform {
        Transform::from_matrix(&self.matrix)
    }
}

/// Recomputes every [`GlobalTransform`] from local transforms and the parent/child hierarchy.
/// Entities that have a `Transform` but no `GlobalTransform` get one.
///
/// Run once per frame after gameplay/animation systems and before extraction. There is no dirty
/// tracking yet: the cost is linear in the number of transformed entities.
pub fn propagate_transforms(world: &mut World) {
    let missing: Vec<Entity> = world
        .entities
        .query::<&Transform>()
        .without::<&GlobalTransform>()
        .iter()
        .map(|(e, _)| e)
        .collect();
    for e in missing {
        let _ = world.entities.insert_one(e, GlobalTransform::default());
    }

    let mut stack: Vec<(Entity, Mat4)> = Vec::new();
    for (e, (local, global)) in world.entities.query::<(&Transform, &mut GlobalTransform)>().without::<&Parent>().iter() {
        global.matrix = local.to_matrix();
        stack.push((e, global.matrix));
    }

    while let Some((entity, parent_matrix)) = stack.pop() {
        let Ok(children) = world.entities.get::<&Children>(entity) else { continue };
        for &child in &children.0 {
            let Ok(mut q) = world.entities.query_one::<(&Transform, &mut GlobalTransform)>(child) else { continue };
            let Some((local, global)) = q.get() else { continue };
            global.matrix = parent_matrix * local.to_matrix();
            stack.push((child, global.matrix));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_math::Quat;

    #[test]
    fn child_inherits_parent_rotation_scale_and_position() {
        let mut w = World::new();
        let parent = w.spawn((Transform::from_xyz(10.0, 0.0, 5.0)
            .with_rotation(Quat::from_rotation_y(core::f32::consts::FRAC_PI_2))
            .with_uniform_scale(2.0),));
        let child = w.spawn_child(parent, (Transform::from_xyz(1.0, 0.0, 0.0),));
        let grandchild = w.spawn_child(child, (Transform::from_xyz(0.0, 1.0, 0.0),));
        propagate_transforms(&mut w);

        // Local +X of the parent points along world -Z after a +90deg yaw.
        let c = w.component::<GlobalTransform>(child).unwrap().translation();
        assert!(c.approx_eq(Vec3::new(10.0, 0.0, 3.0), 1e-5), "{c:?}");
        let g = w.component::<GlobalTransform>(grandchild).unwrap().translation();
        assert!(g.approx_eq(Vec3::new(10.0, 2.0, 3.0), 1e-5), "{g:?}");
    }

    #[test]
    fn moving_the_parent_moves_children() {
        let mut w = World::new();
        let parent = w.spawn((Transform::IDENTITY,));
        let child = w.spawn_child(parent, (Transform::from_xyz(0.0, 0.0, 1.0),));
        propagate_transforms(&mut w);
        w.component_mut::<Transform>(parent).unwrap().position = Vec3::new(3.0, 0.0, 0.0);
        propagate_transforms(&mut w);
        assert!(w.component::<GlobalTransform>(child).unwrap().translation().approx_eq(Vec3::new(3.0, 0.0, 1.0), 1e-6));
    }
}

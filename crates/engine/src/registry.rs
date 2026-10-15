//! Registers the built-in components on a world's [`ComponentRegistry`](engine_world::ComponentRegistry).

use engine_world::{RegistryError, World};

/// Registers world, renderer, and animation components.
///
/// [`crate::Game::new`] does this. Calling it again, when those registrations are unchanged,
/// leaves the registry as it is and returns [`Ok`]. A component type or name that disagrees
/// with the built-in metadata returns that disagreement. The explicit [`World::register_component`]
/// path still rejects an identical second registration.
pub fn register_builtin_components(world: &World) -> Result<(), RegistryError> {
    let mut registry = world.resource_mut();
    engine_world::register_world_components(&mut registry)?;
    engine_renderer::register_render_components(&mut registry)?;
    engine_animation::register_animation_components(&mut registry)?;
    Ok(())
}

use engine_world::{register_world_components, Collider, ComponentRegistry, Persistence, World};

#[test]
fn collider_defaults_and_registry_are_persisted_data() {
    let collider = Collider::sphere(1.0);
    assert_eq!(collider.offset, engine_math::Vec3::ZERO);
    assert_eq!(collider.layers, 1);
    assert_eq!(collider.mask, u32::MAX);
    assert!(!collider.sensor);
    assert!(collider.enabled);

    let world = World::new();
    register_world_components(&mut world.resource_mut::<ComponentRegistry>()).unwrap();
    let registry = world.resource::<ComponentRegistry>();
    let info = registry.get::<Collider>().unwrap();
    assert_eq!(info.name, "Collider");
    assert_eq!(info.persistence, Persistence::Persist);
    assert!(!info.script_visible);
    let names: Vec<_> = info.fields.iter().map(|field| field.name).collect();
    assert_eq!(names, ["shape", "offset", "layers", "mask", "sensor", "enabled"]);
}

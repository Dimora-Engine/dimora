//! Built-in registration through `Game`. Rendering is not involved.

use engine::prelude::*;
use engine_animation::{spawn_skeleton, Bone, Skeleton};
use engine_math::Vec2;

struct Secret;

#[test]
fn game_startup_registers_builtins_and_still_reports_unknown_components() {
    let mut game = Game::new(GameConfig::default());
    let registry = game.world.resource::<ComponentRegistry>();
    for name in [
        "Transform",
        "Name",
        "Tags",
        "Visibility",
        "Sprite",
        "Mesh",
        "Camera",
        "Light",
        "TileMap",
        "Bone",
        "AnimationPlayer",
        "SkeletonInstance",
    ] {
        let info = registry.get_by_name(name).unwrap_or_else(|| panic!("{name} is not registered"));
        assert_eq!(info.name, name);
        assert!(!info.fields.is_empty(), "{name} has no declared fields");
    }
    drop(registry);

    let mut skeleton = Skeleton::new();
    skeleton.add_bone("hips", None, Transform::IDENTITY).unwrap();
    let root = game.world.spawn((
        Name::new("sample"),
        Transform::IDENTITY,
        Sprite::new("hero"),
        RenderOrder::new(1, 2),
        Mesh::new("cube"),
        TileMap::new("atlas", 4, 4, Vec2::new(1.0, 1.0), 1, 1),
        Camera::orthographic(10.0),
        Light::directional(Color::WHITE, 1.0),
        AnimationPlayer::new("walk"),
        Secret,
    ));
    spawn_skeleton(&mut game.world, root, &skeleton);

    let dump = game.world.dump_entity(root).unwrap();
    assert!(dump.contains("Sprite\n"));
    assert!(dump.contains("Mesh\n"));
    assert!(dump.contains("TileMap\n"));
    assert!(dump.contains("Camera\n"));
    assert!(dump.contains("Light\n"));
    assert!(dump.contains("AnimationPlayer\n"));
    assert!(dump.contains("SkeletonInstance\n"));
    assert!(dump.contains("len=1"));
    assert!(dump.contains("unregistered: 1\n"));
    assert!(dump.contains("TypeId("));
    assert!(!dump.contains("formatter returned"));
    assert_eq!(game.world.dump_entity(root).unwrap(), dump);

    assert!(game.world.resource::<ComponentRegistry>().get::<Bone>().is_some());
    let bone_entity = game.world.query::<&Bone>().iter().next().unwrap().0;
    let bone_dump = game.world.dump_entity(bone_entity).unwrap();
    assert!(bone_dump.contains("index: usize script_visible=false script_writable=false = 0"));
    assert!(bone_dump.contains("parent:"));
    assert!(!bone_dump.contains("formatter returned"));
}

#[test]
fn registering_builtins_twice_keeps_the_same_registry() {
    let game = Game::new(GameConfig::default());
    let before = game.world.resource::<ComponentRegistry>().len();
    drop(game.world.resource::<ComponentRegistry>());
    register_builtin_components(&game.world).unwrap();
    assert_eq!(game.world.resource::<ComponentRegistry>().len(), before);

    let bare = World::new();
    bare.register_component(ComponentDesc::<u8>::new("Transform", &[], Persistence::Runtime, None)).unwrap();
    let error = register_builtin_components(&bare).unwrap_err();
    assert!(matches!(error, RegistryError::DuplicateName { name: "Transform", .. }));
}

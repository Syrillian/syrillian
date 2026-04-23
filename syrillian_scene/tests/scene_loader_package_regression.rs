use std::path::PathBuf;

use syrillian::World;
use syrillian_components::{AnimationComponent, SkeletalComponent, SkinnedMeshRenderer};
use syrillian_scene::SceneLoader;

fn asset_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

#[test]
fn packaged_sticky_prefab_resolves_materials_textures_and_animations() {
    let package = asset_path("../assets.sya");
    if !package.exists() {
        eprintln!(
            "skipping packaged_sticky_prefab_resolves_materials_textures_and_animations: missing {}",
            package.display()
        );
        return;
    }

    let (mut world, _render_rx, _event_rx, _assets_rx, _pick_tx, _hit_rect_tx) = World::fresh();
    let mounted = world
        .assets
        .hook_package(&package)
        .expect("failed to mount assets.sya");
    assert!(mounted, "assets.sya should mount on first attempt");

    let textures_before = world.assets.textures.items().count();
    let materials_before = world.assets.material_instances.items().count();

    let root_id =
        SceneLoader::load_prefab_by_path(world.as_mut(), "sticky.glb").expect("sticky prefab");

    let textures_after = world.assets.textures.items().count();
    let materials_after = world.assets.material_instances.items().count();

    assert!(
        textures_after > textures_before,
        "sticky prefab should resolve and load texture assets"
    );
    assert!(
        materials_after > materials_before,
        "sticky prefab should instantiate material instances"
    );

    let root = world
        .get_object(root_id)
        .expect("sticky prefab root should exist");
    let animation = root
        .get_component::<AnimationComponent>()
        .expect("sticky prefab root should get an AnimationComponent");

    assert!(
        !animation.clips().is_empty(),
        "sticky prefab should attach decoded animation clips"
    );
    assert!(
        animation.find_clip_index_by_name("rig.001Action").is_some(),
        "sticky prefab should contain rig.001Action clip"
    );

    let mut targets = std::collections::HashSet::<String>::new();
    let mut skinned_renderer_count = 0usize;
    let mut skeletal_count = 0usize;
    let mut stack = vec![root_id];
    while let Some(object_id) = stack.pop() {
        let object = world
            .get_object(object_id)
            .expect("prefab object id should stay valid during traversal");

        targets.insert(object.name.clone());
        if object.get_component::<SkinnedMeshRenderer>().is_some() {
            skinned_renderer_count += 1;
        }
        if let Some(skeletal) = object.get_component::<SkeletalComponent>() {
            skeletal_count += 1;
            targets.extend(skeletal.bones().names.iter().cloned());
        }

        stack.extend(object.children().iter().copied());
    }
    assert!(
        skinned_renderer_count > 0,
        "sticky prefab should include at least one skinned mesh renderer"
    );
    assert!(
        skeletal_count > 0,
        "sticky prefab should include at least one skeletal component"
    );

    let clip = animation
        .clips()
        .iter()
        .find(|clip| clip.name == "rig.001Action")
        .expect("rig.001Action clip should be available");
    let matched_channels = clip
        .channels
        .iter()
        .filter(|channel| targets.contains(&channel.target_name))
        .count();
    assert!(
        matched_channels > 0,
        "sticky animation channels should target spawned node or bone names"
    );
}

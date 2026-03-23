use std::path::PathBuf;

use syrillian::World;
use syrillian_scene::GltfLoader;

fn asset_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

#[test]
fn load_first_mesh_from_file_has_vertices() {
    let path = asset_path("../syrillian_examples/examples/assets/testmodels/hampter/hampter.glb");
    let path_str = path.to_string_lossy();

    let mesh_data = GltfLoader::load_first_mesh(&path_str)
        .expect("scene should load")
        .expect("mesh should be present");

    let (mesh, materials) = mesh_data;
    assert!(mesh.vertex_count() > 0, "expected vertices in loaded mesh");
    assert!(
        !materials.is_empty(),
        "expected at least one material reference for the mesh"
    );
}

#[test]
fn load_first_mesh_from_memory_matches_file() {
    let path = asset_path("../syrillian_examples/examples/assets/testmodels/hampter/hampter.glb");
    let path_str = path.to_string_lossy();
    let bytes = std::fs::read(&path).expect("failed to read test model");

    let file_mesh = GltfLoader::load_first_mesh(&path_str)
        .expect("scene should load from file")
        .expect("mesh should be present from file");

    let buffer_mesh = GltfLoader::load_first_mesh_from_buffer(&bytes)
        .expect("scene should load from memory")
        .expect("mesh should be present from memory");

    assert_eq!(
        file_mesh.0.vertex_count(),
        buffer_mesh.0.vertex_count(),
        "vertex counts should match between file and memory loads"
    );
    assert_eq!(
        file_mesh.1.len(),
        buffer_mesh.1.len(),
        "material assignments should match between file and memory loads"
    );
}

#[test]
fn load_scene_from_buffer_spawns_world_objects() {
    let bytes = std::fs::read(asset_path(
        "../syrillian_examples/examples/assets/testmodels/hampter/hampter.glb",
    ))
    .expect("failed to read test model");

    let (mut world, _render_rx, _event_rx, _assets_rx, _pick_tx, _hit_rect_tx) = World::fresh();

    let root =
        GltfLoader::load_buffer(world.as_mut(), &bytes).expect("scene should load into the world");

    let root_obj = world
        .get_object(root)
        .expect("root object should be registered in the world");
    assert_eq!(root_obj.name, "Instantiated Prefab");
    assert!(
        !root_obj.children().is_empty(),
        "expected child nodes to be spawned under the scene root"
    );
}

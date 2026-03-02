use std::sync::Arc;
use syrillian::math::{Vec2, Vec3, Vec4};
use syrillian_asset::mesh::static_mesh_data::{RawSkinningVertexBuffers, RawVertexBuffers};
use syrillian_asset::mesh::{Bones, PartialMesh};
use syrillian_asset::{
    AssetStore, Font, HMaterial, HMaterialInstance, HMesh, HShader, HTexture2D, MaterialInstance,
    Mesh, Shader, SkinnedMesh, Sound, Texture2D,
};

fn sample_raw_mesh() -> Arc<RawVertexBuffers> {
    Arc::new(RawVertexBuffers {
        positions: vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        ],
        uvs: vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(0.0, 1.0),
        ],
        normals: vec![
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        ],
        tangents: vec![Vec4::X, Vec4::X, Vec4::X],
        indices: None,
    })
}

fn sample_raw_skinning() -> Arc<RawSkinningVertexBuffers> {
    Arc::new(RawSkinningVertexBuffers {
        bone_indices: vec![[0, 0, 0, 0]; 3],
        bone_weights: vec![[1.0, 0.0, 0.0, 0.0]; 3],
    })
}

#[test]
fn test_predefined_meshes() {
    let (store, _assets_rx) = AssetStore::new();

    store.meshes.try_get(HMesh::UNIT_SQUARE).unwrap();
    store.meshes.try_get(HMesh::UNIT_CUBE).unwrap();
    store.meshes.try_get(HMesh::DEBUG_ARROW).unwrap();
    store.meshes.try_get(HMesh::SPHERE).unwrap();
}

#[test]
fn test_mesh_store() {
    let (store, _assets_rx) = AssetStore::new();

    let mesh = Mesh::builder().data(sample_raw_mesh()).build();
    let handle = store.meshes.add(mesh);
    let retrieved_mesh = store.meshes.try_get(handle);
    assert!(retrieved_mesh.is_some());
    assert_eq!(retrieved_mesh.unwrap().len(), 3);
}

#[test]
fn test_shader_store() {
    let (store, _assets_rx) = AssetStore::new();
    let shader = Shader::new_default("Test Shader", "// Test shader code");
    let handle = store.shaders.add(shader);
    let retrieved_shader = store.shaders.try_get(handle);
    assert!(retrieved_shader.is_some());
    assert_eq!(retrieved_shader.unwrap().name(), "Test Shader");
}

#[test]
fn test_texture_store() {
    let (store, _assets_rx) = AssetStore::new();
    let pixels = vec![255, 0, 0, 255];
    let texture = Texture2D::load_pixels(pixels, 1, 1, wgpu::TextureFormat::Rgba8UnormSrgb);
    let handle = store.textures.add(texture);
    let retrieved_texture = store.textures.try_get(handle);
    assert!(retrieved_texture.is_some());
    let texture = retrieved_texture.unwrap();
    assert_eq!(texture.width, 1);
    assert_eq!(texture.height, 1);
}

#[test]
fn test_material_store() {
    let (store, _assets_rx) = AssetStore::new();
    let material = MaterialInstance::builder().name("Test Material").build();
    let handle = store.material_instances.add(material);
    let retrieved_material = store.material_instances.try_get(handle);
    assert!(retrieved_material.is_some());
    assert_eq!(retrieved_material.unwrap().name, "Test Material");
}

#[test]
#[ignore]
fn test_font_store() {
    let (store, _assets_rx) = AssetStore::new();
    let font = Font::new("Noto Sans", None).expect("default font not found");
    let handle = store.fonts.add(font);
    let retrieved_font = store.fonts.try_get(handle);
    assert!(retrieved_font.is_some());
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn test_sound_store() {
    let (store, _assets_rx) = AssetStore::new();
    let sound = Sound::load_sound("../syrillian_examples/examples/assets/pop.wav")
        .expect("Failed to load sound");
    let handle = store.sounds.add(sound);
    let retrieved_sound = store.sounds.try_get(handle);
    assert!(retrieved_sound.is_some());
}

#[test]
#[ignore]
fn test_find_font() {
    let (store, _assets_rx) = AssetStore::new();
    let font = store.fonts.find("Noto Sans");
    assert!(font.is_some());
}

#[test]
fn test_predefined_materials() {
    let (store, _assets_rx) = AssetStore::new();

    store.materials.try_get(HMaterial::FALLBACK).unwrap();
    store.materials.try_get(HMaterial::DEFAULT).unwrap();
    store
        .material_instances
        .try_get(HMaterialInstance::FALLBACK)
        .unwrap();
    store
        .material_instances
        .try_get(HMaterialInstance::DEFAULT)
        .unwrap();
}

#[test]
fn test_predefined_shaders() {
    let (store, _assets_rx) = AssetStore::new();

    store.shaders.try_get(HShader::FALLBACK).unwrap();
    store.shaders.try_get(HShader::DIM2).unwrap();
    store.shaders.try_get(HShader::DIM3).unwrap();
    store.shaders.try_get(HShader::POST_PROCESS).unwrap();
    store.shaders.try_get(HShader::TEXT_2D).unwrap();
    store.shaders.try_get(HShader::TEXT_3D).unwrap();

    #[cfg(debug_assertions)]
    {
        store.shaders.try_get(HShader::DEBUG_EDGES).unwrap();
        store
            .shaders
            .try_get(HShader::DEBUG_VERTEX_NORMALS)
            .unwrap();
        store.shaders.try_get(HShader::DEBUG_LINES).unwrap();
        store
            .shaders
            .try_get(HShader::DEBUG_TEXT2D_GEOMETRY)
            .unwrap();
        store
            .shaders
            .try_get(HShader::DEBUG_TEXT3D_GEOMETRY)
            .unwrap();
    }
}

#[test]
fn test_predefined_textures() {
    let (store, _assets_rx) = AssetStore::new();

    let _ = store.textures.try_get(HTexture2D::FALLBACK_DIFFUSE);
    let _ = store.textures.try_get(HTexture2D::FALLBACK_NORMAL);
    let _ = store.textures.try_get(HTexture2D::FALLBACK_ROUGHNESS);
}

#[test]
fn test_remove_asset() {
    let (store, _assets_rx) = AssetStore::new();

    let mesh = SkinnedMesh::builder()
        .bones(Bones::new())
        .data(sample_raw_mesh())
        .skinning_data(sample_raw_skinning())
        .build();
    let mesh2 = mesh.clone();

    let handle = store.skinned_meshes.add(mesh);
    let handle2 = store.skinned_meshes.add(mesh2);

    assert!(store.skinned_meshes.try_get(handle).is_some());

    let removed_mesh = store.skinned_meshes.remove(handle);

    assert!(removed_mesh.is_some());
    assert!(store.skinned_meshes.try_get(handle).is_none());
    assert!(store.skinned_meshes.try_get(handle2).is_some());
}

#[test]
fn test_iterate_assets() {
    let (store, _assets_rx) = AssetStore::new();

    let mesh1 = Mesh::builder().data(sample_raw_mesh()).build();
    let mesh2 = Mesh::builder().data(sample_raw_mesh()).build();
    let mesh3 = Mesh::builder().data(sample_raw_mesh()).build();

    store.meshes.add(mesh1);
    store.meshes.add(mesh2);
    store.meshes.add(mesh3);

    let count = store.meshes.items().count();

    assert!(count >= 7);
}

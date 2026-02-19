use snafu::ensure;
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use std::{fs, io};
use syrillian::math::{Mat4, Quat, Vec3};
use syrillian::reflect::serializer::JsonSerializer;
use syrillian::reflect::{ReflectSerialize, Value};
use syrillian_asset::mesh::Vertex3D;
use syrillian_asset::store::packaged_scene::{
    BuiltPayload, PackagedScene, PackedAsset, PackedBlob,
};
use syrillian_asset::store::streaming_asset_store::{
    AssetType, MAGIC_SIGNATURE, STREAMING_ASSET_VERSION, StreamingAssetBlobIndexEntryRaw,
    StreamingAssetBlobKind, StreamingAssetFile, StreamingAssetHeader, StreamingAssetIndexEntryRaw,
    hash_relative_path,
};
use syrillian_asset::store::streaming_error::Result;
use syrillian_asset::store::streaming_error::{PathTooLongErr, StreamingAssetError};
use syrillian_asset::{
    AnimationChannel, AnimationClip, Cubemap, Mesh, Shader, Texture2D, TransformKeys,
};
use syrillian_scene::GltfLoader;
use zerocopy::native_endian::{F32, I32, U32, U64};
use zerocopy::{Immutable, IntoBytes, KnownLayout};

pub trait StreamingAssetFileWriter {
    fn pack_folder<P: AsRef<Path>>(folder_path: P, out_file_path: P) -> Result<()>;

    fn pack_folder_with_progress<P, F>(
        folder_path: P,
        out_file_path: P,
        on_asset_packaged: F,
    ) -> Result<()>
    where
        P: AsRef<Path>,
        F: FnMut(AssetType, &str, Duration);
}

impl StreamingAssetFileWriter for StreamingAssetFile {
    fn pack_folder<P: AsRef<Path>>(folder_path: P, out_file_path: P) -> Result<()> {
        Self::pack_folder_with_progress(folder_path, out_file_path, |_asset_type, _path, _time| {})
    }

    fn pack_folder_with_progress<P, F>(
        folder_path: P,
        out_file_path: P,
        mut on_asset_packaged: F,
    ) -> Result<()>
    where
        P: AsRef<Path>,
        F: FnMut(AssetType, &str, Duration),
    {
        let folder_path = folder_path.as_ref();
        let out_path = with_sya_extension(out_file_path.as_ref());

        if let Some(parent) = out_path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }

        let mut assets: Vec<PackedAsset> = Vec::new();
        collect_assets(
            folder_path,
            folder_path,
            &mut assets,
            &mut on_asset_packaged,
        )?;
        assets.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

        let header_size = size_of::<StreamingAssetHeader>() as u64;
        let index_size = (size_of::<StreamingAssetIndexEntryRaw>() * assets.len()) as u64;
        let path_section_start = header_size + index_size;

        let mut path_lengths = Vec::with_capacity(assets.len());
        let mut path_section_size = 0_u64;
        for asset in &assets {
            let path_len = asset.relative_path.len();
            ensure!(
                path_len <= u32::MAX as usize,
                PathTooLongErr {
                    path: asset.relative_path.clone(),
                    len: path_len
                }
            );
            path_lengths.push(path_len as u32);
            path_section_size += path_len as u64;
        }

        let mut path_offset = path_section_start;
        let mut payload_offset = path_section_start + path_section_size;

        let mut index_entries = Vec::with_capacity(assets.len());
        for (asset, path_len) in assets.iter().zip(path_lengths.iter().copied()) {
            let size = asset.payload.len() as u64;
            index_entries.push(StreamingAssetIndexEntryRaw {
                asset_type: asset.asset_type,
                path_len: U32::new(path_len),
                path_offset: U64::new(path_offset),
                offset: U64::new(payload_offset),
                size: U64::new(size),
                hash: U64::new(hash_relative_path(&asset.relative_path)),
            });
            path_offset += path_len as u64;
            payload_offset += size;
        }

        let blob_index_offset = payload_offset;
        let blob_count = assets
            .iter()
            .map(|asset| asset.blobs.len() as u64)
            .sum::<u64>();
        let blob_index_size = blob_count * size_of::<StreamingAssetBlobIndexEntryRaw>() as u64;
        let blob_data_offset = blob_index_offset + blob_index_size;

        let mut blob_entries = Vec::with_capacity(blob_count as usize);
        let mut blob_offset = blob_data_offset;
        for asset in &assets {
            let owner_hash = hash_relative_path(&asset.relative_path);
            for blob in &asset.blobs {
                let blob_size = blob.data.len() as u64;
                blob_entries.push(StreamingAssetBlobIndexEntryRaw {
                    owner_hash: U64::new(owner_hash),
                    kind: blob.kind.as_u8(),
                    reserved: [0; 7],
                    offset: U64::new(blob_offset),
                    size: U64::new(blob_size),
                    element_count: U64::new(blob.element_count),
                });
                blob_offset += blob_size;
            }
        }

        let header = StreamingAssetHeader {
            magic: I32::new(MAGIC_SIGNATURE),
            version: U32::new(STREAMING_ASSET_VERSION),
            asset_count: U64::new(assets.len() as u64),
            blob_count: U64::new(blob_count),
            blob_index_offset: U64::new(blob_index_offset),
            blob_data_offset: U64::new(blob_data_offset),
        };

        let mut out = File::create(out_path)?;
        out.write_all(header.as_bytes())?;

        for entry in &index_entries {
            out.write_all(entry.as_bytes())?;
        }

        for asset in &assets {
            out.write_all(asset.relative_path.as_bytes())?;
        }

        for asset in &assets {
            out.write_all(&asset.payload)?;
        }

        for entry in &blob_entries {
            out.write_all(entry.as_bytes())?;
        }

        for asset in &assets {
            for blob in &asset.blobs {
                out.write_all(&blob.data)?;
            }
        }

        Ok(())
    }
}

pub fn collect_assets(
    root: &Path,
    current: &Path,
    out: &mut Vec<PackedAsset>,
    on_asset_packaged: &mut dyn FnMut(AssetType, &str, Duration),
) -> Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let path = entry.path();

        if file_type.is_dir() {
            collect_assets(root, &path, out, on_asset_packaged)?;
            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();

        if extension == "gltf" || extension == "glb" {
            let relative = path.strip_prefix(root).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Could not derive relative path for '{}' from '{}'",
                        path.display(),
                        root.display()
                    ),
                )
            })?;
            let virtual_root = normalize_relative_path(relative);

            let extract_start = Instant::now();
            let packaged_scene = GltfLoader::extract_packaged_scene_from_path(&path, &virtual_root)
                .map_err(|source| StreamingAssetError::AssetParse {
                    path: path.display().to_string(),
                    reason: source.to_string(),
                })?;

            append_packaged_scene_assets(
                packaged_scene,
                out,
                on_asset_packaged,
                extract_start.elapsed(),
            );
            continue;
        }

        let Some(asset_type) = asset_type_for_path(&path) else {
            continue;
        };

        let relative = path.strip_prefix(root).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Could not derive relative path for '{}' from '{}'",
                    path.display(),
                    root.display()
                ),
            )
        })?;

        let relative_path = normalize_relative_path(relative);
        let cook_start = Instant::now();
        let built = build_packaged_payload(&path, asset_type)?;
        out.push(PackedAsset {
            asset_type,
            relative_path: relative_path.clone(),
            payload: built.payload,
            blobs: built.blobs,
        });
        on_asset_packaged(asset_type, &relative_path, cook_start.elapsed());
    }

    Ok(())
}

fn append_packaged_scene_assets(
    scene: PackagedScene,
    out: &mut Vec<PackedAsset>,
    on_asset_packaged: &mut dyn FnMut(AssetType, &str, Duration),
    extract_duration: Duration,
) {
    let asset_count = scene.meshes.len()
        + scene.textures.len()
        + scene.materials.len()
        + scene.animations.len()
        + 1;
    let shared_extract = duration_per_asset(extract_duration, asset_count);

    for mesh_asset in scene.meshes {
        let cook_start = Instant::now();
        let built = build_mesh_payload(&mesh_asset.mesh);
        out.push(PackedAsset {
            asset_type: AssetType::Mesh,
            relative_path: mesh_asset.virtual_path.clone(),
            payload: built.payload,
            blobs: built.blobs,
        });
        on_asset_packaged(
            AssetType::Mesh,
            &mesh_asset.virtual_path,
            shared_extract.saturating_add(cook_start.elapsed()),
        );
    }

    for texture_asset in scene.textures {
        let cook_start = Instant::now();
        let built = build_texture_payload(&texture_asset.texture);
        out.push(PackedAsset {
            asset_type: AssetType::Texture2D,
            relative_path: texture_asset.virtual_path.clone(),
            payload: built.payload,
            blobs: built.blobs,
        });
        on_asset_packaged(
            AssetType::Texture2D,
            &texture_asset.virtual_path,
            shared_extract.saturating_add(cook_start.elapsed()),
        );
    }

    for material_asset in scene.materials {
        let cook_start = Instant::now();
        let data = reflection_json_bytes(&material_asset.material);
        out.push(PackedAsset {
            asset_type: AssetType::Material,
            relative_path: material_asset.virtual_path.clone(),
            payload: data,
            blobs: Vec::new(),
        });
        on_asset_packaged(
            AssetType::Material,
            &material_asset.virtual_path,
            shared_extract.saturating_add(cook_start.elapsed()),
        );
    }

    for animation_asset in scene.animations {
        let cook_start = Instant::now();
        let built = build_animation_payload(&animation_asset.clip);
        out.push(PackedAsset {
            asset_type: AssetType::AnimationClip,
            relative_path: animation_asset.virtual_path.clone(),
            payload: built.payload,
            blobs: built.blobs,
        });
        on_asset_packaged(
            AssetType::AnimationClip,
            &animation_asset.virtual_path,
            shared_extract.saturating_add(cook_start.elapsed()),
        );
    }

    let cook_start = Instant::now();
    let prefab_data = reflection_json_bytes(&scene.prefab.prefab);
    out.push(PackedAsset {
        asset_type: AssetType::Prefab,
        relative_path: scene.prefab.virtual_path.clone(),
        payload: prefab_data,
        blobs: Vec::new(),
    });
    on_asset_packaged(
        AssetType::Prefab,
        &scene.prefab.virtual_path,
        shared_extract.saturating_add(cook_start.elapsed()),
    );
}

fn duration_per_asset(duration: Duration, assets: usize) -> Duration {
    if assets == 0 {
        return Duration::ZERO;
    }

    let nanos_per_asset = duration.as_nanos() / assets as u128;
    Duration::from_nanos(nanos_per_asset.min(u64::MAX as u128) as u64)
}

fn normalize_relative_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn build_packaged_payload(path: &Path, asset_type: AssetType) -> Result<BuiltPayload> {
    match asset_type {
        AssetType::Mesh => {
            let source = fs::read(path)?;
            let extension = path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            let mesh = match extension.as_str() {
                "obj" => Mesh::load_from_obj_slice(&source).map_err(|source| {
                    StreamingAssetError::AssetParse {
                        path: path.display().to_string(),
                        reason: source.to_string(),
                    }
                })?,
                _ => {
                    return Err(StreamingAssetError::AssetParse {
                        path: path.display().to_string(),
                        reason: format!("Mesh extension '{extension}' is not supported"),
                    });
                }
            };

            Ok(build_mesh_payload(&mesh))
        }
        AssetType::Texture2D => {
            let source = fs::read(path)?;
            let texture = Texture2D::load_image_from_memory(&source).map_err(|source| {
                StreamingAssetError::AssetParse {
                    path: path.display().to_string(),
                    reason: source.to_string(),
                }
            })?;

            Ok(build_texture_payload(&texture))
        }
        AssetType::Shader => {
            let source = fs::read_to_string(path)?;
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("Shader")
                .to_string();
            let shader = Shader::new_default(name, source);
            Ok(BuiltPayload {
                payload: reflection_json_bytes(&shader),
                blobs: Vec::new(),
            })
        }
        AssetType::Cubemap => {
            let source = fs::read(path)?;
            let cubemap = Cubemap::load_equirect_hdr_from_memory(&source).map_err(|source| {
                StreamingAssetError::AssetParse {
                    path: path.display().to_string(),
                    reason: source.to_string(),
                }
            })?;
            Ok(build_cubemap_payload(&cubemap))
        }
        _ => Err(StreamingAssetError::AssetParse {
            path: path.display().to_string(),
            reason: format!("Asset type {asset_type:?} is not packable by this tool"),
        }),
    }
}

fn build_mesh_payload(mesh: &Mesh) -> BuiltPayload {
    let mut blobs = Vec::new();

    let vertex_blob = encode_vertex_blob(mesh.vertices());
    if !vertex_blob.is_empty() {
        blobs.push(PackedBlob {
            kind: StreamingAssetBlobKind::MeshVertices,
            element_count: mesh.vertices().len() as u64,
            data: vertex_blob,
        });
    }

    if let Some(indices) = mesh.indices() {
        let index_blob = encode_index_blob(indices);
        if !index_blob.is_empty() {
            blobs.push(PackedBlob {
                kind: StreamingAssetBlobKind::MeshIndices,
                element_count: indices.len() as u64,
                data: index_blob,
            });
        }
    }

    let inverse_bind_blob = encode_mat4_blob(&mesh.bones.inverse_bind);
    if !inverse_bind_blob.is_empty() {
        blobs.push(PackedBlob {
            kind: StreamingAssetBlobKind::BonesInverseBind,
            element_count: mesh.bones.inverse_bind.len() as u64,
            data: inverse_bind_blob,
        });
    }

    let bind_global_blob = encode_mat4_blob(&mesh.bones.bind_global);
    if !bind_global_blob.is_empty() {
        blobs.push(PackedBlob {
            kind: StreamingAssetBlobKind::BonesBindGlobal,
            element_count: mesh.bones.bind_global.len() as u64,
            data: bind_global_blob,
        });
    }

    let bind_local_blob = encode_mat4_blob(&mesh.bones.bind_local);
    if !bind_local_blob.is_empty() {
        blobs.push(PackedBlob {
            kind: StreamingAssetBlobKind::BonesBindLocal,
            element_count: mesh.bones.bind_local.len() as u64,
            data: bind_local_blob,
        });
    }

    BuiltPayload {
        payload: reflection_json_bytes(&MeshMeta { mesh }),
        blobs,
    }
}

fn build_texture_payload(texture: &Texture2D) -> BuiltPayload {
    let mut blobs = Vec::new();
    if let Some(data) = texture.data.as_deref()
        && !data.is_empty()
    {
        blobs.push(PackedBlob {
            kind: StreamingAssetBlobKind::TextureData,
            element_count: data.len() as u64,
            data: data.to_vec(),
        });
    }

    BuiltPayload {
        payload: reflection_json_bytes(&TextureMeta { texture }),
        blobs,
    }
}

fn build_cubemap_payload(cubemap: &Cubemap) -> BuiltPayload {
    let mut blobs = Vec::new();
    if let Some(data) = cubemap.data.as_deref()
        && !data.is_empty()
    {
        blobs.push(PackedBlob {
            kind: StreamingAssetBlobKind::TextureData,
            element_count: data.len() as u64,
            data: data.to_vec(),
        });
    }

    BuiltPayload {
        payload: reflection_json_bytes(&CubemapMeta { cubemap }),
        blobs,
    }
}

fn build_animation_payload(clip: &AnimationClip) -> BuiltPayload {
    let mut blobs = Vec::new();

    for channel in &clip.channels {
        let keys = &channel.keys;

        let t_times_blob = encode_f32_blob(&keys.t_times);
        if !t_times_blob.is_empty() {
            blobs.push(PackedBlob {
                kind: StreamingAssetBlobKind::AnimationTranslationTimes,
                element_count: keys.t_times.len() as u64,
                data: t_times_blob,
            });
        }

        let t_values_blob = encode_vec3_blob(&keys.t_values);
        if !t_values_blob.is_empty() {
            blobs.push(PackedBlob {
                kind: StreamingAssetBlobKind::AnimationTranslationValues,
                element_count: keys.t_values.len() as u64,
                data: t_values_blob,
            });
        }

        let r_times_blob = encode_f32_blob(&keys.r_times);
        if !r_times_blob.is_empty() {
            blobs.push(PackedBlob {
                kind: StreamingAssetBlobKind::AnimationRotationTimes,
                element_count: keys.r_times.len() as u64,
                data: r_times_blob,
            });
        }

        let r_values_blob = encode_quat_blob(&keys.r_values);
        if !r_values_blob.is_empty() {
            blobs.push(PackedBlob {
                kind: StreamingAssetBlobKind::AnimationRotationValues,
                element_count: keys.r_values.len() as u64,
                data: r_values_blob,
            });
        }

        let s_times_blob = encode_f32_blob(&keys.s_times);
        if !s_times_blob.is_empty() {
            blobs.push(PackedBlob {
                kind: StreamingAssetBlobKind::AnimationScaleTimes,
                element_count: keys.s_times.len() as u64,
                data: s_times_blob,
            });
        }

        let s_values_blob = encode_vec3_blob(&keys.s_values);
        if !s_values_blob.is_empty() {
            blobs.push(PackedBlob {
                kind: StreamingAssetBlobKind::AnimationScaleValues,
                element_count: keys.s_values.len() as u64,
                data: s_values_blob,
            });
        }
    }

    BuiltPayload {
        payload: reflection_json_bytes(&AnimationClipMeta { clip }),
        blobs,
    }
}

fn reflection_json_bytes<S: ReflectSerialize>(value: &S) -> Vec<u8> {
    JsonSerializer::serialize_to_string(value).into_bytes()
}

#[derive(Clone, Copy, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
struct VertexBlobRaw {
    position: [f32; 3],
    uv: [f32; 2],
    normal: [f32; 3],
    tangent: [f32; 3],
    bone_indices: [u32; 4],
    bone_weights: [f32; 4],
}

impl From<&Vertex3D> for VertexBlobRaw {
    fn from(vertex: &Vertex3D) -> Self {
        Self {
            position: vertex.position.to_array(),
            uv: vertex.uv.to_array(),
            normal: vertex.normal.to_array(),
            tangent: vertex.tangent.to_array(),
            bone_indices: vertex.bone_indices,
            bone_weights: vertex.bone_weights,
        }
    }
}

#[derive(Clone, Copy, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
struct Mat4BlobRaw {
    values: [f32; 16],
}

impl From<&Mat4> for Mat4BlobRaw {
    fn from(value: &Mat4) -> Self {
        Self {
            values: value.to_cols_array(),
        }
    }
}

#[derive(Clone, Copy, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
struct Vec3BlobRaw {
    values: [f32; 3],
}

impl From<&Vec3> for Vec3BlobRaw {
    fn from(value: &Vec3) -> Self {
        Self {
            values: value.to_array(),
        }
    }
}

#[derive(Clone, Copy, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
struct QuatBlobRaw {
    values: [f32; 4],
}

impl From<&Quat> for QuatBlobRaw {
    fn from(value: &Quat) -> Self {
        Self {
            values: value.to_array(),
        }
    }
}

fn encode_vertex_blob(vertices: &[Vertex3D]) -> Vec<u8> {
    if vertices.is_empty() {
        return Vec::new();
    }

    let mut raw = Vec::with_capacity(vertices.len() * size_of::<VertexBlobRaw>());
    for vertex in vertices {
        let entry = VertexBlobRaw::from(vertex);
        raw.extend_from_slice(entry.as_bytes());
    }
    raw
}

fn encode_index_blob(indices: &[u32]) -> Vec<u8> {
    if indices.is_empty() {
        return Vec::new();
    }

    let mut raw = Vec::with_capacity(indices.len() * size_of::<U32>());
    for index in indices {
        let index = U32::new(*index);
        raw.extend_from_slice(index.as_bytes());
    }
    raw
}

fn encode_mat4_blob(matrices: &[Mat4]) -> Vec<u8> {
    if matrices.is_empty() {
        return Vec::new();
    }

    let mut raw = Vec::with_capacity(matrices.len() * size_of::<Mat4BlobRaw>());
    for matrix in matrices {
        let entry = Mat4BlobRaw::from(matrix);
        raw.extend_from_slice(entry.as_bytes());
    }
    raw
}

fn encode_f32_blob(values: &[f32]) -> Vec<u8> {
    if values.is_empty() {
        return Vec::new();
    }

    let mut raw = Vec::with_capacity(values.len() * size_of::<F32>());
    for value in values {
        let value = F32::new(*value);
        raw.extend_from_slice(value.as_bytes());
    }
    raw
}

fn encode_vec3_blob(values: &[Vec3]) -> Vec<u8> {
    if values.is_empty() {
        return Vec::new();
    }

    let mut raw = Vec::with_capacity(values.len() * size_of::<Vec3BlobRaw>());
    for value in values {
        let value = Vec3BlobRaw::from(value);
        raw.extend_from_slice(value.as_bytes());
    }
    raw
}

fn encode_quat_blob(values: &[Quat]) -> Vec<u8> {
    if values.is_empty() {
        return Vec::new();
    }

    let mut raw = Vec::with_capacity(values.len() * size_of::<QuatBlobRaw>());
    for value in values {
        let value = QuatBlobRaw::from(value);
        raw.extend_from_slice(value.as_bytes());
    }
    raw
}

struct TextureMeta<'a> {
    texture: &'a Texture2D,
}

impl ReflectSerialize for TextureMeta<'_> {
    fn serialize(this: &Self) -> Value {
        let data_len = this.texture.data.as_ref().map_or(0, |data| data.len()) as u64;

        Value::Object(BTreeMap::from([
            ("width".to_string(), Value::UInt(this.texture.width)),
            ("height".to_string(), Value::UInt(this.texture.height)),
            (
                "format".to_string(),
                Value::String(format!("{:?}", this.texture.format)),
            ),
            (
                "repeat_mode".to_string(),
                Value::String(format!("{:?}", this.texture.repeat_mode)),
            ),
            (
                "filter_mode".to_string(),
                Value::String(format!("{:?}", this.texture.filter_mode)),
            ),
            (
                "mip_filter_mode".to_string(),
                Value::String(format!("{:?}", this.texture.mip_filter_mode)),
            ),
            (
                "has_transparency".to_string(),
                Value::Bool(this.texture.has_transparency),
            ),
            (
                "has_data".to_string(),
                Value::Bool(this.texture.data.is_some()),
            ),
            ("data_len".to_string(), Value::BigUInt(data_len)),
            ("data".to_string(), Value::None),
        ]))
    }
}

struct CubemapMeta<'a> {
    cubemap: &'a Cubemap,
}

impl<'a> ReflectSerialize for CubemapMeta<'a> {
    fn serialize(this: &Self) -> Value {
        let data_len = this.cubemap.data.as_ref().map_or(0, |data| data.len()) as u64;

        Value::Object(BTreeMap::from([
            ("width".to_string(), Value::UInt(this.cubemap.width)),
            ("height".to_string(), Value::UInt(this.cubemap.height)),
            (
                "mip_level_count".to_string(),
                Value::UInt(this.cubemap.mip_level_count),
            ),
            (
                "format".to_string(),
                Value::String(format!("{:?}", this.cubemap.format)),
            ),
            (
                "repeat_mode".to_string(),
                Value::String(format!("{:?}", this.cubemap.repeat_mode)),
            ),
            (
                "filter_mode".to_string(),
                Value::String(format!("{:?}", this.cubemap.filter_mode)),
            ),
            (
                "mip_filter_mode".to_string(),
                Value::String(format!("{:?}", this.cubemap.mip_filter_mode)),
            ),
            (
                "has_transparency".to_string(),
                Value::Bool(this.cubemap.has_transparency),
            ),
            (
                "has_data".to_string(),
                Value::Bool(this.cubemap.data.is_some()),
            ),
            ("data_len".to_string(), Value::BigUInt(data_len)),
            ("data".to_string(), Value::None),
        ]))
    }
}

struct AnimationClipMeta<'a> {
    clip: &'a AnimationClip,
}

struct AnimationChannelMeta<'a> {
    channel: &'a AnimationChannel,
}

struct AnimationKeysMeta<'a> {
    keys: &'a TransformKeys,
}

impl ReflectSerialize for AnimationKeysMeta<'_> {
    fn serialize(this: &Self) -> Value {
        Value::Object(BTreeMap::from([
            (
                "t_times_count".to_string(),
                Value::BigUInt(this.keys.t_times.len() as u64),
            ),
            (
                "t_values_count".to_string(),
                Value::BigUInt(this.keys.t_values.len() as u64),
            ),
            (
                "r_times_count".to_string(),
                Value::BigUInt(this.keys.r_times.len() as u64),
            ),
            (
                "r_values_count".to_string(),
                Value::BigUInt(this.keys.r_values.len() as u64),
            ),
            (
                "s_times_count".to_string(),
                Value::BigUInt(this.keys.s_times.len() as u64),
            ),
            (
                "s_values_count".to_string(),
                Value::BigUInt(this.keys.s_values.len() as u64),
            ),
        ]))
    }
}

impl ReflectSerialize for AnimationChannelMeta<'_> {
    fn serialize(this: &Self) -> Value {
        Value::Object(BTreeMap::from([
            (
                "target_name".to_string(),
                Value::String(this.channel.target_name.clone()),
            ),
            (
                "keys".to_string(),
                ReflectSerialize::serialize(&AnimationKeysMeta {
                    keys: &this.channel.keys,
                }),
            ),
        ]))
    }
}

impl ReflectSerialize for AnimationClipMeta<'_> {
    fn serialize(this: &Self) -> Value {
        let channels = this
            .clip
            .channels
            .iter()
            .map(|channel| ReflectSerialize::serialize(&AnimationChannelMeta { channel }))
            .collect::<Vec<_>>();

        Value::Object(BTreeMap::from([
            ("name".to_string(), Value::String(this.clip.name.clone())),
            ("duration".to_string(), Value::Float(this.clip.duration)),
            (
                "channel_count".to_string(),
                Value::BigUInt(channels.len() as u64),
            ),
            ("channels".to_string(), Value::Array(channels)),
        ]))
    }
}

struct MeshMeta<'a> {
    mesh: &'a Mesh,
}

struct MeshBonesMeta {
    names: Vec<String>,
    parents: Vec<Option<usize>>,
    children: Vec<Vec<usize>>,
    roots: Vec<usize>,
    index_of: HashMap<String, usize>,
    inverse_bind_count: usize,
    bind_global_count: usize,
    bind_local_count: usize,
}

impl ReflectSerialize for MeshBonesMeta {
    fn serialize(this: &Self) -> Value {
        Value::Object(BTreeMap::from([
            (
                "names".to_string(),
                ReflectSerialize::serialize(&this.names),
            ),
            (
                "parents".to_string(),
                ReflectSerialize::serialize(&this.parents),
            ),
            (
                "children".to_string(),
                ReflectSerialize::serialize(&this.children),
            ),
            (
                "roots".to_string(),
                ReflectSerialize::serialize(&this.roots),
            ),
            (
                "index_of".to_string(),
                ReflectSerialize::serialize(&this.index_of),
            ),
            (
                "inverse_bind_count".to_string(),
                Value::BigUInt(this.inverse_bind_count as u64),
            ),
            (
                "bind_global_count".to_string(),
                Value::BigUInt(this.bind_global_count as u64),
            ),
            (
                "bind_local_count".to_string(),
                Value::BigUInt(this.bind_local_count as u64),
            ),
        ]))
    }
}

impl ReflectSerialize for MeshMeta<'_> {
    fn serialize(this: &Self) -> Value {
        let indices = this.mesh.indices();
        let bones = MeshBonesMeta {
            names: this.mesh.bones.names.clone(),
            parents: this.mesh.bones.parents.clone(),
            children: this.mesh.bones.children.clone(),
            roots: this.mesh.bones.roots.clone(),
            index_of: this.mesh.bones.index_of.clone(),
            inverse_bind_count: this.mesh.bones.inverse_bind.len(),
            bind_global_count: this.mesh.bones.bind_global.len(),
            bind_local_count: this.mesh.bones.bind_local.len(),
        };

        Value::Object(BTreeMap::from([
            (
                "vertex_count".to_string(),
                Value::BigUInt(this.mesh.vertices().len() as u64),
            ),
            (
                "vertex_stride".to_string(),
                Value::UInt(size_of::<VertexBlobRaw>() as u32),
            ),
            (
                "has_indices".to_string(),
                Value::Bool(this.mesh.has_indices()),
            ),
            (
                "index_count".to_string(),
                Value::BigUInt(indices.map_or(0, <[u32]>::len) as u64),
            ),
            (
                "index_element_size".to_string(),
                Value::UInt(size_of::<U32>() as u32),
            ),
            (
                "material_ranges".to_string(),
                ReflectSerialize::serialize(&this.mesh.material_ranges),
            ),
            ("bones".to_string(), ReflectSerialize::serialize(&bones)),
            (
                "bounding_sphere".to_string(),
                Value::Object(BTreeMap::from([
                    (
                        "center".to_string(),
                        ReflectSerialize::serialize(&this.mesh.bounding_sphere.center),
                    ),
                    (
                        "radius".to_string(),
                        Value::Float(this.mesh.bounding_sphere.radius),
                    ),
                ])),
            ),
        ]))
    }
}

fn asset_type_for_path(path: &Path) -> Option<AssetType> {
    let extension = path.extension()?.to_string_lossy().to_ascii_lowercase();
    match extension.as_str() {
        "png" | "jpg" | "jpeg" | "bmp" | "gif" | "webp" | "tga" | "dds" | "ktx2" => {
            Some(AssetType::Texture2D)
        }
        "obj" => Some(AssetType::Mesh),
        "hdr" | "exr" => Some(AssetType::Cubemap),
        "wgsl" => Some(AssetType::Shader),
        _ => None,
    }
}

fn with_sya_extension(path: &Path) -> PathBuf {
    let mut path = path.to_path_buf();
    path.set_extension("sya");
    path
}

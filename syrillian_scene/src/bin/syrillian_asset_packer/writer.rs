use snafu::ensure;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use std::{fs, io};
use syrillian_asset::store::streaming::AssetStreamingError;
use syrillian_asset::store::streaming::asset_store::{
    AssetType, MAGIC_SIGNATURE, STREAMING_ASSET_VERSION, StreamingAssetBlobIndexEntryRaw,
    StreamingAssetFile, StreamingAssetHeader, StreamingAssetIndexEntryRaw, hash_relative_path,
};
use syrillian_asset::store::streaming::error::{PathTooLongErr, Result};
use syrillian_asset::store::streaming::packaged_scene::{BuiltPayload, PackagedScene, PackedAsset};
use syrillian_asset::store::streaming::payload::StreamableAsset;
use syrillian_asset::{Cubemap, Mesh, Shader, Texture2D};
use syrillian_scene::GltfLoader;
use zerocopy::IntoBytes;
use zerocopy::native_endian::{I32, U32, U64};

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
                    kind: blob.kind as u8,
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
                .map_err(|source| AssetStreamingError::AssetParse {
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
            payload: built.payload.into_bytes(),
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
        + scene.skinned_meshes.len()
        + scene.textures.len()
        + scene.materials.len()
        + scene.animations.len()
        + 1;
    let shared_extract = duration_per_asset(extract_duration, asset_count);

    for mesh_asset in scene.meshes {
        let cook_start = Instant::now();
        let built = mesh_asset.asset.encode();
        out.push(PackedAsset {
            asset_type: AssetType::Mesh,
            relative_path: mesh_asset.virtual_path.clone(),
            payload: built.payload.into_bytes(),
            blobs: built.blobs,
        });
        on_asset_packaged(
            AssetType::Mesh,
            &mesh_asset.virtual_path,
            shared_extract.saturating_add(cook_start.elapsed()),
        );
    }

    for skinned_mesh_asset in scene.skinned_meshes {
        let cook_start = Instant::now();
        let built = skinned_mesh_asset.asset.encode();
        out.push(PackedAsset {
            asset_type: AssetType::SkinnedMesh,
            relative_path: skinned_mesh_asset.virtual_path.clone(),
            payload: built.payload.into_bytes(),
            blobs: built.blobs,
        });
        on_asset_packaged(
            AssetType::SkinnedMesh,
            &skinned_mesh_asset.virtual_path,
            shared_extract.saturating_add(cook_start.elapsed()),
        );
    }

    for texture_asset in scene.textures {
        let cook_start = Instant::now();
        let built = texture_asset.asset.encode();
        out.push(PackedAsset {
            asset_type: AssetType::Texture2D,
            relative_path: texture_asset.virtual_path.clone(),
            payload: built.payload.into_bytes(),
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
        let built = material_asset.asset.encode();
        out.push(PackedAsset {
            asset_type: AssetType::Material,
            relative_path: material_asset.virtual_path.clone(),
            payload: built.payload.into_bytes(),
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
        let built = animation_asset.asset.encode();
        out.push(PackedAsset {
            asset_type: AssetType::AnimationClip,
            relative_path: animation_asset.virtual_path.clone(),
            payload: built.payload.into_bytes(),
            blobs: built.blobs,
        });
        on_asset_packaged(
            AssetType::AnimationClip,
            &animation_asset.virtual_path,
            shared_extract.saturating_add(cook_start.elapsed()),
        );
    }

    let cook_start = Instant::now();
    let prefab_data = scene.prefab.asset.encode();
    out.push(PackedAsset {
        asset_type: AssetType::Prefab,
        relative_path: scene.prefab.virtual_path.clone(),
        payload: prefab_data.payload.into_bytes(),
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
                    AssetStreamingError::AssetParse {
                        path: path.display().to_string(),
                        reason: source.to_string(),
                    }
                })?,
                _ => {
                    return Err(AssetStreamingError::AssetParse {
                        path: path.display().to_string(),
                        reason: format!("Mesh extension '{extension}' is not supported"),
                    });
                }
            };

            Ok(mesh.encode())
        }
        AssetType::Texture2D => {
            let source = fs::read(path)?;
            let texture = Texture2D::load_image_from_memory(&source).map_err(|source| {
                AssetStreamingError::AssetParse {
                    path: path.display().to_string(),
                    reason: source.to_string(),
                }
            })?;

            Ok(texture.encode())
        }
        AssetType::Shader => {
            let source = fs::read_to_string(path)?;
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("Shader")
                .to_string();
            let shader = Shader::new_default(name, source);
            Ok(shader.encode())
        }
        AssetType::Cubemap => {
            let source = fs::read(path)?;
            let cubemap = Cubemap::load_equirect_hdr_from_memory(&source).map_err(|source| {
                AssetStreamingError::AssetParse {
                    path: path.display().to_string(),
                    reason: source.to_string(),
                }
            })?;
            Ok(cubemap.encode())
        }
        _ => Err(AssetStreamingError::AssetParse {
            path: path.display().to_string(),
            reason: format!("Asset type {asset_type:?} is not packable by this tool"),
        }),
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

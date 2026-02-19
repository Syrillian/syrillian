use super::asset_store::AssetStore;
use crate::assets::mesh::{Bones, MeshVertexData, Vertex3D};
use crate::assets::prefab::{PrefabAsset, PrefabMaterial, PrefabMeshBinding, PrefabNode};
use crate::assets::{Mesh, Shader, ShaderCode, ShaderType, Texture2D};
use crate::store::streaming_asset_store::{
    AssetType, StreamingAssetBlobInfo, StreamingAssetBlobKind, StreamingAssetEntryInfo,
    StreamingAssetFile, normalize_asset_path,
};
use crate::store::{H, Store, StoreType};
use crate::{AnimationChannel, AnimationClip, Cubemap, TransformKeys};
use glamx::{Mat4, Quat, Vec2, Vec3, Vec4};
use parking_lot::{Condvar, Mutex};
use serde_json::{Map as JsonMap, Value as JsonValue};
use snafu::Snafu;
use std::any::TypeId;
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use syrillian_utils::BoundingSphere;
use tracing::debug;
use wgpu::{
    AddressMode, FilterMode, MipmapFilterMode, PolygonMode, PrimitiveTopology, TextureFormat,
};

pub trait StreamingLoadableAsset: StoreType + Send + Sync + 'static {
    const PACKAGE_TYPE: AssetType;
    fn insert_into(store: &AssetStore, asset: Self) -> H<Self>;
}

impl StreamingLoadableAsset for Mesh {
    const PACKAGE_TYPE: AssetType = AssetType::Mesh;

    fn insert_into(store: &AssetStore, asset: Self) -> H<Self> {
        store.meshes.add(asset)
    }
}

impl StreamingLoadableAsset for Texture2D {
    const PACKAGE_TYPE: AssetType = AssetType::Texture2D;

    fn insert_into(store: &AssetStore, asset: Self) -> H<Self> {
        store.textures.add(asset)
    }
}

impl StreamingLoadableAsset for Shader {
    const PACKAGE_TYPE: AssetType = AssetType::Shader;

    fn insert_into(store: &AssetStore, asset: Self) -> H<Self> {
        store.shaders.add(asset)
    }
}

impl StreamingLoadableAsset for PrefabMaterial {
    const PACKAGE_TYPE: AssetType = AssetType::Material;

    fn insert_into(store: &AssetStore, asset: Self) -> H<Self> {
        store.prefab_materials.add(asset)
    }
}

impl StreamingLoadableAsset for PrefabAsset {
    const PACKAGE_TYPE: AssetType = AssetType::Prefab;

    fn insert_into(store: &AssetStore, asset: Self) -> H<Self> {
        store.prefabs.add(asset)
    }
}

impl StreamingLoadableAsset for AnimationClip {
    const PACKAGE_TYPE: AssetType = AssetType::AnimationClip;

    fn insert_into(store: &AssetStore, asset: Self) -> H<Self> {
        store.animation_clips.add(asset)
    }
}

impl StreamingLoadableAsset for Cubemap {
    const PACKAGE_TYPE: AssetType = AssetType::Cubemap;

    fn insert_into(store: &AssetStore, asset: Self) -> H<Self> {
        store.cubemaps.add(asset)
    }
}

#[derive(Debug, Clone, Snafu)]
pub enum AssetStreamingError {
    #[snafu(display("Failed to load package '{path}': {reason}"))]
    PackageLoad { path: String, reason: String },

    #[snafu(display("Failed to scan package directory '{path}': {reason}"))]
    PackageScan { path: String, reason: String },

    #[snafu(display("No packaged asset entry found for path '{path}'"))]
    AssetNotFound { path: String },

    #[snafu(display("No packaged asset entry found for hash 0x{hash:016x}"))]
    HashNotFound { hash: u64 },

    #[snafu(display(
        "Packaged asset '{path}' has type '{actual}', but '{expected}' was requested"
    ))]
    TypeMismatch {
        path: String,
        expected: String,
        actual: String,
    },

    #[snafu(display("The streaming worker is not running"))]
    WorkerNotRunning,

    #[snafu(display("Failed to enqueue '{path}' because the streaming worker is not available"))]
    WorkerChannelClosed { path: String },

    #[snafu(display("Failed to spawn streaming worker thread: {reason}"))]
    WorkerSpawn { reason: String },

    #[snafu(display("The package entry for '{path}' no longer exists"))]
    PackageIndexMissing { path: String },

    #[snafu(display("Failed to read packaged asset '{path}': {reason}"))]
    PackageRead { path: String, reason: String },

    #[snafu(display("Packaged asset '{path}' has unsupported runtime type '{}'", asset_type.name()))]
    UnsupportedType { path: String, asset_type: AssetType },

    #[snafu(display("Failed to decode packaged asset '{path}': {reason}"))]
    Decode { path: String, reason: String },
}

#[derive(Clone)]
pub struct StreamingAsset<A: StreamingLoadableAsset> {
    state: Arc<StreamingAssetState<A>>,
}

struct StreamingAssetState<A: StreamingLoadableAsset> {
    result: Mutex<Option<Result<H<A>, AssetStreamingError>>>,
    cv: Condvar,
}

impl<A: StreamingLoadableAsset> StreamingAsset<A> {
    fn pending() -> Self {
        Self {
            state: Arc::new(StreamingAssetState {
                result: Mutex::new(None),
                cv: Condvar::new(),
            }),
        }
    }

    fn ready(result: Result<H<A>, AssetStreamingError>) -> Self {
        Self {
            state: Arc::new(StreamingAssetState {
                result: Mutex::new(Some(result)),
                cv: Condvar::new(),
            }),
        }
    }

    fn complete(&self, result: Result<H<A>, AssetStreamingError>) {
        let mut lock = self.state.result.lock();
        *lock = Some(result);
        self.state.cv.notify_all();
    }

    pub fn is_ready(&self) -> bool {
        self.state.result.lock().is_some()
    }

    pub fn try_get(&self) -> Option<Result<H<A>, AssetStreamingError>> {
        self.state.result.lock().clone()
    }

    pub fn wait(&self) -> Result<H<A>, AssetStreamingError> {
        let mut lock = self.state.result.lock();

        while lock.is_none() {
            self.state.cv.wait(&mut lock);
        }

        lock.as_ref()
            .cloned()
            .expect("streaming asset result should always exist here")
    }
}

pub(crate) struct StreamingState {
    backend: Arc<Mutex<StreamingBackend>>,
}

impl StreamingState {
    pub(crate) fn new() -> Self {
        Self {
            backend: Arc::new(Mutex::new(StreamingBackend::default())),
        }
    }
}

#[derive(Default)]
struct StreamingBackend {
    packages: Vec<MountedPackage>,
    mounted_paths: HashSet<PathBuf>,
    path_index: HashMap<String, MountedEntryRef>,
    path_hash_index: HashMap<String, u64>,
    hash_index: HashMap<u64, String>,
    loaded: HashMap<String, ErasedHandle>,
    inflight: HashMap<String, Vec<Completion>>,
    worker_tx: Option<mpsc::Sender<LoadJob>>,
}

struct MountedPackage {
    file: Arc<Mutex<StreamingAssetFile>>,
}

#[derive(Clone)]
struct MountedEntryRef {
    package_index: usize,
    entry: StreamingAssetEntryInfo,
}

#[derive(Clone)]
struct LoadJob {
    path: String,
    package_index: usize,
    entry: StreamingAssetEntryInfo,
}

#[derive(Clone)]
struct BlobWithBytes {
    info: StreamingAssetBlobInfo,
    bytes: Vec<u8>,
}

#[derive(Copy, Clone)]
struct ErasedHandle {
    type_id: TypeId,
    id: u32,
}

impl ErasedHandle {
    fn of<A: StoreType + 'static>(handle: H<A>) -> Self {
        Self {
            type_id: TypeId::of::<A>(),
            id: handle.id(),
        }
    }

    fn to_typed<A: StoreType + 'static>(self) -> Option<H<A>> {
        (self.type_id == TypeId::of::<A>()).then_some(H::new(self.id))
    }
}

type Completion = Box<dyn FnOnce(Result<ErasedHandle, AssetStreamingError>) + Send + 'static>;

#[derive(Clone)]
struct WorkerRuntime {
    meshes: Arc<Store<Mesh>>,
    textures: Arc<Store<Texture2D>>,
    cubemaps: Arc<Store<Cubemap>>,
    shaders: Arc<Store<Shader>>,
    animation_clips: Arc<Store<AnimationClip>>,
    prefab_materials: Arc<Store<PrefabMaterial>>,
    prefabs: Arc<Store<PrefabAsset>>,
}

impl WorkerRuntime {
    fn from_store(store: &AssetStore) -> Self {
        Self {
            meshes: store.meshes.clone(),
            textures: store.textures.clone(),
            cubemaps: store.cubemaps.clone(),
            shaders: store.shaders.clone(),
            animation_clips: store.animation_clips.clone(),
            prefab_materials: store.prefab_materials.clone(),
            prefabs: store.prefabs.clone(),
        }
    }
}

impl AssetStore {
    pub fn hook_package<P: AsRef<Path>>(
        &self,
        package_path: P,
    ) -> Result<bool, AssetStreamingError> {
        let package_path = canonicalize_or_self(&with_sya_extension(package_path.as_ref()));

        {
            let backend = self.streaming.backend.lock();
            if backend.mounted_paths.contains(&package_path) {
                return Ok(false);
            }
        }

        let package = StreamingAssetFile::load(&package_path).map_err(|source| {
            AssetStreamingError::PackageLoad {
                path: package_path.display().to_string(),
                reason: source.to_string(),
            }
        })?;
        let entries = package.entries();
        let mounted_file = Arc::new(Mutex::new(package));

        let mut canceled_waiters: Vec<(String, Vec<Completion>)> = Vec::new();
        {
            let mut backend = self.streaming.backend.lock();
            let package_index = backend.packages.len();
            backend.mounted_paths.insert(package_path.clone());
            backend.packages.push(MountedPackage { file: mounted_file });

            for entry in entries {
                let Some(relative_path) = entry.relative_path.clone() else {
                    continue;
                };
                let normalized_path = normalize_asset_path(&relative_path);
                if let Some(waiters) = backend.inflight.remove(&normalized_path) {
                    canceled_waiters.push((normalized_path.clone(), waiters));
                }
                backend.loaded.remove(&normalized_path);
                if let Some(previous_hash) = backend
                    .path_hash_index
                    .insert(normalized_path.clone(), entry.hash)
                    && previous_hash != entry.hash
                    && backend
                        .hash_index
                        .get(&previous_hash)
                        .is_some_and(|old_path| old_path == &normalized_path)
                {
                    backend.hash_index.remove(&previous_hash);
                }
                backend
                    .hash_index
                    .insert(entry.hash, normalized_path.clone());

                if entry.asset_type == AssetType::Prefab {
                    let shorthand = normalized_path.trim_end_matches("/Prefab/scene_prefab");
                    if shorthand.len() != normalized_path.len() {
                        let shorthand = shorthand.to_string();
                        if let Some(waiters) = backend.inflight.remove(&shorthand) {
                            canceled_waiters.push((shorthand.clone(), waiters));
                        }
                        backend.loaded.remove(&shorthand);
                        backend
                            .path_hash_index
                            .insert(shorthand.clone(), entry.hash);
                        backend.path_index.insert(
                            shorthand,
                            MountedEntryRef {
                                package_index,
                                entry: entry.clone(),
                            },
                        );
                    }
                }

                backend.path_index.insert(
                    normalized_path,
                    MountedEntryRef {
                        package_index,
                        entry,
                    },
                );
            }

            self.ensure_worker_started(&mut backend)?;
        }

        for (path, waiters) in canceled_waiters {
            notify_waiters(
                waiters,
                Err(AssetStreamingError::Decode {
                    path,
                    reason: "asset path was overridden by a newer mounted package".to_string(),
                }),
            );
        }

        debug!("Mounted package {}", package_path.display());
        Ok(true)
    }

    pub fn hook_packages_in_directory<P: AsRef<Path>>(
        &self,
        directory: P,
    ) -> Result<usize, AssetStreamingError> {
        let directory = directory.as_ref();
        let mut package_files = Vec::new();

        let entries =
            std::fs::read_dir(directory).map_err(|source| AssetStreamingError::PackageScan {
                path: directory.display().to_string(),
                reason: source.to_string(),
            })?;

        for entry in entries {
            let entry = entry.map_err(|source| AssetStreamingError::PackageScan {
                path: directory.display().to_string(),
                reason: source.to_string(),
            })?;
            let file_type =
                entry
                    .file_type()
                    .map_err(|source| AssetStreamingError::PackageScan {
                        path: directory.display().to_string(),
                        reason: source.to_string(),
                    })?;
            if !file_type.is_file() {
                continue;
            }

            let path = entry.path();
            let Some(extension) = path.extension().and_then(|ext| ext.to_str()) else {
                continue;
            };
            if extension.eq_ignore_ascii_case("sya") {
                package_files.push(path);
            }
        }

        package_files.sort();

        let mut mounted = 0usize;
        for path in package_files {
            if self.hook_package(path)? {
                mounted += 1;
            }
        }
        Ok(mounted)
    }

    pub fn hook_default_packages(&self) -> Result<usize, AssetStreamingError> {
        let mut search_dirs = Vec::new();
        if let Ok(exe_path) = std::env::current_exe()
            && let Some(parent) = exe_path.parent()
        {
            search_dirs.push(canonicalize_or_self(parent));
        }
        if let Ok(cwd) = std::env::current_dir() {
            search_dirs.push(canonicalize_or_self(&cwd));
        }

        let mut unique_dirs = HashSet::new();
        let mut mounted = 0usize;
        for directory in search_dirs {
            if !unique_dirs.insert(directory.clone()) {
                continue;
            }
            mounted += self.hook_packages_in_directory(directory)?;
        }

        Ok(mounted)
    }

    pub fn request_by_path<A: StreamingLoadableAsset>(
        &self,
        relative_path: &str,
    ) -> Result<StreamingAsset<A>, AssetStreamingError> {
        let path = normalize_asset_path(relative_path);
        let pending = StreamingAsset::<A>::pending();
        let callback_path = path.clone();
        let callback_asset = pending.clone();
        let completion: Completion = Box::new(move |result| {
            let typed_result = result.and_then(|handle| {
                handle
                    .to_typed::<A>()
                    .ok_or_else(|| AssetStreamingError::TypeMismatch {
                        path: callback_path.clone(),
                        expected: A::PACKAGE_TYPE.name().to_string(),
                        actual: "InternalHandleTypeMismatch".to_string(),
                    })
            });
            callback_asset.complete(typed_result);
        });

        let job = {
            let mut backend = self.streaming.backend.lock();
            let Some(indexed_entry) = backend.path_index.get(&path).cloned() else {
                return Err(AssetStreamingError::AssetNotFound { path });
            };

            if indexed_entry.entry.asset_type != A::PACKAGE_TYPE {
                return Err(AssetStreamingError::TypeMismatch {
                    path,
                    expected: A::PACKAGE_TYPE.name().to_string(),
                    actual: indexed_entry.entry.asset_type.name().to_string(),
                });
            }

            if let Some(handle) = backend.loaded.get(&path).copied() {
                if let Some(typed_handle) = handle.to_typed::<A>() {
                    return Ok(StreamingAsset::ready(Ok(typed_handle)));
                }
                backend.loaded.remove(&path);
            }

            if let Some(waiters) = backend.inflight.get_mut(&path) {
                waiters.push(completion);
                return Ok(pending);
            }

            let Some(worker_tx) = backend.worker_tx.clone() else {
                return Err(AssetStreamingError::WorkerNotRunning);
            };
            let load_job = LoadJob {
                path: path.clone(),
                package_index: indexed_entry.package_index,
                entry: indexed_entry.entry,
            };

            backend.inflight.insert(path.clone(), vec![completion]);
            if worker_tx.send(load_job.clone()).is_err() {
                let waiters = backend.inflight.remove(&path).unwrap_or_default();
                drop(backend);
                let err = AssetStreamingError::WorkerChannelClosed { path: path.clone() };
                notify_waiters(waiters, Err(err.clone()));
                return Err(err);
            }
            load_job
        };

        debug!("Queued streaming asset load '{}'", job.path);
        Ok(pending)
    }

    pub fn load_by_path<A: StreamingLoadableAsset>(
        &self,
        relative_path: &str,
    ) -> Result<H<A>, AssetStreamingError> {
        self.request_by_path(relative_path)?.wait()
    }

    pub fn request_by_hash<A: StreamingLoadableAsset>(
        &self,
        hash: u64,
    ) -> Result<StreamingAsset<A>, AssetStreamingError> {
        let path = {
            let backend = self.streaming.backend.lock();
            backend
                .hash_index
                .get(&hash)
                .cloned()
                .ok_or(AssetStreamingError::HashNotFound { hash })?
        };

        self.request_by_path::<A>(&path)
    }

    pub fn load_by_hash<A: StreamingLoadableAsset>(
        &self,
        hash: u64,
    ) -> Result<H<A>, AssetStreamingError> {
        self.request_by_hash::<A>(hash)?.wait()
    }

    pub fn path_for_hash(&self, hash: u64) -> Option<String> {
        self.streaming.backend.lock().hash_index.get(&hash).cloned()
    }

    pub fn hash_for_path(&self, relative_path: &str) -> Option<u64> {
        let path = normalize_asset_path(relative_path);
        self.streaming
            .backend
            .lock()
            .path_hash_index
            .get(&path)
            .copied()
    }

    fn ensure_worker_started(
        &self,
        backend: &mut StreamingBackend,
    ) -> Result<(), AssetStreamingError> {
        if backend.worker_tx.is_some() {
            return Ok(());
        }

        let runtime = WorkerRuntime::from_store(self);
        let worker_tx = spawn_worker(self.streaming.backend.clone(), runtime)?;
        backend.worker_tx = Some(worker_tx);
        Ok(())
    }
}

fn spawn_worker(
    backend: Arc<Mutex<StreamingBackend>>,
    runtime: WorkerRuntime,
) -> Result<mpsc::Sender<LoadJob>, AssetStreamingError> {
    let (tx, rx) = mpsc::channel::<LoadJob>();
    thread::Builder::new()
        .name("syrillian-asset-stream".to_string())
        .spawn(move || {
            while let Ok(job) = rx.recv() {
                let result = process_load_job(&runtime, &backend, &job);
                complete_job(&backend, &job, result);
            }
            debug!("Streaming asset worker thread terminated");
        })
        .map_err(|source| AssetStreamingError::WorkerSpawn {
            reason: source.to_string(),
        })?;

    Ok(tx)
}

fn process_load_job(
    runtime: &WorkerRuntime,
    backend: &Arc<Mutex<StreamingBackend>>,
    job: &LoadJob,
) -> Result<ErasedHandle, AssetStreamingError> {
    let package_file = {
        let backend = backend.lock();
        let Some(package) = backend.packages.get(job.package_index) else {
            return Err(AssetStreamingError::PackageIndexMissing {
                path: job.path.clone(),
            });
        };
        package.file.clone()
    };

    let (payload, blobs) = read_package_payload(&package_file, &job.entry, &job.path)?;
    match job.entry.asset_type {
        AssetType::Mesh => {
            let mesh = decode_mesh(&job.path, &payload, &blobs)?;
            Ok(ErasedHandle::of(runtime.meshes.add(mesh)))
        }
        AssetType::Texture2D => {
            let texture = decode_texture(&job.path, &payload, &blobs)?;
            Ok(ErasedHandle::of(runtime.textures.add(texture)))
        }
        AssetType::Cubemap => {
            let cubemap = decode_cubemap(&job.path, &payload, &blobs)?;
            Ok(ErasedHandle::of(runtime.cubemaps.add(cubemap)))
        }
        AssetType::Shader => {
            let shader = decode_shader(&job.path, &payload)?;
            Ok(ErasedHandle::of(runtime.shaders.add(shader)))
        }
        AssetType::Material => {
            let material = decode_prefab_material(&job.path, &payload)?;
            Ok(ErasedHandle::of(runtime.prefab_materials.add(material)))
        }
        AssetType::Prefab => {
            let prefab = decode_prefab_asset(&job.path, &payload)?;
            Ok(ErasedHandle::of(runtime.prefabs.add(prefab)))
        }
        AssetType::AnimationClip => {
            let animation_clip = decode_animation_clip(&job.path, &payload, &blobs)?;
            Ok(ErasedHandle::of(
                runtime.animation_clips.add(animation_clip),
            ))
        }
        unsupported => Err(AssetStreamingError::UnsupportedType {
            path: job.path.clone(),
            asset_type: unsupported,
        }),
    }
}

fn complete_job(
    backend: &Arc<Mutex<StreamingBackend>>,
    job: &LoadJob,
    result: Result<ErasedHandle, AssetStreamingError>,
) {
    let (waiters, notify_result) = {
        let mut backend = backend.lock();

        let stale = backend.path_index.get(&job.path).is_none_or(|indexed| {
            indexed.package_index != job.package_index || indexed.entry.hash != job.entry.hash
        });

        let notify_result = if stale {
            Err(AssetStreamingError::Decode {
                path: job.path.clone(),
                reason: "asset mapping changed while loading".to_string(),
            })
        } else {
            result
        };

        if let Ok(handle) = notify_result {
            backend.loaded.insert(job.path.clone(), handle);
        } else {
            backend.loaded.remove(&job.path);
        }

        (
            backend.inflight.remove(&job.path).unwrap_or_default(),
            notify_result,
        )
    };

    notify_waiters(waiters, notify_result);
}

fn notify_waiters(waiters: Vec<Completion>, result: Result<ErasedHandle, AssetStreamingError>) {
    for waiter in waiters {
        waiter(result.clone());
    }
}

fn read_package_payload(
    package: &Arc<Mutex<StreamingAssetFile>>,
    entry: &StreamingAssetEntryInfo,
    path: &str,
) -> Result<(Vec<u8>, Vec<BlobWithBytes>), AssetStreamingError> {
    let mut package = package.lock();
    let payload =
        package
            .read_payload_bytes(entry)
            .map_err(|source| AssetStreamingError::PackageRead {
                path: path.to_string(),
                reason: source.to_string(),
            })?;

    let blob_infos = package.blobs_for_hash(entry.hash).to_vec();
    let mut blobs = Vec::with_capacity(blob_infos.len());
    for info in blob_infos {
        let bytes =
            package
                .read_blob_bytes(&info)
                .map_err(|source| AssetStreamingError::PackageRead {
                    path: path.to_string(),
                    reason: source.to_string(),
                })?;
        blobs.push(BlobWithBytes { info, bytes });
    }

    Ok((payload, blobs))
}

fn decode_mesh(
    path: &str,
    payload: &[u8],
    blobs: &[BlobWithBytes],
) -> Result<Mesh, AssetStreamingError> {
    let json = parse_payload_json(path, payload)?;
    let root = expect_object(&json, path, "mesh metadata root")?;

    let vertex_count = as_usize(
        required_field(root, "vertex_count", path)?,
        path,
        "mesh vertex_count",
    )?;
    let has_indices = as_bool(
        required_field(root, "has_indices", path)?,
        path,
        "mesh has_indices",
    )?;
    let index_count = as_usize(
        required_field(root, "index_count", path)?,
        path,
        "mesh index_count",
    )?;

    let material_ranges =
        parse_material_ranges(path, required_field(root, "material_ranges", path)?)?;

    let bones_value = required_field(root, "bones", path)?;
    let bones = parse_bones(path, bones_value, blobs)?;

    let bounds = expect_object(
        required_field(root, "bounding_sphere", path)?,
        path,
        "mesh bounding_sphere",
    )?;
    let center = parse_vec3(
        required_field(bounds, "center", path)?,
        path,
        "mesh bounding sphere center",
    )?;
    let radius = as_f32(
        required_field(bounds, "radius", path)?,
        path,
        "mesh bounding sphere radius",
    )?;

    let vertex_blob = require_blob(path, blobs, StreamingAssetBlobKind::MeshVertices)?;
    let vertices = decode_vertex_blob(path, vertex_blob, vertex_count)?;

    let indices = if has_indices {
        let index_blob = require_blob(path, blobs, StreamingAssetBlobKind::MeshIndices)?;
        Some(decode_u32_blob(path, index_blob, index_count)?)
    } else {
        None
    };

    Ok(Mesh {
        data: Arc::new(MeshVertexData::new(vertices, indices)),
        material_ranges,
        bones,
        bounding_sphere: BoundingSphere { center, radius },
    })
}

fn parse_bones(
    path: &str,
    value: &JsonValue,
    blobs: &[BlobWithBytes],
) -> Result<Bones, AssetStreamingError> {
    let bones = expect_object(value, path, "mesh bones")?;
    let names = parse_string_array(
        required_field(bones, "names", path)?,
        path,
        "mesh bone names",
    )?;
    let parents = parse_optional_usize_array(
        required_field(bones, "parents", path)?,
        path,
        "mesh bone parents",
    )?;
    let children = parse_nested_usize_array(
        required_field(bones, "children", path)?,
        path,
        "mesh bone children",
    )?;
    let roots = parse_usize_array(
        required_field(bones, "roots", path)?,
        path,
        "mesh bone roots",
    )?;
    let index_of = parse_string_usize_map(
        required_field(bones, "index_of", path)?,
        path,
        "mesh bone index map",
    )?;

    let inverse_bind_count = as_usize(
        required_field(bones, "inverse_bind_count", path)?,
        path,
        "mesh inverse bind count",
    )?;
    let bind_global_count = as_usize(
        required_field(bones, "bind_global_count", path)?,
        path,
        "mesh bind global count",
    )?;
    let bind_local_count = as_usize(
        required_field(bones, "bind_local_count", path)?,
        path,
        "mesh bind local count",
    )?;

    let inverse_bind = decode_mat4_blob(
        path,
        find_blob(blobs, StreamingAssetBlobKind::BonesInverseBind),
        inverse_bind_count,
        "mesh inverse bind matrices",
    )?;
    let bind_global = decode_mat4_blob(
        path,
        find_blob(blobs, StreamingAssetBlobKind::BonesBindGlobal),
        bind_global_count,
        "mesh global bind matrices",
    )?;
    let bind_local = decode_mat4_blob(
        path,
        find_blob(blobs, StreamingAssetBlobKind::BonesBindLocal),
        bind_local_count,
        "mesh local bind matrices",
    )?;

    Ok(Bones {
        names,
        parents,
        children,
        roots,
        inverse_bind,
        bind_global,
        bind_local,
        index_of,
    })
}

fn decode_texture(
    path: &str,
    payload: &[u8],
    blobs: &[BlobWithBytes],
) -> Result<Texture2D, AssetStreamingError> {
    let json = parse_payload_json(path, payload)?;
    let root = expect_object(&json, path, "texture metadata root")?;

    let width = as_u32(required_field(root, "width", path)?, path, "texture width")?;
    let height = as_u32(
        required_field(root, "height", path)?,
        path,
        "texture height",
    )?;
    let format = parse_texture_format(
        as_string(
            required_field(root, "format", path)?,
            path,
            "texture format",
        )?,
        path,
    )?;
    let repeat_mode = parse_address_mode(
        as_string(
            required_field(root, "repeat_mode", path)?,
            path,
            "texture repeat mode",
        )?,
        path,
    )?;
    let filter_mode = parse_filter_mode(
        as_string(
            required_field(root, "filter_mode", path)?,
            path,
            "texture filter mode",
        )?,
        path,
    )?;
    let mip_filter_mode = parse_mip_filter_mode(
        as_string(
            required_field(root, "mip_filter_mode", path)?,
            path,
            "texture mip filter mode",
        )?,
        path,
    )?;
    let has_transparency = as_bool(
        required_field(root, "has_transparency", path)?,
        path,
        "texture has_transparency",
    )?;
    let has_data = as_bool(
        required_field(root, "has_data", path)?,
        path,
        "texture has_data",
    )?;
    let data_len = optional_field(root, "data_len")
        .map(|value| as_usize(value, path, "texture data_len"))
        .transpose()?
        .unwrap_or(0usize);

    let data = if has_data {
        let blob = require_blob(path, blobs, StreamingAssetBlobKind::TextureData)?;
        if data_len != 0 && blob.bytes.len() != data_len {
            return Err(decode_error(
                path,
                format!(
                    "texture data blob length {} did not match metadata data_len {}",
                    blob.bytes.len(),
                    data_len
                ),
            ));
        }
        Some(blob.bytes.clone())
    } else {
        None
    };

    Ok(Texture2D {
        width,
        height,
        format,
        data,
        repeat_mode,
        filter_mode,
        mip_filter_mode,
        has_transparency,
    })
}

fn decode_cubemap(
    path: &str,
    payload: &[u8],
    blobs: &[BlobWithBytes],
) -> Result<Cubemap, AssetStreamingError> {
    let json = parse_payload_json(path, payload)?;
    let root = expect_object(&json, path, "texture metadata root")?;

    let width = as_u32(required_field(root, "width", path)?, path, "texture width")?;
    let height = as_u32(
        required_field(root, "height", path)?,
        path,
        "texture height",
    )?;
    let mip_level_count = as_u32(
        required_field(root, "mip_level_count", path)?,
        path,
        "mip level count",
    )?;
    let format = parse_texture_format(
        as_string(
            required_field(root, "format", path)?,
            path,
            "texture format",
        )?,
        path,
    )?;
    let repeat_mode = parse_address_mode(
        as_string(
            required_field(root, "repeat_mode", path)?,
            path,
            "texture repeat mode",
        )?,
        path,
    )?;
    let filter_mode = parse_filter_mode(
        as_string(
            required_field(root, "filter_mode", path)?,
            path,
            "texture filter mode",
        )?,
        path,
    )?;
    let mip_filter_mode = parse_mip_filter_mode(
        as_string(
            required_field(root, "mip_filter_mode", path)?,
            path,
            "texture mip filter mode",
        )?,
        path,
    )?;
    let has_transparency = as_bool(
        required_field(root, "has_transparency", path)?,
        path,
        "texture has_transparency",
    )?;
    let has_data = as_bool(
        required_field(root, "has_data", path)?,
        path,
        "texture has_data",
    )?;
    let data_len = optional_field(root, "data_len")
        .map(|value| as_usize(value, path, "texture data_len"))
        .transpose()?
        .unwrap_or(0usize);

    let data = if has_data {
        let blob = require_blob(path, blobs, StreamingAssetBlobKind::TextureData)?;
        if data_len != 0 && blob.bytes.len() != data_len {
            return Err(decode_error(
                path,
                format!(
                    "texture data blob length {} did not match metadata data_len {}",
                    blob.bytes.len(),
                    data_len
                ),
            ));
        }
        Some(blob.bytes.clone())
    } else {
        None
    };

    Ok(Cubemap {
        width,
        height,
        mip_level_count,
        format,
        data,
        repeat_mode,
        filter_mode,
        mip_filter_mode,
        has_transparency,
    })
}

fn decode_shader(path: &str, payload: &[u8]) -> Result<Shader, AssetStreamingError> {
    let json = parse_payload_json(path, payload)?;
    let root = expect_object(&json, path, "shader metadata root")?;

    let name = as_string(required_field(root, "name", path)?, path, "shader name")?.to_string();
    let code_obj = expect_object(
        required_field(root, "code", path)?,
        path,
        "shader code block",
    )?;
    let code_kind = as_string(
        required_field(code_obj, "kind", path)?,
        path,
        "shader code kind",
    )?;
    let source = as_string(
        required_field(code_obj, "source", path)?,
        path,
        "shader source",
    )?;
    let code = match code_kind {
        "Full" => ShaderCode::Full(source.to_string()),
        "Fragment" => ShaderCode::Fragment(source.to_string()),
        other => {
            return Err(decode_error(
                path,
                format!("unsupported shader code kind '{other}'"),
            ));
        }
    };

    let shader_type = optional_field(root, "shader_type")
        .map(|value| parse_shader_type(as_string(value, path, "shader type")?, path))
        .transpose()?
        .unwrap_or(ShaderType::Default);
    let polygon_mode = optional_field(root, "polygon_mode")
        .map(|value| parse_polygon_mode(as_string(value, path, "shader polygon mode")?, path))
        .transpose()?
        .unwrap_or(PolygonMode::Fill);
    let topology = optional_field(root, "topology")
        .map(|value| parse_topology(as_string(value, path, "shader topology")?, path))
        .transpose()?
        .unwrap_or(PrimitiveTopology::TriangleList);
    let immediate_size = optional_field(root, "immediate_size")
        .map(|value| as_u32(value, path, "shader immediate_size"))
        .transpose()?
        .unwrap_or(0);
    let depth_enabled = optional_field(root, "depth_enabled")
        .map(|value| as_bool(value, path, "shader depth_enabled"))
        .transpose()?
        .unwrap_or(true);
    let shadow_transparency = optional_field(root, "shadow_transparency")
        .map(|value| as_bool(value, path, "shader shadow_transparency"))
        .transpose()?
        .unwrap_or(false);

    Ok(Shader::builder()
        .name(name)
        .code(code)
        .shader_type(shader_type)
        .polygon_mode(polygon_mode)
        .topology(topology)
        .immediate_size(immediate_size)
        .depth_enabled(depth_enabled)
        .shadow_transparency(shadow_transparency)
        .build())
}

fn decode_prefab_material(
    path: &str,
    payload: &[u8],
) -> Result<PrefabMaterial, AssetStreamingError> {
    let json = parse_payload_json(path, payload)?;
    let root = expect_object(&json, path, "prefab material metadata root")?;

    Ok(PrefabMaterial {
        name: as_string(required_field(root, "name", path)?, path, "material name")?.to_string(),
        base_color: parse_vec4(
            required_field(root, "base_color", path)?,
            path,
            "material base_color",
        )?,
        metallic: as_f32(
            required_field(root, "metallic", path)?,
            path,
            "material metallic",
        )?,
        roughness: as_f32(
            required_field(root, "roughness", path)?,
            path,
            "material roughness",
        )?,
        alpha_cutoff: optional_field(root, "alpha_cutoff")
            .map(|value| as_optional_f32(value, path, "material alpha_cutoff"))
            .transpose()?
            .flatten(),
        alpha_mode: as_string(
            required_field(root, "alpha_mode", path)?,
            path,
            "material alpha_mode",
        )?
        .to_string(),
        double_sided: as_bool(
            required_field(root, "double_sided", path)?,
            path,
            "material double_sided",
        )?,
        unlit: as_bool(required_field(root, "unlit", path)?, path, "material unlit")?,
        emissive_factor: parse_vec3(
            required_field(root, "emissive_factor", path)?,
            path,
            "material emissive_factor",
        )?,
        base_color_texture: parse_optional_string(
            optional_field(root, "base_color_texture").unwrap_or(&JsonValue::Null),
            path,
            "material base_color_texture",
        )?,
        normal_texture: parse_optional_string(
            optional_field(root, "normal_texture").unwrap_or(&JsonValue::Null),
            path,
            "material normal_texture",
        )?,
        metallic_roughness_texture: parse_optional_string(
            optional_field(root, "metallic_roughness_texture").unwrap_or(&JsonValue::Null),
            path,
            "material metallic_roughness_texture",
        )?,
        emissive_texture: parse_optional_string(
            optional_field(root, "emissive_texture").unwrap_or(&JsonValue::Null),
            path,
            "material emissive_texture",
        )?,
        occlusion_texture: parse_optional_string(
            optional_field(root, "occlusion_texture").unwrap_or(&JsonValue::Null),
            path,
            "material occlusion_texture",
        )?,
    })
}

fn decode_prefab_asset(path: &str, payload: &[u8]) -> Result<PrefabAsset, AssetStreamingError> {
    let json = parse_payload_json(path, payload)?;
    let root = expect_object(&json, path, "prefab metadata root")?;

    let nodes_value = required_field(root, "nodes", path)?;
    let nodes_array = expect_array(nodes_value, path, "prefab nodes")?;
    let mut nodes = Vec::with_capacity(nodes_array.len());
    for node_value in nodes_array {
        nodes.push(parse_prefab_node(path, node_value)?);
    }

    Ok(PrefabAsset {
        source: as_string(required_field(root, "source", path)?, path, "prefab source")?
            .to_string(),
        root_nodes: parse_u32_array(
            required_field(root, "root_nodes", path)?,
            path,
            "prefab root_nodes",
        )?,
        nodes,
        animation_assets: parse_string_array(
            required_field(root, "animation_assets", path)?,
            path,
            "prefab animation_assets",
        )?,
    })
}

fn decode_animation_clip(
    path: &str,
    payload: &[u8],
    blobs: &[BlobWithBytes],
) -> Result<AnimationClip, AssetStreamingError> {
    let json = parse_payload_json(path, payload)?;
    let root = expect_object(&json, path, "animation metadata root")?;
    let channels_value = required_field(root, "channels", path)?;
    let channel_values = expect_array(channels_value, path, "animation channels")?;
    let declared_channel_count = as_usize(
        required_field(root, "channel_count", path)?,
        path,
        "animation channel_count",
    )?;
    if declared_channel_count != channel_values.len() {
        return Err(decode_error(
            path,
            format!(
                "animation channel_count {} did not match channels length {}",
                declared_channel_count,
                channel_values.len()
            ),
        ));
    }

    let mut cursor = AnimationBlobCursor::new(blobs);
    let mut channels = Vec::with_capacity(channel_values.len());
    for channel_value in channel_values {
        channels.push(parse_animation_channel(path, channel_value, &mut cursor)?);
    }
    cursor.ensure_exhausted(path)?;

    Ok(AnimationClip {
        name: as_string(required_field(root, "name", path)?, path, "animation name")?.to_string(),
        duration: as_f32(
            required_field(root, "duration", path)?,
            path,
            "animation duration",
        )?,
        channels,
    })
}

fn parse_animation_channel(
    path: &str,
    value: &JsonValue,
    cursor: &mut AnimationBlobCursor<'_>,
) -> Result<AnimationChannel, AssetStreamingError> {
    let channel = expect_object(value, path, "animation channel")?;
    let keys = expect_object(
        required_field(channel, "keys", path)?,
        path,
        "animation keys",
    )?;

    let t_times_count = as_usize(
        required_field(keys, "t_times_count", path)?,
        path,
        "animation t_times_count",
    )?;
    let t_values_count = as_usize(
        required_field(keys, "t_values_count", path)?,
        path,
        "animation t_values_count",
    )?;
    let r_times_count = as_usize(
        required_field(keys, "r_times_count", path)?,
        path,
        "animation r_times_count",
    )?;
    let r_values_count = as_usize(
        required_field(keys, "r_values_count", path)?,
        path,
        "animation r_values_count",
    )?;
    let s_times_count = as_usize(
        required_field(keys, "s_times_count", path)?,
        path,
        "animation s_times_count",
    )?;
    let s_values_count = as_usize(
        required_field(keys, "s_values_count", path)?,
        path,
        "animation s_values_count",
    )?;

    Ok(AnimationChannel {
        target_name: as_string(
            required_field(channel, "target_name", path)?,
            path,
            "animation target_name",
        )?
        .to_string(),
        keys: TransformKeys {
            t_times: decode_f32_blob(
                path,
                cursor.take(
                    path,
                    StreamingAssetBlobKind::AnimationTranslationTimes,
                    t_times_count,
                    "animation translation times",
                )?,
                t_times_count,
                "animation translation times",
            )?,
            t_values: decode_vec3_blob(
                path,
                cursor.take(
                    path,
                    StreamingAssetBlobKind::AnimationTranslationValues,
                    t_values_count,
                    "animation translation values",
                )?,
                t_values_count,
                "animation translation values",
            )?,
            r_times: decode_f32_blob(
                path,
                cursor.take(
                    path,
                    StreamingAssetBlobKind::AnimationRotationTimes,
                    r_times_count,
                    "animation rotation times",
                )?,
                r_times_count,
                "animation rotation times",
            )?,
            r_values: decode_quat_blob(
                path,
                cursor.take(
                    path,
                    StreamingAssetBlobKind::AnimationRotationValues,
                    r_values_count,
                    "animation rotation values",
                )?,
                r_values_count,
                "animation rotation values",
            )?,
            s_times: decode_f32_blob(
                path,
                cursor.take(
                    path,
                    StreamingAssetBlobKind::AnimationScaleTimes,
                    s_times_count,
                    "animation scale times",
                )?,
                s_times_count,
                "animation scale times",
            )?,
            s_values: decode_vec3_blob(
                path,
                cursor.take(
                    path,
                    StreamingAssetBlobKind::AnimationScaleValues,
                    s_values_count,
                    "animation scale values",
                )?,
                s_values_count,
                "animation scale values",
            )?,
        },
    })
}

struct AnimationBlobCursor<'a> {
    by_kind: HashMap<StreamingAssetBlobKind, Vec<&'a BlobWithBytes>>,
}

impl<'a> AnimationBlobCursor<'a> {
    fn new(blobs: &'a [BlobWithBytes]) -> Self {
        let mut by_kind: HashMap<StreamingAssetBlobKind, Vec<&'a BlobWithBytes>> = HashMap::new();
        for blob in blobs {
            by_kind.entry(blob.info.kind).or_default().push(blob);
        }
        Self { by_kind }
    }

    fn take(
        &mut self,
        path: &str,
        kind: StreamingAssetBlobKind,
        expected_count: usize,
        label: &str,
    ) -> Result<Option<&'a BlobWithBytes>, AssetStreamingError> {
        if expected_count == 0 {
            return Ok(None);
        }

        let Some(blobs) = self.by_kind.get_mut(&kind) else {
            return Err(decode_error(
                path,
                format!("missing {label} blob for {expected_count} entries"),
            ));
        };
        if blobs.is_empty() {
            return Err(decode_error(
                path,
                format!("missing {label} blob for {expected_count} entries"),
            ));
        }
        Ok(Some(blobs.remove(0)))
    }

    fn ensure_exhausted(&self, path: &str) -> Result<(), AssetStreamingError> {
        for (kind, remaining) in &self.by_kind {
            if !remaining.is_empty() {
                return Err(decode_error(
                    path,
                    format!(
                        "unused '{}' blob sections remained after animation decode ({})",
                        kind.name(),
                        remaining.len()
                    ),
                ));
            }
        }
        Ok(())
    }
}

fn parse_prefab_node(path: &str, value: &JsonValue) -> Result<PrefabNode, AssetStreamingError> {
    let object = expect_object(value, path, "prefab node")?;

    let mesh = match optional_field(object, "mesh") {
        None => None,
        Some(JsonValue::Null) => None,
        Some(mesh_value) => {
            let mesh = expect_object(mesh_value, path, "prefab mesh binding")?;
            Some(PrefabMeshBinding {
                mesh_asset: as_string(
                    required_field(mesh, "mesh_asset", path)?,
                    path,
                    "prefab mesh asset path",
                )?
                .to_string(),
                material_hashes: parse_optional_u64_array(
                    required_field(mesh, "material_hashes", path)?,
                    path,
                    "prefab material hashes",
                )?,
            })
        }
    };

    Ok(PrefabNode {
        name: as_string(
            required_field(object, "name", path)?,
            path,
            "prefab node name",
        )?
        .to_string(),
        local_position: parse_vec3(
            required_field(object, "local_position", path)?,
            path,
            "prefab local_position",
        )?,
        local_rotation: parse_quat(
            required_field(object, "local_rotation", path)?,
            path,
            "prefab local_rotation",
        )?,
        local_scale: parse_vec3(
            required_field(object, "local_scale", path)?,
            path,
            "prefab local_scale",
        )?,
        children: parse_u32_array(
            required_field(object, "children", path)?,
            path,
            "prefab children",
        )?,
        mesh,
        extras_json: parse_optional_string(
            optional_field(object, "extras_json").unwrap_or(&JsonValue::Null),
            path,
            "prefab extras_json",
        )?,
    })
}

fn decode_vertex_blob(
    path: &str,
    blob: &BlobWithBytes,
    expected_count: usize,
) -> Result<Vec<Vertex3D>, AssetStreamingError> {
    const VERTEX_STRIDE: usize = 76;
    validate_blob_length(
        path,
        &blob.info,
        blob.bytes.len(),
        expected_count,
        VERTEX_STRIDE,
        "mesh vertex",
    )?;

    let mut vertices = Vec::with_capacity(expected_count);
    for chunk in blob.bytes.chunks_exact(VERTEX_STRIDE) {
        let position = Vec3::new(
            read_f32(&chunk[0..4]),
            read_f32(&chunk[4..8]),
            read_f32(&chunk[8..12]),
        );
        let uv = Vec2::new(read_f32(&chunk[12..16]), read_f32(&chunk[16..20]));
        let normal = Vec3::new(
            read_f32(&chunk[20..24]),
            read_f32(&chunk[24..28]),
            read_f32(&chunk[28..32]),
        );
        let tangent = Vec3::new(
            read_f32(&chunk[32..36]),
            read_f32(&chunk[36..40]),
            read_f32(&chunk[40..44]),
        );
        let bone_indices = [
            read_u32(&chunk[44..48]),
            read_u32(&chunk[48..52]),
            read_u32(&chunk[52..56]),
            read_u32(&chunk[56..60]),
        ];
        let bone_weights = [
            read_f32(&chunk[60..64]),
            read_f32(&chunk[64..68]),
            read_f32(&chunk[68..72]),
            read_f32(&chunk[72..76]),
        ];

        vertices.push(Vertex3D {
            position,
            uv,
            normal,
            tangent,
            bone_indices,
            bone_weights,
        });
    }

    Ok(vertices)
}

fn decode_u32_blob(
    path: &str,
    blob: &BlobWithBytes,
    expected_count: usize,
) -> Result<Vec<u32>, AssetStreamingError> {
    const STRIDE: usize = 4;
    validate_blob_length(
        path,
        &blob.info,
        blob.bytes.len(),
        expected_count,
        STRIDE,
        "mesh index",
    )?;

    let mut out = Vec::with_capacity(expected_count);
    for chunk in blob.bytes.chunks_exact(STRIDE) {
        out.push(read_u32(chunk));
    }
    Ok(out)
}

fn decode_mat4_blob(
    path: &str,
    blob: Option<&BlobWithBytes>,
    expected_count: usize,
    label: &str,
) -> Result<Vec<Mat4>, AssetStreamingError> {
    const STRIDE: usize = 64;
    if expected_count == 0 {
        return Ok(Vec::new());
    }
    let Some(blob) = blob else {
        return Err(decode_error(
            path,
            format!("missing {label} blob for {expected_count} entries"),
        ));
    };

    validate_blob_length(
        path,
        &blob.info,
        blob.bytes.len(),
        expected_count,
        STRIDE,
        label,
    )?;

    let mut matrices = Vec::with_capacity(expected_count);
    for chunk in blob.bytes.chunks_exact(STRIDE) {
        let mut cols = [0.0_f32; 16];
        for (index, value) in cols.iter_mut().enumerate() {
            let offset = index * 4;
            *value = read_f32(&chunk[offset..offset + 4]);
        }
        matrices.push(Mat4::from_cols_array(&cols));
    }
    Ok(matrices)
}

fn decode_f32_blob(
    path: &str,
    blob: Option<&BlobWithBytes>,
    expected_count: usize,
    label: &str,
) -> Result<Vec<f32>, AssetStreamingError> {
    const STRIDE: usize = 4;
    if expected_count == 0 {
        return Ok(Vec::new());
    }
    let Some(blob) = blob else {
        return Err(decode_error(
            path,
            format!("missing {label} blob for {expected_count} entries"),
        ));
    };

    validate_blob_length(
        path,
        &blob.info,
        blob.bytes.len(),
        expected_count,
        STRIDE,
        label,
    )?;

    let mut values = Vec::with_capacity(expected_count);
    for chunk in blob.bytes.chunks_exact(STRIDE) {
        values.push(read_f32(chunk));
    }
    Ok(values)
}

fn decode_vec3_blob(
    path: &str,
    blob: Option<&BlobWithBytes>,
    expected_count: usize,
    label: &str,
) -> Result<Vec<Vec3>, AssetStreamingError> {
    const STRIDE: usize = 12;
    if expected_count == 0 {
        return Ok(Vec::new());
    }
    let Some(blob) = blob else {
        return Err(decode_error(
            path,
            format!("missing {label} blob for {expected_count} entries"),
        ));
    };

    validate_blob_length(
        path,
        &blob.info,
        blob.bytes.len(),
        expected_count,
        STRIDE,
        label,
    )?;

    let mut values = Vec::with_capacity(expected_count);
    for chunk in blob.bytes.chunks_exact(STRIDE) {
        values.push(Vec3::new(
            read_f32(&chunk[0..4]),
            read_f32(&chunk[4..8]),
            read_f32(&chunk[8..12]),
        ));
    }
    Ok(values)
}

fn decode_quat_blob(
    path: &str,
    blob: Option<&BlobWithBytes>,
    expected_count: usize,
    label: &str,
) -> Result<Vec<Quat>, AssetStreamingError> {
    const STRIDE: usize = 16;
    if expected_count == 0 {
        return Ok(Vec::new());
    }
    let Some(blob) = blob else {
        return Err(decode_error(
            path,
            format!("missing {label} blob for {expected_count} entries"),
        ));
    };

    validate_blob_length(
        path,
        &blob.info,
        blob.bytes.len(),
        expected_count,
        STRIDE,
        label,
    )?;

    let mut values = Vec::with_capacity(expected_count);
    for chunk in blob.bytes.chunks_exact(STRIDE) {
        values.push(Quat::from_array([
            read_f32(&chunk[0..4]),
            read_f32(&chunk[4..8]),
            read_f32(&chunk[8..12]),
            read_f32(&chunk[12..16]),
        ]));
    }
    Ok(values)
}

fn validate_blob_length(
    path: &str,
    info: &StreamingAssetBlobInfo,
    data_len: usize,
    expected_count: usize,
    stride: usize,
    label: &str,
) -> Result<(), AssetStreamingError> {
    if info.element_count != expected_count as u64 {
        return Err(decode_error(
            path,
            format!(
                "{label} blob element_count {} did not match expected {}",
                info.element_count, expected_count
            ),
        ));
    }

    let expected_bytes = expected_count
        .checked_mul(stride)
        .ok_or_else(|| decode_error(path, format!("{label} blob byte-size overflow")))?;
    if data_len != expected_bytes {
        return Err(decode_error(
            path,
            format!("{label} blob byte length {data_len} did not match expected {expected_bytes}"),
        ));
    }
    Ok(())
}

fn find_blob(blobs: &[BlobWithBytes], kind: StreamingAssetBlobKind) -> Option<&BlobWithBytes> {
    blobs.iter().find(|blob| blob.info.kind == kind)
}

fn require_blob<'a>(
    path: &str,
    blobs: &'a [BlobWithBytes],
    kind: StreamingAssetBlobKind,
) -> Result<&'a BlobWithBytes, AssetStreamingError> {
    find_blob(blobs, kind)
        .ok_or_else(|| decode_error(path, format!("missing blob section '{}'", kind.name())))
}

fn parse_material_ranges(
    path: &str,
    value: &JsonValue,
) -> Result<Vec<Range<u32>>, AssetStreamingError> {
    let array = expect_array(value, path, "mesh material ranges")?;
    let mut ranges = Vec::with_capacity(array.len());
    for range in array {
        let object = expect_object(range, path, "mesh material range")?;
        let start = as_u32(required_field(object, "start", path)?, path, "range start")?;
        let end = as_u32(required_field(object, "end", path)?, path, "range end")?;
        ranges.push(start..end);
    }
    Ok(ranges)
}

fn parse_payload_json(path: &str, payload: &[u8]) -> Result<JsonValue, AssetStreamingError> {
    serde_json::from_slice(payload)
        .map_err(|source| decode_error(path, format!("invalid JSON payload: {source}")))
}

fn parse_string_array(
    value: &JsonValue,
    path: &str,
    label: &str,
) -> Result<Vec<String>, AssetStreamingError> {
    let values = expect_array(value, path, label)?;
    let mut out = Vec::with_capacity(values.len());
    for item in values {
        out.push(as_string(item, path, label)?.to_string());
    }
    Ok(out)
}

fn parse_u32_array(
    value: &JsonValue,
    path: &str,
    label: &str,
) -> Result<Vec<u32>, AssetStreamingError> {
    let values = expect_array(value, path, label)?;
    let mut out = Vec::with_capacity(values.len());
    for item in values {
        out.push(as_u32(item, path, label)?);
    }
    Ok(out)
}

fn parse_usize_array(
    value: &JsonValue,
    path: &str,
    label: &str,
) -> Result<Vec<usize>, AssetStreamingError> {
    let values = expect_array(value, path, label)?;
    let mut out = Vec::with_capacity(values.len());
    for item in values {
        out.push(as_usize(item, path, label)?);
    }
    Ok(out)
}

fn parse_optional_u64_array(
    value: &JsonValue,
    path: &str,
    label: &str,
) -> Result<Vec<Option<u64>>, AssetStreamingError> {
    let values = expect_array(value, path, label)?;
    let mut out = Vec::with_capacity(values.len());
    for item in values {
        if item.is_null() {
            out.push(None);
        } else {
            out.push(Some(as_u64(item, path, label)?));
        }
    }
    Ok(out)
}

fn parse_optional_usize_array(
    value: &JsonValue,
    path: &str,
    label: &str,
) -> Result<Vec<Option<usize>>, AssetStreamingError> {
    let values = expect_array(value, path, label)?;
    let mut out = Vec::with_capacity(values.len());
    for item in values {
        if item.is_null() {
            out.push(None);
        } else {
            out.push(Some(as_usize(item, path, label)?));
        }
    }
    Ok(out)
}

fn parse_nested_usize_array(
    value: &JsonValue,
    path: &str,
    label: &str,
) -> Result<Vec<Vec<usize>>, AssetStreamingError> {
    let values = expect_array(value, path, label)?;
    let mut out = Vec::with_capacity(values.len());
    for item in values {
        out.push(parse_usize_array(item, path, label)?);
    }
    Ok(out)
}

fn parse_string_usize_map(
    value: &JsonValue,
    path: &str,
    label: &str,
) -> Result<HashMap<String, usize>, AssetStreamingError> {
    let object = expect_object(value, path, label)?;
    let mut out = HashMap::with_capacity(object.len());
    for (key, value) in object {
        out.insert(key.clone(), as_usize(value, path, label)?);
    }
    Ok(out)
}

fn parse_vec3(value: &JsonValue, path: &str, label: &str) -> Result<Vec3, AssetStreamingError> {
    let array = expect_array(value, path, label)?;
    if array.len() != 3 {
        return Err(decode_error(
            path,
            format!("{label} expected 3 elements but found {}", array.len()),
        ));
    }
    Ok(Vec3::new(
        as_f32(&array[0], path, label)?,
        as_f32(&array[1], path, label)?,
        as_f32(&array[2], path, label)?,
    ))
}

fn parse_vec4(value: &JsonValue, path: &str, label: &str) -> Result<Vec4, AssetStreamingError> {
    let array = expect_array(value, path, label)?;
    if array.len() != 4 {
        return Err(decode_error(
            path,
            format!("{label} expected 4 elements but found {}", array.len()),
        ));
    }
    Ok(Vec4::new(
        as_f32(&array[0], path, label)?,
        as_f32(&array[1], path, label)?,
        as_f32(&array[2], path, label)?,
        as_f32(&array[3], path, label)?,
    ))
}

fn parse_quat(value: &JsonValue, path: &str, label: &str) -> Result<Quat, AssetStreamingError> {
    let vec = parse_vec4(value, path, label)?;
    Ok(Quat::from_array(vec.to_array()))
}

fn parse_optional_string(
    value: &JsonValue,
    path: &str,
    label: &str,
) -> Result<Option<String>, AssetStreamingError> {
    if value.is_null() {
        return Ok(None);
    }
    Ok(Some(as_string(value, path, label)?.to_string()))
}

fn as_optional_f32(
    value: &JsonValue,
    path: &str,
    label: &str,
) -> Result<Option<f32>, AssetStreamingError> {
    if value.is_null() {
        return Ok(None);
    }
    Ok(Some(as_f32(value, path, label)?))
}

fn expect_object<'a>(
    value: &'a JsonValue,
    path: &str,
    label: &str,
) -> Result<&'a JsonMap<String, JsonValue>, AssetStreamingError> {
    value
        .as_object()
        .ok_or_else(|| decode_error(path, format!("{label} must be a JSON object")))
}

fn expect_array<'a>(
    value: &'a JsonValue,
    path: &str,
    label: &str,
) -> Result<&'a [JsonValue], AssetStreamingError> {
    value
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| decode_error(path, format!("{label} must be a JSON array")))
}

fn required_field<'a>(
    object: &'a JsonMap<String, JsonValue>,
    field: &str,
    path: &str,
) -> Result<&'a JsonValue, AssetStreamingError> {
    object
        .get(field)
        .ok_or_else(|| decode_error(path, format!("missing '{field}' field")))
}

fn optional_field<'a>(
    object: &'a JsonMap<String, JsonValue>,
    field: &str,
) -> Option<&'a JsonValue> {
    object.get(field)
}

fn as_u64(value: &JsonValue, path: &str, label: &str) -> Result<u64, AssetStreamingError> {
    if let Some(value) = value.as_u64() {
        return Ok(value);
    }
    if let Some(value) = value.as_i64()
        && value >= 0
    {
        return Ok(value as u64);
    }
    Err(decode_error(
        path,
        format!("{label} must be an unsigned integer"),
    ))
}

fn as_u32(value: &JsonValue, path: &str, label: &str) -> Result<u32, AssetStreamingError> {
    let value = as_u64(value, path, label)?;
    u32::try_from(value)
        .map_err(|_| decode_error(path, format!("{label} value {value} does not fit into u32")))
}

fn as_usize(value: &JsonValue, path: &str, label: &str) -> Result<usize, AssetStreamingError> {
    let value = as_u64(value, path, label)?;
    usize::try_from(value).map_err(|_| {
        decode_error(
            path,
            format!("{label} value {value} does not fit into usize"),
        )
    })
}

fn as_f32(value: &JsonValue, path: &str, label: &str) -> Result<f32, AssetStreamingError> {
    let Some(value) = value.as_f64() else {
        return Err(decode_error(path, format!("{label} must be a number")));
    };
    Ok(value as f32)
}

fn as_bool(value: &JsonValue, path: &str, label: &str) -> Result<bool, AssetStreamingError> {
    value
        .as_bool()
        .ok_or_else(|| decode_error(path, format!("{label} must be a boolean")))
}

fn as_string<'a>(
    value: &'a JsonValue,
    path: &str,
    label: &str,
) -> Result<&'a str, AssetStreamingError> {
    value
        .as_str()
        .ok_or_else(|| decode_error(path, format!("{label} must be a string")))
}

fn parse_texture_format(value: &str, path: &str) -> Result<TextureFormat, AssetStreamingError> {
    match value {
        "R8Unorm" => Ok(TextureFormat::R8Unorm),
        "Rg8Unorm" => Ok(TextureFormat::Rg8Unorm),
        "Rgba8Unorm" => Ok(TextureFormat::Rgba8Unorm),
        "Rgba8UnormSrgb" => Ok(TextureFormat::Rgba8UnormSrgb),
        "Bgra8Unorm" => Ok(TextureFormat::Bgra8Unorm),
        "Bgra8UnormSrgb" => Ok(TextureFormat::Bgra8UnormSrgb),
        "Bc1RgbaUnorm" => Ok(TextureFormat::Bc1RgbaUnorm),
        "Bc1RgbaUnormSrgb" => Ok(TextureFormat::Bc1RgbaUnormSrgb),
        "Bc3RgbaUnorm" => Ok(TextureFormat::Bc3RgbaUnorm),
        "Bc3RgbaUnormSrgb" => Ok(TextureFormat::Bc3RgbaUnormSrgb),
        "R16Unorm" => Ok(TextureFormat::R16Unorm),
        "Rg16Snorm" => Ok(TextureFormat::Rg16Snorm),
        "Rgba16Unorm" => Ok(TextureFormat::Rgba16Unorm),
        "Rgba32Float" => Ok(TextureFormat::Rgba32Float),
        other => Err(decode_error(
            path,
            format!("unsupported texture format '{other}'"),
        )),
    }
}

fn parse_address_mode(value: &str, path: &str) -> Result<AddressMode, AssetStreamingError> {
    match value {
        "ClampToEdge" => Ok(AddressMode::ClampToEdge),
        "Repeat" => Ok(AddressMode::Repeat),
        "MirrorRepeat" => Ok(AddressMode::MirrorRepeat),
        "ClampToBorder" => Ok(AddressMode::ClampToBorder),
        other => Err(decode_error(
            path,
            format!("unsupported texture repeat mode '{other}'"),
        )),
    }
}

fn parse_filter_mode(value: &str, path: &str) -> Result<FilterMode, AssetStreamingError> {
    match value {
        "Nearest" => Ok(FilterMode::Nearest),
        "Linear" => Ok(FilterMode::Linear),
        other => Err(decode_error(
            path,
            format!("unsupported texture filter mode '{other}'"),
        )),
    }
}

fn parse_mip_filter_mode(value: &str, path: &str) -> Result<MipmapFilterMode, AssetStreamingError> {
    match value {
        "Nearest" => Ok(MipmapFilterMode::Nearest),
        "Linear" => Ok(MipmapFilterMode::Linear),
        other => Err(decode_error(
            path,
            format!("unsupported mip filter mode '{other}'"),
        )),
    }
}

fn parse_polygon_mode(value: &str, path: &str) -> Result<PolygonMode, AssetStreamingError> {
    match value {
        "Fill" => Ok(PolygonMode::Fill),
        "Line" => Ok(PolygonMode::Line),
        "Point" => Ok(PolygonMode::Point),
        other => Err(decode_error(
            path,
            format!("unsupported shader polygon mode '{other}'"),
        )),
    }
}

fn parse_topology(value: &str, path: &str) -> Result<PrimitiveTopology, AssetStreamingError> {
    match value {
        "PointList" => Ok(PrimitiveTopology::PointList),
        "LineList" => Ok(PrimitiveTopology::LineList),
        "LineStrip" => Ok(PrimitiveTopology::LineStrip),
        "TriangleList" => Ok(PrimitiveTopology::TriangleList),
        "TriangleStrip" => Ok(PrimitiveTopology::TriangleStrip),
        other => Err(decode_error(
            path,
            format!("unsupported shader topology '{other}'"),
        )),
    }
}

fn parse_shader_type(value: &str, path: &str) -> Result<ShaderType, AssetStreamingError> {
    match value {
        "Default" => Ok(ShaderType::Default),
        "Custom" => Ok(ShaderType::Custom),
        "PostProcessing" => Ok(ShaderType::PostProcessing),
        other => Err(decode_error(
            path,
            format!("unsupported shader type '{other}'"),
        )),
    }
}

fn with_sya_extension(path: &Path) -> PathBuf {
    let mut path = path.to_path_buf();
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_none_or(|extension| !extension.eq_ignore_ascii_case("sya"))
    {
        path.set_extension("sya");
    }
    path
}

fn canonicalize_or_self(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn decode_error(path: &str, reason: impl Into<String>) -> AssetStreamingError {
    AssetStreamingError::Decode {
        path: path.to_string(),
        reason: reason.into(),
    }
}

fn read_f32(bytes: &[u8]) -> f32 {
    let mut raw = [0_u8; 4];
    raw.copy_from_slice(bytes);
    f32::from_ne_bytes(raw)
}

fn read_u32(bytes: &[u8]) -> u32 {
    let mut raw = [0_u8; 4];
    raw.copy_from_slice(bytes);
    u32::from_ne_bytes(raw)
}

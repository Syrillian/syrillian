use crate::assets::prefab::{PrefabAsset, PrefabMaterial};
use crate::assets::{Mesh, Shader, Texture2D};
use crate::store::streaming::AssetStreamingError;
use crate::store::streaming::asset_store::{
    AssetType, StreamingAssetBlobInfo, StreamingAssetEntryInfo, StreamingAssetFile,
    normalize_asset_path,
};
use crate::store::streaming::error::{BlobSizeErr, Result};
use crate::store::streaming::payload::StreamableAsset;
use crate::store::{H, Store, StoreType};
use crate::{AnimationClip, AssetStore, Cubemap, SkinnedMesh};
use parking_lot::{Condvar, Mutex, RwLock};
use snafu::prelude::*;
use std::any::TypeId;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};
use std::thread;
use tracing::debug;
use zerocopy::{FromBytes, Immutable, KnownLayout, TryFromBytes};

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

impl StreamingLoadableAsset for SkinnedMesh {
    const PACKAGE_TYPE: AssetType = AssetType::SkinnedMesh;

    fn insert_into(store: &AssetStore, asset: Self) -> H<Self> {
        store.skinned_meshes.add(asset)
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

#[derive(Clone)]
pub struct StreamingAsset<A: StreamingLoadableAsset> {
    state: Arc<StreamingAssetState<A>>,
}

struct StreamingAssetState<A: StreamingLoadableAsset> {
    result: Mutex<Option<Result<H<A>>>>,
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

    fn ready(result: Result<H<A>>) -> Self {
        Self {
            state: Arc::new(StreamingAssetState {
                result: Mutex::new(Some(result)),
                cv: Condvar::new(),
            }),
        }
    }

    fn complete(&self, result: Result<H<A>>) {
        let mut lock = self.state.result.lock();
        *lock = Some(result);
        self.state.cv.notify_all();
    }

    pub fn is_ready(&self) -> bool {
        self.state.result.lock().is_some()
    }

    pub fn try_get(&self) -> Option<Result<H<A>>> {
        self.state.result.lock().clone()
    }

    pub fn wait(&self) -> Result<H<A>> {
        let mut lock = self.state.result.lock();

        while lock.is_none() {
            self.state.cv.wait(&mut lock);
        }

        lock.as_ref()
            .cloned()
            .expect("decoding asset result should always exist here")
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
    file: Arc<RwLock<StreamingAssetFile>>,
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

#[derive(Copy, Clone)]
struct ErasedHandle {
    type_id: TypeId,
    id: u32,
}

impl StreamingAssetBlobInfo {
    fn validate_count(&self, expected_count: usize, label: &str) -> Result<()> {
        ensure!(
            self.element_count == expected_count,
            BlobSizeErr {
                label,
                expected: expected_count,
                actual: self.element_count,
            }
        );

        Ok(())
    }
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

type Completion = Box<dyn FnOnce(Result<ErasedHandle>) + Send + 'static>;

#[derive(Clone)]
struct WorkerRuntime {
    meshes: Arc<Store<Mesh>>,
    skinned_meshes: Arc<Store<SkinnedMesh>>,
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
            skinned_meshes: store.skinned_meshes.clone(),
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
    pub fn hook_package<P: AsRef<Path>>(&self, package_path: P) -> Result<bool> {
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
        let mounted_file = Arc::new(RwLock::new(package));

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
            notify_waiters(waiters, Err(AssetStreamingError::OverwrittenMount { path }));
        }

        debug!("Mounted package {}", package_path.display());
        Ok(true)
    }

    pub fn hook_packages_in_directory<P: AsRef<Path>>(&self, directory: P) -> Result<usize> {
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

    pub fn hook_default_packages(&self) -> Result<usize> {
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
    ) -> Result<StreamingAsset<A>> {
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

        debug!("Queued decoding asset load '{}'", job.path);
        Ok(pending)
    }

    pub fn load_by_path<A: StreamingLoadableAsset>(&self, relative_path: &str) -> Result<H<A>> {
        self.request_by_path(relative_path)?.wait()
    }

    pub fn request_by_hash<A: StreamingLoadableAsset>(
        &self,
        hash: u64,
    ) -> Result<StreamingAsset<A>> {
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

    pub fn load_by_hash<A: StreamingLoadableAsset>(&self, hash: u64) -> Result<H<A>> {
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

    fn ensure_worker_started(&self, backend: &mut StreamingBackend) -> Result<()> {
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
) -> Result<mpsc::Sender<LoadJob>> {
    let (tx, rx) = mpsc::channel::<LoadJob>();
    thread::Builder::new()
        .name("syrillian-asset-stream".to_string())
        .spawn(move || {
            while let Ok(job) = rx.recv() {
                let result = runtime.process_load_job(&backend, &job);
                complete_job(&backend, &job, result);
            }
            debug!("Streaming asset worker thread terminated");
        })
        .map_err(|source| AssetStreamingError::WorkerSpawn {
            reason: source.to_string(),
        })?;

    Ok(tx)
}

impl WorkerRuntime {
    fn process_load_job(
        &self,
        backend: &Arc<Mutex<StreamingBackend>>,
        job: &LoadJob,
    ) -> Result<ErasedHandle> {
        let package_file = {
            let backend = backend.lock();
            let Some(package) = backend.packages.get(job.package_index) else {
                return Err(AssetStreamingError::PackageIndexMissing {
                    path: job.path.clone(),
                });
            };
            package.file.clone()
        };
        let mut package = package_file.write();

        let payload = package.read_payload(&job.entry, &job.path)?;
        match job.entry.asset_type {
            AssetType::Mesh => {
                let mesh = Mesh::decode(&payload, &mut package)?;
                Ok(ErasedHandle::of(self.meshes.add(mesh)))
            }
            AssetType::SkinnedMesh => {
                let mesh = SkinnedMesh::decode(&payload, &mut package)?;
                Ok(ErasedHandle::of(self.skinned_meshes.add(mesh)))
            }
            AssetType::Texture2D => {
                let texture = Texture2D::decode(&payload, &mut package)?;
                Ok(ErasedHandle::of(self.textures.add(texture)))
            }
            AssetType::Cubemap => {
                let cubemap = Cubemap::decode(&payload, &mut package)?;
                Ok(ErasedHandle::of(self.cubemaps.add(cubemap)))
            }
            AssetType::Shader => {
                let shader = Shader::decode(&payload, &mut package)?;
                Ok(ErasedHandle::of(self.shaders.add(shader)))
            }
            AssetType::Material => {
                let material = PrefabMaterial::decode(&payload, &mut package)?;
                Ok(ErasedHandle::of(self.prefab_materials.add(material)))
            }
            AssetType::Prefab => {
                let prefab = PrefabAsset::decode(&payload, &mut package)?;
                Ok(ErasedHandle::of(self.prefabs.add(prefab)))
            }
            AssetType::AnimationClip => {
                let animation_clip = AnimationClip::decode(&payload, &mut package)?;
                Ok(ErasedHandle::of(self.animation_clips.add(animation_clip)))
            }
            unsupported => Err(AssetStreamingError::UnsupportedType {
                path: job.path.clone(),
                asset_type: unsupported,
            }),
        }
    }
}

fn complete_job(
    backend: &Arc<Mutex<StreamingBackend>>,
    job: &LoadJob,
    result: Result<ErasedHandle>,
) {
    let (waiters, notify_result) = {
        let mut backend = backend.lock();

        let stale = backend.path_index.get(&job.path).is_none_or(|indexed| {
            indexed.package_index != job.package_index || indexed.entry.hash != job.entry.hash
        });

        let notify_result = if stale {
            Err(AssetStreamingError::AssetStale {
                path: job.path.clone(),
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

fn notify_waiters(waiters: Vec<Completion>, result: Result<ErasedHandle>) {
    for waiter in waiters {
        waiter(result.clone());
    }
}

impl StreamingAssetBlobInfo {
    fn decode_from_io<T>(&self, count: usize, package: &mut StreamingAssetFile) -> Result<Vec<T>>
    where
        T: Immutable + FromBytes + KnownLayout + Clone,
    {
        let bytes = package.read_blob_bytes(self)?;
        let bytes_section = &bytes[0..count * size_of::<T>()];

        let elems = <[T]>::try_ref_from_bytes_with_elems(&bytes_section, count).map_err(|e| {
            AssetStreamingError::Decode {
                source: None,
                message: e.to_string(),
            }
        })?;
        Ok(elems.to_vec())
    }
    pub fn decode_exact_from_io<T>(
        &self,
        label: &str,
        expected_count: usize,
        package: &mut StreamingAssetFile,
    ) -> Result<Vec<T>>
    where
        T: Immutable + FromBytes + KnownLayout + Clone,
    {
        self.validate_count(expected_count, label)?;

        self.decode_from_io(expected_count, package)
    }

    pub fn decode_all_from_io<T>(&self, package: &mut StreamingAssetFile) -> Result<Vec<T>>
    where
        T: Immutable + FromBytes + KnownLayout + Clone,
    {
        self.decode_from_io(self.element_count, package)
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

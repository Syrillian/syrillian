use crate::HComputeShader;
use crate::store::streaming::asset_store::AssetType;
use crate::store::{
    AssetKey, AssetRefreshMessage, H, HandleName, Store, StoreDefaults, StoreType,
    StoreTypeFallback, UpdateAssetMessage,
};
use crate::{HBGL, store_add_checked};
use bon::Builder;
use crossbeam_channel::Sender;

const COMPUTE_MESH_SKINNING: &str = include_str!("shader/shaders/compute/mesh_skinning.wgsl");
const COMPUTE_POST_PROCESS_SSR: &str =
    include_str!("shader/shaders/compute/ssr_post_process_compute.wgsl");
const COMPUTE_POST_PROCESS_SSAO: &str =
    include_str!("shader/shaders/compute/ssao_post_process_compute.wgsl");
const COMPUTE_POST_PROCESS_SSAO_BLUR: &str =
    include_str!("shader/shaders/compute/ssao_blur_compute.wgsl");
const COMPUTE_POST_PROCESS_SSAO_APPLY: &str =
    include_str!("shader/shaders/compute/ssao_apply_compute.wgsl");
const COMPUTE_PARTICLE_POSITION: &str =
    include_str!("shader/shaders/compute/particle_position.wgsl");
const COMPUTE_POST_PROCESS_BLOOM_PREFILTER: &str =
    include_str!("shader/shaders/compute/bloom_prefilter_compute.wgsl");
const COMPUTE_POST_PROCESS_BLOOM_BLUR: &str =
    include_str!("shader/shaders/compute/bloom_blur_compute.wgsl");
const COMPUTE_POST_PROCESS_BLOOM_COMPOSITE: &str =
    include_str!("shader/shaders/compute/bloom_composite_compute.wgsl");

#[derive(Debug, Clone, Builder)]
pub struct ComputeShader {
    #[builder(into)]
    name: String,
    #[builder(into)]
    code: String,
    #[builder(default = "cs_main".to_string(), into)]
    entry_point: String,
    #[builder(default)]
    bind_group_layouts: Vec<HBGL>,
}

impl H<ComputeShader> {
    pub const FALLBACK_ID: u32 = 0;
    pub const MESH_SKINNING_ID: u32 = 1;
    pub const POST_PROCESS_SSR_ID: u32 = 2;
    pub const PARTICLE_POSITION_ID: u32 = 3;
    pub const POST_PROCESS_BLOOM_PREFILTER_ID: u32 = 4;
    pub const POST_PROCESS_BLOOM_BLUR_ID: u32 = 5;
    pub const POST_PROCESS_BLOOM_COMPOSITE_ID: u32 = 6;
    pub const POST_PROCESS_SSAO_ID: u32 = 7;
    pub const POST_PROCESS_SSAO_BLUR_X_ID: u32 = 8;
    pub const POST_PROCESS_SSAO_BLUR_Y_ID: u32 = 9;
    pub const POST_PROCESS_SSAO_APPLY_ID: u32 = 10;
    pub const MAX_BUILTIN_ID: u32 = 10;

    pub const FALLBACK: H<ComputeShader> = H::new(Self::FALLBACK_ID);
    pub const MESH_SKINNING: H<ComputeShader> = H::new(Self::MESH_SKINNING_ID);
    pub const POST_PROCESS_SSR: H<ComputeShader> = H::new(Self::POST_PROCESS_SSR_ID);
    pub const PARTICLE_POSITION: H<ComputeShader> = H::new(Self::PARTICLE_POSITION_ID);
    pub const POST_PROCESS_BLOOM_PREFILTER: H<ComputeShader> =
        H::new(Self::POST_PROCESS_BLOOM_PREFILTER_ID);
    pub const POST_PROCESS_BLOOM_BLUR: H<ComputeShader> = H::new(Self::POST_PROCESS_BLOOM_BLUR_ID);
    pub const POST_PROCESS_BLOOM_COMPOSITE: H<ComputeShader> =
        H::new(Self::POST_PROCESS_BLOOM_COMPOSITE_ID);
    pub const POST_PROCESS_SSAO: H<ComputeShader> = H::new(Self::POST_PROCESS_SSAO_ID);
    pub const POST_PROCESS_SSAO_BLUR_X: H<ComputeShader> =
        H::new(Self::POST_PROCESS_SSAO_BLUR_X_ID);
    pub const POST_PROCESS_SSAO_BLUR_Y: H<ComputeShader> =
        H::new(Self::POST_PROCESS_SSAO_BLUR_Y_ID);
    pub const POST_PROCESS_SSAO_APPLY: H<ComputeShader> = H::new(Self::POST_PROCESS_SSAO_APPLY_ID);
}

impl StoreDefaults for ComputeShader {
    fn populate(store: &mut Store<Self>) {
        store_add_checked!(
            store,
            HComputeShader::FALLBACK_ID,
            ComputeShader::new(
                "Compute Fallback",
                "@compute @workgroup_size(1,1,1) fn cs_main() {}",
                vec![]
            )
        );

        store_add_checked!(
            store,
            HComputeShader::MESH_SKINNING_ID,
            ComputeShader::new(
                "Mesh Skinning Compute",
                COMPUTE_MESH_SKINNING,
                vec![HBGL::MESH_SKINNING_COMPUTE]
            )
        );

        store_add_checked!(
            store,
            HComputeShader::POST_PROCESS_SSR_ID,
            ComputeShader::new(
                "SSR Post Process Compute",
                COMPUTE_POST_PROCESS_SSR,
                vec![HBGL::RENDER, HBGL::POST_PROCESS_COMPUTE]
            )
        );

        store_add_checked!(
            store,
            HComputeShader::PARTICLE_POSITION_ID,
            ComputeShader::new(
                "Particle Position Compute",
                COMPUTE_PARTICLE_POSITION,
                vec![HBGL::PARTICLE_COMPUTE]
            )
        );

        store_add_checked!(
            store,
            HComputeShader::POST_PROCESS_BLOOM_PREFILTER_ID,
            ComputeShader::new(
                "Bloom Prefilter Compute",
                COMPUTE_POST_PROCESS_BLOOM_PREFILTER,
                vec![HBGL::BLOOM_COMPUTE]
            )
        );

        store_add_checked!(
            store,
            HComputeShader::POST_PROCESS_BLOOM_BLUR_ID,
            ComputeShader::new(
                "Bloom Blur Compute",
                COMPUTE_POST_PROCESS_BLOOM_BLUR,
                vec![HBGL::BLOOM_COMPUTE]
            )
        );

        store_add_checked!(
            store,
            HComputeShader::POST_PROCESS_BLOOM_COMPOSITE_ID,
            ComputeShader::new(
                "Bloom Composite Compute",
                COMPUTE_POST_PROCESS_BLOOM_COMPOSITE,
                vec![HBGL::BLOOM_COMPUTE]
            )
        );

        store_add_checked!(
            store,
            HComputeShader::POST_PROCESS_SSAO_ID,
            ComputeShader::new(
                "SSAO Post Process Compute",
                COMPUTE_POST_PROCESS_SSAO,
                vec![HBGL::RENDER, HBGL::SSAO_COMPUTE]
            )
        );

        store_add_checked!(
            store,
            HComputeShader::POST_PROCESS_SSAO_BLUR_X_ID,
            ComputeShader::builder()
                .name("SSAO Blur X Compute")
                .code(COMPUTE_POST_PROCESS_SSAO_BLUR)
                .entry_point("cs_blur_x")
                .bind_group_layouts(vec![HBGL::SSAO_COMPUTE])
                .build()
        );

        store_add_checked!(
            store,
            HComputeShader::POST_PROCESS_SSAO_BLUR_Y_ID,
            ComputeShader::builder()
                .name("SSAO Blur Y Compute")
                .code(COMPUTE_POST_PROCESS_SSAO_BLUR)
                .entry_point("cs_blur_y")
                .bind_group_layouts(vec![HBGL::SSAO_COMPUTE])
                .build()
        );

        store_add_checked!(
            store,
            HComputeShader::POST_PROCESS_SSAO_APPLY_ID,
            ComputeShader::new(
                "SSAO Apply Compute",
                COMPUTE_POST_PROCESS_SSAO_APPLY,
                vec![HBGL::SSAO_APPLY_COMPUTE]
            )
        );
    }
}

impl StoreType for ComputeShader {
    const NAME: &str = "Compute Shader";
    const TYPE: AssetType = AssetType::ComputeShader;

    fn ident_fmt(handle: H<Self>) -> HandleName<Self> {
        match handle.id() {
            HComputeShader::FALLBACK_ID => HandleName::Static("Fallback Compute Shader"),
            HComputeShader::MESH_SKINNING_ID => HandleName::Static("Mesh Skinning Compute Shader"),
            HComputeShader::POST_PROCESS_SSR_ID => {
                HandleName::Static("SSR Post Process Compute Shader")
            }
            HComputeShader::PARTICLE_POSITION_ID => {
                HandleName::Static("Particle Position Compute Shader")
            }
            HComputeShader::POST_PROCESS_BLOOM_PREFILTER_ID => {
                HandleName::Static("Bloom Prefilter Compute Shader")
            }
            HComputeShader::POST_PROCESS_BLOOM_BLUR_ID => {
                HandleName::Static("Bloom Blur Compute Shader")
            }
            HComputeShader::POST_PROCESS_BLOOM_COMPOSITE_ID => {
                HandleName::Static("Bloom Composite Compute Shader")
            }
            HComputeShader::POST_PROCESS_SSAO_ID => {
                HandleName::Static("SSAO Post Process Compute Shader")
            }
            HComputeShader::POST_PROCESS_SSAO_BLUR_X_ID => {
                HandleName::Static("SSAO Blur X Compute Shader")
            }
            HComputeShader::POST_PROCESS_SSAO_BLUR_Y_ID => {
                HandleName::Static("SSAO Blur Y Compute Shader")
            }
            HComputeShader::POST_PROCESS_SSAO_APPLY_ID => {
                HandleName::Static("SSAO Apply Compute Shader")
            }
            _ => HandleName::Id(handle),
        }
    }

    fn refresh_dirty(&self, key: AssetKey, assets_tx: &Sender<AssetRefreshMessage>) -> bool {
        assets_tx
            .send(AssetRefreshMessage::updated(
                key,
                UpdateAssetMessage::UpdateComputeShader(self.clone()),
            ))
            .is_ok()
    }

    fn is_builtin(handle: H<Self>) -> bool {
        handle.id() <= HComputeShader::MAX_BUILTIN_ID
    }
}

impl StoreTypeFallback for ComputeShader {
    fn fallback() -> H<Self> {
        HComputeShader::FALLBACK
    }
}

impl ComputeShader {
    pub fn new(
        name: impl Into<String>,
        code: impl Into<String>,
        bind_group_layouts: Vec<HBGL>,
    ) -> Self {
        Self::builder()
            .name(name)
            .code(code)
            .bind_group_layouts(bind_group_layouts)
            .build()
    }

    pub fn new_with_entry_point(
        name: impl Into<String>,
        code: impl Into<String>,
        entry_point: impl Into<String>,
        bind_group_layouts: Vec<HBGL>,
    ) -> Self {
        Self::builder()
            .name(name)
            .code(code)
            .entry_point(entry_point)
            .bind_group_layouts(bind_group_layouts)
            .build()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn entry_point(&self) -> &str {
        &self.entry_point
    }

    pub fn bind_group_layouts(&self) -> &[HBGL] {
        &self.bind_group_layouts
    }
}

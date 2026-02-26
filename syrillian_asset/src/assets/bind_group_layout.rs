use crate::store::{H, HandleName, Store, StoreDefaults, StoreType, UpdateAssetMessage};
use crate::{HBGL, store_add_checked};
use crossbeam_channel::Sender;
use wgpu::{
    BindGroupLayoutEntry, BindingType, BufferBindingType, SamplerBindingType, ShaderStages,
    StorageTextureAccess, TextureFormat, TextureSampleType, TextureViewDimension,
};

#[derive(Clone, Debug)]
pub struct BGL {
    pub label: String,
    pub entries: Vec<BindGroupLayoutEntry>,
}

impl HBGL {
    pub const RENDER_ID: u32 = 0;
    pub const MODEL_ID: u32 = 1;
    pub const MODEL_SKINNING_ID: u32 = 2;
    pub const MATERIAL_ID: u32 = 3;
    pub const LIGHT_ID: u32 = 4;
    pub const SHADOW_ID: u32 = 5;
    pub const POST_PROCESS_ID: u32 = 6;
    pub const EMPTY_ID: u32 = 7;
    pub const POST_PROCESS_COMPUTE_ID: u32 = 8;
    pub const MESH_SKINNING_COMPUTE_ID: u32 = 9;
    pub const PARTICLE_COMPUTE_ID: u32 = 10;
    pub const PARTICLE_RENDER_ID: u32 = 11;
    pub const BLOOM_COMPUTE_ID: u32 = 12;
    pub const SSAO_COMPUTE_ID: u32 = 13;
    pub const SSAO_APPLY_COMPUTE_ID: u32 = 14;
    pub const FONT_ATLAS_ID: u32 = 15;

    const MAX_BUILTIN_ID: u32 = 15;

    pub const RENDER: HBGL = HBGL::new(Self::RENDER_ID);
    pub const MODEL: HBGL = HBGL::new(Self::MODEL_ID);
    pub const MODEL_SKINNING: HBGL = HBGL::new(Self::MODEL_SKINNING_ID);
    pub const MATERIAL: HBGL = HBGL::new(Self::MATERIAL_ID);
    pub const LIGHT: HBGL = HBGL::new(Self::LIGHT_ID);
    pub const SHADOW: HBGL = HBGL::new(Self::SHADOW_ID);
    pub const POST_PROCESS: HBGL = HBGL::new(Self::POST_PROCESS_ID);
    pub const EMPTY: HBGL = HBGL::new(Self::EMPTY_ID);
    pub const POST_PROCESS_COMPUTE: HBGL = HBGL::new(Self::POST_PROCESS_COMPUTE_ID);
    pub const MESH_SKINNING_COMPUTE: HBGL = HBGL::new(Self::MESH_SKINNING_COMPUTE_ID);
    pub const PARTICLE_COMPUTE: HBGL = HBGL::new(Self::PARTICLE_COMPUTE_ID);
    pub const PARTICLE_RENDER: HBGL = HBGL::new(Self::PARTICLE_RENDER_ID);
    pub const BLOOM_COMPUTE: HBGL = HBGL::new(Self::BLOOM_COMPUTE_ID);
    pub const SSAO_COMPUTE: HBGL = HBGL::new(Self::SSAO_COMPUTE_ID);
    pub const SSAO_APPLY_COMPUTE: HBGL = HBGL::new(Self::SSAO_APPLY_COMPUTE_ID);
    pub const FONT_ATLAS: HBGL = HBGL::new(Self::FONT_ATLAS_ID);
}

impl StoreType for BGL {
    const NAME: &str = "Bind Group Layout";

    fn ident_fmt(handle: H<Self>) -> HandleName<Self> {
        match handle.id() {
            HBGL::RENDER_ID => HandleName::Static("Render Bind Group Layout"),
            HBGL::MODEL_ID => HandleName::Static("Model Bind Group Layout"),
            HBGL::MODEL_SKINNING_ID => HandleName::Static("Model Skinning Bind Group Layout"),
            HBGL::MATERIAL_ID => HandleName::Static("Material Bind Group Layout"),
            HBGL::LIGHT_ID => HandleName::Static("Light Bind Group Layout"),
            HBGL::SHADOW_ID => HandleName::Static("Shadow Bind Group Layout"),
            HBGL::POST_PROCESS_ID => HandleName::Static("Post Process Bind Group Layout"),
            HBGL::POST_PROCESS_COMPUTE_ID => {
                HandleName::Static("Post Process Compute Bind Group Layout")
            }
            HBGL::MESH_SKINNING_COMPUTE_ID => {
                HandleName::Static("Mesh Skinning Compute Bind Group Layout")
            }
            HBGL::PARTICLE_COMPUTE_ID => HandleName::Static("Particle Compute Bind Group Layout"),
            HBGL::PARTICLE_RENDER_ID => HandleName::Static("Particle Render Bind Group Layout"),
            HBGL::BLOOM_COMPUTE_ID => HandleName::Static("Bloom Compute Bind Group Layout"),
            HBGL::SSAO_COMPUTE_ID => HandleName::Static("SSAO Compute Bind Group Layout"),
            HBGL::SSAO_APPLY_COMPUTE_ID => {
                HandleName::Static("SSAO Apply Compute Bind Group Layout")
            }
            HBGL::FONT_ATLAS_ID => HandleName::Static("Font Atlas Bind Group Layout"),
            _ => HandleName::Id(handle),
        }
    }

    fn refresh_dirty(
        &self,
        key: crate::store::AssetKey,
        assets_tx: &Sender<(crate::store::AssetKey, UpdateAssetMessage)>,
    ) -> bool {
        assets_tx
            .send((key, UpdateAssetMessage::UpdateBGL(self.clone())))
            .is_ok()
    }

    fn is_builtin(handle: H<Self>) -> bool {
        handle.id() <= H::<Self>::MAX_BUILTIN_ID
    }
}

const RENDER_ENTRIES: [BindGroupLayoutEntry; 5] = [
    BindGroupLayoutEntry {
        binding: 0,
        visibility: ShaderStages::all(),
        ty: BindingType::Buffer {
            ty: BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 1,
        visibility: ShaderStages::all(),
        ty: BindingType::Buffer {
            ty: BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 2,
        visibility: ShaderStages::all(),
        ty: BindingType::Texture {
            sample_type: TextureSampleType::Float { filterable: true },
            view_dimension: TextureViewDimension::Cube,
            multisampled: false,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 3,
        visibility: ShaderStages::all(),
        ty: BindingType::Sampler(SamplerBindingType::Filtering),
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 4,
        visibility: ShaderStages::all(),
        ty: BindingType::Buffer {
            ty: BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    },
];

const MODEL_ENTRIES: [BindGroupLayoutEntry; 1] = [BindGroupLayoutEntry {
    binding: 0,
    visibility: ShaderStages::all(),
    ty: BindingType::Buffer {
        ty: BufferBindingType::Uniform,
        has_dynamic_offset: false,
        min_binding_size: None,
    },
    count: None,
}];

const MODEL_SKINNING_ENTRIES: [BindGroupLayoutEntry; 2] = [
    BindGroupLayoutEntry {
        binding: 0,
        visibility: ShaderStages::all(),
        ty: BindingType::Buffer {
            ty: BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 1,
        visibility: ShaderStages::all(),
        ty: BindingType::Buffer {
            ty: BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    },
];

const MATERIAL_ENTRIES: [BindGroupLayoutEntry; 6] = [
    BindGroupLayoutEntry {
        binding: 0,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Texture {
            sample_type: TextureSampleType::Float { filterable: true },
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 1,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Sampler(SamplerBindingType::Filtering),
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 2,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Texture {
            sample_type: TextureSampleType::Float { filterable: true },
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 3,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Sampler(SamplerBindingType::Filtering),
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 4,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Texture {
            sample_type: TextureSampleType::Float { filterable: true },
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 5,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Sampler(SamplerBindingType::Filtering),
        count: None,
    },
];

const LIGHT_ENTRIES: [BindGroupLayoutEntry; 2] = [
    BindGroupLayoutEntry {
        binding: 0,
        visibility: ShaderStages::VERTEX_FRAGMENT,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 1,
        visibility: ShaderStages::VERTEX_FRAGMENT,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    },
];

const SHADOW_ENTRIES: [BindGroupLayoutEntry; 4] = [
    BindGroupLayoutEntry {
        binding: 0,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Texture {
            sample_type: TextureSampleType::Depth,
            view_dimension: TextureViewDimension::D2Array,
            multisampled: false,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 1,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Sampler(SamplerBindingType::Comparison),
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 2,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 3,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    },
];

const PP_ENTRIES: [BindGroupLayoutEntry; 5] = [
    BindGroupLayoutEntry {
        binding: 0,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Texture {
            sample_type: TextureSampleType::Float { filterable: true },
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 1,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Sampler(SamplerBindingType::Filtering),
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 2,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Texture {
            sample_type: TextureSampleType::Depth,
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 3,
        visibility: ShaderStages::VERTEX_FRAGMENT,
        ty: BindingType::Texture {
            sample_type: TextureSampleType::Float { filterable: false },
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 4,
        visibility: ShaderStages::VERTEX_FRAGMENT,
        ty: BindingType::Texture {
            sample_type: TextureSampleType::Float { filterable: false },
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    },
];

const PP_COMPUTE_ENTRIES: [BindGroupLayoutEntry; 6] = [
    BindGroupLayoutEntry {
        binding: 0,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Texture {
            sample_type: TextureSampleType::Float { filterable: true },
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 1,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Sampler(SamplerBindingType::Filtering),
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 2,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Texture {
            sample_type: TextureSampleType::Depth,
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 3,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Texture {
            sample_type: TextureSampleType::Float { filterable: false },
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 4,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Texture {
            sample_type: TextureSampleType::Float { filterable: false },
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 5,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::StorageTexture {
            access: StorageTextureAccess::WriteOnly,
            format: TextureFormat::Rgba8Unorm,
            view_dimension: TextureViewDimension::D2,
        },
        count: None,
    },
];

const MESH_SKINNING_COMPUTE_ENTRIES: [BindGroupLayoutEntry; 6] = [
    BindGroupLayoutEntry {
        binding: 0,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 1,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 2,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 3,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Storage { read_only: false },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 4,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Storage { read_only: false },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 5,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Storage { read_only: false },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    },
];

const PARTICLE_COMPUTE_ENTRIES: [BindGroupLayoutEntry; 4] = [
    BindGroupLayoutEntry {
        binding: 0,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 1,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 2,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Storage { read_only: false },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 3,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    },
];

const PARTICLE_RENDER_ENTRIES: [BindGroupLayoutEntry; 2] = [
    BindGroupLayoutEntry {
        binding: 0,
        visibility: ShaderStages::all(),
        ty: BindingType::Buffer {
            ty: BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 1,
        visibility: ShaderStages::all(),
        ty: BindingType::Buffer {
            ty: BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    },
];

const BLOOM_COMPUTE_ENTRIES: [BindGroupLayoutEntry; 5] = [
    BindGroupLayoutEntry {
        binding: 0,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Texture {
            sample_type: TextureSampleType::Float { filterable: true },
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 1,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Texture {
            sample_type: TextureSampleType::Float { filterable: true },
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 2,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Sampler(SamplerBindingType::Filtering),
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 3,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 4,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::StorageTexture {
            access: StorageTextureAccess::WriteOnly,
            format: TextureFormat::Rgba8Unorm,
            view_dimension: TextureViewDimension::D2,
        },
        count: None,
    },
];

const SSAO_COMPUTE_ENTRIES: [BindGroupLayoutEntry; 5] = [
    BindGroupLayoutEntry {
        binding: 0,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Texture {
            sample_type: TextureSampleType::Depth,
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 1,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Texture {
            sample_type: TextureSampleType::Float { filterable: false },
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 2,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Texture {
            sample_type: TextureSampleType::Float { filterable: false },
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 3,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Texture {
            sample_type: TextureSampleType::Float { filterable: false },
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 4,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::StorageTexture {
            access: StorageTextureAccess::WriteOnly,
            format: TextureFormat::R32Float,
            view_dimension: TextureViewDimension::D2,
        },
        count: None,
    },
];

const SSAO_APPLY_COMPUTE_ENTRIES: [BindGroupLayoutEntry; 3] = [
    BindGroupLayoutEntry {
        binding: 0,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Texture {
            sample_type: TextureSampleType::Float { filterable: true },
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 1,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Texture {
            sample_type: TextureSampleType::Float { filterable: false },
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 2,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::StorageTexture {
            access: StorageTextureAccess::WriteOnly,
            format: TextureFormat::Rgba8Unorm,
            view_dimension: TextureViewDimension::D2,
        },
        count: None,
    },
];

const FONT_ATLAS_ENTRIES: [BindGroupLayoutEntry; 2] = [
    BindGroupLayoutEntry {
        binding: 0,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Texture {
            sample_type: TextureSampleType::Float { filterable: true },
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 1,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Sampler(SamplerBindingType::Filtering),
        count: None,
    },
];

impl StoreDefaults for BGL {
    fn populate(store: &mut Store<Self>) {
        store_add_checked!(
            store,
            HBGL::RENDER_ID,
            BGL {
                label: HBGL::RENDER.ident(),
                entries: RENDER_ENTRIES.to_vec()
            }
        );

        store_add_checked!(
            store,
            HBGL::MODEL_ID,
            BGL {
                label: HBGL::MODEL.ident(),
                entries: MODEL_ENTRIES.to_vec()
            }
        );

        store_add_checked!(
            store,
            HBGL::MODEL_SKINNING_ID,
            BGL {
                label: HBGL::MODEL_SKINNING.ident(),
                entries: MODEL_SKINNING_ENTRIES.to_vec()
            }
        );

        store_add_checked!(
            store,
            HBGL::MATERIAL_ID,
            BGL {
                label: HBGL::MATERIAL.ident(),
                entries: MATERIAL_ENTRIES.to_vec()
            }
        );

        store_add_checked!(
            store,
            HBGL::LIGHT_ID,
            BGL {
                label: HBGL::LIGHT.ident(),
                entries: LIGHT_ENTRIES.to_vec()
            }
        );

        store_add_checked!(
            store,
            HBGL::SHADOW_ID,
            BGL {
                label: HBGL::SHADOW.ident(),
                entries: SHADOW_ENTRIES.to_vec()
            }
        );

        store_add_checked!(
            store,
            HBGL::POST_PROCESS_ID,
            BGL {
                label: HBGL::POST_PROCESS.ident(),
                entries: PP_ENTRIES.to_vec()
            }
        );

        store_add_checked!(
            store,
            HBGL::EMPTY_ID,
            BGL {
                label: "".to_string(),
                entries: [].to_vec()
            }
        );

        store_add_checked!(
            store,
            HBGL::POST_PROCESS_COMPUTE_ID,
            BGL {
                label: HBGL::POST_PROCESS_COMPUTE.ident(),
                entries: PP_COMPUTE_ENTRIES.to_vec()
            }
        );

        store_add_checked!(
            store,
            HBGL::MESH_SKINNING_COMPUTE_ID,
            BGL {
                label: HBGL::MESH_SKINNING_COMPUTE.ident(),
                entries: MESH_SKINNING_COMPUTE_ENTRIES.to_vec()
            }
        );

        store_add_checked!(
            store,
            HBGL::PARTICLE_COMPUTE_ID,
            BGL {
                label: HBGL::PARTICLE_COMPUTE.ident(),
                entries: PARTICLE_COMPUTE_ENTRIES.to_vec()
            }
        );

        store_add_checked!(
            store,
            HBGL::PARTICLE_RENDER_ID,
            BGL {
                label: HBGL::PARTICLE_RENDER.ident(),
                entries: PARTICLE_RENDER_ENTRIES.to_vec()
            }
        );

        store_add_checked!(
            store,
            HBGL::BLOOM_COMPUTE_ID,
            BGL {
                label: HBGL::BLOOM_COMPUTE.ident(),
                entries: BLOOM_COMPUTE_ENTRIES.to_vec()
            }
        );

        store_add_checked!(
            store,
            HBGL::SSAO_COMPUTE_ID,
            BGL {
                label: HBGL::SSAO_COMPUTE.ident(),
                entries: SSAO_COMPUTE_ENTRIES.to_vec()
            }
        );

        store_add_checked!(
            store,
            HBGL::SSAO_APPLY_COMPUTE_ID,
            BGL {
                label: HBGL::SSAO_APPLY_COMPUTE.ident(),
                entries: SSAO_APPLY_COMPUTE_ENTRIES.to_vec()
            }
        );

        store_add_checked!(
            store,
            HBGL::FONT_ATLAS_ID,
            BGL {
                label: HBGL::FONT_ATLAS.ident(),
                entries: FONT_ATLAS_ENTRIES.to_vec()
            }
        );
    }
}

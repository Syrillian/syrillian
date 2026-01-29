use super::HBGL;
use crate::engine::assets::{H, HandleName, Store, StoreDefaults, StoreType};
use crate::store_add_checked;
use wgpu::{
    BindGroupLayoutEntry, BindingType, BufferBindingType, SamplerBindingType, ShaderStages,
    TextureSampleType, TextureViewDimension,
};

#[derive(Clone, Debug)]
pub struct BGL {
    pub label: String,
    pub entries: Vec<BindGroupLayoutEntry>,
}

impl H<BGL> {
    pub(super) const RENDER_ID: u32 = 0;
    pub(super) const MODEL_ID: u32 = 1;
    pub(super) const MATERIAL_ID: u32 = 2;
    pub(super) const LIGHT_ID: u32 = 3;
    pub(super) const SHADOW_ID: u32 = 4;
    pub(super) const POST_PROCESS_ID: u32 = 5;
    pub(super) const EMPTY_ID: u32 = 6;

    const MAX_BUILTIN_ID: u32 = 6;

    pub const RENDER: HBGL = HBGL::new(Self::RENDER_ID);
    pub const MODEL: HBGL = HBGL::new(Self::MODEL_ID);
    pub const MATERIAL: HBGL = HBGL::new(Self::MATERIAL_ID);
    pub const LIGHT: HBGL = HBGL::new(Self::LIGHT_ID);
    pub const SHADOW: HBGL = HBGL::new(Self::SHADOW_ID);
    pub const POST_PROCESS: HBGL = HBGL::new(Self::POST_PROCESS_ID);
    pub const EMPTY: HBGL = HBGL::new(Self::EMPTY_ID);
}

impl StoreType for BGL {
    fn name() -> &'static str {
        "Bind Group Layout"
    }

    fn ident_fmt(handle: H<Self>) -> HandleName<Self> {
        match handle.id() {
            HBGL::RENDER_ID => HandleName::Static("Render Bind Group Layout"),
            HBGL::MODEL_ID => HandleName::Static("Model Bind Group Layout"),
            HBGL::MATERIAL_ID => HandleName::Static("Material Bind Group Layout"),
            HBGL::LIGHT_ID => HandleName::Static("Light Bind Group Layout"),
            HBGL::SHADOW_ID => HandleName::Static("Shadow Bind Group Layout"),
            HBGL::POST_PROCESS_ID => HandleName::Static("Post Process Bind Group Layout"),
            _ => HandleName::Id(handle),
        }
    }

    fn is_builtin(handle: H<Self>) -> bool {
        handle.id() <= H::<Self>::MAX_BUILTIN_ID
    }
}

const RENDER_ENTRIES: [BindGroupLayoutEntry; 2] = [
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
            ty: BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    },
];

const MATERIAL_ENTRIES: [BindGroupLayoutEntry; 7] = [
    BindGroupLayoutEntry {
        binding: 0,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 1,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Texture {
            sample_type: TextureSampleType::Float { filterable: true },
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 2,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Sampler(SamplerBindingType::Filtering),
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 3,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Texture {
            sample_type: TextureSampleType::Float { filterable: true },
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 4,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Sampler(SamplerBindingType::Filtering),
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 5,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Texture {
            sample_type: TextureSampleType::Float { filterable: true },
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 6,
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

const SHADOW_ENTRIES: [BindGroupLayoutEntry; 2] = [
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
                entries: RENDER_ENTRIES.to_vec()
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
    }
}

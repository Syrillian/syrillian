struct Material {
    diffuse: vec3<f32>,
    roughness: f32,
    metallic: f32,
    alpha: f32,
    use_diffuse_texture: u32,
    use_normal_texture: u32,
    use_roughness_texture: u32,
    lit: u32,
    cast_shadows: u32,
    grayscale_diffuse: u32,
};
var<immediate> material: Material;

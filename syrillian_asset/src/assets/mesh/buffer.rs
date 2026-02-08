use crate::mesh::Vertex3D;
use crate::mesh::vertex::SimpleVertex3D;

#[allow(dead_code)]
#[rustfmt::skip]
pub const TRIANGLE_VERT: [Vertex3D; 3] = [
    SimpleVertex3D {
        position: [0.0, 1.0, 0.0],
        normal:   [0.0, 0.0, -1.0],
        uv:       [0.5, 1.0],
    }.upgrade(),
    SimpleVertex3D {
        position: [0.5, 0.0, 0.0],
        normal:   [0.0, 0.0, -1.0],
        uv:       [1.0, 0.0],
    }.upgrade(),
    SimpleVertex3D {
        position: [-0.5, 0.0, 0.0],
        normal:   [0.0, 0.0, -1.0],
        uv:       [0.0, 0.0],
    }.upgrade(),
];

#[allow(dead_code)]
#[rustfmt::skip]
pub const CUBE_VERT: [Vertex3D; 24] = [  // 4 vertices per face × 6 faces = 24 vertices
    // Front face (z = -0.5)
    SimpleVertex3D { position: [-0.5,  0.5, -0.5], normal: [0.0, 0.0, -1.0], uv: [0.0, 1.0] }.upgrade(),
    SimpleVertex3D { position: [ 0.5,  0.5, -0.5], normal: [0.0, 0.0, -1.0], uv: [1.0, 1.0] }.upgrade(),
    SimpleVertex3D { position: [-0.5, -0.5, -0.5], normal: [0.0, 0.0, -1.0], uv: [0.0, 0.0] }.upgrade(),
    SimpleVertex3D { position: [ 0.5, -0.5, -0.5], normal: [0.0, 0.0, -1.0], uv: [1.0, 0.0] }.upgrade(),

    // Back face (z = 0.5)
    SimpleVertex3D { position: [-0.5,  0.5, 0.5], normal: [0.0, 0.0, 1.0], uv: [0.0, 1.0] }.upgrade(),
    SimpleVertex3D { position: [ 0.5,  0.5, 0.5], normal: [0.0, 0.0, 1.0], uv: [1.0, 1.0] }.upgrade(),
    SimpleVertex3D { position: [-0.5, -0.5, 0.5], normal: [0.0, 0.0, 1.0], uv: [0.0, 0.0] }.upgrade(),
    SimpleVertex3D { position: [ 0.5, -0.5, 0.5], normal: [0.0, 0.0, 1.0], uv: [1.0, 0.0] }.upgrade(),

    // Top face (y = 0.5)
    SimpleVertex3D { position: [-0.5,  0.5, -0.5], normal: [0.0, 1.0, 0.0], uv: [0.0, 1.0] }.upgrade(),
    SimpleVertex3D { position: [ 0.5,  0.5, -0.5], normal: [0.0, 1.0, 0.0], uv: [1.0, 1.0] }.upgrade(),
    SimpleVertex3D { position: [-0.5,  0.5,  0.5], normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0] }.upgrade(),
    SimpleVertex3D { position: [ 0.5,  0.5,  0.5], normal: [0.0, 1.0, 0.0], uv: [1.0, 0.0] }.upgrade(),

    // Bottom face (y = -0.5)
    SimpleVertex3D { position: [-0.5, -0.5, -0.5], normal: [0.0, -1.0, 0.0], uv: [0.0, 1.0] }.upgrade(),
    SimpleVertex3D { position: [ 0.5, -0.5, -0.5], normal: [0.0, -1.0, 0.0], uv: [1.0, 1.0] }.upgrade(),
    SimpleVertex3D { position: [-0.5, -0.5,  0.5], normal: [0.0, -1.0, 0.0], uv: [0.0, 0.0] }.upgrade(),
    SimpleVertex3D { position: [ 0.5, -0.5,  0.5], normal: [0.0, -1.0, 0.0], uv: [1.0, 0.0] }.upgrade(),

    // Right face (x = 0.5)
    SimpleVertex3D { position: [0.5,  0.5, -0.5], normal: [1.0, 0.0, 0.0], uv: [0.0, 1.0] }.upgrade(),
    SimpleVertex3D { position: [0.5,  0.5,  0.5], normal: [1.0, 0.0, 0.0], uv: [1.0, 1.0] }.upgrade(),
    SimpleVertex3D { position: [0.5, -0.5, -0.5], normal: [1.0, 0.0, 0.0], uv: [0.0, 0.0] }.upgrade(),
    SimpleVertex3D { position: [0.5, -0.5,  0.5], normal: [1.0, 0.0, 0.0], uv: [1.0, 0.0] }.upgrade(),

    // Left face (x = -0.5)
    SimpleVertex3D { position: [-0.5,  0.5, -0.5], normal: [-1.0, 0.0, 0.0], uv: [0.0, 1.0] }.upgrade(),
    SimpleVertex3D { position: [-0.5,  0.5,  0.5], normal: [-1.0, 0.0, 0.0], uv: [1.0, 1.0] }.upgrade(),
    SimpleVertex3D { position: [-0.5, -0.5, -0.5], normal: [-1.0, 0.0, 0.0], uv: [0.0, 0.0] }.upgrade(),
    SimpleVertex3D { position: [-0.5, -0.5,  0.5], normal: [-1.0, 0.0, 0.0], uv: [1.0, 0.0] }.upgrade(),
];

#[allow(dead_code)]
#[rustfmt::skip]
pub const CUBE_IDX: [u32; 6 * 6] = [
    // Front face
    0, 1, 2, 1, 3, 2,
    // Back face
    4, 6, 5, 5, 6, 7,
    // Top face
    8, 10, 9, 9, 10, 11,
    // Bottom face
    12, 13, 14, 13, 15, 14,
    // Right face
    16, 17, 18, 17, 19, 18,
    // Left face
    20, 22, 21, 21, 22, 23,
];

#[allow(dead_code)]
#[rustfmt::skip]
pub const UNIT_SQUARE_VERT: [Vertex3D; 6] = [
    SimpleVertex3D { position: [-1.0, -1.0, 0.0], normal: [0.0, 0.0, -1.0], uv: [0.0, 0.0] }.upgrade(),
    SimpleVertex3D { position: [1.0,  -1.0, 0.0], normal: [0.0, 0.0, -1.0], uv: [1.0, 0.0] }.upgrade(),
    SimpleVertex3D { position: [-1.0,  1.0, 0.0], normal: [0.0, 0.0, -1.0], uv: [0.0, 1.0] }.upgrade(),
    SimpleVertex3D { position: [1.0,  -1.0, 0.0], normal: [0.0, 0.0, -1.0], uv: [1.0, 0.0] }.upgrade(),
    SimpleVertex3D { position: [1.0,   1.0, 0.0], normal: [0.0, 0.0, -1.0], uv: [1.0, 1.0] }.upgrade(),
    SimpleVertex3D { position: [-1.0,  1.0, 0.0], normal: [0.0, 0.0, -1.0], uv: [0.0, 1.0] }.upgrade(),
];

#[macro_export]
macro_rules! ensure_aligned {
    ($obj:ty { $( $member:ident ),+ }, align <= $align:literal * $total:expr => size) => {
        $(
            ::static_assertions::const_assert_eq!(std::mem::offset_of!($obj, $member) % $align, 0);
        )*

        ::static_assertions::const_assert_eq!(size_of::<$obj>(), $align * $total);
        ::static_assertions::const_assert_eq!(size_of::<$obj>() % $align, 0);
    };
}

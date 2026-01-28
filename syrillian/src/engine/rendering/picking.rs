use crate::ViewportId;
use crate::core::ObjectHash;

#[derive(Debug, Clone)]
pub struct PickRequest {
    pub id: u64,
    pub target: ViewportId,
    pub position: (u32, u32),
}

#[derive(Debug, Clone, Copy)]
pub struct PickResult {
    pub id: u64,
    pub target: ViewportId,
    pub hash: Option<ObjectHash>,
}

pub fn hash_to_rgba_bytes(hash: ObjectHash) -> [u8; 4] {
    [
        (hash & 0xff) as u8,
        ((hash >> 8) & 0xff) as u8,
        ((hash >> 16) & 0xff) as u8,
        ((hash >> 24) & 0xff) as u8,
    ]
}

pub fn hash_to_rgba(hash: ObjectHash) -> [f32; 4] {
    let bytes = hash_to_rgba_bytes(hash);
    [
        bytes[0] as f32 / 255.0,
        bytes[1] as f32 / 255.0,
        bytes[2] as f32 / 255.0,
        bytes[3] as f32 / 255.0,
    ]
}

pub fn color_bytes_to_hash(bytes: [u8; 4]) -> Option<ObjectHash> {
    let hash = (bytes[0] as ObjectHash)
        | ((bytes[1] as ObjectHash) << 8)
        | ((bytes[2] as ObjectHash) << 16)
        | ((bytes[3] as ObjectHash) << 24);
    (hash != 0).then_some(hash)
}

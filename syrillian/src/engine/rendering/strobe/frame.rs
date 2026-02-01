use crate::strobe::StrobeRoot;

pub type CacheId = u64;

#[derive(Default)]
pub struct StrobeFrame {
    pub strobe_roots: Vec<StrobeRoot>,
}

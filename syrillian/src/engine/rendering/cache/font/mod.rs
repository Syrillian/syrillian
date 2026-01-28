use crate::assets::{Font, HMaterial, HTexture2D};
use crate::rendering::glyph::GlyphBitmap;
use crate::rendering::msdf_atlas::{FontLineMetrics, GlyphAtlasEntry, MsdfAtlas};
use crate::rendering::{AssetCache, CacheType};
use dashmap::DashSet;
use fdsm::bezier::scanline::FillRule;
use fdsm::generate::generate_msdf;
use fdsm::render::correct_sign_msdf;
use fdsm::shape::Shape;
use fdsm::transform::Transform;
use fdsm_ttf_parser::load_shape_from_face;
use image::RgbImage;
use nalgebra::Affine2;
use std::sync::{Arc, RwLock};
use ttf_parser::Face;
use wgpu::{Device, Queue};

#[cfg(not(target_arch = "wasm32"))]
use crossbeam_channel::{Receiver, Sender, TryRecvError, unbounded};
use fdsm::bezier::prepared::PreparedColoredShape;

pub mod glyph;
pub mod msdf_atlas;

pub struct FontAtlas {
    atlas: Arc<RwLock<MsdfAtlas>>,
    requested: DashSet<char>,

    #[cfg(not(target_arch = "wasm32"))]
    gen_tx: Sender<char>,
    #[cfg(not(target_arch = "wasm32"))]
    ready_rx: Receiver<GlyphBitmap>,

    #[cfg(target_arch = "wasm32")]
    pending: RwLock<std::collections::VecDeque<char>>,

    #[cfg(target_arch = "wasm32")]
    wasm_face_bytes: Arc<Vec<u8>>,
    #[cfg(target_arch = "wasm32")]
    wasm_units_per_em: f32,
    #[cfg(target_arch = "wasm32")]
    wasm_shrinkage: f64,
    #[cfg(target_arch = "wasm32")]
    wasm_range: f64,
}

impl CacheType for Font {
    type Hot = FontAtlas;

    fn upload(self, _device: &Device, _queue: &Queue, cache: &AssetCache) -> Self::Hot {
        let msdf = MsdfAtlas::new(
            self.font_bytes.clone(),
            self.atlas_em_px,
            16.0,
            4.0,
            cache.store(),
        );
        let atlas = Arc::new(RwLock::new(msdf));

        #[cfg(not(target_arch = "wasm32"))]
        let (gen_tx, ready_rx) = spawn_native_worker(&atlas);

        #[cfg(target_arch = "wasm32")]
        let (pending, wasm_face_bytes, wasm_units_per_em, wasm_shrinkage, wasm_range) =
            prepare_wasm_state(&atlas);

        FontAtlas {
            atlas,
            requested: DashSet::new(),

            #[cfg(not(target_arch = "wasm32"))]
            gen_tx,
            #[cfg(not(target_arch = "wasm32"))]
            ready_rx,

            #[cfg(target_arch = "wasm32")]
            pending,
            #[cfg(target_arch = "wasm32")]
            wasm_face_bytes,
            #[cfg(target_arch = "wasm32")]
            wasm_units_per_em,
            #[cfg(target_arch = "wasm32")]
            wasm_shrinkage,
            #[cfg(target_arch = "wasm32")]
            wasm_range,
        }
    }
}

impl FontAtlas {
    pub fn atlas(&self) -> HMaterial {
        self.atlas.read().unwrap().material()
    }
    pub fn texture(&self) -> HTexture2D {
        self.atlas.read().unwrap().texture()
    }
    pub fn metrics(&self) -> FontLineMetrics {
        self.atlas.read().unwrap().metrics()
    }

    pub fn face_data(&self) -> (Arc<Vec<u8>>, f32) {
        let atlas = self.atlas.read().unwrap();
        let (bytes, units_per_em, ..) = atlas.font_params();
        (bytes, units_per_em)
    }

    pub fn entry(&self, ch: char) -> Option<GlyphAtlasEntry> {
        self.atlas.read().unwrap().entry(ch)
    }

    pub fn request_glyphs(&self, chars: impl IntoIterator<Item = char>) {
        for ch in chars {
            self.enqueue_glyph_if_missing(ch);
        }
    }

    pub fn pump(&self, cache: &AssetCache, queue: &Queue, max_glyphs: usize) -> bool {
        if self.requested.is_empty() {
            return false;
        }

        let mut processed = 0;
        let mut updated = false;

        #[cfg(not(target_arch = "wasm32"))]
        while processed < max_glyphs {
            match self.ready_rx.try_recv() {
                Ok(bmp) => {
                    updated |= self.integrate_ready_bitmap(cache, queue, bmp);
                    processed += 1;
                }
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }

        #[cfg(target_arch = "wasm32")]
        while processed < max_glyphs {
            let Some(ch) = self.pending.write().unwrap().pop_front() else {
                break;
            };

            if let Some(bmp) = rasterize_msdf_glyph(
                &self.wasm_face_bytes,
                ch,
                self.wasm_shrinkage,
                self.wasm_range,
                self.wasm_units_per_em,
            ) {
                updated |= self.integrate_ready_bitmap(cache, queue, bmp);
            } else {
                self.requested.remove(&ch);
            }
            processed += 1;
        }

        updated
    }

    fn enqueue_glyph_if_missing(&self, ch: char) {
        if self.atlas.read().unwrap().contains(ch) {
            return;
        }

        if !self.requested.insert(ch) {
            return;
        }

        #[cfg(not(target_arch = "wasm32"))]
        let _ = self.gen_tx.send(ch);

        #[cfg(target_arch = "wasm32")]
        self.pending.write().unwrap().push_back(ch);
    }

    fn integrate_ready_bitmap(
        &self,
        cache: &AssetCache,
        queue: &Queue,
        bitmap: GlyphBitmap,
    ) -> bool {
        let ch = bitmap.ch;
        let integrated = self
            .atlas
            .write()
            .ok()
            .and_then(|mut atlas| atlas.integrate_ready_glyph(cache, queue, bitmap))
            .is_some();

        self.requested.remove(&ch);
        integrated
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_native_worker(atlas: &Arc<RwLock<MsdfAtlas>>) -> (Sender<char>, Receiver<GlyphBitmap>) {
    let (tx_req, rx_req) = unbounded();
    let (tx_ready, rx_ready) = unbounded();
    let (face_bytes, units_per_em, shrinkage, range) = atlas.read().unwrap().font_params();

    std::thread::spawn(move || {
        while let Ok(ch) = rx_req.recv() {
            if let Some(bmp) = rasterize_msdf_glyph(&face_bytes, ch, shrinkage, range, units_per_em)
                && tx_ready.send(bmp).is_err()
            {
                break;
            }
        }
    });

    (tx_req, rx_ready)
}

#[cfg(target_arch = "wasm32")]
fn prepare_wasm_state(
    atlas: &Arc<RwLock<MsdfAtlas>>,
) -> (
    RwLock<std::collections::VecDeque<char>>,
    Arc<Vec<u8>>,
    f32,
    f64,
    f64,
) {
    let (fb, upm, s, r) = atlas.read().unwrap().font_params();
    (RwLock::default(), fb, upm, s, r)
}

fn rasterize_msdf_glyph(
    face_bytes: &Arc<Vec<u8>>,
    ch: char,
    shrinkage: f64,
    range: f64,
    metrics_units_per_em: f32,
) -> Option<GlyphBitmap> {
    let face = Face::parse(face_bytes, 0).ok()?;
    let gid = face.glyph_index(ch)?;

    let bbox = glyph_bounds(&face, gid);
    let plane = plane_bounds(metrics_units_per_em, bbox, shrinkage, range);
    let (width_px, height_px) = glyph_dimensions(bbox, shrinkage, range);
    let transform = glyph_transform(bbox, shrinkage, range);

    let mut shape: Shape<_> = load_shape_from_face(&face, gid)?;
    shape.transform(&transform);
    let prepared = Shape::edge_coloring_simple(shape, 0.03, 0xD15EA5u64).prepare();

    let msdf = build_msdf_image(&prepared, width_px, height_px, range);
    let pixels_rgba = expand_to_rgba(&msdf);

    let adv_units = face.glyph_hor_advance(gid).unwrap_or(0) as f32;
    let advance_em = adv_units / metrics_units_per_em;

    Some(GlyphBitmap {
        ch,
        width_px,
        height_px,
        plane_min: plane.min,
        plane_max: plane.max,
        advance_em,
        msdf_range_px: range as f32,
        pixels_rgba,
    })
}

fn glyph_bounds(face: &Face, gid: ttf_parser::GlyphId) -> ttf_parser::Rect {
    face.glyph_bounding_box(gid).unwrap_or(ttf_parser::Rect {
        x_min: 0,
        y_min: 0,
        x_max: 1,
        y_max: 1,
    })
}

struct PlaneBounds {
    min: [f32; 2],
    max: [f32; 2],
}

fn plane_bounds(
    units_per_em: f32,
    bbox: ttf_parser::Rect,
    shrinkage: f64,
    range: f64,
) -> PlaneBounds {
    let upm = units_per_em as f64;
    let left_em = bbox.x_min as f64 / upm;
    let right_em = bbox.x_max as f64 / upm;
    let bottom_em = bbox.y_min as f64 / upm;
    let top_em = bbox.y_max as f64 / upm;
    let pad_em = (range * shrinkage) / upm;

    PlaneBounds {
        min: [(left_em - pad_em) as f32, (bottom_em - pad_em) as f32],
        max: [(right_em + pad_em) as f32, (top_em + pad_em) as f32],
    }
}

fn glyph_dimensions(bbox: ttf_parser::Rect, shrinkage: f64, range: f64) -> (u32, u32) {
    let width_px = (((bbox.x_max - bbox.x_min) as f64) / shrinkage + 2.0 * range)
        .ceil()
        .max(1.0) as u32;
    let height_px = (((bbox.y_max - bbox.y_min) as f64) / shrinkage + 2.0 * range)
        .ceil()
        .max(1.0) as u32;
    (width_px, height_px)
}

fn glyph_transform(bbox: ttf_parser::Rect, shrinkage: f64, range: f64) -> Affine2<f64> {
    let s = 1.0 / shrinkage;
    let tx = range - (bbox.x_min as f64) * s;
    let ty = range + (bbox.y_max as f64) * s;

    Affine2::from_matrix_unchecked(nalgebra::Matrix3::new(
        s, 0.0, tx, 0.0, -s, ty, 0.0, 0.0, 1.0,
    ))
}

fn build_msdf_image(
    prepared: &PreparedColoredShape,
    width_px: u32,
    height_px: u32,
    range: f64,
) -> RgbImage {
    let mut msdf = RgbImage::new(width_px, height_px);
    generate_msdf(prepared, range, &mut msdf);
    correct_sign_msdf(&mut msdf, prepared, FillRule::Nonzero);
    msdf
}

fn expand_to_rgba(msdf: &RgbImage) -> Vec<u8> {
    let mut pixels_rgba = vec![0u8; (msdf.width() as usize) * (msdf.height() as usize) * 4];
    let src = msdf.as_raw();

    for (dst, chunk) in pixels_rgba.chunks_exact_mut(4).zip(src.chunks_exact(3)) {
        dst[..3].copy_from_slice(chunk);
        dst[3] = 255;
    }

    pixels_rgba
}

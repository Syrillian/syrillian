use crate::math::Vec2;
use crate::rendering::FontAtlas;
use crate::rendering::msdf_atlas::{FontLineMetrics, GlyphAtlasEntry};
use crate::rendering::strobe::TextAlignment;
use static_assertions::{const_assert, const_assert_eq};
use ttf_parser::Face;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlyphVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
}

pub const GLYPH_TRIANGLE_COUNT: usize = 2;
pub const GLYPH_VERTEX_COUNT: usize = GLYPH_TRIANGLE_COUNT * 3;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlyphRenderData {
    vertices: [GlyphVertex; GLYPH_VERTEX_COUNT],
}

impl GlyphRenderData {
    pub const TRIANGLE_COUNT: usize = GLYPH_TRIANGLE_COUNT;
    pub const VERTEX_COUNT: usize = GLYPH_VERTEX_COUNT;
}

const_assert_eq!(size_of::<GlyphVertex>(), size_of::<[f32; 4]>());
const_assert_eq!(align_of::<GlyphVertex>(), align_of::<f32>());
const_assert_eq!(
    size_of::<GlyphRenderData>(),
    size_of::<GlyphVertex>() * GlyphRenderData::VERTEX_COUNT
);
const_assert!(size_of::<GlyphRenderData>().is_multiple_of(16));

#[derive(Clone)]
pub struct GlyphBitmap {
    pub ch: char,
    pub width_px: u32,
    pub height_px: u32,
    pub plane_min: [f32; 2],
    pub plane_max: [f32; 2],
    pub advance_em: f32,
    pub msdf_range_px: f32,
    pub pixels_rgba: Vec<u8>,
}

impl GlyphRenderData {
    fn from_entry(origin_em: Vec2, entry: &GlyphAtlasEntry) -> Self {
        let l = origin_em.x + entry.plane_min[0];
        let r = origin_em.x + entry.plane_max[0];
        let b = origin_em.y + entry.plane_min[1];
        let t = origin_em.y + entry.plane_max[1];

        let uv_min = entry.uv_min;
        let uv_max = entry.uv_max;

        let v_tl = GlyphVertex {
            pos: [l, t],
            uv: [uv_min[0], uv_min[1]],
        };
        let v_tr = GlyphVertex {
            pos: [r, t],
            uv: [uv_max[0], uv_min[1]],
        };
        let v_bl = GlyphVertex {
            pos: [l, b],
            uv: [uv_min[0], uv_max[1]],
        };
        let v_br = GlyphVertex {
            pos: [r, b],
            uv: [uv_max[0], uv_max[1]],
        };

        Self {
            vertices: [v_tl, v_bl, v_tr, v_tr, v_bl, v_br],
        }
    }

    pub fn vertices(&self) -> &[GlyphVertex] {
        &self.vertices
    }

    pub fn vertices_mut(&mut self) -> &mut [GlyphVertex] {
        &mut self.vertices
    }
}

fn align_lines(glyphs: &mut [GlyphRenderData], alignment: TextAlignment, rows: &[(usize, f32)]) {
    let shift = |w: f32| match alignment {
        TextAlignment::Left => 0.0,
        TextAlignment::Center => -0.5 * w,
        TextAlignment::Right => -w,
    };
    let mut it = glyphs.iter_mut();
    for &(count, width_em) in rows {
        let dx = shift(width_em);
        for _ in 0..count {
            if let Some(g) = it.next() {
                for v in g.vertices_mut() {
                    v.pos[0] += dx;
                }
            }
        }
    }
}

fn normalize_top_left_origin(glyphs: &mut [GlyphRenderData]) {
    let mut min_x = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for glyph in glyphs.iter() {
        for v in glyph.vertices() {
            min_x = min_x.min(v.pos[0]);
            max_y = max_y.max(v.pos[1]);
        }
    }

    if !min_x.is_finite() || !max_y.is_finite() {
        return;
    }

    for glyph in glyphs.iter_mut() {
        for v in glyph.vertices_mut() {
            v.pos[0] -= min_x;
            v.pos[1] -= max_y;
        }
    }
}

pub fn generate_glyph_geometry_stream(
    text: &str,
    atlas: &FontAtlas,
    alignment: TextAlignment,
    line_height_mul: f32,
    letter_spacing_em: f32,
) -> Vec<GlyphRenderData> {
    if text.is_empty() {
        return vec![];
    }

    let metrics = atlas.metrics();
    let baseline_dy = baseline_step(metrics, line_height_mul);
    let (face_bytes, units_per_em) = atlas.face_data();
    let face = Face::parse(&face_bytes, 0).ok();
    let (mut quads, row_data) = layout_text_lines(
        text,
        atlas,
        baseline_dy,
        face.as_ref(),
        units_per_em,
        letter_spacing_em,
    );
    normalize_top_left_origin(&mut quads);
    align_lines(&mut quads, alignment, &row_data);

    quads
}

fn baseline_step(metrics: FontLineMetrics, line_height_mul: f32) -> f32 {
    (metrics.ascent_em + metrics.descent_em + metrics.line_gap_em) * line_height_mul
}

fn layout_text_lines(
    text: &str,
    atlas: &FontAtlas,
    baseline_dy: f32,
    face: Option<&Face<'_>>,
    units_per_em: f32,
    letter_spacing_em: f32,
) -> (Vec<GlyphRenderData>, Vec<(usize, f32)>) {
    let mut quads = Vec::new();
    let mut row_data = Vec::<(usize, f32)>::new();
    let mut cursor = Vec2::new(0.0f32, 0.0f32);
    let mut row_glyphs = 0usize;
    let mut row_width_em = 0.0f32;
    let mut prev_char: Option<char> = None;

    for ch in text.chars() {
        if ch == '\n' {
            push_row(&mut row_data, row_glyphs, row_width_em);
            begin_new_line(&mut cursor, baseline_dy);
            row_glyphs = 0;
            row_width_em = 0.0;
            prev_char = None;
            continue;
        }

        if prev_char.is_some() {
            cursor.x += letter_spacing_em.max(0.0);
        }

        if let (Some(prev), Some(face_ref)) = (prev_char, face) {
            let kern = kerning_adjustment(face_ref, prev, ch, units_per_em);
            if kern.abs() > f32::EPSILON {
                cursor.x += kern;
            }
        }

        if let Some(entry) = glyph_entry(atlas, ch) {
            quads.push(GlyphRenderData::from_entry(cursor, &entry));
            cursor.x += entry.advance_em;
            row_width_em = row_width_em.max(cursor.x);
            row_glyphs += 1;
        } else if let Some(face_ref) = face
            && let Some(advance) = advance_from_face(face_ref, ch, units_per_em)
        {
            cursor.x += advance;
            row_width_em = row_width_em.max(cursor.x);
        }

        prev_char = Some(ch);
    }

    push_row(&mut row_data, row_glyphs, row_width_em);
    (quads, row_data)
}

fn push_row(rows: &mut Vec<(usize, f32)>, glyphs: usize, width_em: f32) {
    rows.push((glyphs, width_em));
}

fn begin_new_line(cursor: &mut Vec2, baseline_dy: f32) {
    cursor.x = 0.0;
    cursor.y -= baseline_dy;
}

fn glyph_entry(atlas: &FontAtlas, ch: char) -> Option<GlyphAtlasEntry> {
    atlas.entry(ch).or_else(|| atlas.entry(' '))
}

fn kerning_adjustment(face: &Face<'_>, left: char, right: char, units_per_em: f32) -> f32 {
    let Some(kern_table) = face.tables().kern else {
        return 0.0;
    };

    let Some(left_id) = face.glyph_index(left) else {
        return 0.0;
    };
    let Some(right_id) = face.glyph_index(right) else {
        return 0.0;
    };

    let mut adjustment_units = 0i32;
    for subtable in kern_table.subtables.into_iter() {
        if !subtable.horizontal || subtable.has_cross_stream || subtable.has_state_machine {
            continue;
        }
        if let Some(value) = subtable.glyphs_kerning(left_id, right_id) {
            adjustment_units += i32::from(value);
        }
    }

    adjustment_units as f32 / units_per_em
}

fn advance_from_face(face: &Face<'_>, ch: char, units_per_em: f32) -> Option<f32> {
    glyph_advance(face, ch, units_per_em).or_else(|| glyph_advance(face, ' ', units_per_em))
}

fn glyph_advance(face: &Face<'_>, ch: char, units_per_em: f32) -> Option<f32> {
    let glyph = face.glyph_index(ch)?;
    let advance = face.glyph_hor_advance(glyph)? as f32;
    Some(advance / units_per_em)
}

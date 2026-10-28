//! Bounded transport of recovered Pencil3 V3 paint into vector-native masks.
//!
//! The two PNGs are the original single-channel texture uploads transported as
//! alpha. The brush is stored once as a vertically reordered 16-row atlas and
//! every dab selects a row through a clipped SVG `<use>` viewport.
//! Paper repeats/clamps those native texels at their intrinsic density in one
//! bounded image, avoiding pattern resampling and internal image-edge seams.
//! Brush geometry and replay remain vector primitives.

use super::vector::{
    ColorValue, Definitions, Group, Image, Paint, Rectangle, ReplayPart, Scene, StrokeIndex,
    Styled, Svg, SvgMask, Transform, Use, ViewBox,
};
use crate::{BoundingBox, ink::pencil_source::PreparedPencil3V3};
use std::io::Cursor;

const PAPER_PNG: &[u8] = include_bytes!("pencil3_assets/paper.png");
const BRUSH_PNG: &[u8] = include_bytes!("pencil3_assets/brush-atlas.png");
const BRUSH_IMAGE_ID: &str = "sdocx-pencil3-brush-atlas";
const MAX_RECORDS: usize = 2_048;
const MAX_PAPER_RGBA_BYTES: usize = 64 * 1024 * 1024;
const TEXTURE_SIZE: f64 = 150.0;
const BRUSH_ROW_SIZE: f64 = 120.0;
const BRUSH_ROWS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Pencil3PaperSampler {
    Repeat,
    ClampToEdge,
}

/// Explicit recovered projection inputs. `background_scale` controls the
/// document-space period of the 150 texel paper image.
#[derive(Clone, Copy, Debug)]
pub(super) struct Pencil3PaintContext {
    pub canvas_width: u32,
    pub canvas_height: u32,
    pub background_scale: f32,
    pub paper_sampler: Pencil3PaperSampler,
    /// `true` is the canonical native +Y-up fixture projection. The public
    /// adapter can select the independently captured negative-Y profile.
    pub y_up: bool,
}

/// Add one prepared stroke to a vector scene. Invalid dimensions, records, or
/// work budgets return `false` before emitting any nodes.
#[cfg(test)]
pub(super) fn render(
    scene: &mut Scene,
    source: &PreparedPencil3V3,
    context: Pencil3PaintContext,
    replay_index: Option<usize>,
) -> bool {
    render_slice(scene, source, context, replay_index, None)
}

/// Render only particles intersecting an optional page-space vertical range.
/// Replay parts retain their original particle indices when particles are culled.
pub(super) fn render_slice(
    scene: &mut Scene,
    source: &PreparedPencil3V3,
    context: Pencil3PaintContext,
    replay_index: Option<usize>,
    page_y_range: Option<(f64, f64)>,
) -> bool {
    let Some(plane) = preflight(source, context) else {
        return false;
    };
    let records = &source.records;
    let visible = |record: &[f32; 5]| {
        page_y_range.is_none_or(|(top, bottom)| {
            // Invalid ranges cannot safely cull paint.
            if !top.is_finite() || !bottom.is_finite() || top > bottom {
                return true;
            }
            let y = if context.y_up {
                f64::from(context.canvas_height) - f64::from(record[1])
            } else {
                f64::from(record[1])
            };
            let half = f64::from(record[2]);
            y + half >= top && y - half <= bottom
        })
    };
    if !records.iter().any(visible) {
        return true;
    }
    let PaperPlane {
        width,
        height,
        padding,
        plane_width,
        plane_height,
        plane_bytes,
        pad_world,
        plane_width_world,
        plane_height_world,
    } = plane;
    let sampler_key = match context.paper_sampler {
        Pencil3PaperSampler::Repeat => "repeat",
        Pencil3PaperSampler::ClampToEdge => "clamp",
    };
    let paper_asset_key = format!(
        "pencil3-paper-plane-{sampler_key}-{}x{}-scale{:08x}",
        context.canvas_width,
        context.canvas_height,
        context.background_scale.to_bits(),
    );
    let paper_image_id = format!("sdocx-{paper_asset_key}-image");
    let paper_png = if scene.has_shared_definition_key(&paper_asset_key) {
        None
    } else {
        match make_paper_plane(
            context.paper_sampler,
            padding,
            plane_width,
            plane_height,
            plane_bytes,
        ) {
            Some(bytes) => Some(bytes),
            None => return false,
        }
    };
    let instance_id = scene.definition::<super::vector::Mask>().id_text();
    let paper_mask_id = format!("sdocx-pencil3-paper-mask-{instance_id}");

    let mut definitions = Definitions::new();
    if scene.claim_shared_definition("pencil3-brush-atlas") {
        definitions = definitions.add(
            Image::embedded(BRUSH_PNG, "image/png")
                .id(BRUSH_IMAGE_ID)
                .x(0.0)
                .y(0.0)
                .width(BRUSH_ROW_SIZE)
                .height(BRUSH_ROW_SIZE * BRUSH_ROWS as f64)
                .stretched()
                .pixelated(),
        );
    }
    if let Some(bytes) = paper_png
        && scene.claim_shared_definition_key(paper_asset_key)
    {
        definitions = definitions.add(
            Image::embedded(&bytes, "image/png")
                .id(&paper_image_id)
                .x(-pad_world)
                .y(-pad_world)
                .width(plane_width_world)
                .height(plane_height_world)
                .stretched()
                .pixelated(),
        );
    }

    let mut paper_image = Group::new().add(Use::href(&paper_image_id));
    if !context.y_up {
        paper_image =
            paper_image.transformed(Transform::matrix([1.0, 0.0, 0.0, -1.0, 0.0, height], 0, 0));
    }
    definitions =
        definitions.add(SvgMask::alpha(&paper_mask_id, 0.0, 0.0, width, height).add(paper_image));

    for (index, record) in records
        .iter()
        .enumerate()
        .filter(|(_, record)| visible(record))
    {
        let [x, y, half, row, _] = *record;
        let side = f64::from(half) * 2.0;
        let x = f64::from(x);
        let y = f64::from(y);
        let x0 = x - f64::from(half);
        let y0 = y - f64::from(half);
        let row = row as usize;
        let mask_id = format!("sdocx-pencil3-brush-{instance_id}-{index}");
        let atlas_y = atlas_y(row) as f64;
        let clipped_atlas = Svg::new()
            .x(x0)
            .y(y0)
            .width(side)
            .height(side)
            .view_box(ViewBox::unrounded(
                0.0,
                atlas_y,
                BRUSH_ROW_SIZE,
                BRUSH_ROW_SIZE,
            ))
            .preserve_aspect_none()
            .clipped_viewport()
            .add(Use::href(BRUSH_IMAGE_ID));
        definitions =
            definitions.add(SvgMask::alpha(&mask_id, x0, y0, side, side).add(clipped_atlas));
    }
    scene.push(definitions);

    let mut geometry = Group::new();
    if let Some(index) = replay_index {
        geometry = geometry.replay_stroke(StrokeIndex(index));
    }
    if context.y_up {
        geometry =
            geometry.transformed(Transform::matrix([1.0, 0.0, 0.0, -1.0, 0.0, height], 0, 0));
    }
    scene.scope(geometry, |scene| {
        for (index, record) in records
            .iter()
            .enumerate()
            .filter(|(_, record)| visible(record))
        {
            let [x, y, half, _, alpha] = *record;
            let half = f64::from(half);
            let amount = f64::from(alpha) * f64::from(source.opacity) * 1.5;
            scene.scope(Group::new().mask_id(&paper_mask_id), |scene| {
                scene.push(
                    Rectangle::new()
                        .replay_part(replay_index.map(|_| ReplayPart(index + 1)))
                        .x(f64::from(x) - half)
                        .y(f64::from(y) - half)
                        .width(half * 2.0)
                        .height(half * 2.0)
                        .fill(Paint::Solid(ColorValue::Rgb(source.color)))
                        .fill_opacity(amount)
                        .mask_id(&format!("sdocx-pencil3-brush-{instance_id}-{index}")),
                );
            });
        }
    });
    true
}

struct PaperPlane {
    width: f64,
    height: f64,
    padding: usize,
    plane_width: usize,
    plane_height: usize,
    plane_bytes: usize,
    pad_world: f64,
    plane_width_world: f64,
    plane_height_world: f64,
}

/// Check paint constraints without allocating the paper image or changing the scene.
pub(super) fn eligible(source: &PreparedPencil3V3, context: Pencil3PaintContext) -> bool {
    preflight(source, context).is_some()
}

fn preflight(source: &PreparedPencil3V3, context: Pencil3PaintContext) -> Option<PaperPlane> {
    let records = &source.records;
    if records.is_empty()
        || records.len() > MAX_RECORDS
        || context.canvas_width == 0
        || context.canvas_height == 0
        || context.canvas_width > i32::MAX as u32
        || context.canvas_height > i32::MAX as u32
        || !context.background_scale.is_finite()
        || context.background_scale <= 0.0
        || !valid_bounds(source.bounds)
        || !source.opacity.is_finite()
        || !(0.0..=1.0).contains(&source.opacity)
        || records
            .iter()
            .any(|record| f64::from(record[4]) * f64::from(source.opacity) * 1.5 > 1.0)
    {
        return None;
    }
    if records.iter().any(|record| !valid_record(record)) {
        return None;
    }

    let width = f64::from(context.canvas_width);
    let height = f64::from(context.canvas_height);
    let background_scale = f64::from(context.background_scale);
    let padding = paper_padding(background_scale)?;
    let (plane_width, plane_height, plane_bytes) =
        paper_plane_dimensions(width, height, background_scale, padding)?;
    let pad_world = padding as f64 * background_scale;
    let plane_width_world = plane_width as f64 * background_scale;
    let plane_height_world = plane_height as f64 * background_scale;
    // usvg geometry and transforms are binary32 even though SVG attributes
    // are serialized as f64. Reject an unrepresentable material before any
    // scene definitions or shared-image bookkeeping change.
    if [pad_world, plane_width_world, plane_height_world]
        .iter()
        .any(|value| !value.is_finite() || *value > f64::from(f32::MAX))
    {
        return None;
    }
    Some(PaperPlane {
        width,
        height,
        padding,
        plane_width,
        plane_height,
        plane_bytes,
        pad_world,
        plane_width_world,
        plane_height_world,
    })
}

fn valid_record(record: &[f32; 5]) -> bool {
    record.iter().all(|value| value.is_finite())
        && record[2] > 0.0
        && (0.0..BRUSH_ROWS as f32).contains(&record[3])
        && record[3].fract() == 0.0
        && (0.0..=1.0).contains(&record[4])
}

fn valid_bounds(bounds: BoundingBox) -> bool {
    bounds.x_min.is_finite()
        && bounds.y_min.is_finite()
        && bounds.x_max.is_finite()
        && bounds.y_max.is_finite()
        && bounds.x_min <= bounds.x_max
        && bounds.y_min <= bounds.y_max
}

fn paper_padding(background_scale: f64) -> Option<usize> {
    if !background_scale.is_finite() || background_scale <= 0.0 {
        return None;
    }
    let padding = (1.0 / background_scale).ceil() + 1.0;
    if !padding.is_finite() || padding > usize::MAX as f64 {
        return None;
    }
    Some(padding as usize)
}

fn paper_plane_dimensions(
    canvas_width: f64,
    canvas_height: f64,
    background_scale: f64,
    padding: usize,
) -> Option<(usize, usize, usize)> {
    if !canvas_width.is_finite()
        || !canvas_height.is_finite()
        || canvas_width <= 0.0
        || canvas_height <= 0.0
        || !background_scale.is_finite()
        || background_scale <= 0.0
    {
        return None;
    }
    let core_width = usize::try_from((canvas_width / background_scale).ceil() as u64).ok()?;
    let core_height = usize::try_from((canvas_height / background_scale).ceil() as u64).ok()?;
    let pad = padding.checked_mul(2)?;
    let plane_width = core_width.checked_add(pad)?;
    let plane_height = core_height.checked_add(pad)?;
    let bytes = plane_width.checked_mul(plane_height)?.checked_mul(4)?;
    if plane_width == 0
        || plane_height == 0
        || plane_width > u32::MAX as usize
        || plane_height > u32::MAX as usize
        || bytes > MAX_PAPER_RGBA_BYTES
    {
        return None;
    }
    Some((plane_width, plane_height, bytes))
}

fn paper_source_index(
    plane_coordinate: usize,
    padding: usize,
    sampler: Pencil3PaperSampler,
) -> usize {
    if plane_coordinate < padding {
        return match sampler {
            Pencil3PaperSampler::Repeat => {
                let before = (padding - plane_coordinate) % TEXTURE_SIZE as usize;
                (TEXTURE_SIZE as usize - before) % TEXTURE_SIZE as usize
            }
            Pencil3PaperSampler::ClampToEdge => 0,
        };
    }
    let coordinate = plane_coordinate - padding;
    match sampler {
        Pencil3PaperSampler::Repeat => coordinate % TEXTURE_SIZE as usize,
        Pencil3PaperSampler::ClampToEdge => coordinate.min(TEXTURE_SIZE as usize - 1),
    }
}

fn make_paper_plane(
    sampler: Pencil3PaperSampler,
    padding: usize,
    plane_width: usize,
    plane_height: usize,
    expected_bytes: usize,
) -> Option<Vec<u8>> {
    let mut decoder = png::Decoder::new(Cursor::new(PAPER_PNG));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let input_len = reader.output_buffer_size()?;
    if input_len != TEXTURE_SIZE as usize * TEXTURE_SIZE as usize * 4 {
        return None;
    }
    let mut source = vec![0; input_len];
    let frame = reader.next_frame(&mut source).ok()?;
    if frame.width != TEXTURE_SIZE as u32
        || frame.height != TEXTURE_SIZE as u32
        || frame.color_type != png::ColorType::Rgba
        || frame.bit_depth != png::BitDepth::Eight
        || frame.buffer_size() != input_len
    {
        return None;
    }
    reader.finish().ok()?;

    let mut pixels = vec![0; expected_bytes];
    for y in 0..plane_height {
        let source_y = paper_source_index(y, padding, sampler);
        for x in 0..plane_width {
            let source_x = paper_source_index(x, padding, sampler);
            let source_offset = (source_y * TEXTURE_SIZE as usize + source_x) * 4;
            let target_offset = (y * plane_width + x) * 4;
            pixels[target_offset..target_offset + 4]
                .copy_from_slice(&source[source_offset..source_offset + 4]);
        }
    }

    let mut encoded = Vec::new();
    {
        let mut encoder = png::Encoder::new(
            &mut encoded,
            u32::try_from(plane_width).ok()?,
            u32::try_from(plane_height).ok()?,
        );
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(&pixels).ok()?;
        writer.finish().ok()?;
    }
    Some(encoded)
}

fn atlas_y(native_row: usize) -> usize {
    (BRUSH_ROWS - 1 - native_row) * BRUSH_ROW_SIZE as usize
}

#[cfg(test)]
mod tests {
    use super::{BRUSH_PNG, PAPER_PNG};
    use png::{ColorType, Decoder, Transformations};
    use sha2::{Digest, Sha256};
    use std::io::Cursor;

    fn rgba(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
        let mut decoder = Decoder::new(Cursor::new(bytes));
        decoder.set_transformations(Transformations::EXPAND | Transformations::STRIP_16);
        let mut reader = decoder.read_info().unwrap();
        let mut output = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut output).unwrap();
        assert_eq!(info.color_type, ColorType::Rgba);
        output.truncate(info.buffer_size());
        (info.width, info.height, output)
    }

    fn alpha_digest(rgba: &[u8]) -> String {
        let alpha: Vec<u8> = rgba
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| pixel[3])
            .collect();
        format!("{:x}", Sha256::digest(alpha))
    }

    #[test]
    fn intrinsic_png_alpha_matches_recovered_source_pins() {
        let (paper_w, paper_h, paper) = rgba(PAPER_PNG);
        assert_eq!((paper_w, paper_h), (150, 150));
        assert_eq!(
            alpha_digest(&paper),
            "171b3fc88b6e693a06676bcf0e8029cdc122d50bbeb9f03fe969b153104d3994"
        );
        let (brush_w, brush_h, brush) = rgba(BRUSH_PNG);
        assert_eq!((brush_w, brush_h), (120, 1920));
        assert_eq!(
            alpha_digest(&brush),
            "3ba7ad07529987c81ca5552cf91e256548d1cb7de90720fd96aad4948776674b"
        );
    }
}

#[cfg(test)]
mod paper_sampling_tests {
    use super::*;

    fn transparent_source() -> PreparedPencil3V3 {
        PreparedPencil3V3 {
            records: vec![[1.0, 1.0, 0.5, 0.0, 0.0]],
            sample_ends: vec![1],
            bounds: BoundingBox {
                x_min: 0.5,
                y_min: 0.5,
                x_max: 1.5,
                y_max: 1.5,
            },
            color: crate::Color {
                r: 255,
                g: 255,
                b: 255,
            },
            opacity: 0.0,
        }
    }

    fn paper_pixels(context: Pencil3PaintContext) -> Vec<u8> {
        let width = context.canvas_width;
        let height = context.canvas_height;
        let mut scene = Scene::new(
            Svg::new()
                .width(width)
                .height(height)
                .view_box(ViewBox::unrounded(0.0, 0.0, width as f64, height as f64)),
        );
        let source = transparent_source();
        assert!(render(&mut scene, &source, context, None));
        let svg = scene.finish();
        let prefix = "id=\"sdocx-pencil3-paper-mask-";
        let start = svg.find(prefix).unwrap() + "id=\"".len();
        let id = svg[start..].split('"').next().unwrap();
        // Inspect the actual production paper mask, independently of the
        // brush texture and per-stamp opacity (the dummy dab is transparent).
        let overlay = format!(
            "<rect width=\"{width}\" height=\"{height}\" fill=\"white\" mask=\"url(#{id})\"/></svg>"
        );
        let root_end = svg.rfind("</svg>").unwrap();
        let svg = format!("{}{}", &svg[..root_end], overlay);
        let tree = resvg::usvg::Tree::from_str(&svg, &Default::default()).unwrap();
        let mut pixels = resvg::tiny_skia::Pixmap::new(width, height).unwrap();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut pixels.as_mut(),
        );
        pixels.data().to_vec()
    }

    #[test]
    fn paper_samples_follow_native_fractional_non_square_context() {
        let receipt: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/pencil3-fractional-context-native.json"
        ))
        .unwrap();
        let bits = receipt["paper_scale_float32_bits"].as_array().unwrap();
        let native_scale: Vec<_> = bits
            .iter()
            .map(|value| f32::from_bits(u32::from_str_radix(value.as_str().unwrap(), 16).unwrap()))
            .collect();
        assert_eq!(receipt["canvas"], serde_json::json!([321, 179]));
        assert_eq!(
            receipt["paper_texture_dimensions"],
            serde_json::json!([150, 150])
        );
        assert_eq!(receipt["background_scale"], 0.75);
        let decoder = png::Decoder::new(std::io::Cursor::new(PAPER_PNG));
        let mut reader = decoder.read_info().unwrap();
        let mut paper = vec![0; reader.output_buffer_size().unwrap()];
        reader.next_frame(&mut paper).unwrap();
        let context = Pencil3PaintContext {
            canvas_width: 321,
            canvas_height: 179,
            background_scale: 0.75,
            paper_sampler: Pencil3PaperSampler::Repeat,
            y_up: true,
        };
        let pixels = paper_pixels(context);
        // The captured uniform is applied to normalized canvas UVs in the
        // native shader; this test isolates paper sampling from projection.
        for (x, y) in [(5, 8), (111, 23), (113, 23), (225, 41), (300, 170)] {
            let source_x =
                (((x as f32 + 0.5) / 321.0) * native_scale[0] * 150.0).floor() as usize % 150;
            let source_y =
                (((y as f32 + 0.5) / 179.0) * native_scale[1] * 150.0).floor() as usize % 150;
            assert_eq!(
                pixels[(y * 321 + x) * 4 + 3],
                paper[(source_y * 150 + source_x) * 4 + 3],
                "pixel=({x},{y})"
            );
        }
    }

    #[test]
    fn paper_preserves_native_nearest_texels_at_fractional_period_and_seams() {
        let decoder = png::Decoder::new(std::io::Cursor::new(PAPER_PNG));
        let mut reader = decoder.read_info().unwrap();
        let mut paper = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut paper).unwrap();
        assert_eq!(info.color_type, png::ColorType::Rgba);
        assert_eq!((info.width, info.height), (150, 150));
        for paper_sampler in [
            Pencil3PaperSampler::Repeat,
            Pencil3PaperSampler::ClampToEdge,
        ] {
            for y_up in [true, false] {
                let context = Pencil3PaintContext {
                    canvas_width: 256,
                    canvas_height: 256,
                    background_scale: 0.75,
                    paper_sampler,
                    y_up,
                };
                let pixels = paper_pixels(context);
                for (x, y) in [
                    (5, 7),
                    (57, 81),
                    (111, 23),
                    (112, 23),
                    (113, 23),
                    (225, 23),
                    (33, 111),
                    (33, 112),
                    (33, 113),
                    (33, 225),
                ] {
                    let source_x = (x as f64 + 0.5) / 0.75;
                    let source_y = if y_up {
                        y as f64 + 0.5
                    } else {
                        256.0 - y as f64 - 0.5
                    } / 0.75;
                    let index = |v: f64| match paper_sampler {
                        Pencil3PaperSampler::Repeat => v.floor().rem_euclid(150.0) as usize,
                        Pencil3PaperSampler::ClampToEdge => v.floor().clamp(0.0, 149.0) as usize,
                    };
                    let expected = paper[(index(source_y) * 150 + index(source_x)) * 4 + 3];
                    let actual = pixels[(y * 256 + x) * 4 + 3];
                    assert_eq!(
                        actual, expected,
                        "{paper_sampler:?} y_up={y_up} pixel=({x},{y})"
                    );
                }
            }
        }
    }

    #[test]
    fn rejected_paper_contexts_leave_the_scene_unchanged() {
        let source = transparent_source();
        let context = Pencil3PaintContext {
            canvas_width: 256,
            canvas_height: 256,
            background_scale: 0.75,
            paper_sampler: Pencil3PaperSampler::Repeat,
            y_up: true,
        };
        let new_scene = || Scene::new(Svg::new().width(256).height(256));
        let mut scene = new_scene();
        for scale in [f32::MIN_POSITIVE, 0.00001, f32::MAX] {
            assert!(!render(
                &mut scene,
                &source,
                Pencil3PaintContext {
                    background_scale: scale,
                    ..context
                },
                None
            ));
        }
        assert!(render(&mut scene, &source, context, None));
        let mut fresh = new_scene();
        assert!(render(&mut fresh, &source, context, None));
        assert_eq!(scene.finish(), fresh.finish());
    }

    #[test]
    fn slice_culls_in_page_coordinates_and_preserves_particle_indices() {
        let mut source = transparent_source();
        source.records = vec![[5.0, 10.0, 2.0, 0.0, 0.0], [5.0, 90.0, 2.0, 0.0, 0.0]];
        let context = Pencil3PaintContext {
            canvas_width: 100,
            canvas_height: 100,
            background_scale: 1.0,
            paper_sampler: Pencil3PaperSampler::Repeat,
            y_up: true,
        };
        let new_scene = || Scene::new(Svg::new().width(100).height(100));
        let mut scene = new_scene();
        assert!(render_slice(
            &mut scene,
            &source,
            context,
            Some(0),
            Some((8.0, 12.0))
        ));
        let svg = scene.finish();
        assert!(svg.contains("sdocx-pencil3-brush-sdocx-def-0-1"));
        assert!(!svg.contains("sdocx-pencil3-brush-sdocx-def-0-0"));
        let mut scene = new_scene();
        assert!(render_slice(
            &mut scene,
            &source,
            context,
            Some(0),
            Some((40.0, 60.0))
        ));
        assert_eq!(scene.finish(), new_scene().finish());
        let mut scene = new_scene();
        assert!(render_slice(
            &mut scene,
            &source,
            Pencil3PaintContext {
                y_up: false,
                ..context
            },
            Some(0),
            Some((8.0, 12.0))
        ));
        let svg = scene.finish();
        assert!(svg.contains("sdocx-pencil3-brush-sdocx-def-0-0"));
        assert!(!svg.contains("sdocx-pencil3-brush-sdocx-def-0-1"));
        source.records[1][2] = f32::NAN;
        let mut scene = new_scene();
        assert!(!render_slice(
            &mut scene,
            &source,
            context,
            None,
            Some((40.0, 60.0))
        ));
        assert_eq!(scene.finish(), new_scene().finish());
    }

    #[test]
    fn paper_images_are_shared_by_exact_sampler_context() {
        let source = transparent_source();
        let mut scene = Scene::new(Svg::new().width(256).height(256));
        let context = Pencil3PaintContext {
            canvas_width: 256,
            canvas_height: 256,
            background_scale: 0.75,
            paper_sampler: Pencil3PaperSampler::Repeat,
            y_up: true,
        };
        assert!(render(&mut scene, &source, context, None));
        assert!(render(&mut scene, &source, context, None));
        assert!(render(
            &mut scene,
            &source,
            Pencil3PaintContext {
                paper_sampler: Pencil3PaperSampler::ClampToEdge,
                ..context
            },
            None
        ));
        let svg = scene.finish();
        assert_eq!(
            svg.matches("data:image/png;base64,").count(),
            3,
            "one shared brush atlas and two distinct paper samplers"
        );
        assert!(!svg.contains("<pattern"));
    }
}

#[cfg(all(test, feature = "pdf"))]
mod pencil3_transport_probe {
    use super::*;

    // Artifact generator only: deliberately ignored in ordinary test runs.
    // SDOCX_PENCIL3_PROBE_DIR must be an existing scratch directory.
    #[test]
    #[ignore]
    fn write_single_dab_mask_controls() {
        let out = std::path::PathBuf::from(std::env::var_os("SDOCX_PENCIL3_PROBE_DIR").unwrap());
        assert!(out.is_dir());
        let context = Pencil3PaintContext {
            canvas_width: 64,
            canvas_height: 64,
            background_scale: 1.0,
            paper_sampler: Pencil3PaperSampler::Repeat,
            y_up: false,
        };
        for (name, phase) in [("integer", 0.0_f32), ("quarter", 0.25), ("half", 0.5)] {
            let x = 32.0 + phase;
            let y = 32.0 + phase;
            let half = 12.0_f32;
            let source = PreparedPencil3V3 {
                records: vec![[x, y, half, 0.0, 0.0]],
                sample_ends: vec![1],
                bounds: BoundingBox {
                    x_min: f64::from(x - half),
                    y_min: f64::from(y - half),
                    x_max: f64::from(x + half),
                    y_max: f64::from(y + half),
                },
                color: crate::Color { r: 0, g: 0, b: 0 },
                opacity: 0.0,
            };
            let mut scene = Scene::new(
                Svg::new()
                    .width(64)
                    .height(64)
                    .view_box(ViewBox::unrounded(0.0, 0.0, 64.0, 64.0)),
            );
            assert!(render(&mut scene, &source, context, None));
            let svg = scene.finish();
            let id = |prefix: &str| {
                let start = svg.find(prefix).unwrap() + 4;
                svg[start..].split('"').next().unwrap().to_owned()
            };
            let paper = id("id=\"sdocx-pencil3-paper-mask-");
            let brush = id("id=\"sdocx-pencil3-brush-sdocx-def-");
            let end = svg.rfind("</svg>").unwrap();
            // Omission of a mask means constant 1. Both masks control preserves
            // production ordering; these are not native paint comparisons.
            for (kind, outer, inner) in [
                ("brush", "".to_owned(), format!(" mask=\"url(#{brush})\"")),
                ("paper", format!(" mask=\"url(#{paper})\""), "".to_owned()),
                (
                    "both",
                    format!(" mask=\"url(#{paper})\""),
                    format!(" mask=\"url(#{brush})\""),
                ),
                ("solid", "".to_owned(), "".to_owned()),
            ] {
                let overlay = format!(
                    "<g{outer}><rect x=\"{}\" y=\"{}\" width=\"24\" height=\"24\" fill=\"black\"{inner}/></g></svg>",
                    x - half,
                    y - half
                );
                let artifact = format!("{}{}", &svg[..end], overlay);
                let stem = format!("{kind}-{name}");
                std::fs::write(out.join(format!("{stem}.svg")), &artifact).unwrap();
                let page = crate::RenderedPage::new(0, 64, 64, artifact);
                let pdf =
                    crate::render_svg_pages_pdf(&[page], &crate::PdfOptions::default()).unwrap();
                std::fs::write(out.join(format!("{stem}.pdf")), pdf).unwrap();
            }
        }
    }
}

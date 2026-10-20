//! Bounded transport of recovered Pencil3 V3 paint into vector-native masks.
//!
//! The two PNGs are the original single-channel texture uploads transported as
//! alpha. The brush is stored once as a vertically reordered 16-row atlas and
//! every dab selects a row through a clipped SVG `<use>` viewport.

use super::vector::{
    ColorValue, Definitions, Group, Image, Paint, Pattern, PatternId, Rectangle, ReplayPart, Scene,
    StrokeIndex, Styled, Svg, SvgMask, Transform, Use, ViewBox,
};
use crate::{BoundingBox, ink::pencil_source::PreparedPencil3V3};

const PAPER_PNG: &[u8] = include_bytes!("pencil3_assets/paper.png");
const BRUSH_PNG: &[u8] = include_bytes!("pencil3_assets/brush-atlas.png");
const PAPER_IMAGE_ID: &str = "sdocx-pencil3-paper-image";
const BRUSH_IMAGE_ID: &str = "sdocx-pencil3-brush-atlas";
const MAX_RECORDS: usize = 2_048;
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
pub(super) fn render(
    scene: &mut Scene,
    source: &PreparedPencil3V3,
    context: Pencil3PaintContext,
    replay_index: Option<usize>,
) -> bool {
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
        return false;
    }
    if records.iter().any(|record| !valid_record(record)) {
        return false;
    }

    let width = f64::from(context.canvas_width);
    let height = f64::from(context.canvas_height);
    let period = TEXTURE_SIZE * f64::from(context.background_scale);
    let paper_pattern = scene.definition::<PatternId>();
    let instance_id = paper_pattern.id_text();
    let paper_mask_id = format!("sdocx-pencil3-paper-mask-{instance_id}");

    let mut definitions = Definitions::new();
    if scene.claim_shared_definition("pencil3-intrinsic-images") {
        definitions = definitions
            .add(
                Image::embedded(PAPER_PNG, "image/png")
                    .id(PAPER_IMAGE_ID)
                    .x(0.0)
                    .y(0.0)
                    .width(TEXTURE_SIZE)
                    .height(TEXTURE_SIZE)
                    .stretched()
                    .pixelated(),
            )
            .add(
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

    let paper = make_paper_pattern(paper_pattern, width, height, period, context);
    definitions = definitions.add(paper);
    definitions = definitions.add(
        SvgMask::alpha(&paper_mask_id, 0.0, 0.0, width, height).add(
            Rectangle::new()
                .x(0.0)
                .y(0.0)
                .width(width)
                .height(height)
                .fill(Paint::Pattern(paper_pattern)),
        ),
    );

    for (index, record) in records.iter().enumerate() {
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
        for (index, record) in records.iter().enumerate() {
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

fn make_paper_pattern(
    pattern_id: super::vector::Definition<PatternId>,
    canvas_width: f64,
    canvas_height: f64,
    period: f64,
    context: Pencil3PaintContext,
) -> Pattern {
    let (pattern_width, pattern_height) = match context.paper_sampler {
        Pencil3PaperSampler::Repeat => (period, period),
        Pencil3PaperSampler::ClampToEdge => (canvas_width, canvas_height),
    };
    let mut pattern = Pattern::user_space(&pattern_id, pattern_width, pattern_height);
    if !context.y_up {
        pattern = pattern.transformed(Transform::matrix(
            [1.0, 0.0, 0.0, -1.0, 0.0, canvas_height],
            0,
            0,
        ));
    }

    match context.paper_sampler {
        Pencil3PaperSampler::Repeat => pattern.add(
            Svg::new()
                .x(0.0)
                .y(0.0)
                .width(period)
                .height(period)
                .view_box(ViewBox::unrounded(0.0, 0.0, TEXTURE_SIZE, TEXTURE_SIZE))
                .preserve_aspect_none()
                .add(Use::href(PAPER_IMAGE_ID)),
        ),
        Pencil3PaperSampler::ClampToEdge => {
            let source_width = TEXTURE_SIZE * f64::from(context.background_scale);
            let source_height = source_width;
            let mut viewport = Svg::new()
                .x(0.0)
                .y(0.0)
                .width(canvas_width)
                .height(canvas_height)
                .view_box(ViewBox::unrounded(0.0, 0.0, canvas_width, canvas_height))
                .preserve_aspect_none()
                .clipped_viewport()
                .add(
                    Svg::new()
                        .x(0.0)
                        .y(0.0)
                        .width(source_width)
                        .height(source_height)
                        .view_box(ViewBox::unrounded(0.0, 0.0, TEXTURE_SIZE, TEXTURE_SIZE))
                        .preserve_aspect_none()
                        .clipped_viewport()
                        .add(Use::href(PAPER_IMAGE_ID)),
                );

            if canvas_width > source_width {
                viewport = viewport.add(clamp_viewport(
                    source_width,
                    0.0,
                    canvas_width - source_width,
                    source_height.min(canvas_height),
                    TEXTURE_SIZE - 1.0,
                    0.0,
                    1.0,
                    TEXTURE_SIZE,
                ));
            }
            if canvas_height > source_height {
                viewport = viewport.add(clamp_viewport(
                    0.0,
                    source_height,
                    source_width.min(canvas_width),
                    canvas_height - source_height,
                    0.0,
                    TEXTURE_SIZE - 1.0,
                    TEXTURE_SIZE,
                    1.0,
                ));
            }
            if canvas_width > source_width && canvas_height > source_height {
                viewport = viewport.add(clamp_viewport(
                    source_width,
                    source_height,
                    canvas_width - source_width,
                    canvas_height - source_height,
                    TEXTURE_SIZE - 1.0,
                    TEXTURE_SIZE - 1.0,
                    1.0,
                    1.0,
                ));
            }
            pattern.add(viewport)
        }
    }
}

// Explicit target rectangle and source crop describe the edge extrusion.
#[allow(clippy::too_many_arguments)]
fn clamp_viewport(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    view_x: f64,
    view_y: f64,
    view_width: f64,
    view_height: f64,
) -> Svg {
    Svg::new()
        .x(x)
        .y(y)
        .width(width)
        .height(height)
        .view_box(ViewBox::unrounded(view_x, view_y, view_width, view_height))
        .preserve_aspect_none()
        .clipped_viewport()
        .add(Use::href(PAPER_IMAGE_ID))
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

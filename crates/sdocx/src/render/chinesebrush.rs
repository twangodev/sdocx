//! Approximate ChineseBrush vector paint from parsed source records and the
//! native intrinsic atlas. Circles provide geometric coverage; this does not
//! reproduce the native screen-space `fwidth` fringe.
use super::vector::{
    Circle, Clip, ClipPath, Definition, Definitions, Group, Image, ImageId, Paint, ReplayPart,
    Scene, StrokeIndex, Styled, Transform, Use,
};
use crate::Stroke;

const MAX_SOURCE_BYTES: usize = 4 * 1024 * 1024;
const MAX_SOURCE_EVENTS: usize = 65_536;
const MAX_OUTPUT_RECORDS: usize = 32_768;
const MAX_ESTIMATED_MARKUP_BYTES: usize = 32 * 1024 * 1024;
const STAMP_FIXED_MARKUP_BUDGET: usize = 1_024;
const TILE_DEFINITION_FIXED_MARKUP_BUDGET: usize = 256;
const ATLAS_WIDTH: usize = 144;
const TILE_SIZE: usize = 48;
const TILE_COUNT: usize = 7;
// Captured native RGBA atlas: seven 48x48 cells occupy slots 0..6 of a
// zero-filled row-major 3x3 texture. The shader uses its alpha only and gets
// stroke RGB from a separate uniform; retaining the full image pins that split.
const SOURCE_ATLAS: &[u8; ATLAS_WIDTH * ATLAS_WIDTH * 4] =
    include_bytes!("chinesebrush_atlas_rgba_144x144.bin");

fn source_alpha(x: usize, y: usize) -> u8 {
    SOURCE_ATLAS[(y * ATLAS_WIDTH + x) * 4 + 3]
}

#[derive(Debug)]
struct Stamp {
    x: f64,
    y: f64,
    radius: f64,
    tile: usize,
    opacity: f64,
}

#[derive(Debug)]
pub(crate) struct Prepared {
    sample_ends: Vec<usize>,
    stamps: Vec<Stamp>,
    tiles: Vec<(usize, Vec<u8>)>,
}

fn gl_version(setting: &str) -> Option<i32> {
    // These are resolved values from StrokeRendering, not invented source
    // profile labels. V4 has a different getter value despite drawing through
    // GLV3 and stays out until its numeric caller is checked against this model.
    match setting {
        "1;" => Some(1),
        "2;" => Some(2),
        "3;" => Some(3),
        _ => None,
    }
}

fn png_tile(tile: usize, rgb: [u8; 3]) -> Option<Vec<u8>> {
    if tile >= TILE_COUNT {
        return None;
    }
    let mut rgba = Vec::with_capacity(TILE_SIZE * TILE_SIZE * 4);
    for y in 0..TILE_SIZE {
        for x in 0..TILE_SIZE {
            let atlas_x = (tile % 3) * TILE_SIZE + x;
            let atlas_y = (tile / 3) * TILE_SIZE + y;
            let alpha = source_alpha(atlas_x, atlas_y);
            // Native RTV1 subtracts a small RGB bias selected by sampled alpha.
            // RGBA8 output quantizes these exact deltas to three or one bytes.
            let correction = if alpha < 128 {
                4
            } else if alpha < 141 {
                1
            } else {
                0
            };
            rgba.extend(rgb.map(|channel| channel.saturating_sub(correction)));
            rgba.push(alpha);
        }
    }
    let mut png_bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_bytes, TILE_SIZE as u32, TILE_SIZE as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header().ok()?.write_image_data(&rgba).ok()?;
    }
    Some(png_bytes)
}

fn tile_opacity(phase: f32, scalar: f32) -> Option<(usize, f32)> {
    if !phase.is_finite()
        || !(0.0..=(TILE_COUNT - 1) as f32).contains(&phase)
        || phase.fract() != 0.0
        || !scalar.is_finite()
        || !(0.0..=255.0).contains(&scalar)
        || scalar.fract() != 0.0
    {
        return None;
    }
    let selector = phase + scalar * 0.001;
    if !selector.is_finite() {
        return None;
    }
    let tile = selector.floor() as usize;
    let opacity = ((selector - tile as f32) * (1000.0 / 255.0)).clamp(0.0, 1.0);
    (tile < TILE_COUNT).then_some((tile, opacity))
}

/// Build all source geometry and every raster payload before the caller emits
/// SVG. Returning None means the whole stroke must use generic rendering.
pub(crate) fn prepare(
    stroke: &Stroke,
    profile: super::ChineseBrushRenderProfile,
) -> Option<Prepared> {
    let rendering = stroke.rendering.as_ref()?;
    if rendering.metadata.is_none()
        || rendering.pen_name.as_deref()
            != Some("com.samsung.android.sdk.pen.pen.preload.ChineseBrush")
        || rendering.tool_type_raw > 3
        || rendering.properties.eraser
        || rendering.properties.fixed_width
        || rendering.properties.fixed_opacity
        || rendering.properties.rainbow_effect
        || rendering.properties.alpha_lock
        || rendering.properties.straighten
        || rendering.properties.reveal_mode
        || rendering.properties.replay_only
        || rendering.style.stroke_type_raw.unwrap_or(0) != 0
        || rendering.style.line_type_raw.unwrap_or(0) != 0
        || profile.screen_width == 0
        || profile.screen_height == 0
        || profile.screen_width > i32::MAX as u32
        || profile.screen_height > i32::MAX as u32
    {
        return None;
    }
    let version = gl_version(rendering.advanced_settings.as_deref()?)?;
    let n = stroke.points.len();
    if !(2..=MAX_SOURCE_EVENTS).contains(&n)
        || n.checked_mul(48)
            .is_none_or(|bytes| bytes > MAX_SOURCE_BYTES)
        || stroke.pressures.len() != n
        || stroke.timestamps.len() != n
    {
        return None;
    }
    let size = rendering.style.pen_size?;
    // The shared native Pen::SetSize clamps finite sizes to [0.4, 800] before
    // this model's redraw call, so retain the same bounded source behavior.
    if !size.is_finite() {
        return None;
    }
    let xy: Vec<[f32; 2]> = stroke
        .points
        .iter()
        .map(|p| [p.x as f32, p.y as f32])
        .collect();
    let pressure: Vec<f32> = stroke.pressures.iter().map(|p| *p as f32).collect();
    let times: Vec<i32> = stroke
        .timestamps
        .iter()
        .map(|t| i32::try_from(*t).ok())
        .collect::<Option<_>>()?;
    let sample = crate::ink::chinesebrush_glv123::run_model_bounded_with_sample_ends(
        &xy,
        &pressure,
        &times,
        size,
        version,
        profile.screen_width as i32,
        profile.screen_height as i32,
        i32::from(rendering.tool_type_raw),
        rendering.properties.compressed,
        MAX_OUTPUT_RECORDS,
    )
    .ok()?;
    if sample.records.is_empty()
        || sample.records.len() > MAX_OUTPUT_RECORDS
        || sample.sample_ends.len() != n
        || sample.sample_ends.windows(2).any(|ends| ends[0] > ends[1])
        || sample.sample_ends.last().copied()? != sample.records.len()
    {
        return None;
    }

    let argb = rendering.style.color_argb.unwrap_or(0xff00_0000);
    let rgb = [
        ((argb >> 16) & 0xff) as u8,
        ((argb >> 8) & 0xff) as u8,
        (argb & 0xff) as u8,
    ];
    let mut stamps = Vec::with_capacity(sample.records.len());
    let mut used_tiles = [false; TILE_COUNT];
    for [x, y, radius, phase, scalar] in sample.records {
        if ![x, y, radius, phase, scalar]
            .into_iter()
            .all(f32::is_finite)
            || radius <= 0.0
            || phase.fract() != 0.0
            || scalar.fract() != 0.0
        {
            return None;
        }
        let (tile, opacity) = tile_opacity(phase, scalar)?;
        used_tiles[tile] = true;
        let (x, y, radius) = (f64::from(x), f64::from(y), f64::from(radius));
        stamps.push(Stamp {
            x,
            y,
            radius,
            tile,
            opacity: f64::from(opacity),
        });
    }
    let mut tiles = Vec::new();
    for (tile, used) in used_tiles.into_iter().enumerate() {
        if used {
            tiles.push((tile, png_tile(tile, rgb)?));
        }
    }
    // Each intrinsic PNG appears once in defs; stamp references only add
    // bounded transform, opacity, clip, replay, and href markup.
    let estimated = stamps
        .len()
        .checked_mul(STAMP_FIXED_MARKUP_BUDGET)?
        .checked_add(tiles.iter().try_fold(0usize, |total, (_, png)| {
            let png_len = png.len();
            let base64_len = png_len.checked_add(2)?.checked_div(3)?.checked_mul(4)?;
            total
                .checked_add(TILE_DEFINITION_FIXED_MARKUP_BUDGET)?
                .checked_add(base64_len)
        })?)?;
    if estimated > MAX_ESTIMATED_MARKUP_BYTES {
        return None;
    }
    Some(Prepared {
        sample_ends: sample.sample_ends,
        stamps,
        tiles,
    })
}

/// Emit only after `prepare` has validated every record, tile and budget. Sibling
/// stamp groups retain native VBO order and therefore source-over overlap order.
pub(crate) fn render(svg: &mut Scene, prepared: &Prepared, replay_index: Option<usize>) {
    let draw = |svg: &mut Scene| render_stamps(svg, prepared, replay_index.is_some());
    if let Some(index) = replay_index {
        svg.scope(Group::new().replay_stroke(StrokeIndex(index)), draw);
    } else {
        draw(svg);
    }
}

fn render_stamps(svg: &mut Scene, prepared: &Prepared, replay: bool) {
    let clip: Definition<Clip> = svg.definition();
    let tile_definitions: Vec<_> = prepared
        .tiles
        .iter()
        .map(|(tile, _)| (*tile, svg.definition::<ImageId>()))
        .collect();
    svg.scope(Definitions::new(), |svg| {
        svg.scope(ClipPath::new(&clip), |svg| {
            svg.push(
                Circle::new()
                    .cx(0.0)
                    .cy(0.0)
                    .r(1.0)
                    .fill(Paint::from_hex("#ffffff")),
            );
        });
        for ((_, image), (_, definition)) in prepared.tiles.iter().zip(&tile_definitions) {
            svg.push(
                Image::embedded(image, "image/png")
                    .id(&definition.id_text())
                    .x(-1.0)
                    .y(-1.0)
                    .width(2.0)
                    .height(2.0)
                    .stretched()
                    .pixelated(),
            );
        }
    });
    for (record_index, stamp) in prepared.stamps.iter().enumerate() {
        let Some((_, definition)) = tile_definitions
            .iter()
            .find(|(tile, _)| *tile == stamp.tile)
        else {
            unreachable!("all referenced atlas cells were prepared before emission");
        };
        let transform =
            Transform::matrix_unrounded([stamp.radius, 0.0, 0.0, stamp.radius, stamp.x, stamp.y]);
        let source_event = prepared
            .sample_ends
            .partition_point(|&end| end <= record_index);
        svg.scope(
            Group::new()
                .transformed(transform)
                .opacity(stamp.opacity)
                .replay_part(replay.then_some(ReplayPart(source_event + 1))),
            |svg| {
                svg.push(Use::image(definition).clipped(&clip));
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ATLAS_WIDTH, SOURCE_ATLAS, TILE_COUNT, TILE_SIZE, png_tile, source_alpha, tile_opacity,
    };
    use png::{BitDepth, ColorType, Decoder};
    use std::io::Cursor;

    #[test]
    fn atlas_rgba_and_row_major_cells_are_pinned_to_native_asset() {
        use sha2::{Digest, Sha256};
        assert_eq!((ATLAS_WIDTH, TILE_SIZE, TILE_COUNT), (144, 48, 7));
        let digest = Sha256::digest(SOURCE_ATLAS);
        let hex = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(
            hex,
            "da75f5d1190e6fe086aa11007b7ed739421cf5d4522e34cb246df273353d2942"
        );
        for tile in 0..TILE_COUNT {
            let origin_x = (tile % 3) * TILE_SIZE;
            let origin_y = (tile / 3) * TILE_SIZE;
            for y in 0..TILE_SIZE {
                for x in 0..TILE_SIZE {
                    let offset = ((origin_y + y) * ATLAS_WIDTH + origin_x + x) * 4;
                    assert_eq!(
                        source_alpha(origin_x + x, origin_y + y),
                        SOURCE_ATLAS[offset + 3]
                    );
                }
            }
        }
    }

    #[test]
    fn tile_preserves_native_alpha_and_rgb_bias_thresholds() {
        let rgb = [100, 150, 200];
        let bytes = png_tile(6, rgb).unwrap();
        let decoder = Decoder::new(Cursor::new(bytes));
        let mut reader = decoder.read_info().unwrap();
        assert_eq!((reader.info().width, reader.info().height), (48, 48));
        assert_eq!(reader.info().color_type, ColorType::Rgba);
        assert_eq!(reader.info().bit_depth, BitDepth::Eight);
        let mut decoded = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut decoded).unwrap();
        for y in 0..TILE_SIZE {
            for x in 0..TILE_SIZE {
                let alpha = source_alpha(x, 2 * TILE_SIZE + y);
                let correction: u8 = if alpha < 128 {
                    4
                } else if alpha < 141 {
                    1
                } else {
                    0
                };
                let offset = (y * TILE_SIZE + x) * 4;
                assert_eq!(decoded[offset..offset + 3], rgb.map(|c| c - correction));
                assert_eq!(decoded[offset + 3], alpha);
            }
        }
        assert_eq!(info.buffer_size(), decoded.len());
    }

    #[test]
    fn atlas_selector_preserves_cell_and_byte_opacity() {
        let (tile, opacity) = tile_opacity(3.0, 128.0).unwrap();
        assert_eq!(tile, 3);
        assert!((opacity - 128.0 / 255.0).abs() < 1e-6);
        let (tile, opacity) = tile_opacity(6.0, 255.0).unwrap();
        assert_eq!(tile, 6);
        assert!((opacity - 1.0).abs() < 1e-6);
        assert!(tile_opacity(7.0, 0.0).is_none());
        assert!(tile_opacity(6.0, 255.5).is_none());
    }
}

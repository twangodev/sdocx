//! Approximate ChineseBrush vector paint from parsed source records and the
//! native intrinsic atlas. Optional output-scale masks approximate native
//! screen-space `fwidth` coverage.
use super::vector::{
    Circle, Clip, ClipPath, ColorValue, Data, Definition, Definitions, Gradient, Group, Image,
    ImageId, Mask, Paint, Path, RadialGradient, ReplayPart, Scene, Stop, StrokeIndex, Styled,
    SvgMask, Transform, Use,
};
use crate::Stroke;
use std::collections::HashMap;

const MAX_SOURCE_BYTES: usize = 4 * 1024 * 1024;
const MAX_SOURCE_EVENTS: usize = 65_536;
const MAX_OUTPUT_RECORDS: usize = 32_768;
const MAX_ESTIMATED_MARKUP_BYTES: usize = 32 * 1024 * 1024;
const STAMP_FIXED_MARKUP_BUDGET: usize = 1_024;
const TILE_DEFINITION_FIXED_MARKUP_BUDGET: usize = 256;
const FWIDTH_SECTORS: usize = 64;
const FWIDTH_SECTOR_BUDGET: usize = 4_096;
const FWIDTH_STAMP_BUDGET: usize = 512;
const ATLAS_WIDTH: usize = 144;
const TILE_SIZE: usize = 48;
const TILE_COUNT: usize = 7;
// Captured native RGBA atlas: seven 48x48 cells occupy slots 0..6 of a
// zero-filled row-major 3x3 texture. The shader uses its alpha only and gets
// stroke RGB from a separate uniform; retaining the full image pins that split.
const SOURCE_ATLAS: &[u8; ATLAS_WIDTH * ATLAS_WIDTH * 4] =
    include_bytes!("chinesebrush_atlas_rgba_144x144.bin");

fn estimated_markup_bytes(
    stamp_count: usize,
    tiles: &[(usize, Vec<u8>)],
    mask_count: usize,
) -> Option<usize> {
    stamp_count
        .checked_mul(STAMP_FIXED_MARKUP_BUDGET)?
        .checked_add(mask_count.checked_mul(FWIDTH_SECTORS.checked_mul(FWIDTH_SECTOR_BUDGET)?)?)?
        .checked_add(if mask_count == 0 {
            0
        } else {
            stamp_count.checked_mul(FWIDTH_STAMP_BUDGET)?
        })?
        .checked_add(tiles.iter().try_fold(0usize, |total, (_, png)| {
            let base64_len = png.len().checked_add(2)?.checked_div(3)?.checked_mul(4)?;
            total
                .checked_add(TILE_DEFINITION_FIXED_MARKUP_BUDGET)?
                .checked_add(base64_len)
        })?)
}

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
    mask: Option<usize>,
}

#[derive(Debug)]
struct FringeMask {
    widths: [f64; FWIDTH_SECTORS],
    core_radius: f64,
}

#[derive(Debug)]
pub(crate) struct Prepared {
    sample_ends: Vec<usize>,
    stamps: Vec<Stamp>,
    tiles: Vec<(usize, Vec<u8>)>,
    fringe_masks: Vec<FringeMask>,
}

fn gl_version(setting: &str) -> Option<i32> {
    // The setting is the stored Pen getter. V4 is admitted only because the
    // pinned paired Drawing capture verifies getter 4 selecting the same
    // GLV3 drawable, AddPoint stream and complete host-Mesa frame as getter 3.
    match setting {
        "1;" => Some(1),
        "2;" => Some(2),
        "3;" => Some(3),
        "4;" => Some(3),
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
    output_profile: Option<super::ChineseBrushFwidthOutputProfile>,
) -> Option<Prepared> {
    let pixels_per_page_unit =
        output_profile.map(|profile| f64::from(profile.pixels_per_page_unit));
    if pixels_per_page_unit.is_some_and(|scale| !scale.is_finite() || scale <= 0.0) {
        return None;
    }
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
    let mut fringe_masks = Vec::new();
    let mut fringe_indices = HashMap::<u64, usize>::new();
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
        let mask = if let Some(scale) = pixels_per_page_unit {
            let device_radius = radius * scale;
            if !device_radius.is_finite() || device_radius <= 0.0 {
                return None;
            }
            let key = radius.to_bits();
            Some(if let Some(index) = fringe_indices.get(&key) {
                *index
            } else {
                let inverse_radius = device_radius.recip();
                if !inverse_radius.is_finite() || inverse_radius <= 0.0 {
                    return None;
                }
                let widths = std::array::from_fn(|sector| {
                    let angle =
                        (sector as f64 + 0.5) * (std::f64::consts::TAU / FWIDTH_SECTORS as f64);
                    (angle.cos().abs() + angle.sin().abs()) * inverse_radius
                });
                if widths.iter().any(|width| {
                    !width.is_finite() || *width <= 0.0 || (*width < 1.0 && 1.0 - *width == 1.0)
                }) {
                    return None;
                }
                let core_radius = (1.0 - std::f64::consts::SQRT_2 * inverse_radius).max(0.0);
                if !core_radius.is_finite() || core_radius >= 1.0 {
                    return None;
                }
                let index = fringe_masks.len();
                fringe_masks.push(FringeMask {
                    widths,
                    core_radius,
                });
                fringe_indices.insert(key, index);
                index
            })
        } else {
            None
        };
        stamps.push(Stamp {
            x,
            y,
            radius,
            tile,
            opacity: f64::from(opacity),
            mask,
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
    let estimated = estimated_markup_bytes(stamps.len(), &tiles, fringe_masks.len())?;
    if estimated > MAX_ESTIMATED_MARKUP_BYTES {
        return None;
    }
    Some(Prepared {
        sample_ends: sample.sample_ends,
        stamps,
        tiles,
        fringe_masks,
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
    let fringe_definitions: Vec<_> = prepared
        .fringe_masks
        .iter()
        .map(|_| {
            (
                svg.definition::<Mask>(),
                std::array::from_fn::<_, FWIDTH_SECTORS, _>(|_| svg.definition::<Gradient>()),
            )
        })
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
        for (mask, (mask_definition, gradients)) in
            prepared.fringe_masks.iter().zip(&fringe_definitions)
        {
            let mut definition = SvgMask::alpha(&mask_definition.id_text(), -1.0, -1.0, 2.0, 2.0);
            for (sector, (width, gradient_id)) in mask.widths.iter().zip(gradients).enumerate() {
                let gradient = if *width < 1.0 {
                    RadialGradient::new(gradient_id)
                        .cx(0.0)
                        .cy(0.0)
                        .r(1.0)
                        .add(Stop::new(0.0, Some(ColorValue::WHITE)).opacity(1.0))
                        .add(Stop::new(1.0 - *width, Some(ColorValue::WHITE)).opacity(1.0))
                        .add(Stop::new(1.0, Some(ColorValue::WHITE)).opacity(0.0))
                } else {
                    RadialGradient::new(gradient_id)
                        .cx(0.0)
                        .cy(0.0)
                        .r(1.0)
                        .add(Stop::new(0.0, Some(ColorValue::WHITE)).opacity(1.0 / *width))
                        .add(Stop::new(1.0, Some(ColorValue::WHITE)).opacity(0.0))
                };
                svg.push(gradient);
                let start = sector as f64 * (std::f64::consts::TAU / FWIDTH_SECTORS as f64);
                let end = (sector + 1) as f64 * (std::f64::consts::TAU / FWIDTH_SECTORS as f64);
                let wedge = Data::new()
                    .move_to((0.0, 0.0))
                    .line_to((start.cos(), start.sin()))
                    .elliptical_arc_by((
                        1.0,
                        1.0,
                        0.0,
                        false,
                        true,
                        (end.cos() - start.cos()) as f32,
                        (end.sin() - start.sin()) as f32,
                    ))
                    .close();
                definition =
                    definition.add(Path::new().data(wedge).fill(Paint::Gradient(*gradient_id)));
            }
            if mask.core_radius > 0.0 {
                definition = definition.add(
                    Circle::new()
                        .cx(0.0)
                        .cy(0.0)
                        .r(mask.core_radius)
                        .fill(Paint::from_hex("#ffffff")),
                );
            }
            svg.push(definition);
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
                let image = Use::image(definition);
                if let Some(mask) = stamp.mask {
                    svg.push(image.masked(&fringe_definitions[mask].0));
                } else {
                    svg.push(image.clipped(&clip));
                }
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ATLAS_WIDTH, MAX_ESTIMATED_MARKUP_BYTES, Prepared, SOURCE_ATLAS, TILE_COUNT, TILE_SIZE,
        estimated_markup_bytes, png_tile, prepare, source_alpha, tile_opacity,
    };
    use crate::render::ChineseBrushRenderProfile;
    use crate::{
        BoundingBox, Color, ObjectMetadata, Point, Stroke, StrokeProperties, StrokeRendering,
        StrokeStyle,
    };
    use png::{BitDepth, ColorType, Decoder};
    use serde_json::Value;
    use sha2::{Digest, Sha256};
    use std::io::Cursor;

    fn fixture_bytes(hex: &str) -> Vec<u8> {
        assert_eq!(hex.len() % 2, 0);
        let (pairs, remainder) = hex.as_bytes().as_chunks::<2>();
        assert!(remainder.is_empty());
        pairs
            .iter()
            .map(|pair| {
                let digit = |byte: u8| match byte {
                    b'0'..=b'9' => byte - b'0',
                    b'a'..=b'f' => byte - b'a' + 10,
                    b'A'..=b'F' => byte - b'A' + 10,
                    _ => panic!("invalid fixture hex"),
                };
                digit(pair[0]) * 16 + digit(pair[1])
            })
            .collect()
    }

    fn fixture_f32s(source: &Value, channel: &str) -> Vec<f32> {
        let bytes = fixture_bytes(
            source["source_arrays"][channel]["data_hex"]
                .as_str()
                .unwrap(),
        );
        let (words, remainder) = bytes.as_chunks::<4>();
        assert!(remainder.is_empty());
        words
            .iter()
            .map(|bytes| f32::from_le_bytes(*bytes))
            .collect()
    }

    fn capture_stroke(source: &Value, setting: &str) -> Stroke {
        let point_bytes = fixture_bytes(
            source["source_arrays"]["points"]["data_hex"]
                .as_str()
                .unwrap(),
        );
        let (point_pairs, remainder) = point_bytes.as_chunks::<8>();
        assert!(remainder.is_empty());
        let points: Vec<_> = point_pairs
            .iter()
            .map(|xy| Point {
                x: f64::from(f32::from_le_bytes([xy[0], xy[1], xy[2], xy[3]])),
                y: f64::from(f32::from_le_bytes([xy[4], xy[5], xy[6], xy[7]])),
            })
            .collect();
        let pressures: Vec<_> = fixture_f32s(source, "pressures")
            .into_iter()
            .map(f64::from)
            .collect();
        let timestamp_bytes = fixture_bytes(
            source["source_arrays"]["timestamps"]["data_hex"]
                .as_str()
                .unwrap(),
        );
        let (timestamp_words, remainder) = timestamp_bytes.as_chunks::<4>();
        assert!(remainder.is_empty());
        let timestamps: Vec<_> = timestamp_words
            .iter()
            .map(|bytes| i64::from(i32::from_le_bytes(*bytes)))
            .collect();
        let tilts: Vec<_> = fixture_f32s(source, "tilts")
            .into_iter()
            .map(f64::from)
            .collect();
        let orientations: Vec<_> = fixture_f32s(source, "orientations")
            .into_iter()
            .map(f64::from)
            .collect();
        let bbox = BoundingBox {
            x_min: 1.0,
            y_min: -2.0,
            x_max: 6.0,
            y_max: 4.0,
        };
        let metadata = ObjectMetadata {
            format_version: 1,
            uuid: "synthetic-native-glv3-pair".into(),
            modified_time_raw: 0,
            bbox,
            replay_timestamp_raw: 0,
            resize_mode_raw: 0,
            rotatable: true,
            selectable: true,
            movable: true,
            visible: true,
            replayable: true,
            out_of_canvas_enabled: false,
            template: false,
            flip_enabled: false,
            float_drawn_rect: false,
            locked: false,
            removable: true,
            rotation_degrees: None,
            property_mask: Vec::new(),
            field_mask: Vec::new(),
            fixed_trailing_data: Vec::new(),
            flexible_trailing_data: Vec::new(),
        };
        let argb = u32::from_str_radix(
            source["color_argb_u32"]
                .as_str()
                .unwrap()
                .trim_start_matches("0x"),
            16,
        )
        .unwrap();
        let size_bytes = fixture_bytes(source["size_f32_bits"].as_str().unwrap());
        let (size_words, remainder) = size_bytes.as_chunks::<4>();
        assert!(remainder.is_empty() && size_words.len() == 1);
        let size_bits = u32::from_le_bytes(size_words[0]);
        Stroke {
            rendering: Some(StrokeRendering {
                metadata: Some(Box::new(metadata)),
                pen_name: Some(source["pen_name"].as_str().unwrap().to_owned()),
                advanced_settings: Some(setting.to_owned()),
                tool_type_raw: source["tool_type"].as_u64().unwrap() as u16,
                properties: StrokeProperties {
                    compressed: true,
                    replay_only: false,
                    stylus_channels: true,
                    eraser: false,
                    fixed_width: false,
                    millisecond_timestamps: true,
                    top_layer_pen: false,
                    alpha_lock: false,
                    binary_added: true,
                    generated: false,
                    fixed_opacity: false,
                    rainbow_effect: false,
                    straighten: false,
                    reveal_mode: false,
                },
                style: StrokeStyle {
                    color_argb: Some(argb),
                    pen_size: Some(f32::from_bits(size_bits)),
                    stroke_type_raw: Some(0),
                    line_type_raw: Some(0),
                    ..StrokeStyle::default()
                },
            }),
            bbox,
            points,
            pressures,
            timestamps,
            tilts,
            orientations,
            color: Some(Color {
                r: (argb >> 16) as u8,
                g: (argb >> 8) as u8,
                b: argb as u8,
            }),
            pen_width: f32::from_bits(size_bits),
        }
    }

    fn stamp_fingerprint(prepared: &Prepared) -> Vec<(u64, u64, u64, usize, u64, Option<usize>)> {
        prepared
            .stamps
            .iter()
            .map(|stamp| {
                (
                    stamp.x.to_bits(),
                    stamp.y.to_bits(),
                    stamp.radius.to_bits(),
                    stamp.tile,
                    stamp.opacity.to_bits(),
                    stamp.mask,
                )
            })
            .collect()
    }

    #[test]
    fn captured_setting4_selects_glv3_and_matches_native_setting3_model_and_paint_pair() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/chinesebrush-glv3-setting4-native.json"
        ))
        .unwrap();
        assert_eq!(
            fixture["schema"],
            "chinesebrush-native-glv3-setting-pair-v1"
        );
        assert_eq!(
            fixture["scope"]["pixel_claim"],
            "native pair equality only; no assertion that this raster matches Rust SVG or Samsung device output"
        );
        let source = &fixture["source_input"];
        let profile = ChineseBrushRenderProfile {
            screen_width: 1920,
            screen_height: 1080,
        };
        let native_stream = fixture_bytes(
            fixture["native_addpoint_stream"]["buffer_hex"]
                .as_str()
                .unwrap(),
        );
        let native_stream_sha = format!("{:x}", Sha256::digest(&native_stream));
        assert_eq!(
            native_stream_sha,
            fixture["native_addpoint_stream"]["buffer_sha256"]
                .as_str()
                .unwrap()
        );
        assert_eq!(native_stream.len(), 80);
        assert_eq!(fixture["native_paint_frame"]["nonzero_pixels"], 11);
        assert_eq!(
            fixture["cases"]["3;"]["paint_readback_sha256"],
            fixture["cases"]["4;"]["paint_readback_sha256"]
        );
        assert_eq!(
            fixture["cases"]["3;"]["paint_nonzero_pixels"],
            fixture["cases"]["4;"]["paint_nonzero_pixels"]
        );
        let paired_frame_sha = fixture["paired_assertions"]["frame_rgba8_sha256_equal"]
            .as_str()
            .unwrap();
        assert_eq!(
            paired_frame_sha,
            fixture["cases"]["3;"]["paint_readback_sha256"]
        );
        assert_eq!(
            paired_frame_sha,
            fixture["cases"]["4;"]["paint_readback_sha256"]
        );
        let native_frame = &fixture["native_paint_frame"];
        assert_eq!(native_frame["format"], "GL_RGBA/GL_UNSIGNED_BYTE");
        assert_eq!(native_frame["row_origin"], "GL lower-left");
        let width = usize::try_from(native_frame["width"].as_u64().unwrap()).unwrap();
        let height = usize::try_from(native_frame["height"].as_u64().unwrap()).unwrap();
        let initial: Vec<u8> = native_frame["initial_rgba8"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| u8::try_from(value.as_u64().unwrap()).unwrap())
            .collect();
        assert_eq!(initial.len(), 4);
        let pixels_len = width.checked_mul(height).unwrap();
        let mut reconstructed_frame = Vec::with_capacity(pixels_len.checked_mul(4).unwrap());
        for _ in 0..pixels_len {
            reconstructed_frame.extend_from_slice(&initial);
        }
        let mut seen_pixels = vec![false; pixels_len];
        for pixel in fixture["native_paint_sparse_pixels"].as_array().unwrap() {
            let x = usize::try_from(pixel["x"].as_u64().unwrap()).unwrap();
            let y = usize::try_from(pixel["y_gl_lower_left"].as_u64().unwrap()).unwrap();
            assert!(
                x < width && y < height,
                "sparse native pixel outside captured frame"
            );
            let index = y.checked_mul(width).unwrap().checked_add(x).unwrap();
            assert!(
                !seen_pixels[index],
                "duplicate sparse native pixel coordinate"
            );
            seen_pixels[index] = true;
            let rgba: Vec<u8> = pixel["rgba8"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| u8::try_from(value.as_u64().unwrap()).unwrap())
                .collect();
            assert_eq!(rgba.len(), 4);
            assert!(rgba.iter().any(|channel| *channel != 0));
            let offset = index.checked_mul(4).unwrap();
            reconstructed_frame[offset..offset + 4].copy_from_slice(&rgba);
        }
        assert_eq!(seen_pixels.iter().filter(|seen| **seen).count(), 11);
        assert_eq!(
            seen_pixels.iter().filter(|seen| **seen).count(),
            usize::try_from(native_frame["nonzero_pixels"].as_u64().unwrap()).unwrap()
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&reconstructed_frame)),
            native_frame["sha256"].as_str().unwrap(),
            "the compact sparse native samples must reconstruct the exact captured RGBA8 frame"
        );
        assert_eq!(paired_frame_sha, native_frame["sha256"].as_str().unwrap());

        let points: Vec<[f32; 2]> = {
            let bytes = fixture_bytes(
                source["source_arrays"]["points"]["data_hex"]
                    .as_str()
                    .unwrap(),
            );
            let (pairs, remainder) = bytes.as_chunks::<8>();
            assert!(remainder.is_empty());
            pairs
                .iter()
                .map(|pair| {
                    [
                        f32::from_le_bytes([pair[0], pair[1], pair[2], pair[3]]),
                        f32::from_le_bytes([pair[4], pair[5], pair[6], pair[7]]),
                    ]
                })
                .collect()
        };
        let pressure = fixture_f32s(source, "pressures");
        let timestamp_bytes = fixture_bytes(
            source["source_arrays"]["timestamps"]["data_hex"]
                .as_str()
                .unwrap(),
        );
        let (timestamp_words, remainder) = timestamp_bytes.as_chunks::<4>();
        assert!(remainder.is_empty());
        let times: Vec<i32> = timestamp_words
            .iter()
            .map(|bytes| i32::from_le_bytes(*bytes))
            .collect();
        let size_bytes = fixture_bytes(source["size_f32_bits"].as_str().unwrap());
        let (size_words, remainder) = size_bytes.as_chunks::<4>();
        assert!(remainder.is_empty() && size_words.len() == 1);
        let size = f32::from_bits(u32::from_le_bytes(size_words[0]));
        let model = crate::ink::chinesebrush_glv123::run_model_bounded(
            &points, &pressure, &times, size, 3, 1920, 1080, 2, true, 1024,
        )
        .unwrap();
        let mut model_stream = Vec::with_capacity(model.len() * 16);
        for [x, y, radius, phase, scalar] in model {
            for value in [
                x,
                y,
                radius,
                crate::ink::chinesebrush_glv123::packed_w(phase, scalar),
            ] {
                model_stream.extend_from_slice(&value.to_le_bytes());
            }
        }
        assert_eq!(
            model_stream, native_stream,
            "bounded Rust model matches the paired native AddPoint Vector4 stream"
        );

        let mut prepared = Vec::new();
        for (setting, stored_version) in [("3;", 3), ("4;", 4)] {
            let case = &fixture["cases"][setting];
            assert_eq!(case["stored_pen_getter_version"], stored_version);
            assert_eq!(case["selected_drawable_profile_version"], 3);
            assert_eq!(case["canvas_dimensions"], serde_json::json!([1920, 1080]));
            let stroke = capture_stroke(source, setting);
            assert_eq!(
                stroke
                    .rendering
                    .as_ref()
                    .unwrap()
                    .style
                    .pen_size
                    .unwrap()
                    .to_bits(),
                0x4049_374c
            );
            prepared.push(
                prepare(&stroke, profile, None)
                    .expect("captured saved setting routes through bounded GLV3 paint"),
            );
        }
        assert_eq!(prepared[0].stamps.len(), 5);
        assert_eq!(
            stamp_fingerprint(&prepared[0]),
            stamp_fingerprint(&prepared[1])
        );
        assert_eq!(prepared[0].sample_ends, prepared[1].sample_ends);
        assert_eq!(prepared[0].tiles, prepared[1].tiles);
        assert!(prepared[0].fringe_masks.is_empty() && prepared[1].fringe_masks.is_empty());
        assert!(prepare(&capture_stroke(source, "5;"), profile, None).is_none());
    }

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

    #[test]
    fn fwidth_definition_budget_bounds_unique_masks_before_emission() {
        let tiles = vec![(0, vec![0; 48 * 48 * 4])];
        let one_mask = estimated_markup_bytes(33, &tiles, 1).unwrap();
        let two_masks = estimated_markup_bytes(33, &tiles, 2).unwrap();
        assert_eq!(two_masks - one_mask, 64 * 4_096);
        let many_unique_masks = estimated_markup_bytes(33, &tiles, 129).unwrap();
        assert!(many_unique_masks > MAX_ESTIMATED_MARKUP_BYTES);
        assert!(estimated_markup_bytes(usize::MAX, &tiles, 1).is_none());
    }
}

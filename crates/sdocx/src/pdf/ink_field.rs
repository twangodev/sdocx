//! PDF-native transport for registered ink coverage and color fields.
//!
//! The SVG renderer emits an opaque, unique image marker at the exact paint
//! position. After Krilla serializes the page, this module replaces only those
//! marker Image XObjects with Forms that evaluate the retained field.

use crate::{BoundingBox, CoverageMesh};
use i_float::adapter::FloatPointAdapter;
use i_overlay::{
    core::{fill_rule::FillRule, overlay::ShapeType, overlay_rule::OverlayRule},
    float::overlay::{FloatOverlay, OverlayOptions},
};
use krilla::{
    Data,
    geom::{Size, Transform},
    image::Image,
    surface::Surface,
};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};

#[path = "ink_field/radial.rs"]
mod radial;
#[path = "ink_field/radial_circles.rs"]
pub(crate) mod radial_circles;

pub(crate) use radial::{PdfRadialCoverage, PdfRadialRecord};
pub(crate) use radial_circles::PdfRadialCircleRecord;

/// Validate a native radial payload before it is retained by the page
/// registry. This performs all normalization and Type 4 size checks without
/// mutating a PDF document.
pub(crate) fn validate_radial(
    records: &[PdfRadialRecord],
    saved_alpha: f32,
    coverage: PdfRadialCoverage,
    bounds: BoundingBox,
) -> Result<(), String> {
    radial::compile(
        records,
        [bounds.x_min, bounds.y_min, bounds.x_max, bounds.y_max],
        saved_alpha,
        coverage,
    )
    .map(|_| ())
}

/// Native coverage field retained alongside its unique in-stream marker.
pub(crate) struct PdfInkField<'a> {
    pub marker_id: u64,
    pub bounds: BoundingBox,
    pub payload: PdfInkPayload<'a>,
}

pub(crate) enum PdfInkPayload<'a> {
    Mesh {
        mesh: &'a CoverageMesh,
        color: [u8; 3],
    },
    Radial {
        records: &'a [PdfRadialRecord],
        saved_alpha: f32,
        coverage: PdfRadialCoverage,
    },
    RadialCircles {
        records: &'a [PdfRadialCircleRecord],
        saved_alpha: f32,
    },
}

impl<'a> PdfInkField<'a> {
    pub(crate) fn mesh(
        marker_id: u64,
        mesh: &'a CoverageMesh,
        color: [u8; 3],
        bounds: BoundingBox,
    ) -> Self {
        Self {
            marker_id,
            bounds,
            payload: PdfInkPayload::Mesh { mesh, color },
        }
    }

    pub(crate) fn radial(
        marker_id: u64,
        records: &'a [PdfRadialRecord],
        saved_alpha: f32,
        coverage: PdfRadialCoverage,
        bounds: BoundingBox,
    ) -> Self {
        Self {
            marker_id,
            bounds,
            payload: PdfInkPayload::Radial {
                records,
                saved_alpha,
                coverage,
            },
        }
    }

    pub(crate) fn radial_circles(
        marker_id: u64,
        records: &'a [PdfRadialCircleRecord],
        saved_alpha: f32,
        bounds: BoundingBox,
    ) -> Self {
        Self {
            marker_id,
            bounds,
            payload: PdfInkPayload::RadialCircles {
                records,
                saved_alpha,
            },
        }
    }
}

const MARKER_WIDTH: u32 = 8;
const MARKER_HEIGHT: u32 = 1;
const MAX_FIELD_TRIANGLES: usize = 16_384;
const MAX_COVERAGE_LEVELS: usize = 255;
const MAX_CONTOUR_WORK: usize = 100_000_000;
const MAX_BBOX_COMPARISONS: usize = 10_000_000;
const MAX_OVERLAY_STAGING_BYTES: usize = 128 * 1024 * 1024;
const MAX_MASK_STREAM_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug)]
enum ContourFailure {
    LegacyRequired(String),
    Invalid(String),
}

impl From<String> for ContourFailure {
    fn from(message: String) -> Self {
        Self::Invalid(message)
    }
}

impl From<&'static str> for ContourFailure {
    fn from(message: &'static str) -> Self {
        Self::Invalid(message.into())
    }
}

#[derive(Clone, Copy)]
struct ContourLimits {
    max_work: usize,
    max_bbox_comparisons: usize,
    max_staging_bytes: usize,
    max_stream_bytes: usize,
}

impl Default for ContourLimits {
    fn default() -> Self {
        Self {
            max_work: MAX_CONTOUR_WORK,
            max_bbox_comparisons: MAX_BBOX_COMPARISONS,
            max_staging_bytes: MAX_OVERLAY_STAGING_BYTES,
            max_stream_bytes: MAX_MASK_STREAM_BYTES,
        }
    }
}

#[derive(Default)]
struct ContourWork {
    limits: ContourLimits,
    work: usize,
    bbox_comparisons: usize,
}

impl ContourWork {
    fn charge(&mut self, amount: usize, reason: &str) -> Result<(), ContourFailure> {
        self.work = self.work.checked_add(amount).ok_or_else(|| {
            ContourFailure::LegacyRequired("native ink contour work counter overflow".into())
        })?;
        if self.work > self.limits.max_work {
            return Err(ContourFailure::LegacyRequired(format!(
                "native ink contour work budget exceeded ({reason})"
            )));
        }
        Ok(())
    }

    fn bbox_comparison(&mut self) -> Result<(), ContourFailure> {
        self.bbox_comparisons = self.bbox_comparisons.checked_add(1).ok_or_else(|| {
            ContourFailure::LegacyRequired("native ink bbox comparison counter overflow".into())
        })?;
        if self.bbox_comparisons > self.limits.max_bbox_comparisons {
            return Err(ContourFailure::LegacyRequired(
                "native ink bbox comparison budget exceeded".into(),
            ));
        }
        self.charge(1, "bbox comparisons")
    }
}

fn marker_pixels(marker_id: u64) -> Vec<u8> {
    let mut payload = Vec::with_capacity(24);
    payload.extend_from_slice(b"SDOCXIF1");
    payload.extend_from_slice(&marker_id.to_le_bytes());
    payload.extend_from_slice(
        &marker_id
            .rotate_left(29)
            .wrapping_add(0x9e37_79b9_7f4a_7c15)
            .to_le_bytes(),
    );
    payload
}

fn marker_id_from_image(stream: &Stream) -> Option<u64> {
    if stream.dict.get(b"Subtype").ok()?.as_name().ok()? != b"Image"
        || stream.dict.get(b"Width").ok()?.as_i64().ok()? != i64::from(MARKER_WIDTH)
        || stream.dict.get(b"Height").ok()?.as_i64().ok()? != i64::from(MARKER_HEIGHT)
        || stream.dict.get(b"ColorSpace").ok()?.as_name().ok()? != b"DeviceRGB"
        || stream.dict.get(b"BitsPerComponent").ok()?.as_i64().ok()? != 8
        || stream.dict.has(b"SMask")
    {
        return None;
    }

    let payload = stream.decompressed_content().ok()?;
    if payload.len() != 24 || &payload[..8] != b"SDOCXIF1" {
        return None;
    }
    let marker_id = u64::from_le_bytes(payload[8..16].try_into().ok()?);
    (payload[16..24]
        == marker_id
            .rotate_left(29)
            .wrapping_add(0x9e37_79b9_7f4a_7c15)
            .to_le_bytes())
    .then_some(marker_id)
}

fn marker_png(marker_id: u64) -> Result<Vec<u8>, String> {
    let pixels = marker_pixels(marker_id);
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, MARKER_WIDTH, MARKER_HEIGHT);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
        writer
            .write_image_data(&pixels)
            .map_err(|error| error.to_string())?;
    }
    Ok(png)
}

/// Emit a unique opaque marker image in local source coordinates. The caller
/// has already applied the registered SVG group's transform, clip and opacity.
pub(super) fn emit_marker(
    surface: &mut Surface<'_>,
    marker_id: u64,
    bounds: BoundingBox,
) -> Result<(), String> {
    let width = (bounds.x_max - bounds.x_min) as f32;
    let height = (bounds.y_max - bounds.y_min) as f32;
    if ![bounds.x_min, bounds.y_min, bounds.x_max, bounds.y_max]
        .into_iter()
        .all(f64::is_finite)
        || !width.is_finite()
        || !height.is_finite()
        || width <= 0.0
        || height <= 0.0
    {
        return Err("native ink field has invalid marker bounds".into());
    }
    let image = Image::from_png(Data::from(marker_png(marker_id)?), false)?;
    let size = Size::from_wh(width, height).ok_or("native ink marker size is invalid")?;
    surface.push_transform(&Transform::from_translate(
        bounds.x_min as f32,
        bounds.y_min as f32,
    ));
    surface.draw_image(image, size);
    surface.pop();
    Ok(())
}

#[derive(Clone, Copy)]
struct Triangle([[f64; 3]; 3]);

fn triangles(mesh: &CoverageMesh) -> Result<Vec<Triangle>, String> {
    mesh.triangles
        .iter()
        .map(|triangle| {
            let vertices = triangle.vertices.map(|vertex| {
                [
                    f64::from(vertex.x),
                    f64::from(vertex.y),
                    f64::from(vertex.coverage),
                ]
            });
            if vertices.iter().flatten().all(|value| value.is_finite()) {
                Ok(Triangle(vertices))
            } else {
                Err("native ink mesh contains non-finite vertices".into())
            }
        })
        .collect()
}

fn fmt(value: f64) -> String {
    if value == 0.0 {
        return "0".into();
    }
    // Rust's Display formatter emits the shortest round-tripping decimal
    // without exponent notation, preserving small normalized coordinates and
    // exact floating-point gray values in PDF numeric operands.
    value.to_string()
}

fn plane(triangle: Triangle) -> (f64, f64, f64, f64) {
    let [[x0, y0, z0], [x1, y1, z1], [x2, y2, z2]] = triangle.0;
    let determinant = (x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0);
    let a = ((z1 - z0) * (y2 - y0) - (z2 - z0) * (y1 - y0)) / determinant;
    let b = ((x1 - x0) * (z2 - z0) - (x2 - x0) * (z1 - z0)) / determinant;
    (a, b, z0 - a * x0 - b * y0, determinant)
}

fn edge_intersection(a: [f64; 3], b: [f64; 3], level: f64) -> [f64; 2] {
    // Shared source edges can be visited in opposite directions by
    // neighboring triangles. Canonicalize endpoint order to make their
    // mathematical intersection bit-identical.
    let order = a[0]
        .total_cmp(&b[0])
        .then_with(|| a[1].total_cmp(&b[1]))
        .then_with(|| a[2].total_cmp(&b[2]));
    let (start, end) = if order.is_le() { (a, b) } else { (b, a) };
    let t = (level - start[2]) / (end[2] - start[2]);
    [
        start[0] + t * (end[0] - start[0]),
        start[1] + t * (end[1] - start[1]),
    ]
}

fn clip_triangle(triangle: Triangle, level: f64) -> Result<Option<Vec<[f64; 2]>>, String> {
    let input = triangle.0.to_vec();
    let mut output = Vec::with_capacity(4);
    for index in 0..input.len() {
        let a = input[index];
        let b = input[(index + 1) % input.len()];
        let a_inside = a[2] >= level;
        let b_inside = b[2] >= level;
        if a_inside {
            output.push([a[0], a[1]]);
        }
        if a_inside != b_inside {
            output.push(edge_intersection(a, b, level));
        }
    }
    if output.len() < 3 {
        return Ok(None);
    }
    // Translate to the first point before cross products. Shoelace terms in
    // document coordinates can cancel away a small but positive contour.
    let origin = output[0];
    let area = (1..output.len() - 1)
        .map(|index| {
            let a = [output[index][0] - origin[0], output[index][1] - origin[1]];
            let b = [
                output[index + 1][0] - origin[0],
                output[index + 1][1] - origin[1],
            ];
            a[0] * b[1] - a[1] * b[0]
        })
        .sum::<f64>();
    if area == 0.0 {
        return Ok(None);
    }
    if !area.is_finite() {
        return Err("native ink clipped polygon has non-finite area".into());
    }
    if area < 0.0 {
        output.reverse();
    }
    Ok(Some(output))
}

fn union_polygons(
    polygons: &[Vec<[f64; 2]>],
    _bounds: BoundingBox,
    work: &mut ContourWork,
) -> Result<Vec<Vec<Vec<[f64; 2]>>>, ContourFailure> {
    if polygons.is_empty() {
        return Ok(Vec::new());
    }
    let point_count = polygons.iter().try_fold(0usize, |count, polygon| {
        count.checked_add(polygon.len()).ok_or_else(|| {
            ContourFailure::LegacyRequired("native ink overlay point count overflow".into())
        })
    })?;
    let staging_bytes = point_count
        .checked_mul(std::mem::size_of::<[f64; 2]>())
        .and_then(|bytes| {
            polygons
                .len()
                .checked_mul(std::mem::size_of::<Vec<[f64; 2]>>())
                .and_then(|headers| bytes.checked_add(headers))
        })
        .ok_or_else(|| {
            ContourFailure::LegacyRequired("native ink overlay staging size overflow".into())
        })?;
    if staging_bytes > work.limits.max_staging_bytes {
        return Err(ContourFailure::LegacyRequired(
            "native ink threshold region exceeds overlay staging budget".into(),
        ));
    }

    // Union only polygons whose bounding boxes touch or overlap. Disjoint
    // components need no shared overlay grid, so one remote island cannot
    // erase precision for a nearby small one.
    let mut order: Vec<usize> = (0..polygons.len()).collect();
    let boxes: Vec<[f64; 4]> = polygons
        .iter()
        .map(|poly| {
            poly.iter().fold(
                [
                    f64::INFINITY,
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    f64::NEG_INFINITY,
                ],
                |mut b, [x, y]| {
                    b[0] = b[0].min(*x);
                    b[1] = b[1].min(*y);
                    b[2] = b[2].max(*x);
                    b[3] = b[3].max(*y);
                    b
                },
            )
        })
        .collect();
    let sort_work = order
        .len()
        .checked_mul(order.len().max(2).ilog2() as usize)
        .ok_or_else(|| {
            ContourFailure::LegacyRequired("native ink contour sort-work overflow".into())
        })?;
    work.charge(sort_work, "bbox sorting")?;
    order.sort_by(|a, b| boxes[*a][0].total_cmp(&boxes[*b][0]));
    let mut parent: Vec<usize> = (0..polygons.len()).collect();
    fn root(parent: &mut [usize], mut n: usize) -> usize {
        while parent[n] != n {
            parent[n] = parent[parent[n]];
            n = parent[n];
        }
        n
    }
    for oi in 0..order.len() {
        let i = order[oi];
        for &j in &order[oi + 1..] {
            work.bbox_comparison()?;
            if boxes[j][0] > boxes[i][2] {
                break;
            }
            if boxes[i][1] <= boxes[j][3] && boxes[j][1] <= boxes[i][3] {
                let ri = root(&mut parent, i);
                let rj = root(&mut parent, j);
                if ri != rj {
                    parent[rj] = ri;
                }
            }
        }
    }
    let mut components: std::collections::BTreeMap<usize, Vec<usize>> =
        std::collections::BTreeMap::new();
    for i in 0..polygons.len() {
        let r = root(&mut parent, i);
        components.entry(r).or_default().push(i);
    }
    let mut result = Vec::new();
    for indices in components.values() {
        let origin_x = indices
            .iter()
            .map(|i| boxes[*i][0])
            .fold(f64::INFINITY, f64::min);
        let origin_y = indices
            .iter()
            .map(|i| boxes[*i][1])
            .fold(f64::INFINITY, f64::min);
        let max_x = indices
            .iter()
            .map(|i| boxes[*i][2])
            .fold(f64::NEG_INFINITY, f64::max);
        let max_y = indices
            .iter()
            .map(|i| boxes[*i][3])
            .fold(f64::NEG_INFINITY, f64::max);
        let extent = (max_x - origin_x).max(max_y - origin_y);
        if !extent.is_finite() || extent <= 0.0 {
            return Err(ContourFailure::LegacyRequired(
                "native ink component extent is not representable".into(),
            ));
        }
        let scale = ((1u64 << 58) as f64 / extent).min(f64::MAX);
        if !scale.is_finite() || scale <= 0.0 {
            return Err(ContourFailure::LegacyRequired(
                "native ink component cannot be represented by the polygon overlay".into(),
            ));
        }
        let local: Vec<Vec<[f64; 2]>> = indices
            .iter()
            .map(|i| {
                polygons[*i]
                    .iter()
                    .map(|[x, y]| [x - origin_x, y - origin_y])
                    .collect()
            })
            .collect();
        let component_point_count = local.iter().try_fold(0usize, |count, polygon| {
            count.checked_add(polygon.len()).ok_or_else(|| {
                ContourFailure::LegacyRequired("native ink component point count overflow".into())
            })
        })?;
        let overlay_work = component_point_count
            .checked_mul(component_point_count)
            .ok_or_else(|| {
                ContourFailure::LegacyRequired("native ink overlay-work estimate overflow".into())
            })?;
        work.charge(overlay_work, "overlay component estimate")?;
        let points = local.iter().flat_map(|poly| poly.iter());
        let adapter = FloatPointAdapter::try_with_iter_and_scale_conservative(points, scale)
            .map_err(|error| {
                ContourFailure::LegacyRequired(format!(
                    "native ink polygon overlay adapter failed: {error:?}"
                ))
            })?;
        for (polygon, &source_index) in local.iter().zip(indices) {
            // Check the full world -> component-local -> integer-grid -> world
            // round trip against each source polygon's own spans. A component
            // can join shapes whose boxes merely touch; subtracting a remote
            // origin or scaling by its extent can otherwise move a contour
            // while it remains non-degenerate on the integer grid.
            let source_polygon = &polygons[source_index];
            let (min_x, min_y, max_x, max_y) = source_polygon.iter().fold(
                (
                    f64::INFINITY,
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    f64::NEG_INFINITY,
                ),
                |(min_x, min_y, max_x, max_y), [x, y]| {
                    (min_x.min(*x), min_y.min(*y), max_x.max(*x), max_y.max(*y))
                },
            );
            let spans = [max_x - min_x, max_y - min_y];
            let mut restored_polygon = Vec::with_capacity(polygon.len());
            for (local_point, source_point) in polygon.iter().zip(source_polygon) {
                let source = [source_point[0], source_point[1]];
                let origin = [origin_x, origin_y];
                for axis in 0..2 {
                    let allowed_error = 4096.0 * f64::EPSILON * spans[axis]
                        + 8.0 * f64::EPSILON * source[axis].abs();
                    let half_grid_step = 0.5 * adapter.inv_scale().abs();
                    if spans[axis] > 0.0 && half_grid_step > allowed_error {
                        return Err(ContourFailure::LegacyRequired(format!(
                            "native ink overlay grid is coarser than local precision budget (axis={axis}, half_step={half_grid_step}, allowed={allowed_error})"
                        )));
                    }
                }
                let integer = adapter.try_float_to_int(local_point).map_err(|error| {
                    ContourFailure::LegacyRequired(format!(
                        "native ink polygon coordinate is outside the overlay grid: {error:?}"
                    ))
                })?;
                let restored_local = adapter.try_int_to_float(&integer).map_err(|error| {
                    ContourFailure::LegacyRequired(format!(
                        "native ink polygon round trip is outside the overlay grid: {error:?}"
                    ))
                })?;
                for axis in 0..2 {
                    let allowed_error = 4096.0 * f64::EPSILON * spans[axis]
                        + 8.0 * f64::EPSILON * source[axis].abs();
                    let restored_world = restored_local[axis] + origin[axis];
                    let error = (restored_world - source[axis]).abs();
                    if !error.is_finite() || error > allowed_error {
                        return Err(ContourFailure::LegacyRequired(format!(
                            "native ink polygon loses precision on world-to-grid round trip (axis={axis}, error={error}, allowed={allowed_error})"
                        )));
                    }
                }
                restored_polygon
                    .push([restored_local[0] + origin[0], restored_local[1] + origin[1]]);
            }
            let signed_twice_area = |ring: &[[f64; 2]]| {
                let origin = ring[0];
                (1..ring.len() - 1)
                    .map(|index| {
                        let a = [ring[index][0] - origin[0], ring[index][1] - origin[1]];
                        let b = [
                            ring[index + 1][0] - origin[0],
                            ring[index + 1][1] - origin[1],
                        ];
                        a[0] * b[1] - a[1] * b[0]
                    })
                    .sum::<f64>()
            };
            let original_area = signed_twice_area(source_polygon);
            let restored_area = signed_twice_area(&restored_polygon);
            if !original_area.is_finite()
                || !restored_area.is_finite()
                || original_area == 0.0
                || restored_area == 0.0
                || (restored_area - original_area).abs()
                    > 4096.0 * f64::EPSILON * original_area.abs()
            {
                return Err(ContourFailure::LegacyRequired(
                    "native ink polygon loses relative area precision on overlay round trip".into(),
                ));
            }
            let snapped = polygon
                .iter()
                .map(|point| {
                    adapter.try_float_to_int(point).map_err(|error| {
                        ContourFailure::LegacyRequired(format!(
                            "native ink polygon coordinate is outside the overlay grid: {error:?}"
                        ))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let origin = snapped[0];
            let twice_area = snapped
                .iter()
                .skip(1)
                .take(snapped.len().saturating_sub(2))
                .enumerate()
                .map(|(k, p)| {
                    let a = [i128::from(p.x - origin.x), i128::from(p.y - origin.y)];
                    let q = snapped[k + 2];
                    let b = [i128::from(q.x - origin.x), i128::from(q.y - origin.y)];
                    a[0] * b[1] - a[1] * b[0]
                })
                .sum::<i128>();
            if twice_area == 0 {
                return Err(ContourFailure::LegacyRequired(
                    "native ink polygon collapses at the overlay grid resolution".into(),
                ));
            }
        }
        let mut options = OverlayOptions::<f64, i64>::default();
        options.preserve_input_collinear = false;
        options.preserve_output_collinear = false;
        options.min_output_area = 0.0;
        options.clean_result = false;
        let mut overlay = FloatOverlay::<[f64; 2], i64>::new_custom(
            adapter,
            options,
            Default::default(),
            component_point_count,
        )
        .unsafe_add_source(&local, ShapeType::Subject);
        let component = overlay.overlay(OverlayRule::Subject, FillRule::NonZero);
        result.extend(component.into_iter().map(|shape| {
            shape
                .into_iter()
                .map(|ring| {
                    ring.into_iter()
                        .map(|[x, y]| [x + origin_x, y + origin_y])
                        .collect()
                })
                .collect()
        }));
    }
    Ok(result)
}

fn append_shape_paths(
    chunk: &mut String,
    shape: &[Vec<[f64; 2]>],
    bounds: BoundingBox,
    already_written: usize,
    work: &mut ContourWork,
) -> Result<(), ContourFailure> {
    let max_bytes = work.limits.max_stream_bytes;
    let points = shape.iter().flat_map(|ring| ring.iter());
    let (min_x, min_y, max_x, max_y) = points.fold(
        (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ),
        |(min_x, min_y, max_x, max_y), [x, y]| {
            (min_x.min(*x), min_y.min(*y), max_x.max(*x), max_y.max(*y))
        },
    );
    let field_width = bounds.x_max - bounds.x_min;
    let field_height = bounds.y_max - bounds.y_min;
    let shape_width = max_x - min_x;
    let shape_height = max_y - min_y;
    if ![field_width, field_height, shape_width, shape_height]
        .iter()
        .all(|v| v.is_finite())
        || field_width <= 0.0
        || field_height <= 0.0
        || shape_width <= 0.0
        || shape_height <= 0.0
    {
        return Err(ContourFailure::LegacyRequired(
            "native ink overlay produced an unrepresentable shape extent".into(),
        ));
    }
    let sx = shape_width / field_width;
    let sy = shape_height / field_height;
    let tx = (min_x - bounds.x_min) / field_width;
    let ty = 1.0 - (max_y - bounds.y_min) / field_height;
    let transform = format!(
        "q\n{} 0 0 {} {} {} cm\n",
        fmt(sx),
        fmt(sy),
        fmt(tx),
        fmt(ty)
    );
    if already_written
        .checked_add(chunk.len())
        .and_then(|n| n.checked_add(transform.len()))
        .is_none_or(|n| n > max_bytes)
    {
        return Err("native ink field exceeds vector mask stream budget".into());
    }
    chunk.push_str(&transform);
    for contour in shape {
        if contour.len() < 3 {
            return Err(ContourFailure::LegacyRequired(
                "native ink overlay produced a degenerate contour".into(),
            ));
        }
        let origin = contour[0];
        let original_area = (1..contour.len() - 1)
            .map(|i| {
                let a = [contour[i][0] - origin[0], contour[i][1] - origin[1]];
                let b = [contour[i + 1][0] - origin[0], contour[i + 1][1] - origin[1]];
                a[0] * b[1] - a[1] * b[0]
            })
            .sum::<f64>();
        if !original_area.is_finite() {
            return Err(ContourFailure::LegacyRequired(
                "native ink contour area exceeds floating-point precision".into(),
            ));
        }
        if original_area == 0.0 {
            continue;
        }
        let mut serialized = Vec::with_capacity(contour.len());
        for [x, y] in contour {
            let u = (x - min_x) / shape_width;
            let v = (max_y - y) / shape_height;
            if !u.is_finite() || !v.is_finite() {
                return Err("native ink contour contains non-finite coordinates".into());
            }
            serialized.push([
                fmt(u).parse::<f64>().map_err(|e| e.to_string())?,
                fmt(v).parse::<f64>().map_err(|e| e.to_string())?,
            ]);
        }
        let origin = serialized[0];
        let area = (1..serialized.len() - 1)
            .map(|i| {
                let a = [serialized[i][0] - origin[0], serialized[i][1] - origin[1]];
                let b = [
                    serialized[i + 1][0] - origin[0],
                    serialized[i + 1][1] - origin[1],
                ];
                a[0] * b[1] - a[1] * b[0]
            })
            .sum::<f64>();
        if !area.is_finite() || area == 0.0 {
            return Err(ContourFailure::LegacyRequired(format!(
                "native ink contour collapses at local PDF coordinate precision; shape=({min_x},{min_y},{max_x},{max_y}); contour={contour:?}; local={serialized:?}"
            )));
        }
        let global: Vec<[f64; 2]> = serialized
            .iter()
            .map(|[u, v]| [sx * u + tx, sy * v + ty])
            .collect();
        for index in 0..global.len() {
            let next = (index + 1) % global.len();
            if serialized[index] != serialized[next] && global[index] == global[next] {
                return Err(ContourFailure::LegacyRequired(
                    "native ink contour collapses after field normalization".into(),
                ));
            }
        }
        work.charge(serialized.len(), "serialized contour vertices")?;
        for (index, [u, v]) in serialized.into_iter().enumerate() {
            let segment = format!(
                "{} {} {}\n",
                fmt(u),
                fmt(v),
                if index == 0 { "m" } else { "l" }
            );
            if already_written
                .checked_add(chunk.len())
                .and_then(|n| n.checked_add(segment.len()))
                .is_none_or(|n| n > max_bytes)
            {
                return Err(ContourFailure::LegacyRequired(
                    "native ink field exceeds vector mask stream budget".into(),
                ));
            }
            chunk.push_str(&segment);
        }
        if already_written
            .checked_add(chunk.len())
            .and_then(|n| n.checked_add(2))
            .is_none_or(|n| n > max_bytes)
        {
            return Err(ContourFailure::LegacyRequired(
                "native ink field exceeds vector mask stream budget".into(),
            ));
        }
        chunk.push_str("h\n");
    }
    chunk.push_str("f\nQ\n");
    Ok(())
}

fn isocontour_stream_with_limits(
    triangles: &[Triangle],
    bounds: BoundingBox,
    limits: ContourLimits,
) -> Result<Vec<u8>, ContourFailure> {
    // Emit round-to-nearest thresholds for the 255 nonzero values of a
    // 256-value coverage scale; the first threshold is 0.5/255 and the last
    // is 254.5/255, so full coverage still reaches exactly 1.
    const LEVEL_INTERVALS: usize = MAX_COVERAGE_LEVELS;
    // Every supported source triangle may intersect every nonzero threshold.
    // This cap therefore does not reject any mesh admitted by MAX_FIELD_TRIANGLES.
    const MAX_CLIPPED_POLYGONS: usize = MAX_FIELD_TRIANGLES * MAX_COVERAGE_LEVELS;
    let mut stream = Vec::new();
    let mut clipped_count = 0usize;
    let mut work = ContourWork {
        limits,
        work: 0,
        bbox_comparisons: 0,
    };
    for index in 1..=LEVEL_INTERVALS {
        let threshold = (index as f64 - 0.5) / LEVEL_INTERVALS as f64;
        let gray = index as f64 / LEVEL_INTERVALS as f64;
        let mut polygons = Vec::new();
        for triangle in triangles {
            work.charge(1, "source triangle scan")?;
            if plane(*triangle).3 == 0.0
                || triangle
                    .0
                    .iter()
                    .map(|vertex| vertex[2])
                    .fold(f64::NEG_INFINITY, f64::max)
                    < threshold
            {
                continue;
            }
            if let Some(polygon) =
                clip_triangle(*triangle, threshold).map_err(ContourFailure::Invalid)?
            {
                clipped_count = clipped_count
                    .checked_add(1)
                    .ok_or("native ink clipped polygon count overflow")?;
                if clipped_count > MAX_CLIPPED_POLYGONS {
                    return Err(ContourFailure::LegacyRequired(
                        "native ink field exceeds clipped polygon budget".into(),
                    ));
                }
                work.charge(polygon.len(), "clipped polygon vertices")?;
                polygons.push(polygon);
            }
        }
        if polygons.is_empty() {
            continue;
        }
        let shapes = union_polygons(&polygons, bounds, &mut work)?;
        if shapes.is_empty() {
            return Err(ContourFailure::LegacyRequired(
                "native ink polygon union dropped a nonempty threshold region".into(),
            ));
        }
        let mut chunk = format!("{} g\n", fmt(gray));
        for shape in shapes {
            append_shape_paths(&mut chunk, &shape, bounds, stream.len(), &mut work)?;
        }
        if stream
            .len()
            .checked_add(chunk.len())
            .is_none_or(|size| size > work.limits.max_stream_bytes)
        {
            return Err(ContourFailure::LegacyRequired(
                "native ink field exceeds vector mask stream budget".into(),
            ));
        }
        stream.extend_from_slice(chunk.as_bytes());
    }
    // An empty luminosity-mask Form is a valid transparent field. Preserve
    // the supported input domain for zero and sub-1/255 coverage meshes.
    Ok(stream)
}

#[cfg(test)]
fn isocontour_stream(
    triangles: &[Triangle],
    bounds: BoundingBox,
) -> Result<Vec<u8>, ContourFailure> {
    isocontour_stream_with_limits(triangles, bounds, ContourLimits::default())
}

fn legacy_fmt(value: f64) -> String {
    let text = format!("{value:.17}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text.is_empty() || text == "-0" {
        "0".into()
    } else {
        text.into()
    }
}

fn edge_code(p: [f64; 2], q: [f64; 2], orientation: i32) -> String {
    format!(
        " {} sub {} mul exch {} sub {} mul sub {} mul 0 ge",
        legacy_fmt(p[1]),
        legacy_fmt(q[0] - p[0]),
        legacy_fmt(p[0]),
        legacy_fmt(q[1] - p[1]),
        orientation
    )
}

fn legacy_function_code(triangles: &[Triangle], bounds: BoundingBox) -> Result<Vec<u8>, String> {
    let width = bounds.x_max - bounds.x_min;
    let height = bounds.y_max - bounds.y_min;
    let mut code = format!(
        "{{ 1 exch sub {} mul {} add exch {} mul {} add exch 0",
        legacy_fmt(height),
        legacy_fmt(bounds.y_min),
        legacy_fmt(width),
        legacy_fmt(bounds.x_min)
    );
    for triangle in triangles {
        let [[x0, y0, _], [x1, y1, _], [x2, y2, _]] = triangle.0;
        let (a, b, c, determinant) = plane(*triangle);
        if determinant == 0.0 {
            continue;
        }
        if ![a, b, c].into_iter().all(f64::is_finite) {
            return Err("native ink mesh produced an invalid affine plane".into());
        }
        for [x, y, z] in triangle.0 {
            let evaluated = a * x + b * y + c;
            let magnitude = (a * x).abs() + (b * y).abs() + c.abs() + z.abs() + 1.0;
            let tolerance = 128.0 * f64::EPSILON * magnitude;
            if (evaluated - z).abs() > tolerance {
                return Err("native ink affine plane failed its vertex check".into());
            }
        }
        let xy = [[x0, y0], [x1, y1], [x2, y2]];
        let orientation = if determinant > 0.0 { 1 } else { -1 };
        code.push_str(" 3 copy pop");
        code.push_str(&edge_code(xy[0], xy[1], orientation));
        for edge in 1..3 {
            code.push_str(" 4 copy pop pop");
            code.push_str(&edge_code(xy[edge], xy[(edge + 1) % 3], orientation));
            code.push_str(" and");
        }
        code.push_str(&format!(
            " {{ 3 copy pop {} mul exch {} mul add {} add 2 copy lt {{ exch pop }} {{ pop }} ifelse }} if",
            legacy_fmt(b), legacy_fmt(a), legacy_fmt(c)
        ));
    }
    code.push_str(" exch pop exch pop }");
    Ok(code.into_bytes())
}

type LegacyTile = ([f64; 4], Vec<Triangle>);

fn legacy_tiles(triangles: &[Triangle], bounds: BoundingBox) -> Result<Vec<LegacyTile>, String> {
    let width = bounds.x_max - bounds.x_min;
    let height = bounds.y_max - bounds.y_min;
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return Err("native ink field has invalid bounds".into());
    }
    const MAX_AXIS_TILES: usize = 64;
    const MAX_TILES: usize = MAX_AXIS_TILES * MAX_AXIS_TILES;
    const MAX_CANDIDATE_REFERENCES: usize = 1_000_000;
    let step = (width.max(height) / MAX_AXIS_TILES as f64).max(4.0);
    if !step.is_finite() || step <= 0.0 {
        return Err("native ink field bounds cannot be divided into finite tiles".into());
    }
    let mut result = Vec::new();
    let mut candidate_references = 0usize;
    // Indexed boundaries avoid an accumulated rounding remainder creating a
    // 65th tile on an axis that should have exactly 64 subdivisions.
    let columns = ((width / step).ceil() as usize).clamp(1, MAX_AXIS_TILES);
    let rows = ((height / step).ceil() as usize).clamp(1, MAX_AXIS_TILES);
    for column in 0..columns {
        let x = bounds.x_min + column as f64 * step;
        let x1 = if column + 1 == columns {
            bounds.x_max
        } else {
            bounds.x_min + (column + 1) as f64 * step
        };
        if !x1.is_finite() || x1 <= x {
            return Err("native ink x tiling did not advance".into());
        }
        for row in 0..rows {
            let y = bounds.y_min + row as f64 * step;
            let y1 = if row + 1 == rows {
                bounds.y_max
            } else {
                bounds.y_min + (row + 1) as f64 * step
            };
            if !y1.is_finite() || y1 <= y {
                return Err("native ink y tiling did not advance".into());
            }
            let candidates = triangles
                .iter()
                .copied()
                .filter(|triangle| {
                    plane(*triangle).3 != 0.0 && {
                        let xs = triangle.0.map(|vertex| vertex[0]);
                        let ys = triangle.0.map(|vertex| vertex[1]);
                        xs.iter().copied().fold(f64::NEG_INFINITY, f64::max) >= x
                            && xs.iter().copied().fold(f64::INFINITY, f64::min) <= x1
                            && ys.iter().copied().fold(f64::NEG_INFINITY, f64::max) >= y
                            && ys.iter().copied().fold(f64::INFINITY, f64::min) <= y1
                    }
                })
                .collect::<Vec<_>>();
            if !candidates.is_empty() {
                candidate_references = candidate_references
                    .checked_add(candidates.len())
                    .ok_or("native ink candidate count overflow")?;
                if candidate_references > MAX_CANDIDATE_REFERENCES {
                    return Err("native ink field exceeds candidate reference budget".into());
                }
                let u0 = (x - bounds.x_min) / width;
                let u1 = (x1 - bounds.x_min) / width;
                let v0 = 1.0 - (y1 - bounds.y_min) / height;
                let v1 = 1.0 - (y - bounds.y_min) / height;
                result.push(([u0, v0, u1, v1], candidates));
                if result.len() > MAX_TILES {
                    return Err("native ink field exceeds tile budget".into());
                }
            }
        }
    }
    if result.is_empty() {
        return Err("native ink field has no non-degenerate coverage triangles".into());
    }
    Ok(result)
}

fn legacy_type1_field_form(
    document: &mut Document,
    mesh: &CoverageMesh,
    color: [u8; 3],
    bounds: BoundingBox,
) -> Result<Object, String> {
    let triangles = triangles(mesh)?;
    const MAX_TRIANGLES: usize = 16_384;
    if triangles.len() > MAX_TRIANGLES {
        return Err("native ink field exceeds triangle budget".into());
    }
    if triangles.is_empty() || triangles.iter().all(|triangle| plane(*triangle).3 == 0.0) {
        return Err("native ink field has no non-degenerate coverage triangles".into());
    }
    for triangle in &triangles {
        for [x, y, _] in triangle.0 {
            if x < bounds.x_min || x > bounds.x_max || y < bounds.y_min || y > bounds.y_max {
                return Err("native ink bounds do not contain every mesh vertex".into());
            }
        }
    }
    let tile_data = legacy_tiles(&triangles, bounds)?;
    let mut shadings = Dictionary::new();
    let mut operators = Vec::with_capacity(tile_data.len());
    let mut generated_code_bytes = 0usize;
    for (index, (bbox, candidates)) in tile_data.iter().enumerate() {
        let code = legacy_function_code(candidates, bounds)?;
        generated_code_bytes = generated_code_bytes
            .checked_add(code.len())
            .ok_or("native ink calculator size overflow")?;
        if generated_code_bytes > 32 * 1024 * 1024 {
            return Err("native ink field exceeds calculator byte budget".into());
        }
        let function = document.add_object(Stream::new(
            Dictionary::from_iter([
                (b"FunctionType".to_vec(), Object::Integer(4)),
                (
                    b"Domain".to_vec(),
                    Object::Array(vec![0.into(), 1.into(), 0.into(), 1.into()]),
                ),
                (b"Range".to_vec(), Object::Array(vec![0.into(), 1.into()])),
            ]),
            code,
        ));
        let shading = document.add_object(Dictionary::from_iter([
            (b"ShadingType".to_vec(), Object::Integer(1)),
            (b"ColorSpace".to_vec(), Object::Name(b"DeviceGray".to_vec())),
            (
                b"Domain".to_vec(),
                Object::Array(vec![
                    bbox[0].into(),
                    bbox[2].into(),
                    bbox[1].into(),
                    bbox[3].into(),
                ]),
            ),
            (
                b"BBox".to_vec(),
                Object::Array(
                    bbox.iter()
                        .map(|value| Object::Real(*value as f32))
                        .collect(),
                ),
            ),
            (b"Function".to_vec(), Object::Reference(function)),
        ]));
        shadings.set(format!("S{index}"), Object::Reference(shading));
        operators.push(format!("/S{index} sh"));
    }

    let mut group_resources = Dictionary::new();
    group_resources.set("Shading", shadings);
    let mut group_dict = Dictionary::new();
    group_dict.set("Type", "XObject");
    group_dict.set("Subtype", "Form");
    group_dict.set("FormType", 1);
    group_dict.set("BBox", vec![0.into(), 0.into(), 1.into(), 1.into()]);
    group_dict.set("Resources", group_resources);
    group_dict.set(
        "Group",
        Dictionary::from_iter([
            (b"S".to_vec(), Object::Name(b"Transparency".to_vec())),
            (b"CS".to_vec(), Object::Name(b"DeviceGray".to_vec())),
            (b"I".to_vec(), Object::Boolean(true)),
            (b"K".to_vec(), Object::Boolean(false)),
        ]),
    );
    let mask = document.add_object(Stream::new(group_dict, operators.join("\n").into_bytes()));
    let state = document.add_object(Dictionary::from_iter([(
        b"SMask".to_vec(),
        Object::Dictionary(Dictionary::from_iter([
            (b"S".to_vec(), Object::Name(b"Luminosity".to_vec())),
            (b"G".to_vec(), Object::Reference(mask)),
            (b"BC".to_vec(), Object::Array(vec![0.into()])),
            (b"TR".to_vec(), Object::Name(b"Identity".to_vec())),
        ])),
    )]));
    let mut states = Dictionary::new();
    states.set("GS", Object::Reference(state));
    let mut resources = Dictionary::new();
    resources.set("ExtGState", states);
    let mut dict = Dictionary::new();
    dict.set("Type", "XObject");
    dict.set("Subtype", "Form");
    dict.set("FormType", 1);
    dict.set("BBox", vec![0.into(), 0.into(), 1.into(), 1.into()]);
    dict.set("Resources", resources);
    Ok(Object::Stream(Stream::new(
        dict,
        format!(
            "/GS gs {} {} {} rg 0 0 1 1 re f",
            color[0] as f64 / 255.0,
            color[1] as f64 / 255.0,
            color[2] as f64 / 255.0
        )
        .into_bytes(),
    )))
}

fn field_form(document: &mut Document, field: &PdfInkField<'_>) -> Result<Object, String> {
    field_form_with_limits(document, field, ContourLimits::default())
}

fn field_form_with_limits(
    document: &mut Document,
    field: &PdfInkField<'_>,
    limits: ContourLimits,
) -> Result<Object, String> {
    match &field.payload {
        PdfInkPayload::Mesh { mesh, color } => {
            mesh_field_form_with_limits(document, mesh, *color, field.bounds, limits)
        }
        PdfInkPayload::Radial {
            records,
            saved_alpha,
            coverage,
        } => radial_field_form(document, records, *saved_alpha, *coverage, field.bounds),
        PdfInkPayload::RadialCircles {
            records,
            saved_alpha,
        } => radial_circles_field_form(document, records, *saved_alpha, field.bounds),
    }
}

fn radial_circles_field_form(
    document: &mut Document,
    records: &[PdfRadialCircleRecord],
    saved_alpha: f32,
    bounds: BoundingBox,
) -> Result<Object, String> {
    // Compile the entire form before adding objects; unsupported input never
    // leaves a partially populated PDF object graph.
    let program = radial_circles::compile(
        records,
        [bounds.x_min, bounds.y_min, bounds.x_max, bounds.y_max],
        saved_alpha,
    )?;
    let mut states = Dictionary::new();
    let state = document.add_object(Dictionary::from_iter([(
        b"ca".to_vec(),
        Object::Real(program.alpha),
    )]));
    states.set("GS0", Object::Reference(state));
    let mut resources = Dictionary::new();
    resources.set("ExtGState", states);
    Ok(Object::Stream(Stream::new(
        Dictionary::from_iter([
            (b"Type".to_vec(), Object::Name(b"XObject".to_vec())),
            (b"Subtype".to_vec(), Object::Name(b"Form".to_vec())),
            (b"FormType".to_vec(), Object::Integer(1)),
            (
                b"BBox".to_vec(),
                Object::Array(vec![0.into(), 0.into(), 1.into(), 1.into()]),
            ),
            (b"Resources".to_vec(), Object::Dictionary(resources)),
            (
                b"Group".to_vec(),
                Object::Dictionary(Dictionary::from_iter([
                    (b"S".to_vec(), Object::Name(b"Transparency".to_vec())),
                    (b"CS".to_vec(), Object::Name(b"DeviceRGB".to_vec())),
                    (b"I".to_vec(), Object::Boolean(true)),
                    (b"K".to_vec(), Object::Boolean(false)),
                ])),
            ),
        ]),
        program.content,
    )))
}

fn mesh_field_form_with_limits(
    document: &mut Document,
    mesh: &CoverageMesh,
    color: [u8; 3],
    bounds: BoundingBox,
    limits: ContourLimits,
) -> Result<Object, String> {
    let triangles = triangles(mesh)?;
    if triangles.len() > MAX_FIELD_TRIANGLES {
        return Err("native ink field exceeds triangle budget".into());
    }
    if triangles.is_empty() || triangles.iter().all(|triangle| plane(*triangle).3 == 0.0) {
        return Err("native ink field has no non-degenerate coverage triangles".into());
    }
    for triangle in &triangles {
        for [x, y, _] in triangle.0 {
            if x < bounds.x_min || x > bounds.x_max || y < bounds.y_min || y > bounds.y_max {
                return Err("native ink bounds do not contain every mesh vertex".into());
            }
        }
    }
    let mask_stream = match isocontour_stream_with_limits(&triangles, bounds, limits) {
        Ok(stream) => stream,
        Err(ContourFailure::LegacyRequired(_reason)) => {
            return legacy_type1_field_form(document, mesh, color, bounds);
        }
        Err(ContourFailure::Invalid(reason)) => return Err(reason),
    };

    let mut group_dict = Dictionary::new();
    group_dict.set("Type", "XObject");
    group_dict.set("Subtype", "Form");
    group_dict.set("FormType", 1);
    group_dict.set("BBox", vec![0.into(), 0.into(), 1.into(), 1.into()]);
    group_dict.set("Resources", Dictionary::new());
    group_dict.set(
        "Group",
        Dictionary::from_iter([
            (b"S".to_vec(), Object::Name(b"Transparency".to_vec())),
            (b"CS".to_vec(), Object::Name(b"DeviceGray".to_vec())),
            (b"I".to_vec(), Object::Boolean(true)),
            (b"K".to_vec(), Object::Boolean(false)),
        ]),
    );
    let mask = document.add_object(Stream::new(group_dict, mask_stream));
    let state = document.add_object(Dictionary::from_iter([(
        b"SMask".to_vec(),
        Object::Dictionary(Dictionary::from_iter([
            (b"S".to_vec(), Object::Name(b"Luminosity".to_vec())),
            (b"G".to_vec(), Object::Reference(mask)),
            (b"BC".to_vec(), Object::Array(vec![0.into()])),
            (b"TR".to_vec(), Object::Name(b"Identity".to_vec())),
        ])),
    )]));
    let mut states = Dictionary::new();
    states.set("GS", Object::Reference(state));
    let mut resources = Dictionary::new();
    resources.set("ExtGState", states);
    let mut dict = Dictionary::new();
    dict.set("Type", "XObject");
    dict.set("Subtype", "Form");
    dict.set("FormType", 1);
    dict.set("BBox", vec![0.into(), 0.into(), 1.into(), 1.into()]);
    dict.set("Resources", resources);
    Ok(Object::Stream(Stream::new(
        dict,
        format!(
            "/GS gs {} {} {} rg 0 0 1 1 re f",
            color[0] as f64 / 255.0,
            color[1] as f64 / 255.0,
            color[2] as f64 / 255.0
        )
        .into_bytes(),
    )))
}

fn radial_field_form(
    document: &mut Document,
    records: &[PdfRadialRecord],
    saved_alpha: f32,
    coverage: PdfRadialCoverage,
    bounds: BoundingBox,
) -> Result<Object, String> {
    // Compile and validate both programs before adding any objects. If this
    // field is not representable, the caller discards the in-memory PDF and
    // keeps the original serialized bytes unchanged.
    let program = radial::compile(
        records,
        [bounds.x_min, bounds.y_min, bounds.x_max, bounds.y_max],
        saved_alpha,
        coverage,
    )?;
    let function_domain = Object::Array(vec![0.into(), 1.into(), 0.into(), 1.into()]);
    let rgb_function = document.add_object(Stream::new(
        Dictionary::from_iter([
            (b"FunctionType".to_vec(), Object::Integer(4)),
            (b"Domain".to_vec(), function_domain.clone()),
            (
                b"Range".to_vec(),
                Object::Array(vec![
                    0.into(),
                    1.into(),
                    0.into(),
                    1.into(),
                    0.into(),
                    1.into(),
                ]),
            ),
        ]),
        program.rgb,
    ));
    let rgb_shading = document.add_object(Dictionary::from_iter([
        (b"ShadingType".to_vec(), Object::Integer(1)),
        (b"ColorSpace".to_vec(), Object::Name(b"DeviceRGB".to_vec())),
        (b"Domain".to_vec(), function_domain.clone()),
        (b"Function".to_vec(), Object::Reference(rgb_function)),
    ]));
    let mask_function = document.add_object(Stream::new(
        Dictionary::from_iter([
            (b"FunctionType".to_vec(), Object::Integer(4)),
            (b"Domain".to_vec(), function_domain.clone()),
            (b"Range".to_vec(), Object::Array(vec![0.into(), 1.into()])),
        ]),
        program.mask,
    ));
    let mask_shading = document.add_object(Dictionary::from_iter([
        (b"ShadingType".to_vec(), Object::Integer(1)),
        (b"ColorSpace".to_vec(), Object::Name(b"DeviceGray".to_vec())),
        (b"Domain".to_vec(), function_domain),
        (b"Function".to_vec(), Object::Reference(mask_function)),
    ]));

    let mut mask_resources = Dictionary::new();
    mask_resources.set(
        "Shading",
        Dictionary::from_iter([(b"M".to_vec(), Object::Reference(mask_shading))]),
    );
    let mask_group = Dictionary::from_iter([
        (b"S".to_vec(), Object::Name(b"Transparency".to_vec())),
        (b"CS".to_vec(), Object::Name(b"DeviceGray".to_vec())),
        (b"I".to_vec(), Object::Boolean(true)),
        (b"K".to_vec(), Object::Boolean(false)),
    ]);
    let mask = document.add_object(Stream::new(
        Dictionary::from_iter([
            (b"Type".to_vec(), Object::Name(b"XObject".to_vec())),
            (b"Subtype".to_vec(), Object::Name(b"Form".to_vec())),
            (b"FormType".to_vec(), Object::Integer(1)),
            (
                b"BBox".to_vec(),
                Object::Array(vec![0.into(), 0.into(), 1.into(), 1.into()]),
            ),
            (b"Resources".to_vec(), Object::Dictionary(mask_resources)),
            (b"Group".to_vec(), Object::Dictionary(mask_group)),
        ]),
        b"/M sh".to_vec(),
    ));

    let soft_mask = Dictionary::from_iter([
        (b"S".to_vec(), Object::Name(b"Luminosity".to_vec())),
        (b"G".to_vec(), Object::Reference(mask)),
        (b"BC".to_vec(), Object::Array(vec![0.into()])),
        (b"TR".to_vec(), Object::Name(b"Identity".to_vec())),
    ]);
    let state = document.add_object(Dictionary::from_iter([(
        b"SMask".to_vec(),
        Object::Dictionary(soft_mask),
    )]));
    let mut states = Dictionary::new();
    states.set("GS", Object::Reference(state));
    let mut form_resources = Dictionary::new();
    form_resources.set("ExtGState", states);
    form_resources.set(
        "Shading",
        Dictionary::from_iter([(b"C".to_vec(), Object::Reference(rgb_shading))]),
    );
    Ok(Object::Stream(Stream::new(
        Dictionary::from_iter([
            (b"Type".to_vec(), Object::Name(b"XObject".to_vec())),
            (b"Subtype".to_vec(), Object::Name(b"Form".to_vec())),
            (b"FormType".to_vec(), Object::Integer(1)),
            (
                b"BBox".to_vec(),
                Object::Array(vec![0.into(), 0.into(), 1.into(), 1.into()]),
            ),
            (b"Resources".to_vec(), Object::Dictionary(form_resources)),
        ]),
        b"/GS gs\n/C sh".to_vec(),
    )))
}

/// Replace exact registered marker Image XObjects with native field Forms.
pub(crate) fn replace_markers(bytes: &[u8], fields: &[PdfInkField<'_>]) -> Result<Vec<u8>, String> {
    if fields.is_empty() {
        return Ok(bytes.to_vec());
    }
    let mut document = Document::load_mem(bytes).map_err(|error| error.to_string())?;
    let mut ids = std::collections::HashSet::new();
    for field in fields {
        if !ids.insert(field.marker_id) {
            return Err(format!(
                "duplicate native ink marker id {}",
                field.marker_id
            ));
        }
    }

    // Index only the original Image streams before adding any Forms/functions.
    // This keeps matching linear in document objects plus marker count even
    // when a page contains thousands of native fields.
    let expected = ids;
    let mut marker_objects: std::collections::HashMap<u64, Vec<ObjectId>> =
        std::collections::HashMap::new();
    for (&object_id, object) in &document.objects {
        let Object::Stream(stream) = object else {
            continue;
        };
        let Some(marker_id) = marker_id_from_image(stream) else {
            continue;
        };
        if expected.contains(&marker_id) {
            marker_objects.entry(marker_id).or_default().push(object_id);
        }
    }

    for field in fields {
        let matches = marker_objects
            .get(&field.marker_id)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        if matches.len() != 1 {
            return Err(format!(
                "native ink marker {} matched {} Image XObjects",
                field.marker_id,
                matches.len()
            ));
        }
        let form = field_form(&mut document, field)?;
        document.objects.insert(matches[0], form);
    }
    for object in document.objects.values_mut() {
        if let Object::Stream(stream) = object
            && !stream.dict.has(b"Filter")
        {
            stream.compress().map_err(|error| error.to_string())?;
        }
    }
    let mut output = Vec::new();
    document
        .save_to(&mut output)
        .map_err(|error| error.to_string())?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CoverageTriangle, CoverageVertex};
    use krilla::{Document as PdfDocument, page::PageSettings};

    fn calculator_numbers_are_fixed(code: &[u8]) -> bool {
        std::str::from_utf8(code).is_ok_and(|code| {
            code.split_whitespace()
                .filter(|token| token.parse::<f64>().is_ok())
                .all(|token| !token.contains('e') && !token.contains('E'))
        })
    }

    fn mesh(vertices: [[f32; 3]; 3]) -> CoverageMesh {
        CoverageMesh {
            triangles: vec![CoverageTriangle {
                vertices: vertices.map(|[x, y, coverage]| CoverageVertex { x, y, coverage }),
            }],
            sample_ends: Vec::new(),
            terminal_start: 0,
            terminal_end: 1,
            bounds: None,
        }
    }

    fn marker_pdf(marker_id: u64, include_unrelated_image: bool) -> Vec<u8> {
        let mut document = PdfDocument::new();
        let mut page =
            document.start_page_with(PageSettings::new(Size::from_wh(20.0, 20.0).unwrap()));
        let mut surface = page.surface();
        emit_marker(
            &mut surface,
            marker_id,
            BoundingBox {
                x_min: 1.0,
                y_min: 1.0,
                x_max: 5.0,
                y_max: 5.0,
            },
        )
        .unwrap();
        if include_unrelated_image {
            surface.draw_image(
                Image::from_rgba8(vec![255, 0, 0, 255], 1, 1),
                Size::from_wh(1.0, 1.0).unwrap(),
            );
        }
        surface.finish();
        page.finish();
        document.finish().unwrap()
    }

    fn field() -> (CoverageMesh, BoundingBox) {
        let bounds = BoundingBox {
            x_min: 1.0,
            y_min: 1.0,
            x_max: 5.0,
            y_max: 5.0,
        };
        (
            mesh([[1.0, 1.0, 0.0], [5.0, 1.0, 0.5], [1.0, 5.0, 1.0]]),
            bounds,
        )
    }

    fn image_count(document: &Document) -> usize {
        document
            .objects
            .values()
            .filter(|object| {
                object
                    .as_stream()
                    .ok()
                    .and_then(|stream| stream.dict.get(b"Subtype").ok())
                    .and_then(|subtype| subtype.as_name().ok())
                    == Some(b"Image")
            })
            .count()
    }

    fn has_type1_shading_resource(document: &Document) -> bool {
        document.objects.values().any(|object| {
            object
                .as_stream()
                .ok()
                .and_then(|stream| stream.dict.get(b"Resources").ok())
                .and_then(|resources| resources.as_dict().ok())
                .and_then(|resources| resources.get(b"Shading").ok())
                .and_then(|shadings| shadings.as_dict().ok())
                .is_some_and(|shadings| !shadings.is_empty())
        })
    }

    fn save(document: &mut Document) -> Vec<u8> {
        let mut bytes = Vec::new();
        document.save_to(&mut bytes).unwrap();
        bytes
    }

    #[test]
    fn same_level_union_removes_internal_edges_and_keeps_small_regions() {
        let bounds = BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 10.0,
            y_max: 10.0,
        };
        let left = vec![[1.0, 1.0], [5.0, 1.0], [5.0, 5.0], [1.0, 5.0]];
        let right = vec![[5.0, 1.0], [9.0, 1.0], [9.0, 5.0], [5.0, 5.0]];
        let union = union_polygons(&[left, right], bounds, &mut ContourWork::default()).unwrap();
        assert_eq!(union.len(), 1);
        assert_eq!(union[0].len(), 1);
        assert!(
            union[0][0].len() <= 8,
            "shared internal edge must disappear"
        );

        let tiny = vec![vec![[1.0, 1.0], [1.00001, 1.0], [1.0, 1.00001]]];
        let union = union_polygons(&tiny, bounds, &mut ContourWork::default()).unwrap();
        assert_eq!(union.len(), 1, "no minimum-area cutoff may discard it");
    }

    #[test]
    fn union_preserves_duplicate_coverage_and_nonzero_hole_winding() {
        let bounds = BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 10.0,
            y_max: 10.0,
        };
        let rect = vec![[1.0, 1.0], [9.0, 1.0], [9.0, 9.0], [1.0, 9.0]];
        let duplicate = union_polygons(
            &[rect.clone(), rect.clone()],
            bounds,
            &mut ContourWork::default(),
        )
        .unwrap();
        assert_eq!(duplicate.len(), 1);
        assert_eq!(duplicate[0].len(), 1);

        let ring = vec![
            vec![[0.0, 0.0], [10.0, 0.0], [10.0, 3.0], [0.0, 3.0]],
            vec![[0.0, 7.0], [10.0, 7.0], [10.0, 10.0], [0.0, 10.0]],
            vec![[0.0, 3.0], [3.0, 3.0], [3.0, 7.0], [0.0, 7.0]],
            vec![[7.0, 3.0], [10.0, 3.0], [10.0, 7.0], [7.0, 7.0]],
        ];
        let union = union_polygons(&ring, bounds, &mut ContourWork::default()).unwrap();
        assert_eq!(union.len(), 1);
        assert_eq!(union[0].len(), 2, "one outer ring and one hole expected");
        let areas: Vec<f64> = union[0]
            .iter()
            .map(|contour| {
                let origin = contour[0];
                (1..contour.len() - 1)
                    .map(|index| {
                        let a = [contour[index][0] - origin[0], contour[index][1] - origin[1]];
                        let b = [
                            contour[index + 1][0] - origin[0],
                            contour[index + 1][1] - origin[1],
                        ];
                        a[0] * b[1] - a[1] * b[0]
                    })
                    .sum()
            })
            .collect();
        assert!(
            areas[0] * areas[1] < 0.0,
            "nonzero fill needs opposite contour winding"
        );
    }

    #[test]
    fn thin_high_threshold_region_survives_large_document_offset() {
        let base = 4_000_000.0;
        let triangle = Triangle([
            [base, base, 0.0],
            [base + 4.0, base, 1.0],
            [base, base + 4.0, 1.0],
        ]);
        let polygon = clip_triangle(triangle, 0.9999).unwrap().unwrap();
        let bounds = BoundingBox {
            x_min: base,
            y_min: base,
            x_max: base + 4.0,
            y_max: base + 4.0,
        };
        let union = union_polygons(&[polygon], bounds, &mut ContourWork::default()).unwrap();
        assert_eq!(union.len(), 1);
        assert_eq!(union[0].len(), 1);
        let contour = &union[0][0];
        assert!(contour.len() >= 3);
        let twice_area = (1..contour.len() - 1)
            .map(|index| {
                let a = [
                    contour[index][0] - contour[0][0],
                    contour[index][1] - contour[0][1],
                ];
                let b = [
                    contour[index + 1][0] - contour[0][0],
                    contour[index + 1][1] - contour[0][1],
                ];
                a[0] * b[1] - a[1] * b[0]
            })
            .sum::<f64>();
        assert!(twice_area > 0.0);
    }

    #[test]
    fn tiny_high_threshold_polygon_survives_large_offset_area_cancellation_case() {
        let base = 4_000_000.0;
        let triangle = Triangle([
            [base, base, 1.0],
            [base + 0.25, base, 0.0],
            [base, base + 0.25, 0.0],
        ]);
        let polygon = clip_triangle(triangle, 254.0 / 255.0)
            .unwrap()
            .expect("threshold leaves a positive-area cap");
        let naive_area = (0..polygon.len())
            .map(|index| {
                let a = polygon[index];
                let b = polygon[(index + 1) % polygon.len()];
                a[0] * b[1] - a[1] * b[0]
            })
            .sum::<f64>();
        assert_eq!(
            naive_area, 0.0,
            "fixture must expose raw shoelace cancellation"
        );

        let origin = polygon[0];
        let translated_twice_area = (1..polygon.len() - 1)
            .map(|index| {
                let a = [polygon[index][0] - origin[0], polygon[index][1] - origin[1]];
                let b = [
                    polygon[index + 1][0] - origin[0],
                    polygon[index + 1][1] - origin[1],
                ];
                a[0] * b[1] - a[1] * b[0]
            })
            .sum::<f64>();
        assert!(translated_twice_area > 0.0);
    }

    #[test]
    fn isocontour_stream_has_bounded_gray_levels_and_no_shading_dependency() {
        let bounds = BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 1.0,
            y_max: 1.0,
        };
        let triangle = Triangle([[0.0, 0.0, 0.0], [1.0, 0.0, 1.0], [0.0, 1.0, 1.0]]);
        let stream = isocontour_stream(&[triangle], bounds).unwrap();
        let text = std::str::from_utf8(&stream).unwrap();
        assert!(text.starts_with("0.00392156862745098 g\n"));
        assert_eq!(text.matches(" g\n").count(), 255);
        assert_eq!(text.matches("f\n").count(), 255);
        let painted_grays: Vec<f64> = text
            .lines()
            .filter_map(|line| line.strip_suffix(" g"))
            .map(|value| value.parse().unwrap())
            .collect();
        assert_eq!(
            painted_grays.last(),
            Some(&1.0),
            "full coverage is exactly one"
        );
        assert!(!text.contains(" sh\n"));
    }

    #[test]
    fn nearest_quantization_separates_threshold_from_painted_gray() {
        let bounds = BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 1.0,
            y_max: 1.0,
        };
        let constant = |coverage| {
            Triangle([
                [0.0, 0.0, coverage],
                [1.0, 0.0, coverage],
                [0.0, 1.0, coverage],
            ])
        };
        assert!(
            isocontour_stream(&[constant(0.49 / 255.0)], bounds)
                .unwrap()
                .is_empty()
        );

        let at_half = isocontour_stream(&[constant(0.5 / 255.0)], bounds).unwrap();
        let at_half = std::str::from_utf8(&at_half).unwrap();
        assert!(at_half.starts_with("0.00392156862745098 g\n"));
        assert_eq!(at_half.matches(" g\n").count(), 1);

        let below_next = isocontour_stream(&[constant(1.49 / 255.0)], bounds).unwrap();
        assert_eq!(
            std::str::from_utf8(&below_next)
                .unwrap()
                .matches(" g\n")
                .count(),
            1
        );
        let above_next = isocontour_stream(&[constant(1.51 / 255.0)], bounds).unwrap();
        let above_next = std::str::from_utf8(&above_next).unwrap();
        assert_eq!(above_next.matches(" g\n").count(), 2);
        assert!(above_next.contains("0.00784313725490196 g\n"));

        let full = isocontour_stream(&[constant(1.0)], bounds).unwrap();
        let full = std::str::from_utf8(&full).unwrap();
        assert_eq!(full.matches(" g\n").count(), 255);
        let grays: Vec<f64> = full
            .lines()
            .filter_map(|line| line.strip_suffix(" g"))
            .map(|value| value.parse().unwrap())
            .collect();
        let expected: Vec<f64> = (1..=255).map(|index| index as f64 / 255.0).collect();
        assert_eq!(
            grays, expected,
            "each threshold paints its UNORM8 index value"
        );
    }

    #[test]
    fn shared_edge_intersection_is_direction_independent() {
        let a = [17.173768997192383, -1.4142135381698608, 1.0];
        let b = [19.021528244018555, -2.1795806884765625, 0.0];
        let level = 0.6607843137254902;
        assert_eq!(
            edge_intersection(a, b, level),
            edge_intersection(b, a, level)
        );
    }

    #[test]
    fn tiny_island_with_remote_bounds_uses_component_local_overlay_and_path_mapping() {
        let remote = f64::from(1.0e20_f32);
        let remote_step = f64::from(1.0e14_f32);
        let bounds = BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: remote + remote_step,
            y_max: remote + remote_step,
        };
        let tiny = Triangle([[0.0, 0.0, 1.0], [450.0, 0.0, 1.0], [0.0, 450.0, 1.0]]);
        let far = Triangle([
            [remote, remote, 0.0],
            [remote + remote_step, remote, 1.0],
            [remote, remote + remote_step, 1.0],
        ]);
        let mesh = CoverageMesh {
            triangles: [tiny, far]
                .map(|triangle| CoverageTriangle {
                    vertices: triangle.0.map(|[x, y, coverage]| CoverageVertex {
                        x: x as f32,
                        y: y as f32,
                        coverage: coverage as f32,
                    }),
                })
                .to_vec(),
            sample_ends: Vec::new(),
            terminal_start: 0,
            terminal_end: 1,
            bounds: None,
        };
        let field = PdfInkField::mesh(1, &mesh, [0, 0, 0], bounds);
        let mut document = Document::with_version("1.7");
        assert!(field_form(&mut document, &field).is_ok());
        assert!(
            has_type1_shading_resource(&document),
            "normalization precision failure must use legacy native vectors"
        );
    }

    #[test]
    fn tiny_islands_at_both_bbox_ends_remain_in_native_vector_forms() {
        let remote = f64::from(1.0e20_f32);
        let remote_step = f64::from(1.0e14_f32);
        let bounds = BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: remote + remote_step,
            y_max: remote + remote_step,
        };
        let low = Triangle([[0.0, 0.0, 1.0], [450.0, 0.0, 1.0], [0.0, 450.0, 1.0]]);
        let high = Triangle([
            [remote, remote, 1.0],
            [remote + remote_step, remote, 1.0],
            [remote, remote + remote_step, 1.0],
        ]);
        let mesh = CoverageMesh {
            triangles: [low, high]
                .map(|triangle| CoverageTriangle {
                    vertices: triangle.0.map(|[x, y, coverage]| CoverageVertex {
                        x: x as f32,
                        y: y as f32,
                        coverage: coverage as f32,
                    }),
                })
                .to_vec(),
            sample_ends: Vec::new(),
            terminal_start: 0,
            terminal_end: 1,
            bounds: None,
        };
        let field = PdfInkField::mesh(1, &mesh, [0, 0, 0], bounds);
        let mut document = Document::with_version("1.7");
        assert!(field_form(&mut document, &field).is_ok());
        // Component-local overlay succeeds, but the far-low component loses
        // distinct vertices after mapping into the common field coordinates.
        // Keep the complete field via the legacy vector calculator.
        assert!(has_type1_shading_resource(&document));
    }

    #[test]
    fn overlay_precision_failure_uses_legacy_type1_for_overlapping_bboxes() {
        let remote = f64::from(1.0e18_f32);
        let small = Triangle([[0.0, 0.0, 1.0], [0.125, 0.0, 0.0], [0.0, 0.125, 0.0]]);
        let large = Triangle([
            [0.0, remote, 1.0],
            [remote, 0.0, 1.0],
            [remote, remote, 1.0],
        ]);
        let mesh = CoverageMesh {
            triangles: [small, large]
                .map(|triangle| CoverageTriangle {
                    vertices: triangle.0.map(|[x, y, coverage]| CoverageVertex {
                        x: x as f32,
                        y: y as f32,
                        coverage: coverage as f32,
                    }),
                })
                .to_vec(),
            sample_ends: Vec::new(),
            terminal_start: 0,
            terminal_end: 1,
            bounds: None,
        };
        let field = PdfInkField::mesh(
            1,
            &mesh,
            [0, 0, 0],
            BoundingBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: remote,
                y_max: remote,
            },
        );
        let mut document = Document::with_version("1.7");
        assert!(field_form(&mut document, &field).is_ok());
        assert!(
            has_type1_shading_resource(&document),
            "precision fallback must retain the native Type 1 field"
        );
    }

    #[test]
    fn adapter_roundtrip_loss_uses_legacy_for_touching_extreme_aspect_islands() {
        let remote = f64::from(1.0e20_f32);
        let small = Triangle([
            [0.0, 10_000.0, 1.0],
            [10_000.0, 10_000.0, 1.0],
            [0.0, 20_000.0, 1.0],
        ]);
        for remote_x in [remote, -remote] {
            let large = Triangle([[0.0, 0.0, 1.0], [remote_x, 0.0, 1.0], [0.0, 10_000.0, 1.0]]);
            let bounds = BoundingBox {
                x_min: remote_x.min(0.0),
                y_min: 0.0,
                x_max: remote_x.max(10_000.0),
                y_max: 20_000.0,
            };
            let triangles = [small, large];
            let mesh_data = CoverageMesh {
                triangles: triangles
                    .map(|triangle| CoverageTriangle {
                        vertices: triangle.0.map(|[x, y, coverage]| CoverageVertex {
                            x: x as f32,
                            y: y as f32,
                            coverage: coverage as f32,
                        }),
                    })
                    .to_vec(),
                sample_ends: Vec::new(),
                terminal_start: 0,
                terminal_end: 1,
                bounds: None,
            };
            let clipped = triangles
                .iter()
                .map(|triangle| clip_triangle(*triangle, 0.5 / 255.0).unwrap().unwrap())
                .collect::<Vec<_>>();
            let error = union_polygons(&clipped, bounds, &mut ContourWork::default())
                .expect_err("adapter round trip must reject material local-geometry loss");
            assert!(matches!(error, ContourFailure::LegacyRequired(_)));

            let field = PdfInkField::mesh(1, &mesh_data, [0, 0, 0], bounds);
            let mut document = Document::with_version("1.7");
            assert!(field_form(&mut document, &field).is_ok());
            assert!(
                has_type1_shading_resource(&document),
                "lossy adapter precision must preserve the field through the legacy vector route"
            );
        }

        let normal = mesh([[0.0, 0.0, 1.0], [10_000.0, 0.0, 1.0], [0.0, 10_000.0, 1.0]]);
        let normal_field = PdfInkField::mesh(
            2,
            &normal,
            [0, 0, 0],
            BoundingBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: 10_000.0,
                y_max: 10_000.0,
            },
        );
        let mut normal_document = Document::with_version("1.7");
        assert!(field_form(&mut normal_document, &normal_field).is_ok());
        assert!(
            !has_type1_shading_resource(&normal_document),
            "well-scaled ordinary contours should remain on the contour route"
        );
    }

    #[test]
    fn contour_work_budget_routes_to_legacy_type1_without_dropping_field() {
        let bounds = BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 2.0,
            y_max: 2.0,
        };
        let first = Triangle([[0.0, 0.0, 1.0], [2.0, 0.0, 1.0], [0.0, 2.0, 1.0]]);
        let second = Triangle([[0.0, 0.0, 1.0], [2.0, 2.0, 1.0], [0.0, 2.0, 1.0]]);
        let mesh = CoverageMesh {
            triangles: [first, second]
                .map(|triangle| CoverageTriangle {
                    vertices: triangle.0.map(|[x, y, coverage]| CoverageVertex {
                        x: x as f32,
                        y: y as f32,
                        coverage: coverage as f32,
                    }),
                })
                .to_vec(),
            sample_ends: Vec::new(),
            terminal_start: 0,
            terminal_end: 1,
            bounds: None,
        };
        let field = PdfInkField::mesh(1, &mesh, [0, 0, 0], bounds);
        let limits = ContourLimits {
            max_bbox_comparisons: 0,
            ..ContourLimits::default()
        };
        let mut document = Document::with_version("1.7");
        assert!(field_form_with_limits(&mut document, &field, limits).is_ok());
        assert!(
            has_type1_shading_resource(&document),
            "resource exhaustion must preserve the field via legacy vectors"
        );
    }

    #[test]
    fn pdf_number_formatter_round_trips_small_normalized_values_without_exponents() {
        let value = 4.5e-18;
        let encoded = fmt(value);
        assert!(!encoded.contains('e'));
        assert_eq!(encoded.parse::<f64>().unwrap(), value);
    }

    #[test]
    fn empty_positive_meshes_produce_transparent_masks() {
        let bounds = BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 1.0,
            y_max: 1.0,
        };
        let zero = Triangle([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]);
        let subthreshold = Triangle([[0.0, 0.0, 0.001], [1.0, 0.0, 0.001], [0.0, 1.0, 0.001]]);
        assert!(isocontour_stream(&[zero], bounds).unwrap().is_empty());
        assert!(
            isocontour_stream(&[subthreshold], bounds)
                .unwrap()
                .is_empty()
        );
        for vertices in [
            [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            [[0.0, 0.0, 0.001], [1.0, 0.0, 0.001], [0.0, 1.0, 0.001]],
        ] {
            let mesh = mesh(vertices);
            let field = PdfInkField::mesh(1, &mesh, [0, 0, 0], bounds);
            assert!(field_form(&mut Document::with_version("1.7"), &field).is_ok());
        }
    }

    #[test]
    fn over_budget_mesh_is_rejected_explicitly_before_contouring() {
        let triangle = CoverageTriangle {
            vertices: [
                CoverageVertex {
                    x: 0.0,
                    y: 0.0,
                    coverage: 1.0,
                },
                CoverageVertex {
                    x: 1.0,
                    y: 0.0,
                    coverage: 1.0,
                },
                CoverageVertex {
                    x: 0.0,
                    y: 1.0,
                    coverage: 1.0,
                },
            ],
        };
        let mut mesh = mesh([[0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [0.0, 1.0, 1.0]]);
        mesh.triangles
            .resize_with(MAX_FIELD_TRIANGLES + 1, || CoverageTriangle {
                vertices: triangle.vertices.map(|vertex| CoverageVertex {
                    x: vertex.x,
                    y: vertex.y,
                    coverage: vertex.coverage,
                }),
            });
        let field = PdfInkField::mesh(
            1,
            &mesh,
            [0, 0, 0],
            BoundingBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: 1.0,
                y_max: 1.0,
            },
        );
        assert!(
            field_form(&mut Document::with_version("1.7"), &field)
                .unwrap_err()
                .contains("exceeds triangle budget")
        );
    }

    #[test]
    fn rejects_mesh_without_any_non_degenerate_triangle() {
        let mesh = mesh([[0.0, 0.0, 0.0], [1.0, 1.0, 0.5], [2.0, 2.0, 1.0]]);
        let bounds = BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 2.0,
            y_max: 2.0,
        };
        let field = PdfInkField::mesh(1, &mesh, [0, 0, 0], bounds);
        assert!(
            field_form(&mut Document::with_version("1.7"), &field)
                .unwrap_err()
                .contains("no non-degenerate")
        );
    }

    #[test]
    fn radial_payload_uses_bounded_type4_forms_for_native_max_and_direct_modes() {
        let records: [PdfRadialRecord; 25] = std::array::from_fn(|index| {
            let n = index as f32;
            [
                0.1 + n * 0.03,
                0.5,
                0.02,
                0.1,
                0.08 + n * 0.001,
                0.9,
                (index % 3 == 0) as u8 as f32 * 0.8,
                (index % 3 == 1) as u8 as f32 * 0.6,
                (index % 3 == 2) as u8 as f32 * 0.7,
            ]
        });
        let bounds = BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 1.0,
            y_max: 1.0,
        };
        for mode in [
            PdfRadialCoverage::DirectSourceOver,
            PdfRadialCoverage::EnhancedMax,
        ] {
            let field = PdfInkField::radial(7, &records, 128.0 / 255.0, mode, bounds);
            let mut document = Document::with_version("1.7");
            let form = field_form(&mut document, &field).unwrap();
            assert_eq!(
                form.as_stream()
                    .unwrap()
                    .dict
                    .get(b"Subtype")
                    .unwrap()
                    .as_name()
                    .unwrap(),
                b"Form"
            );
            let functions = document
                .objects
                .values()
                .filter_map(|object| object.as_stream().ok())
                .filter(|stream| {
                    stream
                        .dict
                        .get(b"FunctionType")
                        .ok()
                        .and_then(|value| value.as_i64().ok())
                        == Some(4)
                })
                .map(|stream| stream.decompressed_content().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(functions.len(), 2);
            assert!(
                functions
                    .iter()
                    .all(|code| calculator_numbers_are_fixed(code))
            );
        }
    }

    #[test]
    fn radial_preflight_failure_does_not_add_partial_pdf_objects() {
        let mut records = [[0.5, 0.5, 0.25, 0.25, 0.1, 1.0, 0.2, 0.4, 0.6]; 1];
        records[0][2] = f32::NAN;
        let field = PdfInkField::radial(
            1,
            &records,
            0.5,
            PdfRadialCoverage::EnhancedMax,
            BoundingBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: 1.0,
                y_max: 1.0,
            },
        );
        let mut document = Document::with_version("1.7");
        let before = document.objects.len();
        assert!(field_form(&mut document, &field).is_err());
        assert_eq!(document.objects.len(), before);
    }

    #[test]
    fn validates_planes_at_large_document_coordinates() {
        let base = 1.0e30_f32;
        let delta = 1.0e24_f32;
        let mesh = mesh([
            [base, base, 0.0],
            [base + delta, base, 0.5],
            [base, base + delta, 1.0],
        ]);
        let bounds = BoundingBox {
            x_min: f64::from(base),
            y_min: f64::from(base),
            x_max: f64::from(base + delta),
            y_max: f64::from(base + delta),
        };
        let triangles = triangles(&mesh).unwrap();
        assert!(plane(triangles[0]).3 != 0.0);
        assert!(!isocontour_stream(&triangles, bounds).unwrap().is_empty());
    }

    #[test]
    fn marker_index_replaces_expected_payload_and_preserves_unrelated_images() {
        let marker_id = 0x1234_5678;
        let bytes = marker_pdf(marker_id, true);
        let (mesh, bounds) = field();
        let result = replace_markers(
            &bytes,
            &[PdfInkField::mesh(
                marker_id,
                &mesh,
                [0x12, 0x34, 0x56],
                bounds,
            )],
        )
        .unwrap();
        let document = Document::load_mem(&result).unwrap();
        assert_eq!(image_count(&document), 1, "unrelated image must remain");
        assert_eq!(
            document
                .objects
                .values()
                .filter_map(|object| object.as_stream().ok())
                .filter(|stream| {
                    stream
                        .dict
                        .get(b"Subtype")
                        .ok()
                        .and_then(|subtype| subtype.as_name().ok())
                        == Some(b"Form")
                })
                .filter(|stream| {
                    stream
                        .decompressed_content()
                        .ok()
                        .is_some_and(|content| content.windows(4).any(|w| w == b" rg "))
                })
                .count(),
            1,
            "the marker must become one colored Form (Krilla may have other Forms)"
        );
    }

    #[test]
    fn marker_index_rejects_missing_and_duplicate_expected_payloads() {
        let (mesh, bounds) = field();
        let missing = replace_markers(
            &marker_pdf(0, false),
            &[PdfInkField::mesh(1, &mesh, [0, 0, 0], bounds)],
        )
        .unwrap_err();
        assert!(missing.contains("marker 1 matched 0 Image XObjects"));

        let mut document = Document::load_mem(&marker_pdf(0, false)).unwrap();
        let marker = document
            .objects
            .values()
            .find(|object| {
                object
                    .as_stream()
                    .ok()
                    .is_some_and(|stream| marker_id_from_image(stream) == Some(0))
            })
            .unwrap()
            .clone();
        document.add_object(marker);
        let duplicate = replace_markers(
            &save(&mut document),
            &[PdfInkField::mesh(0, &mesh, [0, 0, 0], bounds)],
        )
        .unwrap_err();
        assert!(duplicate.contains("marker 0 matched 2 Image XObjects"));
    }

    #[test]
    fn marker_index_requires_magic_checksum_and_exact_image_shape() {
        let mut stream = Stream::new(
            Dictionary::from_iter([
                (b"Subtype".to_vec(), Object::Name(b"Image".to_vec())),
                (b"Width".to_vec(), Object::Integer(MARKER_WIDTH as i64)),
                (b"Height".to_vec(), Object::Integer(MARKER_HEIGHT as i64)),
                (b"ColorSpace".to_vec(), Object::Name(b"DeviceRGB".to_vec())),
                (b"BitsPerComponent".to_vec(), Object::Integer(8)),
            ]),
            marker_pixels(7),
        );
        assert_eq!(marker_id_from_image(&stream), Some(7));

        let mut bad_magic = stream.clone();
        bad_magic.content[0] ^= 1;
        assert_eq!(marker_id_from_image(&bad_magic), None);
        let mut bad_checksum = stream.clone();
        bad_checksum.content[23] ^= 1;
        assert_eq!(marker_id_from_image(&bad_checksum), None);
        stream.dict.set("Width", Object::Integer(7));
        assert_eq!(marker_id_from_image(&stream), None);
        let mut soft_mask = Stream::new(
            Dictionary::from_iter([
                (b"Subtype".to_vec(), Object::Name(b"Image".to_vec())),
                (b"Width".to_vec(), Object::Integer(MARKER_WIDTH as i64)),
                (b"Height".to_vec(), Object::Integer(MARKER_HEIGHT as i64)),
                (b"ColorSpace".to_vec(), Object::Name(b"DeviceRGB".to_vec())),
                (b"BitsPerComponent".to_vec(), Object::Integer(8)),
                (b"SMask".to_vec(), Object::Null),
            ]),
            marker_pixels(7),
        );
        assert_eq!(marker_id_from_image(&soft_mask), None);
        soft_mask.dict.remove(b"SMask");
        soft_mask
            .dict
            .set("ColorSpace", Object::Name(b"DeviceGray".to_vec()));
        assert_eq!(marker_id_from_image(&soft_mask), None);
    }

    #[test]
    fn radial_circle_marker_becomes_transparent_ordered_rgb_vector_form() {
        let bounds = BoundingBox {
            x_min: 1.0,
            y_min: 1.0,
            x_max: 5.0,
            y_max: 5.0,
        };
        let records = [
            [3.0, 3.0, 1.5, 1.5, 0.0, 1.0, 0.25, 0.5, 0.75],
            [3.25, 3.0, 1.25, 1.25, 0.0, 1.0, 0.8, 0.1, 0.2],
        ];
        let replaced = replace_markers(
            &marker_pdf(0, false),
            &[PdfInkField::radial_circles(0, &records, 0.5, bounds)],
        )
        .unwrap();
        let document = Document::load_mem(&replaced).unwrap();
        assert_eq!(image_count(&document), 0, "the marker image is replaced");
        let form = document
            .objects
            .values()
            .filter_map(|object| object.as_stream().ok())
            .find(|stream| {
                stream
                    .dict
                    .get(b"Subtype")
                    .ok()
                    .and_then(|value| value.as_name().ok())
                    == Some(b"Form")
                    && stream
                        .decompressed_content()
                        .ok()
                        .is_some_and(|content| content.windows(2).any(|w| w == b" c"))
            })
            .expect("radial circle form exists");
        let group = form.dict.get(b"Group").unwrap().as_dict().unwrap();
        assert_eq!(group.get(b"S").unwrap().as_name().unwrap(), b"Transparency");
        assert_eq!(group.get(b"CS").unwrap().as_name().unwrap(), b"DeviceRGB");
        assert!(matches!(group.get(b"I"), Ok(Object::Boolean(true))));
        assert!(matches!(group.get(b"K"), Ok(Object::Boolean(false))));
        let content = String::from_utf8(form.decompressed_content().unwrap()).unwrap();
        let colors: Vec<Vec<f32>> = lopdf::content::Content::decode(content.as_bytes())
            .unwrap()
            .operations
            .iter()
            .filter(|operation| operation.operator == "rg")
            .map(|operation| {
                operation
                    .operands
                    .iter()
                    .map(|value| value.as_float().unwrap())
                    .collect()
            })
            .collect();
        assert_eq!(colors, vec![vec![0.25, 0.5, 0.75], vec![0.8, 0.1, 0.2]]);
        assert_eq!(content.matches(" c ").count(), 8);
        assert!(
            !content.contains(" re f"),
            "no test/background fill is emitted"
        );
    }
}

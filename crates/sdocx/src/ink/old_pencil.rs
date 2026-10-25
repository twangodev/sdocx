//! Source-derived saved-stroke sampler for the shipped legacy `Pencil` GLV1.
//!
//! Test-only source model for the shipped legacy `Pencil` GLV1 saved-stroke
//! route. It is registered from `ink.rs` only under `cfg(test)` and reuses the
//! shared production SmPath implementation.
//!
//! Samsung Notes 4.4.45.37 Pencil DSO SHA256:
//! 39eb599262676ca5ec67b86e702b43474a04e9ca9677072f247b131222189df9.
//! Native Drawing capture SHA256:
//! dd739e33eb3a0ad4bfd8237976cae6162982f79d3d0bab84f77af0679a05eb06.
//! Synthetic fresh/cached stylus controls; no texture or pixel parity claim.

use super::path::{P, Quad};

const MAX_SOURCE_POINTS: usize = 65_536;
const MAX_OUTPUT_RECORDS: usize = 1_000_000;
const ATLAS_ROW_COUNT: usize = 7;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SavedPoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug)]
pub(crate) struct SavedStroke<'a> {
    pub pen_name: &'a str,
    pub tool_type: i32,
    pub stroke_type: i32,
    pub curve_enabled: bool,
    pub points: &'a [SavedPoint],
    pub pressures: &'a [f32],
    pub size: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PencilStyleProfile {
    /// PencilData size used by the native PencilStrokeStyle.
    pub size: f32,
    /// drawPoint can replace its interpolated width with `*(PencilData + 12)`
    /// when byte `(PencilData + 9)` is nonzero. The V9 profile observed byte
    /// zero and width zero, so its test passes `None`.
    pub point_width_override: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SampledStroke {
    /// Exact observed AddPoint lane order: x, y, drawPoint-width / 2, row.
    pub records: Vec<[f32; 4]>,
    /// Cumulative output endpoint after each source-derived quadratic leg.
    pub leg_ends: Vec<usize>,
}

fn clamped_pressure(value: f32) -> f32 {
    value.min(1.0)
}

fn half_width(size: f32, pressure: f32) -> f32 {
    size * (clamped_pressure(pressure) + 1.0) * 0.5
}

fn emit_leg(
    path: &Quad,
    old_half_width: f32,
    target_half_width: f32,
    point_width_override: Option<f32>,
    row_counter: &mut usize,
    records: &mut Vec<[f32; 4]>,
    leg_ends: &mut Vec<usize>,
) -> Result<f32, &'static str> {
    let length = path.length;
    if !length.is_finite() || length < 0.0 {
        return Err("SmPath length must be finite and nonnegative");
    }
    if length == 0.0 {
        leg_ends.push(records.len());
        return Ok(target_half_width);
    }
    if !old_half_width.is_finite()
        || !target_half_width.is_finite()
        || old_half_width <= 0.0
        || target_half_width <= 0.0
    {
        return Err("Pencil pressure-derived half-width must be finite and positive");
    }

    // PencilStrokeCurveStyle::drawLine uses FCVTPS, i.e. round toward +inf,
    // for length / ((old + target) * 0.25). A width jump larger than the path
    // is the source's separate one-sample branch.
    let width_delta = target_half_width - old_half_width;
    let count = if width_delta.abs() > length {
        1
    } else {
        let density_step = (old_half_width + target_half_width) * 0.25;
        let ratio = length / density_step;
        if !ratio.is_finite() || ratio > MAX_OUTPUT_RECORDS as f32 {
            return Err("Pencil path sample count exceeds the bounded output budget");
        }
        ratio.ceil() as usize
    };
    if count == 0 || records.len().saturating_add(count) > MAX_OUTPUT_RECORDS {
        return Err("Pencil path sample count exceeds the bounded output budget");
    }

    let distance_step = length / count as f32;
    let width_step = width_delta / count as f32;
    let mut distance = 0.0f32;
    let mut width = old_half_width;
    for _ in 0..count {
        let point = path.at(distance);
        let emitted_width = point_width_override.unwrap_or(width);
        if !point.x.is_finite() || !point.y.is_finite() || !emitted_width.is_finite() {
            return Err("Pencil emitted a non-finite point or width");
        }
        records.push([
            point.x,
            point.y,
            emitted_width * 0.5,
            (*row_counter % ATLAS_ROW_COUNT) as f32,
        ]);
        *row_counter = row_counter.wrapping_add(1);
        distance += distance_step;
        width += width_step;
    }
    leg_ends.push(records.len());
    Ok(target_half_width)
}

/// Sample the captured native Pencil GLV1 saved-stroke geometry route.
///
/// The source curve controller joins saved points through midpoint-ended
/// quadratics and runs one terminal leg to the last saved point. `Quad` is the
/// shared 15-bit adaptive SmPath implementation used by production ink.
pub(crate) fn sample_saved_stroke(
    stroke: &SavedStroke<'_>,
    style: PencilStyleProfile,
) -> Result<SampledStroke, &'static str> {
    if stroke.pen_name != "com.samsung.android.sdk.pen.pen.preload.Pencil"
        || stroke.tool_type != 2
        || stroke.stroke_type != 0
        || !stroke.curve_enabled
    {
        return Err("outside the legacy Pencil GLV1 saved-stroke route");
    }
    let n = stroke.points.len();
    if !(2..=MAX_SOURCE_POINTS).contains(&n)
        || stroke.pressures.len() != n
        || !style.size.is_finite()
        || style.size <= 0.0
        || stroke.size.to_bits() != style.size.to_bits()
        || stroke
            .points
            .iter()
            .any(|p| !p.x.is_finite() || !p.y.is_finite())
        || stroke.pressures.iter().any(|p| !p.is_finite())
        || style
            .point_width_override
            .is_some_and(|width| !width.is_finite() || width <= 0.0)
    {
        return Err("invalid or unbounded Pencil saved-stroke/style channels");
    }

    let points: Vec<P> = stroke
        .points
        .iter()
        .map(|point| P {
            x: point.x,
            y: point.y,
        })
        .collect();
    let mut records = Vec::new();
    let mut leg_ends = Vec::with_capacity(n);
    let mut row_counter = 0usize;
    let mut previous_width = half_width(style.size, stroke.pressures[0]);

    // First MotionEvent move: p0 -> midpoint(p0,p1), controlled by p0.
    let first_mid = points[0].mid(points[1]);
    previous_width = emit_leg(
        &Quad::new(points[0], points[0], first_mid),
        previous_width,
        half_width(style.size, stroke.pressures[1]),
        style.point_width_override,
        &mut row_counter,
        &mut records,
        &mut leg_ends,
    )?;

    // Each following saved event starts at the previous midpoint, uses the
    // prior saved point as its control, and ends at the next midpoint.
    let mut last_mid = first_mid;
    for i in 1..n - 1 {
        let next_mid = points[i].mid(points[i + 1]);
        previous_width = emit_leg(
            &Quad::new(last_mid, points[i], next_mid),
            previous_width,
            half_width(style.size, stroke.pressures[i + 1]),
            style.point_width_override,
            &mut row_counter,
            &mut records,
            &mut leg_ends,
        )?;
        last_mid = next_mid;
    }

    // The terminal saved event closes midpoint -> final point. Native EndPen
    // uses the terminal sample's pressure and shares the style row counter.
    let _ = emit_leg(
        &Quad::new(last_mid, points[n - 1], points[n - 1]),
        previous_width,
        half_width(style.size, stroke.pressures[n - 1]),
        style.point_width_override,
        &mut row_counter,
        &mut records,
        &mut leg_ends,
    )?;

    Ok(SampledStroke { records, leg_ends })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    const NATIVE_RECORDS: &[u8] =
        include_bytes!("../../../../conformance/old-pencil-glv1-v9-native.bin");

    fn assert_case(
        name: &str,
        offset: usize,
        expected_records: usize,
        size: f32,
        pressure: &[f32],
    ) {
        let point_pairs = [(0.0, 0.0), (10.0, 0.0), (20.0, 0.0), (30.0, 10.0)];
        let points: Vec<SavedPoint> = point_pairs
            .iter()
            .map(|&(x, y)| SavedPoint { x, y })
            .collect();
        let stroke = SavedStroke {
            pen_name: "com.samsung.android.sdk.pen.pen.preload.Pencil",
            tool_type: 2,
            stroke_type: 0,
            curve_enabled: true,
            points: &points,
            pressures: pressure,
            size,
        };
        let sampled = sample_saved_stroke(
            &stroke,
            PencilStyleProfile {
                size,
                point_width_override: None,
            },
        )
        .expect("captured legacy Pencil GLV1 profile is supported");

        assert_eq!(
            sampled.records.len(),
            expected_records,
            "{name}: record count"
        );
        let byte_len = expected_records * 16;
        let expected = NATIVE_RECORDS
            .get(offset..offset + byte_len)
            .unwrap_or_else(|| panic!("{name}: native fixture range"));
        for (record_index, record) in sampled.records.iter().enumerate() {
            for (lane, value) in record.iter().enumerate() {
                let at = record_index * 16 + lane * 4;
                let expected_bits = u32::from_le_bytes(expected[at..at + 4].try_into().unwrap());
                assert_eq!(
                    value.to_bits(),
                    expected_bits,
                    "{name}: record {record_index}, lane {lane}"
                );
            }
        }
    }

    #[test]
    fn saved_glv1_points_pressure_width_and_all_packed_lanes_match_native_v9() {
        // Native V9 owner byte+9 was zero and float+12 was zero in all 109
        // drawPoint windows; `None` is exactly that width-override-off profile.
        // The native saved redraw clamps pressure above 1.0 (last sample 1.25
        // in each ramp), and this test compares the resulting width lane too.
        assert_eq!(NATIVE_RECORDS.len(), 1_744);
        assert_eq!(
            format!("{:x}", Sha256::digest(NATIVE_RECORDS)),
            "950c0a90aa0e42cf3e9319c09dc16f1ceab5942361f4e55e3b644e297cbe7183"
        );
        assert_case("fresh size2 pressure .5", 0, 47, 2.0, &[0.5, 0.5, 0.5, 0.5]);
        assert_case(
            "cached size2 pressure ramp",
            752,
            42,
            2.0,
            &[0.125, 0.375, 0.75, 1.25],
        );
        assert_case(
            "cached size4 pressure ramp",
            1_424,
            20,
            4.0,
            &[0.25, 0.5, 0.875, 1.25],
        );
    }
}

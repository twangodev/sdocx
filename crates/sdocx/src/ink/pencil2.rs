//! Test-only source-derived Pencil2 GLV14 saved geometry sampler.
//!
//! This covers captured RTV4 geometry records only; it does not model the GPU shader,
//! material, texture sampling, alpha compositing, or PDF/SVG paint output.
//!
//! Scope is intentionally narrow: tool 2, source 0, stroke type 0, curve path,
//! finite point channels, identity inverse scale, and captured native alpha state.
//! This samples RTV4 records; it does not render the shader or claim pixel parity.

use std::fmt;

use super::path::{P, Quad};

#[derive(Clone, Copy, Debug)]
pub struct AlphaState {
    /// Byte at `*(data_manager)+7` in getPressureAlpha.
    pub alpha_byte: u8,
    /// Byte at `*(data_manager)+16`; zero selects pressure squared.
    pub square_pressure_flag: u8,
    /// f32 at `*(data_manager)+0`; also initializes current radius in startPen.
    pub base: f32,
    /// i32 at drawable +820.
    pub size_level: i32,
}

#[derive(Clone, Debug)]
pub struct SavedInput {
    pub advanced_version: u8,
    pub tool_type: u8,
    pub source: u32,
    pub stroke_type: u8,
    pub curve_enabled: bool,
    pub color_argb: u32,
    pub size: f32,
    pub effective_size: f32,
    pub points: Vec<P>,
    pub pressure: Vec<f32>,
    pub tilt: Vec<f32>,
    pub orientation: Vec<f32>,
    pub timestamps: Vec<i64>,
}

#[derive(Clone, Copy, Debug)]
pub struct SampledRecord {
    /// Complete source-derived RTV4 record in native 14-f32 lane order.
    pub lanes: [f32; 14],
}

#[derive(Debug)]
pub enum SamplerError {
    Unsupported(&'static str),
    Invalid(&'static str),
    MissingNativeState(&'static str),
}

impl fmt::Display for SamplerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(s) => write!(f, "unsupported Pencil2 V14 input: {s}"),
            Self::Invalid(s) => write!(f, "invalid Pencil2 V14 input: {s}"),
            Self::MissingNativeState(s) => write!(f, "missing captured Pencil2 V14 state: {s}"),
        }
    }
}
impl std::error::Error for SamplerError {}

#[inline]
fn f32add(a: f32, b: f32) -> f32 {
    a + b
}
#[inline]
fn f32sub(a: f32, b: f32) -> f32 {
    a - b
}

/// ARM64 getPressureAlpha at libSPenPencil2.so RVA 0x744c4, preserving the
/// instruction order and fused multiply-adds. FRINTA/FCVTAS are ties-away.
pub fn pressure_alpha(p: f32, s: AlphaState) -> f32 {
    let numerator = f32::from(s.alpha_byte).mul_add(99.0, -45.0);
    let q = (numerator / 252.0).round();
    let idx_float = q.mul_add(2.474_747_4, 7.525_252_3);
    let idx = (idx_float.round() as i32).max(2);
    let multiplier = if s.square_pressure_flag == 0 {
        p * p
    } else {
        1.0
    };
    let mut value = multiplier * (idx as f32 / 80.0);
    let curve = ((idx - 1) as f32 / 254.0).mul_add(0.5, 0.05);
    value *= curve;
    value *= 15.0;
    value /= s.base;
    let level = s.size_level as f32 / 5000.0;
    (value + level) / 6.0
}

fn adjusted_tilt(mean: f32) -> f32 {
    if mean > 0.75 {
        (mean - 0.75) / 0.08
    } else {
        0.0
    }
}

fn finite_pressure(p: f32, tool: u8, source: u32) -> f32 {
    let p = if tool == 2 && source == 0x1002 {
        0.6
    } else {
        p
    };
    if p.is_finite() { p.min(1.0) } else { 0.0 }
}
fn finite_tilt(t: f32) -> f32 {
    if t.is_finite() { t.min(0.79) } else { 0.0 }
}
fn finite_orientation(t: f32) -> f32 {
    if t.is_finite() { t } else { 0.0 }
}

/// Portable Rust math candidate for the native host-glibc sincosf boundary.
/// The conformance test compares every output bit and will expose any ULP drift.
fn host_sincosf(angle: f32) -> (f32, f32) {
    libm::sincosf(angle)
}

#[derive(Clone, Debug)]
struct ChannelRing<const N: usize> {
    values: [f32; N],
    cursor: usize,
    initialized: bool,
}
impl<const N: usize> ChannelRing<N> {
    fn new() -> Self {
        Self {
            values: [0.0; N],
            cursor: 0,
            initialized: false,
        }
    }
    fn initialize(&mut self, first: f32) {
        self.values.fill(first);
        self.cursor = 0;
        self.initialized = true;
    }
    fn push_and_mean(&mut self, value: f32) -> f32 {
        if !self.initialized {
            self.initialize(value);
        }
        self.values[self.cursor] = value;
        self.cursor = (self.cursor + 1) % N;
        let mut sum = 0.0f32;
        for v in self.values {
            sum = f32add(sum, v);
        }
        sum / N as f32
    }
}

fn finite_input(input: &SavedInput) -> Result<(), SamplerError> {
    if input.advanced_version != 14 {
        return Err(SamplerError::Unsupported("advanced setting is not 14"));
    }
    if input.tool_type != 2 {
        return Err(SamplerError::Unsupported("only stylus tool 2 is recovered"));
    }
    if input.source != 0 {
        return Err(SamplerError::Unsupported(
            "only MotionEvent source 0 is recovered",
        ));
    }
    if input.stroke_type != 0 {
        return Err(SamplerError::Unsupported("stroke type is not 0"));
    }
    if !input.curve_enabled {
        return Err(SamplerError::Unsupported(
            "noncurve branch is not implemented",
        ));
    }
    let n = input.points.len();
    if n < 2
        || input.pressure.len() != n
        || input.tilt.len() != n
        || input.orientation.len() != n
        || input.timestamps.len() != n
    {
        return Err(SamplerError::Invalid(
            "channel counts must match at least two points",
        ));
    }
    if input.color_argb != 0xff00_0000 {
        return Err(SamplerError::Unsupported(
            "only opaque black RGB is captured",
        ));
    }
    if !input.size.is_finite()
        || input.size <= 0.0
        || !input.effective_size.is_finite()
        || input.effective_size <= 0.0
        || input
            .points
            .iter()
            .any(|p| !p.x.is_finite() || !p.y.is_finite())
    {
        return Err(SamplerError::Invalid("nonfinite or nonpositive geometry"));
    }
    Ok(())
}

/// Reconstructs the captured curve-enabled saved redraw path and all 14 source-backed
/// RTV4 lanes. It accepts only the captured style/context profile.
pub fn sample(input: &SavedInput, alpha: AlphaState) -> Result<Vec<SampledRecord>, SamplerError> {
    finite_input(input)?;
    let base_radius = alpha.base;
    if !base_radius.is_finite() || base_radius <= 0.0 {
        return Err(SamplerError::Invalid("finite positive base radius"));
    }
    if input.effective_size.to_bits() != base_radius.to_bits() {
        return Err(SamplerError::Unsupported(
            "native alpha base differs from the observed effective pen size",
        ));
    }
    let mut out = Vec::new();
    let mut tilt_ring = ChannelRing::<70>::new();
    let mut orientation_ring = ChannelRing::<50>::new();
    tilt_ring.initialize(finite_tilt(input.tilt[0]));
    orientation_ring.initialize(finite_orientation(input.orientation[0]));

    let spacing = 0.4f32;
    let mut residual = spacing; // startPen resets +92 to +88.
    let mut current_radius = base_radius; // startPen copies data_manager[0] to +248.
    let mut current_alpha = 0.0f32; // startPen clears +252.
    let mut previous = P {
        x: input.points[0].x,
        y: input.points[0].y,
    };
    let mut midpoint = previous;
    let mut tile = 0u32;

    for i in 1..input.points.len() {
        let current = P {
            x: input.points[i].x,
            y: input.points[i].y,
        };
        let dx = f32sub(current.x, previous.x);
        let dy = f32sub(current.y, previous.y);
        let squared = dx.mul_add(dx, dy * dy);
        let chord = squared.sqrt();
        // V14 drawLine uses 5 units as the tool-2 movement gate.
        if chord < 5.0 {
            continue;
        }

        let end = P {
            x: f32add(previous.x, current.x) * 0.5,
            y: f32add(previous.y, current.y) * 0.5,
        };
        let path = Quad::new(midpoint, previous, end);
        let length = path.length;
        if !length.is_finite() {
            return Err(SamplerError::Invalid("nonfinite SmPath length"));
        }
        let pressure = finite_pressure(input.pressure[i], input.tool_type, input.source);
        let target_alpha = pressure_alpha(pressure, alpha);
        let mean_tilt = tilt_ring.push_and_mean(finite_tilt(input.tilt[i]));
        let mean_orientation =
            orientation_ring.push_and_mean(finite_orientation(input.orientation[i]));
        let tilt_factor = adjusted_tilt(mean_tilt);
        let size = base_radius;
        let inner = tilt_factor.mul_add(-2.0, 1.0);
        let target_radius = size.mul_add(inner, (size * 3.5) * tilt_factor);
        let count = (((length - residual) / spacing) as i32)
            .saturating_add(1)
            .max(1);
        let radius_step = (target_radius - current_radius) / count as f32;
        let alpha_step = (target_alpha - current_alpha) / count as f32;
        let mut distance = residual;
        let mut radius = current_radius;
        let mut opacity = current_alpha;
        while distance <= length {
            let point = path.at(distance);
            emit_record(
                &mut out,
                point,
                radius,
                opacity,
                mean_orientation,
                tile,
                input.color_argb,
            );
            radius = f32add(radius, radius_step);
            opacity = f32add(opacity, alpha_step);
            tile = (tile + 1) % 3;
            let next = f32add(distance, spacing);
            if next <= distance {
                return Err(SamplerError::Invalid("spacing failed to advance"));
            }
            distance = next;
        }
        residual = f32sub(distance, length);
        current_radius = target_radius;
        current_alpha = target_alpha;
        previous = current;
        midpoint = end;
    }

    // endPen's final quadratic runs from the carried midpoint through the last
    // accepted point to the actual terminal point. Terminal alpha/radius state
    // follows the same channel normalization and interpolation contract.
    let terminal = P {
        x: input.points.last().unwrap().x,
        y: input.points.last().unwrap().y,
    };
    let path = Quad::new(midpoint, previous, terminal);
    let pressure = finite_pressure(
        *input.pressure.last().unwrap(),
        input.tool_type,
        input.source,
    );
    let target_alpha = pressure_alpha(pressure, alpha);
    let mean_tilt = tilt_ring.push_and_mean(finite_tilt(*input.tilt.last().unwrap()));
    let mean_orientation =
        orientation_ring.push_and_mean(finite_orientation(*input.orientation.last().unwrap()));
    let tilt_factor = adjusted_tilt(mean_tilt);
    let target_radius = base_radius.mul_add(
        tilt_factor.mul_add(-2.0, 1.0),
        (base_radius * 3.5) * tilt_factor,
    );
    if path.length > 0.0 && path.length.is_finite() {
        let count = (((path.length - residual) / spacing) as i32)
            .saturating_add(1)
            .max(1);
        let radius_step = (target_radius - current_radius) / count as f32;
        let alpha_step = (target_alpha - current_alpha) / count as f32;
        let mut distance = residual;
        let mut radius = current_radius;
        let mut opacity = current_alpha;
        while distance <= path.length {
            emit_record(
                &mut out,
                path.at(distance),
                radius,
                opacity,
                mean_orientation,
                tile,
                input.color_argb,
            );
            radius = f32add(radius, radius_step);
            opacity = f32add(opacity, alpha_step);
            tile = (tile + 1) % 3;
            let next = f32add(distance, spacing);
            if next <= distance {
                return Err(SamplerError::Invalid("terminal spacing failed to advance"));
            }
            distance = next;
        }
        emit_record(
            &mut out,
            terminal,
            target_radius,
            target_alpha,
            mean_orientation,
            tile,
            input.color_argb,
        );
    } else if out.is_empty() {
        emit_record(
            &mut out,
            terminal,
            target_radius,
            target_alpha,
            mean_orientation,
            tile,
            input.color_argb,
        );
    }
    Ok(out)
}

fn emit_record(
    out: &mut Vec<SampledRecord>,
    point: P,
    radius: f32,
    alpha: f32,
    orientation: f32,
    tile: u32,
    color: u32,
) {
    // The source-confirmed AddPoint writer: x/y center unchanged, extents use
    // radius/inverse-scale with 0.5 padding. Capture profile's inverse scales are 1.
    let stamp_radius = radius * 0.5; // drawPoint passes half the pen-width state to RTV4.
    let a = stamp_radius;
    let b = stamp_radius;
    let t = a.min(b);
    let k = if t < 0.5 { 0.5 / t } else { 1.0 }.max(1.0);
    let extent_x = (a * k) + 0.5;
    let extent_y = (b * k) + 0.5;
    let alpha_by_width = 1.0 / k;
    let (sine, cosine) = host_sincosf(orientation);
    let rgb = [
        ((color >> 16) & 0xff) as f32 / 255.0,
        ((color >> 8) & 0xff) as f32 / 255.0,
        (color & 0xff) as f32 / 255.0,
    ];
    let mut lanes = [0.0; 14];
    lanes[0] = point.x;
    lanes[1] = point.y;
    lanes[2] = extent_x;
    lanes[3] = extent_y;
    lanes[4] = alpha;
    lanes[5] = 0.0; // no dash/effect in the supported native fixture profile.
    lanes[6] = tile as f32;
    lanes[7] = -sine;
    lanes[8] = cosine;
    let min_extent = extent_x.min(extent_y);
    lanes[9] = ((min_extent - 1.0) / min_extent) * 0.5;
    lanes[10] = alpha_by_width;
    lanes[11..14].copy_from_slice(&rgb);
    out.push(SampledRecord { lanes });
}

fn decode_hex(s: &str) -> Result<Vec<u8>, SamplerError> {
    if !s.len().is_multiple_of(2) {
        return Err(SamplerError::Invalid("odd-length hex"));
    }
    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| SamplerError::Invalid("invalid hex"))
        })
        .collect()
}

fn capture_f32(hex: &str) -> Result<f32, SamplerError> {
    let bytes = decode_hex(hex)?;
    if bytes.len() != 4 {
        return Err(SamplerError::Invalid("expected four f32 bytes"));
    }
    Ok(f32::from_le_bytes(bytes.try_into().unwrap()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use sha2::{Digest, Sha256};

    const FIXTURE: &str = include_str!("../../../../conformance/pencil2-v14-native.json");
    const NATIVE_RECORDS: &[u8] = include_bytes!("../../../../conformance/pencil2-v14-native.bin");

    fn fixture() -> Value {
        serde_json::from_str(FIXTURE).expect("valid compact Pencil2 V14 fixture")
    }

    fn sha256(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    fn hex_u32(value: &str) -> Result<u32, SamplerError> {
        u32::from_str_radix(value.trim_start_matches("0x"), 16)
            .map_err(|_| SamplerError::Invalid("u32 hex"))
    }

    fn array_f32(value: &Value, key: &str) -> Result<Vec<f32>, SamplerError> {
        value
            .get(key)
            .and_then(Value::as_array)
            .ok_or(SamplerError::Invalid("input channel array"))?
            .iter()
            .map(|v| {
                v.as_f64()
                    .map(|x| x as f32)
                    .ok_or(SamplerError::Invalid("input channel value"))
            })
            .collect()
    }

    fn saved_input(case: &Value) -> Result<SavedInput, SamplerError> {
        let input = case.get("input").ok_or(SamplerError::Invalid("input"))?;
        let route = case.get("route").ok_or(SamplerError::Invalid("route"))?;
        let profile = case
            .get("profile")
            .ok_or(SamplerError::Invalid("controlled context"))?;
        if profile.get("width").and_then(Value::as_u64) != Some(1920)
            || profile.get("height").and_then(Value::as_u64) != Some(1080)
        {
            return Err(SamplerError::Unsupported("unobserved IContext dimensions"));
        }
        if route.get("selected_drawable_class").and_then(Value::as_str)
            != Some("SPen::Pencil2StrokeDrawableGLV14")
            || route.get("selected_rt_class").and_then(Value::as_str)
                != Some("Pencil2StrokeDrawableRTV4")
        {
            return Err(SamplerError::Unsupported(
                "drawable/RT route is not V14/RTV4",
            ));
        }
        if input.get("advanced_setting_state").and_then(Value::as_str)
            != Some("explicit literal 14; set on ObjectStroke")
        {
            return Err(SamplerError::Unsupported(
                "advanced setting is not explicit 14",
            ));
        }
        let motion_sources = case
            .get("motion_sources")
            .and_then(Value::as_array)
            .ok_or(SamplerError::Invalid("native MotionEvent source reads"))?;
        if motion_sources.is_empty() || motion_sources.iter().any(|v| v.as_str() != Some("0x0")) {
            return Err(SamplerError::Unsupported("MotionEvent source is not 0"));
        }
        let points = input
            .get("points")
            .and_then(Value::as_array)
            .ok_or(SamplerError::Invalid("points"))?
            .iter()
            .map(|p| {
                let pair = p.as_array().ok_or(SamplerError::Invalid("point pair"))?;
                if pair.len() != 2 {
                    return Err(SamplerError::Invalid("point pair length"));
                }
                Ok(P {
                    x: pair[0].as_f64().ok_or(SamplerError::Invalid("point x"))? as f32,
                    y: pair[1].as_f64().ok_or(SamplerError::Invalid("point y"))? as f32,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let timestamps = input
            .get("timestamps")
            .and_then(Value::as_array)
            .ok_or(SamplerError::Invalid("timestamps"))?
            .iter()
            .map(|t| t.as_i64().ok_or(SamplerError::Invalid("timestamp")))
            .collect::<Result<Vec<_>, _>>()?;
        let size_bits = input
            .get("object_stroke_size_f32_bits")
            .and_then(Value::as_str)
            .ok_or(SamplerError::Invalid("object stroke size bits"))?;
        let effective_size_bits = case
            .get("effective_size_f32_bits")
            .and_then(Value::as_str)
            .ok_or(SamplerError::Invalid("effective pen size bits"))?;
        let color = input
            .get("object_stroke_color_argb")
            .and_then(Value::as_str)
            .ok_or(SamplerError::Invalid("ARGB color"))?;
        Ok(SavedInput {
            advanced_version: if route
                .get("selected_drawable_version")
                .and_then(Value::as_str)
                == Some("v14")
            {
                14
            } else {
                0
            },
            tool_type: input
                .get("tool_type")
                .and_then(Value::as_u64)
                .ok_or(SamplerError::Invalid("tool type"))? as u8,
            source: 0,
            stroke_type: input
                .get("stroke_type")
                .and_then(Value::as_u64)
                .ok_or(SamplerError::Invalid("stroke type"))? as u8,
            curve_enabled: true,
            color_argb: hex_u32(color)?,
            size: capture_f32(size_bits)?,
            effective_size: capture_f32(effective_size_bits)?,
            points,
            pressure: array_f32(input, "pressure")?,
            tilt: array_f32(input, "tilt")?,
            orientation: array_f32(input, "orientation")?,
            timestamps,
        })
    }

    fn native_alpha_state(case: &Value) -> Result<AlphaState, SamplerError> {
        let native = case
            .get("alpha_state")
            .ok_or(SamplerError::MissingNativeState("alpha_state"))?;
        if native.get("entry_rva").and_then(Value::as_str) != Some("0x744c4")
            || native.get("entry_word").and_then(Value::as_str) != Some("082440f9")
        {
            return Err(SamplerError::Unsupported(
                "getPressureAlpha entry is unpinned",
            ));
        }
        let wrapper = decode_hex(
            native
                .get("wrapper_20_bytes")
                .and_then(Value::as_str)
                .ok_or(SamplerError::MissingNativeState("first-level state bytes"))?,
        )?;
        let data = decode_hex(
            native
                .get("data_20_bytes")
                .and_then(Value::as_str)
                .ok_or(SamplerError::MissingNativeState("second-level state bytes"))?,
        )?;
        if wrapper.len() != 20 || data.len() != 20 {
            return Err(SamplerError::MissingNativeState("20-byte state snapshots"));
        }
        let level_hex = native
            .get("size_level_i32")
            .and_then(Value::as_str)
            .ok_or(SamplerError::MissingNativeState("native size level"))?;
        let level = decode_hex(level_hex)?;
        if level.len() != 4 {
            return Err(SamplerError::MissingNativeState("4-byte size level"));
        }
        let state = AlphaState {
            alpha_byte: data[7],
            square_pressure_flag: wrapper[16],
            base: f32::from_le_bytes(data[0..4].try_into().unwrap()),
            size_level: i32::from_le_bytes(level.try_into().unwrap()),
        };
        if state.base.to_bits()
            != capture_f32(
                case.get("effective_size_f32_bits")
                    .and_then(Value::as_str)
                    .ok_or(SamplerError::Invalid("effective pen size bits"))?,
            )?
            .to_bits()
        {
            return Err(SamplerError::Unsupported(
                "native alpha base does not match effective size",
            ));
        }
        if native.get("pending_returns").and_then(Value::as_u64) != Some(0) {
            return Err(SamplerError::MissingNativeState("all native alpha returns"));
        }
        let observations = native
            .get("input_return_bits")
            .and_then(Value::as_array)
            .ok_or(SamplerError::MissingNativeState(
                "native alpha return observations",
            ))?;
        if observations.is_empty() {
            return Err(SamplerError::MissingNativeState(
                "native alpha return observations",
            ));
        }
        for pair in observations {
            let pair = pair
                .as_array()
                .ok_or(SamplerError::Invalid("alpha return pair"))?;
            if pair.len() != 2 {
                return Err(SamplerError::Invalid("alpha return pair length"));
            }
            let pressure = f32::from_bits(hex_u32(
                pair[0]
                    .as_str()
                    .ok_or(SamplerError::Invalid("pressure bits"))?,
            )?);
            let expected = hex_u32(
                pair[1]
                    .as_str()
                    .ok_or(SamplerError::Invalid("native alpha bits"))?,
            )?;
            if pressure_alpha(pressure, state).to_bits() != expected {
                return Err(SamplerError::Unsupported(
                    "pressure-alpha equation disagrees with native return",
                ));
            }
        }
        Ok(state)
    }

    fn compare_case(case: &Value) {
        let alpha = native_alpha_state(case).unwrap();
        let input = saved_input(case).unwrap();
        assert_eq!(input.effective_size.to_bits(), alpha.base.to_bits());
        let got = sample(&input, alpha).unwrap();
        let desc = &case["expected_stream"];
        let offset = desc["offset"].as_u64().unwrap() as usize;
        let length = desc["length"].as_u64().unwrap() as usize;
        assert_eq!(length, got.len() * 56, "{} length", case["id"]);
        let end = offset.checked_add(length).expect("fixture range overflow");
        let expected = &NATIVE_RECORDS[offset..end];
        assert_eq!(sha256(expected), desc["sha256"].as_str().unwrap());
        assert_eq!(got.len(), desc["records"].as_u64().unwrap() as usize);
        for (record_index, record) in got.iter().enumerate() {
            for lane in 0..14 {
                let start = record_index * 56 + lane * 4;
                let expected_bits =
                    u32::from_le_bytes(expected[start..start + 4].try_into().unwrap());
                assert_eq!(
                    record.lanes[lane].to_bits(),
                    expected_bits,
                    "{} record {record_index}, lane {lane}",
                    case["id"]
                );
            }
        }
    }

    #[test]
    fn three_native_v14_streams_match_all_fourteen_lanes() {
        let f = fixture();
        assert_eq!(f["schema"], "sdocx-pencil2-glv14-rtv4-native-geometry-v1");
        let provenance = &f["provenance"];
        assert_eq!(
            provenance["apk_sha256"],
            "daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667"
        );
        assert_eq!(
            provenance["pencil2_dso_sha256"],
            "3e7d7118ea0fea370d491a5582ac81c09dc063ac75dbcc396abdc32976a42b0f"
        );
        assert_eq!(
            provenance["base_dso_sha256"],
            "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb"
        );
        assert_eq!(f["record_layout"]["stride_bytes"], 56);
        assert_eq!(
            NATIVE_RECORDS.len(),
            f["binary"]["length"].as_u64().unwrap() as usize
        );
        assert_eq!(
            sha256(NATIVE_RECORDS),
            f["binary"]["sha256"].as_str().unwrap()
        );
        let cases = f["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 3);
        for case in cases {
            compare_case(case);
        }
    }

    #[test]
    fn unsupported_saved_inputs_are_rejected() {
        let f = fixture();
        let mut input = saved_input(&f["cases"][0]).unwrap();
        input.advanced_version = 13;
        assert!(matches!(
            sample(&input, native_alpha_state(&f["cases"][0]).unwrap()),
            Err(SamplerError::Unsupported(_))
        ));
        input.advanced_version = 14;
        input.color_argb = 0x0000_0000;
        assert!(matches!(
            sample(&input, native_alpha_state(&f["cases"][0]).unwrap()),
            Err(SamplerError::Unsupported(_))
        ));
    }
}

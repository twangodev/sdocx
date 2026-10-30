//! Bounded, test-only Crayon2 V2 saved-ObjectStroke geometry reconstruction.
//!
//! This models the captured stylus/curve V2 path through Start, DrawLine,
//! DrawEndLine, Crayon2 drawPoint and six-f32 RTV1 AddPoint records. The RNG
//! profile is the captured host TYPE_3 adapter, not Android runtime execution.
//! The optional material experiment consumes explicit host inputs; it does not
//! participate in public renderer dispatch.
#[cfg(feature = "render")]
mod material;

use super::path::{P, Quad};
use std::fmt;

const MAX_SOURCE_POINTS: usize = 65_536;
const MAX_DRAWPOINT_INPUTS: usize = 65_536;
const MAX_RNG_CALLS: usize = MAX_DRAWPOINT_INPUTS * 4;
const MAX_RECORDS: usize = MAX_DRAWPOINT_INPUTS;
const NATIVE_SEED_MODULUS: i32 = 100_000;
const PHASE_MODULUS: f32 = 150.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct SourceInput<'a> {
    pub advanced_version: u8,
    pub tool_type: u8,
    pub stroke_type: u8,
    pub curve_enabled: bool,
    pub points: &'a [P],
    pub pressure: &'a [f32],
    pub size: f32,
}

/// Captured native phase tables and host RNG boundary. Table values come from
/// Crayon2 DSO SHA 4e9de0e6… at RVAs 0x3c278 and 0x3c340.
#[derive(Clone, Copy, Debug)]
pub(crate) struct NativeProfile {
    pub phase_x: [u32; 50],
    pub phase_y: [u32; 50],
    pub rng: RngProfile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RngProfile {
    /// Captured host TYPE_3 test adapter; each drawPoint seeds from the current
    /// native uint32 state, then advances that state with signed int32 + 1
    /// and signed remainder by 100000. This does not promise glibc, Bionic, or
    /// Android RNG behavior.
    CapturedHostType3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Record {
    pub lanes: [f32; 6],
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Output {
    /// drawPoint arguments before Crayon2 jitter, retaining pressure and size.
    pub drawpoint_inputs: Vec<[f32; 4]>,
    pub records: Vec<Record>,
    pub rand_values: Vec<u32>,
    pub phase_updates: u32,
}

/// Adapt retained stroke channels to the captured host sampler. The caller
/// supplies the host RNG and phase tables; this does not select a renderer.
fn sample_stroke(stroke: &crate::Stroke, profile: &NativeProfile) -> Result<Output, Error> {
    if stroke.points.len() > MAX_SOURCE_POINTS || stroke.pressures.len() > MAX_SOURCE_POINTS {
        return Err(Error::Limit("source point count"));
    }
    let rendering = stroke
        .rendering
        .as_ref()
        .ok_or(Error::Unsupported("missing rendering metadata"))?;
    if rendering.pen_name.as_deref() != Some("com.samsung.android.sdk.pen.pen.preload.Crayon2") {
        return Err(Error::Unsupported("pen is not Crayon2"));
    }
    if rendering.advanced_settings.as_deref() != Some("2;") {
        return Err(Error::Unsupported("advanced setting is not captured V2"));
    }
    if rendering.style.stroke_type_raw != Some(0) {
        return Err(Error::Unsupported("stroke type is missing or not zero"));
    }
    let size = rendering
        .style
        .pen_size
        .ok_or(Error::Unsupported("missing native pen size"))?;
    let points: Vec<P> = stroke
        .points
        .iter()
        .map(|p| P {
            x: p.x as f32,
            y: p.y as f32,
        })
        .collect();
    let pressure: Vec<f32> = stroke.pressures.iter().map(|p| *p as f32).collect();
    sample(
        &SourceInput {
            advanced_version: 2,
            tool_type: u8::try_from(rendering.tool_type_raw)
                .map_err(|_| Error::Unsupported("tool type out of range"))?,
            stroke_type: 0,
            curve_enabled: rendering.properties.compressed,
            points: &points,
            pressure: &pressure,
            size,
        },
        profile,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Error {
    Unsupported(&'static str),
    Invalid(&'static str),
    Limit(&'static str),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(s) => write!(f, "unsupported Crayon2 V2 input: {s}"),
            Self::Invalid(s) => write!(f, "invalid Crayon2 V2 input: {s}"),
            Self::Limit(s) => write!(f, "Crayon2 V2 sample limit: {s}"),
        }
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug)]
struct Point {
    x: f32,
    y: f32,
}
impl Point {
    fn midpoint(a: Self, b: Self) -> Self {
        Self {
            x: (a.x + b.x) * 0.5,
            y: (a.y + b.y) * 0.5,
        }
    }
    fn path(self) -> P {
        P {
            x: self.x,
            y: self.y,
        }
    }
}
impl From<P> for Point {
    fn from(p: P) -> Self {
        Self { x: p.x, y: p.y }
    }
}

/// Reconstruct saved V2 drawing from source channels with a named native
/// profile. Inputs and outputs are capped before allocation/emission.
pub(crate) fn sample<'a, 'source>(
    input: &'a SourceInput<'source>,
    profile: &'a NativeProfile,
) -> Result<Output, Error> {
    if input.advanced_version != 2 {
        return Err(Error::Unsupported("advanced setting is not V2"));
    }
    if input.tool_type != 2 {
        return Err(Error::Unsupported("only stylus tool type 2 is captured"));
    }
    if input.stroke_type != 0 {
        return Err(Error::Unsupported("only stroke type 0 is captured"));
    }
    if !input.curve_enabled {
        return Err(Error::Unsupported("noncurve V2 branch is not captured"));
    }
    if input.points.len() < 2 || input.points.len() != input.pressure.len() {
        return Err(Error::Invalid("point and pressure channel lengths"));
    }
    if input.points.len() > MAX_SOURCE_POINTS {
        return Err(Error::Limit("source point count"));
    }
    let extent = input.size + 1.5;
    if !input.size.is_finite()
        || input.size <= 0.0
        || !extent.is_finite()
        || extent < 1.0
        || extent >= i32::MAX as f32
    {
        return Err(Error::Invalid("native pen size"));
    }
    if input
        .points
        .iter()
        .any(|p| !p.x.is_finite() || !p.y.is_finite())
        || input.pressure.iter().any(|p| !p.is_finite() || *p < 0.0)
    {
        return Err(Error::Invalid("nonfinite or negative source channel"));
    }
    let first_x = input.points[0].x;
    if first_x < i32::MIN as f32 || first_x >= -(i32::MIN as f32) {
        return Err(Error::Invalid(
            "initial native seed coordinate is outside int32",
        ));
    }
    if profile.rng != RngProfile::CapturedHostType3 {
        return Err(Error::Unsupported("unobserved RNG profile"));
    }

    let mut core = Core {
        input,
        profile,
        output: Output {
            drawpoint_inputs: Vec::new(),
            records: Vec::new(),
            rand_values: Vec::new(),
            phase_updates: 0,
        },
        seed: Point::from(input.points[0]).x as i32 as u32,
        rng: Type3::seeded(Point::from(input.points[0]).x as i32 as u32),
        phase: Point::from(input.points[0]),
        phase_counter: 0,
        draw_line_counter: 0,
    };
    core.start(Point::from(input.points[0]))?;
    let mut midpoint = Point::from(input.points[0]);
    for i in 1..input.points.len() - 1 {
        core.draw_line(
            Point::from(input.points[i - 1]),
            Point::from(input.points[i]),
            input.pressure[i],
            midpoint,
        )?;
        midpoint = Point::midpoint(
            Point::from(input.points[i - 1]),
            Point::from(input.points[i]),
        );
    }
    let n = input.points.len();
    core.draw_end(
        Point::from(input.points[n - 2]),
        Point::from(input.points[n - 1]),
        input.pressure[n - 1],
        midpoint,
    )?;
    Ok(core.output)
}

struct Type3 {
    state: [u32; 31],
    front: usize,
    rear: usize,
}
impl Type3 {
    fn seeded(seed: u32) -> Self {
        let mut state = [0u32; 31];
        state[0] = seed;
        for i in 1..31 {
            let previous = state[i - 1];
            let mut next = 16_807i64 * i64::from(previous % 127_773)
                - 2_836i64 * i64::from(previous / 127_773);
            if next <= 0 {
                next += 0x7fff_ffff;
            }
            state[i] = next as u32;
        }
        let mut rng = Self {
            state,
            front: 3,
            rear: 0,
        };
        for _ in 0..310 {
            rng.next();
        }
        rng
    }
    fn next(&mut self) -> u32 {
        let sum = self.state[self.front].wrapping_add(self.state[self.rear]);
        self.state[self.front] = sum;
        let value = (sum >> 1) & 0x7fff_ffff;
        self.front = (self.front + 1) % 31;
        self.rear = (self.rear + 1) % 31;
        value
    }
}

struct Core<'a, 'source> {
    input: &'a SourceInput<'source>,
    profile: &'a NativeProfile,
    output: Output,
    seed: u32,
    rng: Type3,
    phase: Point,
    phase_counter: u32,
    draw_line_counter: u32,
}
impl Core<'_, '_> {
    fn rand(&mut self) -> Result<u32, Error> {
        if self.output.rand_values.len() >= MAX_RNG_CALLS {
            return Err(Error::Limit("RNG call count"));
        }
        let value = self.rng.next();
        self.output.rand_values.push(value);
        Ok(value)
    }
    fn stamp(&mut self, p: Point) -> Result<(), Error> {
        if self.output.drawpoint_inputs.len() >= MAX_DRAWPOINT_INPUTS {
            return Err(Error::Limit("drawPoint input count"));
        }
        self.rng = Type3::seeded(self.seed);
        self.seed = ((self.seed as i32).wrapping_add(1) % NATIVE_SEED_MODULUS) as u32;
        let extent = self.input.size + 1.5;
        let divisor = extent as i32;
        let half = extent * 0.5;
        let jx = (self.rand()? as i32 % divisor) as f32 - half;
        let jy = (self.rand()? as i32 % divisor) as f32 - half;
        let q = Point {
            x: p.x + jx,
            y: p.y + jy,
        };
        self.output
            .drawpoint_inputs
            .push([p.x, p.y, self.input.size, self.input.size]);
        let dx = p.x - q.x;
        let dy = p.y - q.y;
        if (dx * dx + dy * dy).sqrt() > half {
            return Ok(());
        }
        let mut radius_extent = extent / 6.0 + 1.0;
        let radius_half = radius_extent * 0.5;
        radius_extent = radius_half + (self.rand()? as i32 % radius_extent as i32) as f32;
        let selector = (self.rand()? % 3) as f32;
        if self.output.records.len() >= MAX_RECORDS {
            return Err(Error::Limit("RTV1 record count"));
        }
        self.output.records.push(Record {
            lanes: [
                q.x,
                q.y,
                radius_extent * 0.5,
                selector,
                self.phase.x,
                self.phase.y,
            ],
        });
        Ok(())
    }
    fn start(&mut self, first: Point) -> Result<(), Error> {
        for _ in 0..7 {
            self.stamp(first)?;
        }
        Ok(())
    }
    fn emit_path(
        &mut self,
        start: Point,
        control: Point,
        end: Point,
        pressure: f32,
    ) -> Result<(), Error> {
        let path = Quad::new(start.path(), control.path(), end.path());
        let length = path.length;
        if !length.is_finite() {
            return Err(Error::Invalid("nonfinite quadratic length"));
        }
        if length < 1.0 {
            return Ok(());
        }
        let base_float = 20.0 / self.input.size;
        let max_factor = (i32::MAX / 40) as f32;
        if !base_float.is_finite() || base_float <= 0.0 || base_float > max_factor {
            return Err(Error::Limit("native spacing factor"));
        }
        let base = base_float as i32;
        let factor_float = if length < 2.0 {
            base as f32
        } else {
            base as f32 + length * 0.25
        };
        if !factor_float.is_finite() || factor_float < 0.0 || factor_float > max_factor {
            return Err(Error::Limit("native spacing factor"));
        }
        let factor = (factor_float as i32).max(1);
        let pressure_factor = (pressure.sqrt() as f64 + 0.2f64) as f32;
        let count = (pressure_factor * ((factor * 40) as f32)) as i32;
        if count <= 0 {
            return Ok(());
        }
        if count as usize > MAX_DRAWPOINT_INPUTS.saturating_sub(self.output.drawpoint_inputs.len())
        {
            return Err(Error::Limit("path drawPoint count"));
        }
        let step = length / count as f32;
        let mut distance = 0.0f32;
        for _ in 0..count {
            let p = Point::from(path.at(distance));
            self.stamp(p)?;
            distance += step;
        }
        Ok(())
    }
    fn advance_line_phase(&mut self) {
        self.draw_line_counter = self.draw_line_counter.wrapping_add(1);
        let hash = self
            .draw_line_counter
            .wrapping_mul(0x8af8_af8b)
            .rotate_right(1);
        if hash > 0x03a8_3a83 {
            return;
        }
        let i = (self.phase_counter % 50) as usize;
        self.phase.x = (self.phase.x + self.profile.phase_x[i] as f32) % PHASE_MODULUS;
        self.phase.y = (self.phase.y + self.profile.phase_y[i] as f32) % PHASE_MODULUS;
        self.phase_counter = self.phase_counter.wrapping_add(1);
        self.draw_line_counter = 0;
        self.output.phase_updates = self.phase_counter;
    }
    fn draw_line(
        &mut self,
        previous: Point,
        current: Point,
        pressure: f32,
        midpoint: Point,
    ) -> Result<(), Error> {
        self.emit_path(
            midpoint,
            previous,
            Point::midpoint(previous, current),
            pressure,
        )?;
        self.advance_line_phase();
        Ok(())
    }
    fn draw_end(
        &mut self,
        previous: Point,
        end: Point,
        pressure: f32,
        midpoint: Point,
    ) -> Result<(), Error> {
        self.emit_path(midpoint, previous, end, pressure)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use sha2::{Digest, Sha256};

    const FIXTURE: &str = include_str!("../../tests/assets/crayon2-v2-native/fixture.json");
    const BINARY: &[u8] = include_bytes!("../../tests/assets/crayon2-v2-native/records.bin");

    fn fixture() -> Value {
        serde_json::from_str(FIXTURE).expect("valid compact Crayon2 V2 fixture")
    }
    fn sha(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }
    fn stream<'a>(f: &Value, name: &str) -> &'a [u8] {
        let desc = &f["native"]["streams"][name];
        let start = desc["offset"].as_u64().unwrap() as usize;
        let len = desc["length"].as_u64().unwrap() as usize;
        let data = &BINARY[start..start + len];
        assert_eq!(
            sha(data),
            desc["sha256"].as_str().unwrap(),
            "{name} stream hash"
        );
        data
    }
    fn bits_f32(s: &str) -> f32 {
        f32::from_bits(u32::from_str_radix(s, 16).unwrap())
    }
    fn native_f32(s: &str) -> f32 {
        let bytes = hex_bytes(s);
        f32::from_le_bytes(bytes.try_into().unwrap())
    }
    fn hex_bytes(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }
    fn parse_source(f: &Value) -> (Vec<P>, Vec<f32>) {
        parse_source_channels(&f["source"])
    }
    fn parse_source_channels(source: &Value) -> (Vec<P>, Vec<f32>) {
        let xy = source["points_f32_bits"].as_array().unwrap();
        let points = xy
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| P {
                x: bits_f32(p[0].as_str().unwrap()),
                y: bits_f32(p[1].as_str().unwrap()),
            })
            .collect();
        let pressure = source["pressure_f32_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| bits_f32(p.as_str().unwrap()))
            .collect();
        (points, pressure)
    }
    fn parse_profile(f: &Value) -> NativeProfile {
        let read = |name: &str| {
            let bytes = stream(f, name);
            let mut table = [0u32; 50];
            for (i, slot) in table.iter_mut().enumerate() {
                *slot = u32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap());
            }
            table
        };
        NativeProfile {
            phase_x: read("phase_x_u32_le"),
            phase_y: read("phase_y_u32_le"),
            rng: RngProfile::CapturedHostType3,
        }
    }
    fn output_bytes(output: &Output) -> Vec<u8> {
        output
            .records
            .iter()
            .flat_map(|r| r.lanes.iter().flat_map(|x| x.to_le_bytes()))
            .collect()
    }
    fn drawpoint_xy(output: &Output) -> Vec<u8> {
        output
            .drawpoint_inputs
            .iter()
            .flat_map(|p| p[..2].iter().flat_map(|x| x.to_le_bytes()))
            .collect()
    }
    fn drawpoint_pressure(output: &Output) -> Vec<u8> {
        output
            .drawpoint_inputs
            .iter()
            .flat_map(|p| p[2].to_le_bytes())
            .collect()
    }
    fn drawpoint_xy_size(output: &Output) -> Vec<u8> {
        output
            .drawpoint_inputs
            .iter()
            .flat_map(|p| p[..3].iter().flat_map(|x| x.to_le_bytes()))
            .collect()
    }
    fn rng_bytes(output: &Output) -> Vec<u8> {
        output
            .rand_values
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect()
    }

    #[test]
    fn native_v2_geometry_matches_source_channels_and_all_outputs() {
        let f = fixture();
        assert_eq!(f["schema"], "sdocx-crayon2-v2-native-geometry-v1");
        assert_eq!(
            f["provenance"]["crayon2_dso_sha256"],
            "4e9de0e699eb3781e7cdb17d6816370ce3e2f75cf6fe2087fada50cfa816739e"
        );
        assert_eq!(
            f["provenance"]["base_dso_sha256"],
            "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb"
        );
        assert_eq!(
            f["native"]["binary_file"],
            "conformance/crayon2-v2-native.bin"
        );
        assert_eq!(sha(BINARY), f["native"]["binary_sha256"].as_str().unwrap());
        let (points, pressure) = parse_source(&f);
        assert_eq!(f["profile"]["advanced_setting"], "2;");
        assert_eq!(f["profile"]["tool_type"], 2);
        assert_eq!(f["profile"]["stroke_type"], 0);
        assert_eq!(f["profile"]["curve_enabled"], true);
        let input = SourceInput {
            advanced_version: 2,
            tool_type: 2,
            stroke_type: 0,
            curve_enabled: true,
            points: &points,
            pressure: &pressure,
            size: native_f32(f["profile"]["size_f32_le_hex"].as_str().unwrap()),
        };
        let output = sample(&input, &parse_profile(&f)).expect("bounded V2 source sampler");
        assert_eq!(
            output.drawpoint_inputs.len(),
            f["native"]["drawpoint_count"]
        );
        assert_eq!(output.records.len(), f["native"]["record_count"]);
        assert_eq!(output.rand_values.len(), f["native"]["rand_count"]);
        for (name, got) in [
            ("drawpoint_xy", drawpoint_xy(&output)),
            ("drawpoint_pressure", drawpoint_pressure(&output)),
            ("rng_u32_le", rng_bytes(&output)),
            ("rtv1_records", output_bytes(&output)),
        ] {
            let expected = stream(&f, name);
            assert_eq!(got.len(), expected.len(), "{name} byte length");
            assert_eq!(
                sha(&got),
                f["native"]["expected_sha256"][name].as_str().unwrap(),
                "{name} SHA-256"
            );
            assert_eq!(got, expected, "{name} native stream parity");
        }
    }

    #[test]
    fn changed_path_paint_capture_matches_source_derived_records() {
        let points = [
            P { x: 0.0, y: 0.0 },
            P { x: 8.0, y: 6.0 },
            P { x: 18.0, y: -4.0 },
        ];
        let pressure = [0.3, 0.6, 1.0];
        let input = SourceInput {
            advanced_version: 2,
            tool_type: 2,
            stroke_type: 0,
            curve_enabled: true,
            points: &points,
            pressure: &pressure,
            size: 10.0,
        };
        let output = sample(&input, &parse_profile(&fixture())).unwrap();
        assert_eq!(output.records.len(), 303);
        assert_eq!(
            sha(&output_bytes(&output)),
            "4e7d8107c77d80a2d7fa9d16816c7fa994109cd692a2c38a51d807f4b1a15ec9"
        );
    }

    fn matched_stroke(points: &[P], pressure: &[f32]) -> crate::Stroke {
        let style = crate::StrokeStyle {
            pen_size: Some(10.0),
            stroke_type_raw: Some(0),
            ..Default::default()
        };
        let rendering = crate::StrokeRendering {
            metadata: None,
            pen_name: Some("com.samsung.android.sdk.pen.pen.preload.Crayon2".into()),
            advanced_settings: Some("2;".into()),
            tool_type_raw: 2,
            style,
            properties: crate::StrokeProperties {
                compressed: true,
                replay_only: false,
                stylus_channels: false,
                eraser: false,
                fixed_width: false,
                millisecond_timestamps: false,
                top_layer_pen: false,
                alpha_lock: false,
                binary_added: true,
                generated: false,
                fixed_opacity: false,
                rainbow_effect: false,
                straighten: false,
                reveal_mode: false,
            },
        };
        crate::Stroke {
            rendering: Some(rendering),
            bbox: crate::BoundingBox::default(),
            points: points
                .iter()
                .map(|p| crate::Point {
                    x: p.x as f64,
                    y: p.y as f64,
                })
                .collect(),
            pressures: pressure.iter().map(|p| *p as f64).collect(),
            timestamps: vec![100, 110, 120],
            tilts: vec![0.0; 3],
            orientations: vec![0.0; 3],
            color: Some(crate::Color {
                r: 51,
                g: 102,
                b: 153,
            }),
            pen_width: 10.0,
        }
    }

    #[cfg(feature = "render")]
    #[test]
    #[ignore = "requires local native texture inputs and writes candidate SVG"]
    fn write_stroke_material_svg() {
        let points = [
            P { x: 64.0, y: 64.0 },
            P { x: 72.0, y: 70.0 },
            P { x: 82.0, y: 60.0 },
        ];
        let stroke = matched_stroke(&points, &[0.3, 0.6, 1.0]);
        let output = sample_stroke(&stroke, &parse_profile(&fixture())).unwrap();
        let textures = std::env::var_os("SDOCX_CRAYON2_TEXTURE_DIR").expect("texture directory");
        let textures = std::path::Path::new(&textures);
        let point = std::fs::read(textures.join("g_Crayon2PointPtn.source.bin")).unwrap();
        let background = std::fs::read(textures.join("g_Crayon2BgPtn.source.bin")).unwrap();
        assert_eq!(
            sha(&point),
            "673423c6fdae1ba344c452deefeb51b7b11365fead0bfbdd4b345eed4027f405"
        );
        assert_eq!(
            sha(&background),
            "2df7013777862212381e424903db7b1194379db9c6fe6bb3dafa71a56573b2f3"
        );
        let projection = material::Projection {
            width: 256,
            height: 256,
            matrix: [
                0.0078125,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0078125,
                0.0,
                0.0,
                0.0,
                0.0,
                -0.000030518509447574615,
                0.0,
                -1.0,
                -1.0,
                0.0,
                1.0,
            ],
            bg_scale: [2.2755556106567383, 2.2755556106567383],
        };
        let svg = material::emit_svg(
            &output.records,
            &point,
            &background,
            projection,
            [51.0 / 255.0, 102.0 / 255.0, 153.0 / 255.0, 128.0 / 255.0],
            Some([64, 128, 192]),
            0.0,
        )
        .unwrap();
        std::fs::write(
            std::env::var_os("SDOCX_CRAYON2_SVG").expect("output SVG"),
            svg,
        )
        .unwrap();
    }

    #[test]
    fn translated_source_paint_capture_matches_source_derived_records() {
        use std::io::Read;
        let compressed =
            include_bytes!("../../tests/assets/crayon2-v2-native/matched-opaque.json.gz");
        let mut json = Vec::new();
        flate2::read::GzDecoder::new(&compressed[..])
            .read_to_end(&mut json)
            .unwrap();
        let capture: Value = serde_json::from_slice(&json).unwrap();
        let case = &capture["source_capture"]["cases"][4];
        let metadata = &case["native_model_metadata"];
        assert_eq!(metadata["tool_type"], 2);
        assert_eq!(metadata["stroke_type"], 0);
        assert_eq!(metadata["curve_enabled"], true);
        assert_eq!(metadata["color_argb"], "0x80336699");
        assert_eq!(case["advanced_setting"], "2;");
        assert_eq!(case["source_model_get_pen_size_f32_bits"], "00002041");
        let (points, pressure) = parse_source_channels(&case["source_arrays"]);
        assert_eq!(
            points.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>(),
            [(64.0, 64.0), (72.0, 70.0), (82.0, 60.0)]
        );
        assert_eq!(pressure, [0.3, 0.6, 1.0]);
        let input = SourceInput {
            advanced_version: 2,
            tool_type: 2,
            stroke_type: 0,
            curve_enabled: true,
            points: &points,
            pressure: &pressure,
            size: 10.0,
        };
        let output = sample(&input, &parse_profile(&fixture())).unwrap();
        let native = hex_bytes(capture["paint"]["native_vector_hex"].as_str().unwrap());
        assert_eq!(
            sha(&native),
            capture["paint"]["native_vector_sha256"].as_str().unwrap()
        );
        assert_eq!(output.records.len(), native.len() / 24);
        assert_eq!(output_bytes(&output), native);
        let mut stroke = matched_stroke(&points, &pressure);
        let profile = parse_profile(&fixture());
        assert_eq!(
            output_bytes(&sample_stroke(&stroke, &profile).unwrap()),
            native
        );
        stroke.rendering.as_mut().unwrap().style.stroke_type_raw = None;
        assert!(matches!(
            sample_stroke(&stroke, &profile),
            Err(Error::Unsupported(_))
        ));
        stroke.rendering.as_mut().unwrap().style.stroke_type_raw = Some(0);
        stroke.rendering.as_mut().unwrap().properties.compressed = false;
        assert!(matches!(
            sample_stroke(&stroke, &profile),
            Err(Error::Unsupported(_))
        ));
        stroke.rendering.as_mut().unwrap().properties.compressed = true;
        stroke.rendering.as_mut().unwrap().pen_name = None;
        assert!(matches!(
            sample_stroke(&stroke, &profile),
            Err(Error::Unsupported(_))
        ));
        stroke.rendering.as_mut().unwrap().pen_name =
            Some("com.samsung.android.sdk.pen.pen.preload.Crayon2".into());
        stroke.rendering.as_mut().unwrap().advanced_settings = Some("1;".into());
        assert!(matches!(
            sample_stroke(&stroke, &profile),
            Err(Error::Unsupported(_))
        ));
        stroke.rendering.as_mut().unwrap().advanced_settings = Some("2;".into());
        stroke.rendering.as_mut().unwrap().tool_type_raw = 1;
        assert!(matches!(
            sample_stroke(&stroke, &profile),
            Err(Error::Unsupported(_))
        ));
        stroke.rendering.as_mut().unwrap().tool_type_raw = 2;
        stroke.rendering.as_mut().unwrap().style.pen_size = None;
        assert!(matches!(
            sample_stroke(&stroke, &profile),
            Err(Error::Unsupported(_))
        ));
        stroke.rendering.as_mut().unwrap().style.pen_size = Some(10.0);
        stroke.pressures.pop();
        assert!(matches!(
            sample_stroke(&stroke, &profile),
            Err(Error::Invalid(_))
        ));
    }

    #[test]
    fn native_negative_and_phase_controls_match_source_derived_stream_hashes() {
        let f = fixture();
        let profile = parse_profile(&f);
        for control in f["additional_native_controls"].as_array().unwrap() {
            let id = control["control_id"].as_str().unwrap();
            assert_eq!(control["profile"]["advanced_setting"], "2;", "{id}");
            assert_eq!(control["profile"]["tool_type"], 2, "{id}");
            assert_eq!(control["profile"]["stroke_type"], 0, "{id}");
            assert_eq!(control["profile"]["curve_enabled"], true, "{id}");
            let (points, pressure) = parse_source_channels(&control["source"]);
            let input = SourceInput {
                advanced_version: 2,
                tool_type: 2,
                stroke_type: 0,
                curve_enabled: true,
                points: &points,
                pressure: &pressure,
                size: native_f32(control["profile"]["size_f32_le_hex"].as_str().unwrap()),
            };
            let output = sample(&input, &profile).expect("bounded native control sampler");
            let native = &control["native"];
            assert_eq!(
                output.drawpoint_inputs.len(),
                native["drawpoint_count"],
                "{id}"
            );
            assert_eq!(output.records.len(), native["record_count"], "{id}");
            assert_eq!(output.rand_values.len(), native["rand_count"], "{id}");
            assert_eq!(output.phase_updates, native["phase_updates"], "{id}");
            for (name, bytes) in [
                ("drawpoint_xy_size_f32_le", drawpoint_xy_size(&output)),
                ("rng_u32_le", rng_bytes(&output)),
                ("rtv1_six_f32_lanes_le", output_bytes(&output)),
            ] {
                let expected = &native["streams"][name];
                assert_eq!(bytes.len(), expected["bytes"], "{id} {name} byte length");
                assert_eq!(
                    sha(&bytes),
                    expected["sha256"].as_str().unwrap(),
                    "{id} {name} SHA-256"
                );
            }
            if id == "phase-advance-size16" {
                assert_eq!(
                    output.phase_updates, 1,
                    "phase-advance control must cross one native phase step"
                );
                assert_eq!(
                    native["phase_counter_transitions"]
                        .as_array()
                        .unwrap()
                        .len(),
                    1
                );
            }
        }
    }

    #[test]
    fn unsupported_crayon2_routes_are_rejected() {
        let f = fixture();
        let (points, pressure) = parse_source(&f);
        let mut input = SourceInput {
            advanced_version: 2,
            tool_type: 2,
            stroke_type: 0,
            curve_enabled: true,
            points: &points,
            pressure: &pressure,
            size: 10.0,
        };
        let profile = parse_profile(&f);
        input.advanced_version = 1;
        assert!(matches!(
            sample(&input, &profile),
            Err(Error::Unsupported(_))
        ));
        input.advanced_version = 2;
        input.curve_enabled = false;
        assert!(matches!(
            sample(&input, &profile),
            Err(Error::Unsupported(_))
        ));
        input.curve_enabled = true;
        input.size = f32::MAX;
        assert!(matches!(sample(&input, &profile), Err(Error::Invalid(_))));
        input.size = 10.0;
        let out_of_range_seed = [
            P {
                x: 2_147_483_648.0,
                y: 0.0,
            },
            P { x: 0.0, y: 0.0 },
        ];
        input.points = &out_of_range_seed;
        assert!(matches!(sample(&input, &profile), Err(Error::Invalid(_))));
        input.points = &points;
        let short_pressure = [1.0];
        input.pressure = &short_pressure;
        assert!(matches!(sample(&input, &profile), Err(Error::Invalid(_))));
    }
}

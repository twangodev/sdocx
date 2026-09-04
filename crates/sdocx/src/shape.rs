use crate::binary::Reader;
use crate::frame::{Frame, Mask};
use crate::note::parse_shape_text;
use crate::object::read_bbox;
use crate::{BoundingBox, Error, ObjectMetadata, ParseLimits, Result, RichTextBox};

/// Paint supported by the shape renderer, or an uninterpreted native effect.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ShapePaint {
    /// No visible paint.
    None,
    /// Solid color, including alpha, in native ARGB order.
    Solid(u32),
    /// An effect whose rendering semantics are not yet supported.
    Unsupported {
        /// Native effect/color kind, depending on the containing field.
        kind: u8,
        /// Bounded native effect bytes.
        data: Vec<u8>,
    },
}

/// Outline settings shared by native shapes and lines.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct ShapeStyle {
    /// Outline color effect.
    pub paint: ShapePaint,
    /// Width in document coordinates; native default is 2.0.
    pub width: f32,
    /// Native compound-line enum (0 means simple).
    pub compound: u8,
    /// Native dash enum (0 means solid).
    pub dash: u8,
    /// Native cap enum (0 butt, 1 round, 2 square).
    pub cap: u8,
    /// Native join enum (0 miter, 1 round, 2 bevel).
    pub join: u8,
    /// Native begin-arrow type and size.
    pub begin_arrow: [u8; 2],
    /// Native end-arrow type and size.
    pub end_arrow: [u8; 2],
}

impl Default for ShapeStyle {
    fn default() -> Self {
        Self {
            paint: ShapePaint::Solid(0xff000000),
            width: 2.0,
            compound: 0,
            dash: 0,
            cap: 0,
            join: 0,
            begin_arrow: [0; 2],
            end_arrow: [0; 2],
        }
    }
}

/// Geometry and styles decoded from the native `0 + 6 + 7` shape chain.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct NativeShape {
    /// Stored type-0 metadata. Normal shape writers put drawn bounds here.
    pub metadata: ObjectMetadata,
    /// Native shape-template ID; unfamiliar values are preserved.
    pub shape_type: u32,
    /// Unrotated geometry rectangle from type 7.
    pub geometry_bbox: BoundingBox,
    /// Drawn-bounds snapshot stored after control points.
    pub drawn_bbox: BoundingBox,
    /// Geometry rotation from type 7; the normal writer clears type-0 rotation.
    pub rotation_degrees: f32,
    /// Native control points, retained for template-specific interpretation.
    pub control_points: Vec<[f64; 2]>,
    /// Native custom path bytes, retained without guessing path semantics.
    pub path_data: Vec<u8>,
    /// Outline settings.
    pub style: ShapeStyle,
    /// Interior paint.
    pub fill: ShapePaint,
    /// Optional string-resource ID for the native pen name.
    pub pen_name_id: Option<i32>,
    /// Optional string-resource ID for advanced native pen settings.
    pub pen_settings_id: Option<i32>,
    /// Embedded shape text, decoded with the rich-text limits.
    pub text: Option<Box<RichTextBox>>,
}

/// Geometry and styles decoded from the native `0 + 6 + 8` line chain.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct NativeLine {
    /// Stored common identity and placement.
    pub metadata: ObjectMetadata,
    /// Native line kind: 0 straight, 1 elbow, 2 curve.
    pub line_type: u8,
    /// Native routing setting, retained without interpretation.
    pub routing: u8,
    /// Control points in page coordinates.
    pub control_points: Vec<[f64; 2]>,
    /// Start point in page coordinates, already including native rotation.
    pub begin: [f64; 2],
    /// End point in page coordinates, already including native rotation.
    pub end: [f64; 2],
    /// First native geometry rectangle.
    pub geometry_bbox: BoundingBox,
    /// Second native geometry rectangle.
    pub reference_bbox: BoundingBox,
    /// Remaining fixed setting, preserved without assigning a semantic name.
    pub raw_setting: u32,
    /// Optional string-resource ID for the native pen name.
    pub pen_name_id: Option<i32>,
    /// Optional string-resource ID for advanced native pen settings.
    pub pen_settings_id: Option<i32>,
    /// Native custom path bytes, when present.
    pub path_data: Vec<u8>,
    /// Outline settings.
    pub style: ShapeStyle,
}

pub(crate) struct Decoded<T> {
    pub(crate) value: T,
    pub(crate) unsupported: Vec<&'static str>,
}

pub(crate) fn decode_shape(data: &[u8], limits: &ParseLimits) -> Result<Decoded<NativeShape>> {
    let mut reader = Reader::new(data, "shape object");
    let metadata = ObjectMetadata::read(&mut reader)?;
    let mut unsupported = Vec::new();
    let style = read_style(&Frame::read(&mut reader)?, &mut unsupported)?;
    let frame = Frame::read(&mut reader)?;
    frame.expect_kind(7)?;
    let mut fixed = Reader::new(frame.fixed, "shape geometry");
    let shape_type = fixed.read_u32("shape type")?;
    let geometry_bbox = read_bbox(&mut fixed)?;
    let rotation_degrees = finite_f32(&mut fixed, "shape rotation")?;
    let path_data = sized(&mut fixed, "shape path")?.to_vec();
    if !path_data.is_empty() {
        visit_path(&path_data, |_, _| {})?;
    }
    let control_points = read_points(&mut fixed)?;
    let drawn_bbox = read_bbox(&mut fixed)?;
    if fixed.remaining() != 0 || frame.properties.has_other_bits(0) {
        unsupported.push("shape geometry extensions or properties");
    }
    if !path_data.is_empty() || !control_points.is_empty() {
        unsupported.push("custom paths or template control points");
    }
    if !matches!(shape_type, 1..=4 | 8) {
        unsupported.push("shape template");
    }
    if metadata.rotation_degrees.is_some_and(|angle| angle != 0.0)
        || !same_bbox(metadata.bbox, drawn_bbox)
    {
        unsupported.push("additional base placement transform");
    }
    let mut fields = Reader::new(frame.flexible, "shape fields");
    let text = if frame.fields.contains(0) {
        let mut text_metadata = metadata.clone();
        text_metadata.bbox = geometry_bbox;
        text_metadata.rotation_degrees = Some(f64::from(rotation_degrees));
        let decoded = parse_shape_text(&mut fields, limits, text_metadata)?;
        unsupported.extend(decoded.unsupported);
        Some(Box::new(decoded.text_box))
    } else {
        None
    };
    if frame.fields.contains(1) && fields.read_u8("text control")? != 0 {
        unsupported.push("shape text control");
    }
    let pen_name_id = frame
        .fields
        .contains(2)
        .then(|| fields.read_i32("pen name ID"))
        .transpose()?;
    let mut pen_settings_id = None;
    let mut fill = ShapePaint::None;
    if frame.fields.contains(3) {
        unsupported.push("unknown field before shape fill");
    } else {
        pen_settings_id = frame
            .fields
            .contains(4)
            .then(|| fields.read_i32("pen settings ID"))
            .transpose()?;
        if frame.fields.contains(5) {
            let size = fields.read_u32("fill size")? as usize;
            let kind = fields.read_u8("fill kind")?;
            let data = fields.read_bytes(size, "fill effect")?;
            fill = if kind == 1 {
                read_paint(data, false, &mut unsupported)?
            } else {
                unsupported.push("non-color shape fill");
                ShapePaint::Unsupported {
                    kind,
                    data: data.to_vec(),
                }
            };
        }
    }
    if pen_name_id.is_some() || pen_settings_id.is_some() {
        unsupported.push("native pen rendering");
    }
    if fields.remaining() != 0 || frame.fields.has_other_bits(0x37) {
        unsupported.push("additional shape fields");
    }
    read_extensions(&mut reader, &mut unsupported)?;
    Ok(Decoded {
        value: NativeShape {
            metadata,
            shape_type,
            geometry_bbox,
            drawn_bbox,
            rotation_degrees,
            control_points,
            path_data,
            style,
            fill,
            pen_name_id,
            pen_settings_id,
            text,
        },
        unsupported,
    })
}

pub(crate) fn decode_line(data: &[u8]) -> Result<Decoded<NativeLine>> {
    let mut reader = Reader::new(data, "line object");
    let metadata = ObjectMetadata::read(&mut reader)?;
    let mut unsupported = Vec::new();
    let style = read_style(&Frame::read(&mut reader)?, &mut unsupported)?;
    let frame = Frame::read(&mut reader)?;
    frame.expect_kind(8)?;
    let mut fixed = Reader::new(frame.fixed, "line geometry");
    let line_type = fixed.read_u8("line type")?;
    let routing = fixed.read_u8("line routing")?;
    let control_points = read_points(&mut fixed)?;
    let begin = read_point(&mut fixed)?;
    let end = read_point(&mut fixed)?;
    let geometry_bbox = read_bbox(&mut fixed)?;
    let reference_bbox = read_bbox(&mut fixed)?;
    let raw_setting = fixed.read_u32("line setting")?;
    if line_type > 2 || routing != 0 || raw_setting != 0 {
        unsupported.push("line routing, control points or settings");
    }
    if fixed.remaining() != 0 || frame.properties.has_other_bits(0) {
        unsupported.push("line geometry extensions or properties");
    }
    let mut fields = Reader::new(frame.flexible, "line fields");
    let mut pen_name_id = None;
    let mut pen_settings_id = None;
    let mut path_data = Vec::new();
    if !frame.fields.contains(0) {
        pen_settings_id = frame
            .fields
            .contains(1)
            .then(|| fields.read_i32("pen settings ID"))
            .transpose()?;
        pen_name_id = frame
            .fields
            .contains(2)
            .then(|| fields.read_i32("pen name ID"))
            .transpose()?;
        if frame.fields.contains(3) {
            let bytes = &frame.flexible[fields.position()..];
            let (size, supported) = visit_path(bytes, |_, _| {})?;
            path_data = fields.read_bytes(size, "line path")?.to_vec();
            if !supported {
                unsupported.push("unsupported native path commands");
            }
        }
    }
    if pen_name_id.is_some() || pen_settings_id.is_some() {
        unsupported.push("native pen rendering");
    }
    if fields.remaining() != 0 || frame.fields.has_other_bits(0x0e) {
        unsupported.push("additional line fields");
    }
    if line_type != 0 && path_data.is_empty() {
        unsupported.push("non-straight line without a supported path");
    }
    read_extensions(&mut reader, &mut unsupported)?;
    Ok(Decoded {
        value: NativeLine {
            metadata,
            line_type,
            routing,
            control_points,
            begin,
            end,
            geometry_bbox,
            reference_bbox,
            raw_setting,
            pen_name_id,
            pen_settings_id,
            path_data,
            style,
        },
        unsupported,
    })
}

fn read_style(frame: &Frame<'_>, unsupported: &mut Vec<&'static str>) -> Result<ShapeStyle> {
    frame.expect_kind(6)?;
    let mut fixed = Reader::new(frame.fixed, "shape base");
    let magnetic_count = fixed.read_u32("magnetic point count")? as usize;
    let point_bytes = magnetic_count
        .checked_mul(16)
        .ok_or_else(|| Error::Format("magnetic point size overflow".into()))?;
    let mut points = Reader::new(
        fixed.read_bytes(point_bytes, "magnetic points")?,
        "magnetic points",
    );
    for _ in 0..magnetic_count {
        read_point(&mut points)?;
    }
    let mut connections = Reader::new(sized(&mut fixed, "connection block")?, "shape connections");
    let connection_count = connections.read_u32("connection count")?;
    let reserved = fixed.read_u8("reserved shape byte")?;
    if connection_count != 0
        || connections.remaining() != 0
        || reserved != 0
        || fixed.remaining() != 0
        || frame.properties.has_other_bits(0)
    {
        unsupported.push("shape connections or base extensions");
    }
    let mut style = ShapeStyle::default();
    let mut fields = Reader::new(frame.flexible, "shape outline");
    if frame.fields.contains(0) || frame.fields.contains(1) {
        unsupported.push("unknown field before outline effects");
    } else {
        if frame.fields.contains(2) {
            style.paint = read_paint(sized(&mut fields, "line color effect")?, true, unsupported)?;
        }
        if frame.fields.contains(3) {
            let mut effect = Reader::new(sized(&mut fields, "line style effect")?, "line style");
            style.width = finite_f32(&mut effect, "line width")?;
            if style.width < 0.0 {
                return Err(Error::Format("negative line width".into()));
            }
            style.compound = effect.read_u8("compound type")?;
            style.dash = effect.read_u8("dash type")?;
            style.cap = effect.read_u8("cap type")?;
            style.join = effect.read_u8("join type")?;
            style.begin_arrow = [
                effect.read_u8("begin arrow type")?,
                effect.read_u8("begin arrow size")?,
            ];
            style.end_arrow = [
                effect.read_u8("end arrow type")?,
                effect.read_u8("end arrow size")?,
            ];
            if style.compound != 0
                || style.dash != 0
                || style.cap > 2
                || style.join > 2
                || style.begin_arrow != [0; 2]
                || style.end_arrow != [0; 2]
                || effect.remaining() != 0
            {
                unsupported.push("compound/dashed outlines, arrows or style extensions");
            }
        }
    }
    if fields.remaining() != 0 || frame.fields.has_other_bits(0x0c) {
        unsupported.push("additional outline fields");
    }
    Ok(style)
}

fn read_paint(
    data: &[u8],
    outline: bool,
    unsupported: &mut Vec<&'static str>,
) -> Result<ShapePaint> {
    let mut reader = Reader::new(data, "shape color effect");
    let properties = Mask::read(&mut reader)?;
    let kind = if outline {
        reader.read_u8("color type")?
    } else {
        u8::from(properties.contains(0))
    };
    let argb = reader.read_u32("ARGB")?;
    reader.read_u8("gradient type")?;
    reader.read_u16("gradient angle")?;
    finite_f32(&mut reader, "gradient x")?;
    finite_f32(&mut reader, "gradient y")?;
    let stops = reader.read_u8("gradient stop count")?;
    for _ in 0..stops {
        reader.read_u32("gradient color")?;
        finite_f32(&mut reader, "gradient stop")?;
    }
    if properties.has_other_bits(if outline { 1 } else { 3 }) || reader.remaining() != 0 {
        unsupported.push("color effect extensions");
    }
    match kind {
        0 => Ok(ShapePaint::Solid(argb)),
        2 if outline => Ok(ShapePaint::None),
        _ => {
            unsupported.push("gradient or unknown color effect");
            Ok(ShapePaint::Unsupported {
                kind,
                data: data.to_vec(),
            })
        }
    }
}

fn read_points(reader: &mut Reader<'_>) -> Result<Vec<[f64; 2]>> {
    let count = usize::from(reader.read_u8("control point count")?);
    let mut points = Reader::new(
        reader.read_bytes(count * 16, "control points")?,
        "control points",
    );
    (0..count).map(|_| read_point(&mut points)).collect()
}

fn read_point(reader: &mut Reader<'_>) -> Result<[f64; 2]> {
    let point = [reader.read_f64("point x")?, reader.read_f64("point y")?];
    if point.iter().any(|value| !value.is_finite()) {
        return Err(Error::Format("non-finite shape/line point".into()));
    }
    Ok(point)
}

fn finite_f32(reader: &mut Reader<'_>, field: &'static str) -> Result<f32> {
    let value = reader.read_f32(field)?;
    if !value.is_finite() {
        return Err(Error::Format(format!("non-finite {field}")));
    }
    Ok(value)
}

fn sized<'a>(reader: &mut Reader<'a>, field: &'static str) -> Result<&'a [u8]> {
    let size = reader.read_u32(field)? as usize;
    reader.read_bytes(size, field)
}

fn same_bbox(a: BoundingBox, b: BoundingBox) -> bool {
    a.x_min == b.x_min && a.y_min == b.y_min && a.x_max == b.x_max && a.y_max == b.y_max
}

/// Visit native WDoc path commands without allocating from the untrusted count.
/// Unknown verbs have unknown widths; their bounded remainder stays opaque.
pub(crate) fn visit_path(
    data: &[u8],
    mut visitor: impl FnMut(u8, &[f64]),
) -> Result<(usize, bool)> {
    let mut reader = Reader::new(data, "native shape path");
    let count = reader.read_u32("path command count")? as usize;
    if count > reader.remaining() {
        return Err(Error::Format(
            "path command count exceeds its bounded payload".into(),
        ));
    }
    let mut supported = count != 0;
    for index in 0..count {
        let verb = reader.read_u8("path verb")?;
        let values = match verb {
            1 | 2 => 2,
            3 | 7 => 4,
            4 | 5 => 6,
            6 => 0,
            _ => return Ok((data.len(), false)),
        };
        let mut coordinates = [0.0; 6];
        for value in &mut coordinates[..values] {
            *value = reader.read_f64("path coordinate")?;
            if !value.is_finite() {
                return Err(Error::Format("non-finite path coordinate".into()));
            }
        }
        supported &= matches!(verb, 1..=4 | 6) && (index != 0 || verb == 1);
        visitor(verb, &coordinates[..values]);
    }
    Ok((reader.position(), supported))
}

fn read_extensions(reader: &mut Reader<'_>, unsupported: &mut Vec<&'static str>) -> Result<()> {
    if reader.remaining() != 0 {
        while reader.remaining() != 0 {
            Frame::read(reader)?;
        }
        unsupported.push("additional shape/line frames");
    }
    Ok(())
}

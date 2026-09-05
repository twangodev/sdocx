use crate::binary::Reader;
use crate::frame::Frame;
use crate::{BoundingBox, Error, ObjectMetadata, ObjectType, ParseLimits, Result, StoredObject};

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct MathMetadata {
    pub base: ObjectMetadata,
    pub editable: bool,
    pub formula_objects: Vec<Vec<u8>>,
    pub margins: Option<MathMargins>,
    pub angle_type: Option<MathAngleType>,
    pub connected_plot_uuids: Vec<String>,
    pub property_mask: Vec<u8>,
    pub field_mask: Vec<u8>,
    pub fixed_trailing_data: Vec<u8>,
    pub flexible_trailing_data: Vec<u8>,
    pub trailing_data: Vec<u8>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct PlotMetadata {
    pub base: ObjectMetadata,
    pub legacy_field_0: Option<u32>,
    pub coordinate_rect: Option<BoundingBox>,
    pub coordinate_color: Option<u32>,
    pub background_color: Option<u32>,
    pub graphs: Vec<PlotGraph>,
    pub angle_type: Option<MathAngleType>,
    pub property_mask: Vec<u8>,
    pub field_mask: Vec<u8>,
    pub fixed_trailing_data: Vec<u8>,
    pub flexible_trailing_data: Vec<u8>,
    pub trailing_data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct PlotGraph {
    pub latex: String,
    pub color: u32,
    pub line_width: f32,
    pub visibility_raw: u8,
    pub substitution_latex: Vec<String>,
}

impl PlotGraph {
    pub const fn is_visible(&self) -> bool {
        self.visibility_raw == 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct MathMargins {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum MathAngleType {
    Degree,
    Radian,
    All,
    Other(u32),
}

impl From<u32> for MathAngleType {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::Degree,
            1 => Self::Radian,
            2 => Self::All,
            value => Self::Other(value),
        }
    }
}

impl StoredObject {
    pub fn math_metadata(&self, page_bytes: &[u8]) -> Result<MathMetadata> {
        self.math_metadata_with_limits(page_bytes, &ParseLimits::default())
    }

    pub fn math_metadata_with_limits(
        &self,
        page_bytes: &[u8],
        limits: &ParseLimits,
    ) -> Result<MathMetadata> {
        let payload = checked_payload(self, page_bytes, ObjectType::Math, limits)?;
        parse_math_metadata(payload, limits)
    }

    pub fn plot_metadata(&self, page_bytes: &[u8]) -> Result<PlotMetadata> {
        self.plot_metadata_with_limits(page_bytes, &ParseLimits::default())
    }

    pub fn plot_metadata_with_limits(
        &self,
        page_bytes: &[u8],
        limits: &ParseLimits,
    ) -> Result<PlotMetadata> {
        let payload = checked_payload(self, page_bytes, ObjectType::Plot, limits)?;
        parse_plot_metadata(payload, limits)
    }
}

pub(crate) fn checked_payload<'a>(
    object: &StoredObject,
    page_bytes: &'a [u8],
    expected: ObjectType,
    limits: &ParseLimits,
) -> Result<&'a [u8]> {
    if object.object_type != expected {
        return Err(Error::Format(format!(
            "expected a {expected:?} object, found {:?}",
            object.object_type
        )));
    }
    let payload = object
        .payload(page_bytes)
        .ok_or_else(|| Error::Format("math payload is outside its page".into()))?;
    if payload.len() as u64 > limits.max_entry_size {
        return Err(Error::LimitExceeded {
            resource: "math payload size",
            limit: limits.max_entry_size,
            actual: payload.len() as u64,
        });
    }
    Ok(payload)
}

fn parse_math_metadata(payload: &[u8], limits: &ParseLimits) -> Result<MathMetadata> {
    let mut reader = Reader::new(payload, "math object");
    let base = ObjectMetadata::read(&mut reader)?;
    let frame = Frame::read(&mut reader)?;
    frame.expect_kind(21)?;
    let mut flexible = Reader::new(frame.flexible, "math flexible fields");
    let mut entries = 0;
    let mut formula_objects = Vec::new();
    if frame.fields.contains(0) {
        let count = read_count(&mut flexible, 4, &mut entries, limits)?;
        formula_objects.reserve(count);
        for _ in 0..count {
            let size = flexible.read_u32("formula object size")? as usize;
            formula_objects.push(flexible.read_bytes(size, "formula object")?.to_vec());
        }
    }
    let margins = frame
        .fields
        .contains(1)
        .then(|| {
            let values = crate::object::read_bbox(&mut flexible)?;
            Ok::<_, Error>(MathMargins {
                left: values.x_min,
                top: values.y_min,
                right: values.x_max,
                bottom: values.y_max,
            })
        })
        .transpose()?;
    let angle_type = frame
        .fields
        .contains(2)
        .then(|| flexible.read_u32("math angle type").map(Into::into))
        .transpose()?;
    let mut connected_plot_uuids = Vec::new();
    if frame.fields.contains(3) {
        let count = read_count(&mut flexible, 2, &mut entries, limits)?;
        connected_plot_uuids.reserve(count);
        for _ in 0..count {
            connected_plot_uuids.push(flexible.read_utf8_u16("connected plot UUID")?);
        }
    }
    Ok(MathMetadata {
        base,
        editable: frame.properties.contains(0),
        formula_objects,
        margins,
        angle_type,
        connected_plot_uuids,
        property_mask: frame.properties.bytes().to_vec(),
        field_mask: frame.fields.bytes().to_vec(),
        fixed_trailing_data: frame.fixed.to_vec(),
        flexible_trailing_data: flexible
            .read_bytes(flexible.remaining(), "math flexible trailing data")?
            .to_vec(),
        trailing_data: reader
            .read_bytes(reader.remaining(), "math trailing data")?
            .to_vec(),
    })
}

pub(crate) fn read_count(
    reader: &mut Reader<'_>,
    minimum_record_size: usize,
    entries: &mut u64,
    limits: &ParseLimits,
) -> Result<usize> {
    let count = reader.read_u32("math entry count")? as usize;
    *entries += count as u64;
    if *entries > limits.max_objects_per_page as u64 {
        return Err(Error::LimitExceeded {
            resource: "math entries",
            limit: limits.max_objects_per_page as u64,
            actual: *entries,
        });
    }
    if count > reader.remaining() / minimum_record_size {
        return Err(Error::Format(
            "math entry count exceeds its bounded payload".into(),
        ));
    }
    Ok(count)
}

fn parse_plot_metadata(payload: &[u8], limits: &ParseLimits) -> Result<PlotMetadata> {
    let mut reader = Reader::new(payload, "plot object");
    let base = ObjectMetadata::read(&mut reader)?;
    let frame = Frame::read(&mut reader)?;
    frame.expect_kind(20)?;
    let mut flexible = Reader::new(frame.flexible, "plot flexible fields");
    let legacy_field_0 = frame
        .fields
        .contains(0)
        .then(|| flexible.read_u32("legacy plot field 0"))
        .transpose()?;
    let coordinate_rect = frame
        .fields
        .contains(1)
        .then(|| crate::object::read_bbox(&mut flexible))
        .transpose()?;
    let coordinate_color = frame
        .fields
        .contains(2)
        .then(|| flexible.read_u32("plot coordinate color"))
        .transpose()?;
    let background_color = frame
        .fields
        .contains(3)
        .then(|| flexible.read_u32("plot background color"))
        .transpose()?;
    let mut entries = 0;
    let mut graphs = Vec::new();
    if frame.fields.contains(4) {
        let count = read_count(&mut flexible, 15, &mut entries, limits)?;
        graphs.reserve(count);
        for _ in 0..count {
            let latex = read_latex(&mut flexible, limits)?;
            let color = flexible.read_u32("graph color")?;
            let line_width = flexible.read_f32("graph line width")?;
            let visibility_raw = flexible.read_u8("graph visibility")?;
            let count = read_count(&mut flexible, 2, &mut entries, limits)?;
            let mut substitution_latex = Vec::with_capacity(count);
            for _ in 0..count {
                substitution_latex.push(read_latex(&mut flexible, limits)?);
            }
            graphs.push(PlotGraph {
                latex,
                color,
                line_width,
                visibility_raw,
                substitution_latex,
            });
        }
    }
    let angle_type = frame
        .fields
        .contains(5)
        .then(|| flexible.read_u32("plot angle type").map(Into::into))
        .transpose()?;
    Ok(PlotMetadata {
        base,
        legacy_field_0,
        coordinate_rect,
        coordinate_color,
        background_color,
        graphs,
        angle_type,
        property_mask: frame.properties.bytes().to_vec(),
        field_mask: frame.fields.bytes().to_vec(),
        fixed_trailing_data: frame.fixed.to_vec(),
        flexible_trailing_data: flexible
            .read_bytes(flexible.remaining(), "plot flexible trailing data")?
            .to_vec(),
        trailing_data: reader
            .read_bytes(reader.remaining(), "plot trailing data")?
            .to_vec(),
    })
}

pub(crate) fn read_latex(reader: &mut Reader<'_>, limits: &ParseLimits) -> Result<String> {
    let value = reader.read_utf8_u16("LaTeX")?;
    let units = value.encode_utf16().count();
    if units > limits.max_text_characters {
        return Err(Error::LimitExceeded {
            resource: "text characters",
            limit: limits.max_text_characters as u64,
            actual: units as u64,
        });
    }
    Ok(value)
}

use crate::binary::Reader;
use crate::frame::Frame;
use crate::{Error, ObjectMetadata, ObjectType, ParseLimits, Result, StoredObject};

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
        if self.object_type != ObjectType::Math {
            return Err(Error::Format(format!(
                "expected a math object, found {:?}",
                self.object_type
            )));
        }
        let payload = self
            .payload(page_bytes)
            .ok_or_else(|| Error::Format("math payload is outside its page".into()))?;
        if payload.len() as u64 > limits.max_entry_size {
            return Err(Error::LimitExceeded {
                resource: "math payload size",
                limit: limits.max_entry_size,
                actual: payload.len() as u64,
            });
        }
        parse_math_metadata(payload, limits)
    }
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

fn read_count(
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

use crate::binary::Reader;
use crate::frame::Frame;
use crate::{BoundingBox, Error, Result};

/// Common identity and placement decoded from an object's type-0 frame.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct ObjectMetadata {
    /// Format version recorded by the object writer.
    pub format_version: u32,
    /// Persistent object identifier.
    pub uuid: String,
    /// Raw Samsung modification timestamp.
    pub modified_time_raw: i64,
    /// Placement rectangle in document coordinates.
    pub bbox: BoundingBox,
    /// Clockwise rotation in degrees, when explicitly stored.
    pub rotation_degrees: Option<f64>,
}

impl ObjectMetadata {
    pub(crate) fn read(reader: &mut Reader<'_>) -> Result<Self> {
        let frame = Frame::read(reader)?;
        frame.expect_kind(0)?;
        let mut fixed = Reader::new(frame.fixed, "object base");
        let format_version = fixed.read_u32("format version")?;
        let uuid = fixed.read_utf8_u16("UUID")?;
        let modified_time_raw = fixed.read_i64("modification timestamp")?;
        let bbox = read_bbox(&mut fixed)?;
        fixed.read_i32("replay timestamp")?;
        fixed.read_u8("resize mode")?;
        let rotation_degrees = if frame.fields.contains(0) {
            let mut flexible = Reader::new(frame.flexible, "object base flexible fields");
            let rotation = flexible.read_f32("rotation")?;
            if !rotation.is_finite() {
                return Err(Error::Format("non-finite object rotation".into()));
            }
            Some(f64::from(rotation))
        } else {
            None
        };
        Ok(Self {
            format_version,
            uuid,
            modified_time_raw,
            bbox,
            rotation_degrees,
        })
    }
}

pub(crate) fn read_bbox(reader: &mut Reader<'_>) -> Result<BoundingBox> {
    let bbox = BoundingBox {
        x_min: reader.read_f64("left")?,
        y_min: reader.read_f64("top")?,
        x_max: reader.read_f64("right")?,
        y_max: reader.read_f64("bottom")?,
    };
    if [bbox.x_min, bbox.y_min, bbox.x_max, bbox.y_max]
        .iter()
        .any(|value| !value.is_finite())
    {
        return Err(Error::Format("non-finite bounding rectangle".into()));
    }
    Ok(bbox)
}

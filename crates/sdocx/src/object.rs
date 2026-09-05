use crate::binary::Reader;
use crate::frame::Frame;
use crate::{BoundingBox, Error, Result};

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct ObjectMetadata {
    pub format_version: u32,
    pub uuid: String,
    pub modified_time_raw: i64,
    pub bbox: BoundingBox,
    pub replay_timestamp_raw: i32,
    pub resize_mode_raw: u8,
    pub rotatable: bool,
    pub selectable: bool,
    pub movable: bool,
    pub visible: bool,
    pub replayable: bool,
    pub template: bool,
    pub flip_enabled: bool,
    pub locked: bool,
    pub removable: bool,
    pub rotation_degrees: Option<f64>,
    pub property_mask: Vec<u8>,
    pub field_mask: Vec<u8>,
    pub fixed_trailing_data: Vec<u8>,
    pub flexible_trailing_data: Vec<u8>,
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
        let replay_timestamp_raw = fixed.read_i32("replay timestamp")?;
        let resize_mode_raw = fixed.read_u8("resize mode")?;
        let mut flexible = Reader::new(frame.flexible, "object base flexible fields");
        let rotation_degrees = if frame.fields.contains(0) {
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
            replay_timestamp_raw,
            resize_mode_raw,
            rotatable: frame.properties.contains(0),
            selectable: frame.properties.contains(1),
            movable: frame.properties.contains(2),
            visible: frame.properties.contains(3),
            replayable: frame.properties.contains(4),
            template: frame.properties.contains(6),
            flip_enabled: frame.properties.contains(7),
            locked: frame.properties.contains(9),
            removable: !frame.properties.contains(12),
            rotation_degrees,
            property_mask: frame.properties.bytes().to_vec(),
            field_mask: frame.fields.bytes().to_vec(),
            fixed_trailing_data: fixed
                .read_bytes(fixed.remaining(), "fixed trailing data")?
                .to_vec(),
            flexible_trailing_data: flexible
                .read_bytes(flexible.remaining(), "flexible trailing data")?
                .to_vec(),
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

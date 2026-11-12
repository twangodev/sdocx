use crate::binary::Reader;
use crate::frame::{Frame, Mask};
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
    pub out_of_canvas_enabled: bool,
    pub template: bool,
    pub flip_enabled: bool,
    pub float_drawn_rect: bool,
    pub locked: bool,
    pub removable: bool,
    pub rotation_degrees: Option<f64>,
    pub property_mask: Vec<u8>,
    pub field_mask: Vec<u8>,
    pub fixed_trailing_data: Vec<u8>,
    pub flexible_trailing_data: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ObjectResizeMode {
    Free,
    KeepRatio,
    Disabled,
    Other(u8),
}

#[derive(Clone, Copy)]
pub(crate) struct ObjectPlacement {
    pub(crate) bbox: BoundingBox,
    pub(crate) rotation_degrees: Option<f64>,
}

impl ObjectMetadata {
    pub const fn resize_mode(&self) -> ObjectResizeMode {
        match self.resize_mode_raw {
            0 => ObjectResizeMode::Free,
            1 => ObjectResizeMode::KeepRatio,
            2 => ObjectResizeMode::Disabled,
            value => ObjectResizeMode::Other(value),
        }
    }

    pub(crate) fn read(reader: &mut Reader<'_>) -> Result<Self> {
        ObjectBaseSource::read(reader).map(ObjectBaseSource::into_owned)
    }
}

impl ObjectPlacement {
    pub(crate) fn read(reader: &mut Reader<'_>) -> Result<Self> {
        ObjectBaseSource::read(reader).map(|source| source.placement)
    }
}

pub(crate) struct ObjectBaseSource<'a> {
    format_version: u32,
    uuid: &'a str,
    modified_time_raw: i64,
    placement: ObjectPlacement,
    replay_timestamp_raw: i32,
    resize_mode_raw: u8,
    properties: Mask<'a>,
    fields: Mask<'a>,
    fixed_trailing_data: &'a [u8],
    flexible_trailing_data: &'a [u8],
}

impl<'a> ObjectBaseSource<'a> {
    pub(crate) fn uuid(&self) -> &str {
        self.uuid
    }

    pub(crate) fn modified_time_raw(&self) -> i64 {
        self.modified_time_raw
    }

    pub(crate) fn is_visible(&self) -> bool {
        self.properties.contains(3)
    }

    pub(crate) fn flexible_source(&self) -> crate::object_flexible::ObjectFlexibleSource<'_> {
        crate::object_flexible::ObjectFlexibleSource {
            format_version: self.format_version,
            field_mask: self.fields.bytes(),
            trailing_data: self.flexible_trailing_data,
        }
    }

    pub(crate) fn read(reader: &mut Reader<'a>) -> Result<Self> {
        let frame = Frame::read(reader)?;
        frame.expect_kind(0)?;
        let mut fixed = Reader::new(frame.fixed, "object base");
        let format_version = fixed.read_u32("format version")?;
        let uuid = fixed.read_utf8_u16_borrowed("UUID")?;
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
            placement: ObjectPlacement {
                bbox,
                rotation_degrees,
            },
            replay_timestamp_raw,
            resize_mode_raw,
            properties: frame.properties,
            fields: frame.fields,
            fixed_trailing_data: fixed.read_bytes(fixed.remaining(), "fixed trailing data")?,
            flexible_trailing_data: flexible
                .read_bytes(flexible.remaining(), "flexible trailing data")?,
        })
    }

    pub(crate) fn into_owned(self) -> ObjectMetadata {
        ObjectMetadata {
            format_version: self.format_version,
            uuid: self.uuid.to_owned(),
            modified_time_raw: self.modified_time_raw,
            bbox: self.placement.bbox,
            replay_timestamp_raw: self.replay_timestamp_raw,
            resize_mode_raw: self.resize_mode_raw,
            rotatable: self.properties.contains(0),
            selectable: self.properties.contains(1),
            movable: self.properties.contains(2),
            visible: self.properties.contains(3),
            replayable: self.properties.contains(4),
            out_of_canvas_enabled: self.properties.contains(5),
            template: self.properties.contains(6),
            flip_enabled: self.properties.contains(7),
            float_drawn_rect: self.properties.contains(8),
            locked: self.properties.contains(9),
            removable: !self.properties.contains(12),
            rotation_degrees: self.placement.rotation_degrees,
            property_mask: self.properties.bytes().to_vec(),
            field_mask: self.fields.bytes().to_vec(),
            fixed_trailing_data: self.fixed_trailing_data.to_vec(),
            flexible_trailing_data: self.flexible_trailing_data.to_vec(),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn base_frame(uuid: &[u8], left: f64, rotation: f32) -> Vec<u8> {
        let mut fixed = 4000_u32.to_le_bytes().to_vec();
        fixed.extend((uuid.len() as u16).to_le_bytes());
        fixed.extend(uuid);
        fixed.extend(123_i64.to_le_bytes());
        for value in [left, 0.0, 20.0, 30.0] {
            fixed.extend(value.to_le_bytes());
        }
        fixed.extend(0_i32.to_le_bytes());
        fixed.push(0);
        let flexible_offset = 14 + fixed.len();
        let size = flexible_offset + 4;
        [
            (size as u32).to_le_bytes().to_vec(),
            0_i16.to_le_bytes().to_vec(),
            (flexible_offset as u32).to_le_bytes().to_vec(),
            vec![1, 8, 1, 1],
            fixed,
            rotation.to_le_bytes().to_vec(),
        ]
        .concat()
    }

    #[test]
    fn placement_projection_validates_discarded_identity_and_preserves_error_precedence() {
        let cases = [
            (
                base_frame(&[0xff], f64::NAN, f32::NAN),
                "format error: object base: invalid UTF-8 in UUID",
            ),
            (
                base_frame(b"u", f64::NAN, f32::NAN),
                "format error: non-finite bounding rectangle",
            ),
            (
                base_frame(b"u", 0.0, f32::NAN),
                "format error: non-finite object rotation",
            ),
        ];
        for (frame, expected) in cases {
            let bytes = [frame.as_slice(), &[99]].concat();
            let mut reader = Reader::new(&bytes, "placement");
            let Err(error) = ObjectPlacement::read(&mut reader) else {
                panic!("invalid base accepted")
            };
            assert_eq!(error.to_string(), expected);
            assert_eq!(reader.position(), frame.len());
            assert_eq!(reader.read_u8("next frame").unwrap(), 99);
        }
    }
}

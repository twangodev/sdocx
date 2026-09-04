use crate::binary::Reader;
use crate::{Error, Result};

/// A length-prefixed mask. Keep all bytes so newer bits cannot shift the fields
/// that follow the header, even when this reader does not know their meaning.
#[derive(Clone, Copy)]
pub(crate) struct Mask<'a>(&'a [u8]);

impl<'a> Mask<'a> {
    pub(crate) fn read(reader: &mut Reader<'a>) -> Result<Self> {
        let size = usize::from(reader.read_u8("mask byte count")?);
        Ok(Self(reader.read_bytes(size, "mask")?))
    }

    pub(crate) fn contains(self, bit: usize) -> bool {
        self.0
            .get(bit / 8)
            .is_some_and(|byte| byte & (1 << (bit % 8)) != 0)
    }

    pub(crate) fn low_u32(self) -> u32 {
        let mut bytes = [0; 4];
        let count = self.0.len().min(bytes.len());
        bytes[..count].copy_from_slice(&self.0[..count]);
        u32::from_le_bytes(bytes)
    }

    pub(crate) fn byte_count(self) -> u8 {
        self.0.len() as u8
    }

    fn is_empty(self) -> bool {
        self.0.iter().all(|byte| *byte == 0)
    }
}

/// One native typed frame, split into independently bounded fixed and flexible
/// data. Neither decoder can consume bytes from the next frame or object hash.
pub(crate) struct Frame<'a> {
    pub(crate) kind: i16,
    pub(crate) properties: Mask<'a>,
    pub(crate) fields: Mask<'a>,
    pub(crate) fixed: &'a [u8],
    pub(crate) flexible: &'a [u8],
}

impl<'a> Frame<'a> {
    pub(crate) fn read(reader: &mut Reader<'a>) -> Result<Self> {
        let size = usize::try_from(reader.read_u32("frame size")?)
            .map_err(|_| Error::Format("frame size does not fit in memory".into()))?;
        if size < 12 {
            return Err(Error::Format(format!("invalid frame size {size}")));
        }
        let data = reader.read_bytes(size - 4, "frame data")?;
        let mut header = Reader::new(data, "frame header");
        let kind = header.read_i16("frame type")?;
        let flexible_offset = header.read_u32("flexible-data offset")? as usize;
        let properties = Mask::read(&mut header)?;
        let fields = Mask::read(&mut header)?;
        let fixed_end = if flexible_offset == 0 && fields.is_empty() {
            size
        } else {
            flexible_offset
        };
        if fixed_end < header.position() + 4 || fixed_end > size {
            return Err(Error::Format(format!(
                "frame type {kind}: flexible-data offset {flexible_offset} is outside its data"
            )));
        }
        Ok(Self {
            kind,
            properties,
            fields,
            fixed: &data[header.position()..fixed_end - 4],
            flexible: &data[fixed_end - 4..],
        })
    }

    pub(crate) fn expect_kind(&self, expected: i16) -> Result<()> {
        if self.kind != expected {
            return Err(Error::Format(format!(
                "expected frame type {expected}, found {}",
                self.kind
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variable_masks_and_data_stay_inside_their_frame() {
        // 18-byte header, three fixed bytes and two flexible bytes. The fifth
        // field-mask byte contains a future bit beyond a u32.
        let data = [
            23, 0, 0, 0, 1, 0, 21, 0, 0, 0, 1, 5, 5, 8, 0, 0, 0, 1, 10, 11, 12, 13, 14, 99,
        ];
        let mut reader = Reader::new(&data, "object");
        let frame = Frame::read(&mut reader).unwrap();
        frame.expect_kind(1).unwrap();
        assert!(frame.properties.contains(2));
        assert!(frame.fields.contains(32));
        assert_eq!(frame.fields.low_u32(), 8);
        assert_eq!(frame.fixed, [10, 11, 12]);
        assert_eq!(frame.flexible, [13, 14]);
        assert_eq!(reader.read_u8("next record").unwrap(), 99);
    }

    #[test]
    fn malformed_headers_cannot_borrow_from_the_next_frame() {
        for size in 0_u32..14 {
            let mut bytes = vec![0; 40];
            bytes[..4].copy_from_slice(&size.to_le_bytes());
            bytes[10] = 2; // property mask plus field count need 15 bytes
            bytes[13] = 1;
            assert!(Frame::read(&mut Reader::new(&bytes, "object")).is_err());
        }
    }

    #[test]
    fn rejects_flexible_offsets_outside_the_frame_data() {
        for offset in [0_u32, 1, 11, 14, u32::MAX] {
            let mut bytes = vec![13, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1, 8];
            bytes[6..10].copy_from_slice(&offset.to_le_bytes());
            assert!(Frame::read(&mut Reader::new(&bytes, "object")).is_err());
        }
    }

    #[test]
    fn frame_without_flexible_fields_can_use_a_zero_offset() {
        let bytes = [14, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 42, 43];
        let frame = Frame::read(&mut Reader::new(&bytes, "object")).unwrap();
        assert_eq!(frame.fixed, [42, 43]);
        assert!(frame.flexible.is_empty());
        assert!(frame.expect_kind(0).is_err());
    }
}

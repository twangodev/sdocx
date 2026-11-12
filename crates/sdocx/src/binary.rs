use crate::error::{Error, Result};

/// Bounds-checked reader for Samsung's little-endian binary records.
pub(crate) struct Reader<'a> {
    data: &'a [u8],
    position: usize,
    context: &'static str,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(data: &'a [u8], context: &'static str) -> Self {
        Self {
            data,
            position: 0,
            context,
        }
    }

    pub(crate) fn at(data: &'a [u8], position: usize, context: &'static str) -> Result<Self> {
        if position > data.len() {
            return Err(Error::Format(format!(
                "{context}: offset 0x{position:x} is past the end of the record"
            )));
        }
        Ok(Self {
            data,
            position,
            context,
        })
    }

    pub(crate) const fn position(&self) -> usize {
        self.position
    }

    pub(crate) fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.position)
    }

    pub(crate) fn remaining_bytes(&self) -> &'a [u8] {
        &self.data[self.position..]
    }

    pub(crate) fn read_u8(&mut self, field: &'static str) -> Result<u8> {
        Ok(self.read_array::<1>(field)?[0])
    }

    pub(crate) fn read_u16(&mut self, field: &'static str) -> Result<u16> {
        Ok(u16::from_le_bytes(self.read_array(field)?))
    }

    pub(crate) fn read_i16(&mut self, field: &'static str) -> Result<i16> {
        Ok(i16::from_le_bytes(self.read_array(field)?))
    }

    pub(crate) fn read_u32(&mut self, field: &'static str) -> Result<u32> {
        Ok(u32::from_le_bytes(self.read_array(field)?))
    }

    pub(crate) fn read_i32(&mut self, field: &'static str) -> Result<i32> {
        Ok(i32::from_le_bytes(self.read_array(field)?))
    }

    pub(crate) fn read_u64(&mut self, field: &'static str) -> Result<u64> {
        Ok(u64::from_le_bytes(self.read_array(field)?))
    }

    pub(crate) fn read_i64(&mut self, field: &'static str) -> Result<i64> {
        Ok(i64::from_le_bytes(self.read_array(field)?))
    }

    pub(crate) fn read_f32(&mut self, field: &'static str) -> Result<f32> {
        Ok(f32::from_le_bytes(self.read_array(field)?))
    }

    pub(crate) fn read_f64(&mut self, field: &'static str) -> Result<f64> {
        Ok(f64::from_le_bytes(self.read_array(field)?))
    }

    pub(crate) fn read_bytes(&mut self, length: usize, field: &'static str) -> Result<&'a [u8]> {
        let start = self.position;
        let end = start.checked_add(length).ok_or_else(|| {
            Error::Format(format!(
                "{}: {field} length overflows at offset 0x{start:x}",
                self.context
            ))
        })?;
        let bytes = self.data.get(start..end).ok_or_else(|| {
            Error::Format(format!(
                "{}: truncated {field} at offset 0x{start:x} (need {length} bytes, have {})",
                self.context,
                self.remaining()
            ))
        })?;
        self.position = end;
        Ok(bytes)
    }

    pub(crate) fn skip(&mut self, length: usize, field: &'static str) -> Result<()> {
        self.read_bytes(length, field).map(|_| ())
    }

    pub(crate) fn read_utf16_u16(&mut self, field: &'static str) -> Result<String> {
        self.read_utf16_u16_with_limit(field, usize::MAX)
    }

    pub(crate) fn read_utf16_u16_with_limit(
        &mut self,
        field: &'static str,
        max_units: usize,
    ) -> Result<String> {
        self.read_nullable_utf16_u16(field, max_units)?
            .ok_or_else(|| {
                Error::Format(format!(
                    "{}: {field} uses the null string sentinel",
                    self.context
                ))
            })
    }

    pub(crate) fn read_nullable_utf16_u16(
        &mut self,
        field: &'static str,
        max_units: usize,
    ) -> Result<Option<String>> {
        let unit_count = usize::from(self.read_u16(field)?);
        if unit_count == usize::from(u16::MAX) {
            return Ok(None);
        }
        self.read_utf16_units(unit_count, field, max_units)
            .map(Some)
    }

    pub(crate) fn read_utf16_u16_without_null_sentinel(
        &mut self,
        field: &'static str,
        max_units: usize,
    ) -> Result<String> {
        let unit_count = usize::from(self.read_u16(field)?);
        self.read_utf16_units(unit_count, field, max_units)
    }

    pub(crate) fn read_utf16_u32(
        &mut self,
        field: &'static str,
        max_units: usize,
    ) -> Result<String> {
        let unit_count = usize::try_from(self.read_u32(field)?)
            .map_err(|_| Error::Format(format!("{}: {field} is too long", self.context)))?;
        self.read_utf16_units(unit_count, field, max_units)
    }

    pub(crate) fn read_utf16_units(
        &mut self,
        unit_count: usize,
        field: &'static str,
        max_units: usize,
    ) -> Result<String> {
        if unit_count > max_units {
            return Err(Error::LimitExceeded {
                resource: "text characters",
                limit: u64::try_from(max_units).unwrap_or(u64::MAX),
                actual: u64::try_from(unit_count).unwrap_or(u64::MAX),
            });
        }
        let byte_count = unit_count
            .checked_mul(2)
            .ok_or_else(|| Error::Format(format!("{}: {field} length overflows", self.context)))?;
        decode_utf16(self.read_bytes(byte_count, field)?, self.context, field)
    }

    pub(crate) fn read_utf8_u16(&mut self, field: &'static str) -> Result<String> {
        let byte_count = usize::from(self.read_u16(field)?);
        let bytes = self.read_bytes(byte_count, field)?;
        std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|_| Error::Format(format!("{}: invalid UTF-8 in {field}", self.context)))
    }

    fn read_array<const N: usize>(&mut self, field: &'static str) -> Result<[u8; N]> {
        self.read_bytes(N, field)?
            .try_into()
            .map_err(|_| Error::Format(format!("{}: invalid {field}", self.context)))
    }
}

fn decode_utf16(bytes: &[u8], context: &'static str, field: &'static str) -> Result<String> {
    let pairs = bytes.as_chunks::<2>().0;
    let mut text = String::with_capacity(pairs.len());
    for character in char::decode_utf16(pairs.iter().map(|pair| u16::from_le_bytes(*pair))) {
        let character = character
            .map_err(|_| Error::Format(format!("{context}: invalid UTF-16 in {field}")))?;
        text.push(character);
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::{Reader, decode_utf16};

    #[test]
    fn reports_the_field_and_offset_for_truncated_data() {
        let mut reader = Reader::new(&[0x01, 0x02], "test record");

        let error = reader.read_u32("declared size").unwrap_err();

        assert_eq!(
            error.to_string(),
            "format error: test record: truncated declared size at offset 0x0 (need 4 bytes, have 2)"
        );
    }

    #[test]
    fn decodes_length_prefixed_utf16() {
        let mut reader = Reader::new(&[2, 0, b'A', 0, 0x3d, 0xd8], "test record");

        let error = reader.read_utf16_u16("name").unwrap_err();

        assert_eq!(
            error.to_string(),
            "format error: test record: invalid UTF-16 in name"
        );
    }

    #[test]
    fn utf16_preserves_unicode_and_rejects_unpaired_surrogates() {
        for expected in ["", "ASCII", "A\0答", "😀", "A😀答\0"] {
            let bytes: Vec<_> = expected.encode_utf16().flat_map(u16::to_le_bytes).collect();
            assert_eq!(
                decode_utf16(&bytes, "text record", "name").unwrap(),
                expected
            );
        }
        for units in [
            vec![0xdc00],
            vec![0xd800],
            vec![0xd800, 0x0041],
            vec![0xd800, 0xd800],
        ] {
            let bytes: Vec<_> = units.into_iter().flat_map(u16::to_le_bytes).collect();
            assert_eq!(
                decode_utf16(&bytes, "text record", "name")
                    .unwrap_err()
                    .to_string(),
                "format error: text record: invalid UTF-16 in name"
            );
        }
    }

    #[test]
    fn utf16_helper_ignores_incomplete_trailing_unit() {
        assert_eq!(
            decode_utf16(&[b'A', 0, 0xff], "text record", "name").unwrap(),
            "A"
        );
    }

    #[test]
    fn utf16_errors_preserve_limits_payload_offsets_and_cursor_progress() {
        let mut limited = Reader::new(&[2, 0, 0, 0xd8], "text record");
        assert!(matches!(
            limited.read_utf16_u16_with_limit("name", 1),
            Err(crate::Error::LimitExceeded {
                resource: "text characters",
                limit: 1,
                actual: 2
            })
        ));
        assert_eq!(limited.position(), 2);

        let mut truncated = Reader::new(&[2, 0, 0, 0xd8], "text record");
        assert_eq!(
            truncated.read_utf16_u16("name").unwrap_err().to_string(),
            "format error: text record: truncated name at offset 0x2 (need 4 bytes, have 2)"
        );
        assert_eq!(truncated.position(), 2);

        let mut invalid = Reader::new(&[2, 0, 0, 0xd8, b'A', 0, 99], "text record");
        assert_eq!(
            invalid.read_utf16_u16("name").unwrap_err().to_string(),
            "format error: text record: invalid UTF-16 in name"
        );
        assert_eq!(invalid.position(), 6);
        assert_eq!(invalid.read_u8("next field").unwrap(), 99);
    }
}

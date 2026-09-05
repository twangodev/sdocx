use crate::ParseLimits;
use crate::binary::Reader;
use crate::error::{Error, Result};

pub(crate) const END_TAG_SIGNATURE: &[u8] = b"Document for S-Pen SDK";

#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct StoredEndTag {
    pub format_version: u32,
    pub note_id: Option<String>,
    pub modified_time: i64,
    pub property_flags: u32,
    pub cover_image: Option<String>,
    pub note_width: u32,
    pub note_height: f32,
    pub application_name: Option<String>,
    pub application_major_version: i32,
    pub application_minor_version: i32,
    pub application_patch_name: Option<String>,
    pub minimum_format_version: u32,
    pub created_time: i64,
    pub last_viewed_page_index: i32,
    pub page_mode: u16,
    pub document_type: Option<u16>,
    pub owner_id: Option<String>,
    pub reserved_data: Option<Vec<u8>>,
    pub encryption_data: Option<Vec<u8>>,
    pub display_timestamps: Option<EndTagDisplayTimestamps>,
    pub last_recognized_data_modified_time: Option<i64>,
    pub fixed_style: Option<EndTagFixedStyle>,
    pub server_checkpoint: Option<i64>,
    pub new_orientation: Option<i32>,
    pub minimum_unknown_version: Option<i32>,
    pub application_custom_data: Option<String>,
    pub trailing_data: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EndTagDisplayTimestamps {
    pub created_time: i64,
    pub modified_time: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EndTagFixedStyle {
    pub font: Option<String>,
    pub text_direction: i32,
    pub background_theme: i32,
}

pub fn parse_end_tag_bytes(data: &[u8]) -> Result<StoredEndTag> {
    parse_end_tag_bytes_with_limits(data, &ParseLimits::default())
}

pub fn parse_end_tag_bytes_with_limits(data: &[u8], limits: &ParseLimits) -> Result<StoredEndTag> {
    if data.len() as u64 > limits.max_entry_size {
        return Err(Error::LimitExceeded {
            resource: "end tag size",
            limit: limits.max_entry_size,
            actual: data.len() as u64,
        });
    }
    let mut record = Reader::new(data, "end tag");
    let declared_size = usize::from(record.read_u16("record size")?);
    if declared_size != record.remaining() {
        return Err(Error::Format(format!(
            "end tag: declared size {declared_size} differs from payload size {}",
            record.remaining()
        )));
    }
    let payload = record.read_bytes(declared_size, "payload")?;
    let fields = payload
        .strip_suffix(END_TAG_SIGNATURE)
        .ok_or_else(|| Error::Format("end tag: missing Document for S-Pen SDK signature".into()))?;
    let mut reader = Reader::new(fields, "end tag");
    let format_version = reader.read_u32("format version")?;
    if format_version < 2034 {
        return Err(Error::Format(format!(
            "end tag: unsupported WDoc format version {format_version}"
        )));
    }
    let mut tag = StoredEndTag {
        format_version,
        note_id: read_string(&mut reader, "note ID", limits)?,
        modified_time: reader.read_i64("modified time")?,
        property_flags: reader.read_u32("property flags")?,
        cover_image: read_string(&mut reader, "cover image", limits)?,
        note_width: reader.read_u32("note width")?,
        note_height: reader.read_f32("note height")?,
        application_name: read_string(&mut reader, "application name", limits)?,
        application_major_version: reader.read_i32("application major version")?,
        application_minor_version: reader.read_i32("application minor version")?,
        application_patch_name: read_string(&mut reader, "application patch name", limits)?,
        minimum_format_version: reader.read_u32("minimum format version")?,
        created_time: reader.read_i64("created time")?,
        last_viewed_page_index: reader.read_i32("last viewed page index")?,
        page_mode: reader.read_u16("page mode")?,
        ..StoredEndTag::default()
    };
    tag.document_type = read_extension(&mut reader, |r| r.read_u16("document type"))?;
    tag.owner_id = read_extension(&mut reader, |r| read_string(r, "owner ID", limits))?.flatten();
    tag.reserved_data = read_extension(&mut reader, |r| read_blob(r, "reserved data"))?;
    tag.encryption_data = read_extension(&mut reader, |r| read_blob(r, "encryption data"))?;
    tag.display_timestamps = read_extension(&mut reader, |r| {
        Ok(EndTagDisplayTimestamps {
            created_time: r.read_i64("display created time")?,
            modified_time: r.read_i64("display modified time")?,
        })
    })?;
    tag.last_recognized_data_modified_time = read_extension(&mut reader, |r| {
        r.read_i64("last recognized data modified time")
    })?;
    tag.fixed_style = read_extension(&mut reader, |r| {
        Ok(EndTagFixedStyle {
            font: read_string(r, "fixed font", limits)?,
            text_direction: r.read_i32("fixed text direction")?,
            background_theme: r.read_i32("fixed background theme")?,
        })
    })?;
    tag.server_checkpoint = read_extension(&mut reader, |r| r.read_i64("server checkpoint"))?;
    tag.new_orientation = read_extension(&mut reader, |r| r.read_i32("new orientation"))?;
    tag.minimum_unknown_version =
        read_extension(&mut reader, |r| r.read_i32("minimum unknown version"))?;
    tag.application_custom_data = read_extension(&mut reader, |r| {
        let units = r.read_u32("application custom data length")?;
        read_string_units(r, units, u32::MAX, "application custom data", limits)
    })?
    .flatten();
    tag.trailing_data = reader
        .read_bytes(reader.remaining(), "trailing data")?
        .to_vec();
    Ok(tag)
}

fn read_extension<T>(
    reader: &mut Reader<'_>,
    read: impl FnOnce(&mut Reader<'_>) -> Result<T>,
) -> Result<Option<T>> {
    if reader.remaining() == 0 {
        Ok(None)
    } else {
        read(reader).map(Some)
    }
}

fn read_blob(reader: &mut Reader<'_>, field: &'static str) -> Result<Vec<u8>> {
    let length = reader.read_u32(field)? as usize;
    Ok(reader.read_bytes(length, field)?.to_vec())
}

fn read_string(
    reader: &mut Reader<'_>,
    field: &'static str,
    limits: &ParseLimits,
) -> Result<Option<String>> {
    let units = u32::from(reader.read_u16(field)?);
    read_string_units(reader, units, u32::from(u16::MAX), field, limits)
}

fn read_string_units(
    reader: &mut Reader<'_>,
    units: u32,
    null_sentinel: u32,
    field: &'static str,
    limits: &ParseLimits,
) -> Result<Option<String>> {
    if units == null_sentinel {
        return Ok(None);
    }
    if u64::from(units) > limits.max_text_characters as u64 {
        return Err(Error::LimitExceeded {
            resource: "text characters",
            limit: limits.max_text_characters as u64,
            actual: u64::from(units),
        });
    }
    let byte_count = (units as usize)
        .checked_mul(2)
        .ok_or_else(|| Error::Format(format!("end tag: {field} length overflows")))?;
    let bytes = reader.read_bytes(byte_count, field)?;
    char::decode_utf16(
        bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| u16::from_le_bytes(*b)),
    )
    .collect::<std::result::Result<String, _>>()
    .map(Some)
    .map_err(|_| Error::Format(format!("end tag: invalid UTF-16 in {field}")))
}

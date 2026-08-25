use crate::ParseLimits;
use crate::binary::Reader;
use crate::error::{Error, Result};
use crate::types::{
    BoundingBox, RichTextBox, RichTextParagraph, RichTextParagraphType, RichTextRun, RichTextSpan,
    RichTextSpanType,
};

/// Structured contents of `note.note` needed by the document model.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StoredNote {
    /// Fixed document header.
    pub header: StoredNoteHeader,
    /// Rich-text title object.
    pub title: RichTextBox,
    /// Document-level flowing rich-text body.
    pub body: RichTextBox,
}

/// Fixed header fields at the beginning of `note.note`.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StoredNoteHeader {
    /// Absolute offset of the note integrity block.
    pub integrity_offset: u32,
    /// First raw header marker byte.
    pub header_constant_1: u8,
    /// Raw header flags.
    pub header_flags: u32,
    /// Second raw header marker byte.
    pub header_constant_2: u8,
    /// Bit mask for optional note properties.
    pub property_flags: u32,
    /// Samsung Notes binary format version.
    pub format_version: u32,
    /// Note identifier.
    pub note_id: String,
    /// File revision.
    pub file_revision: u32,
    /// Raw creation timestamp.
    pub created_time_raw: i64,
    /// Raw modification timestamp.
    pub modified_time_raw: i64,
    /// Width of the document flow canvas.
    pub width: u32,
    /// Height of the document flow canvas.
    pub height: u32,
    /// Horizontal page padding.
    pub page_horizontal_padding: u32,
    /// Vertical page padding.
    pub page_vertical_padding: u32,
    /// Minimum reader format version.
    pub minimum_format_version: u32,
}

/// Parse the structured title and body from an uncompressed `note.note` entry.
pub fn parse_note_bytes(data: &[u8]) -> Result<StoredNote> {
    parse_note_bytes_with_limits(data, &ParseLimits::default())
}

/// Parse an uncompressed `note.note` entry with explicit resource limits.
pub fn parse_note_bytes_with_limits(data: &[u8], limits: &ParseLimits) -> Result<StoredNote> {
    let mut reader = Reader::new(data, "note header");
    let integrity_offset = reader.read_u32("integrity offset")?;
    let header_constant_1 = reader.read_u8("header marker 1")?;
    let header_flags = reader.read_u32("header flags")?;
    let header_constant_2 = reader.read_u8("header marker 2")?;
    let property_flags = reader.read_u32("property flags")?;
    let format_version = reader.read_u32("format version")?;
    let note_id = reader.read_utf16_u16("note ID")?;
    let file_revision = reader.read_u32("file revision")?;
    let created_time_raw = reader.read_i64("creation timestamp")?;
    let modified_time_raw = reader.read_i64("modification timestamp")?;
    let width = reader.read_u32("document width")?;
    let height = reader.read_u32("document height")?;
    let page_horizontal_padding = reader.read_u32("horizontal page padding")?;
    let page_vertical_padding = reader.read_u32("vertical page padding")?;
    let minimum_format_version = reader.read_u32("minimum format version")?;

    let title_size = usize::try_from(reader.read_u32("title object size")?)
        .map_err(|_| Error::Format("title object size does not fit in memory".into()))?;
    let title = parse_text_object(
        reader.read_bytes(title_size, "title object")?,
        limits,
        "note title",
    )?;
    let body_size = usize::try_from(reader.read_u32("body object size")?)
        .map_err(|_| Error::Format("body object size does not fit in memory".into()))?;
    let body = parse_text_object(
        reader.read_bytes(body_size, "body object")?,
        limits,
        "note body",
    )?;

    Ok(StoredNote {
        header: StoredNoteHeader {
            integrity_offset,
            header_constant_1,
            header_flags,
            header_constant_2,
            property_flags,
            format_version,
            note_id,
            file_revision,
            created_time_raw,
            modified_time_raw,
            width,
            height,
            page_horizontal_padding,
            page_vertical_padding,
            minimum_format_version,
        },
        title,
        body,
    })
}

struct ObjectBase {
    bbox: BoundingBox,
    rotation_degrees: Option<f64>,
}

struct ObjectFrame {
    start: usize,
    end: usize,
    flexible_offset: u32,
    fields: u32,
}

fn parse_text_object(
    data: &[u8],
    limits: &ParseLimits,
    context: &'static str,
) -> Result<RichTextBox> {
    let mut reader = Reader::new(data, context);
    let object_base = parse_object_base(data, &mut reader)?;
    skip_frame(&mut reader, "shape base")?;
    let shape_text = open_object_frame(&mut reader, 7, "shape text")?;

    let common = if shape_text.flexible_offset != 0 && shape_text.fields & 1 != 0 {
        let common_offset = shape_text
            .start
            .checked_add(shape_text.flexible_offset as usize)
            .ok_or_else(|| Error::Format(format!("{context}: text payload offset overflows")))?;
        parse_text_common_at(data, common_offset, limits, context)?
    } else {
        TextCommon::default()
    };
    reader.set_position(shape_text.end, "shape text end")?;

    Ok(common.into_rich_text_box(object_base))
}

fn parse_object_base(data: &[u8], reader: &mut Reader<'_>) -> Result<ObjectBase> {
    let frame = open_object_frame(reader, 0, "object base")?;
    reader.read_u32("object format version")?;
    reader.read_utf8_u16("object UUID")?;
    reader.read_i64("object modification timestamp")?;
    let bbox = BoundingBox {
        x_min: reader.read_f64("object left")?,
        y_min: reader.read_f64("object top")?,
        x_max: reader.read_f64("object right")?,
        y_max: reader.read_f64("object bottom")?,
    };
    reader.read_u32("object timestamp")?;
    reader.read_u8("object resize mode")?;

    let rotation_degrees = if frame.flexible_offset != 0 && frame.fields & 1 != 0 {
        let rotation_offset = frame
            .start
            .checked_add(frame.flexible_offset as usize)
            .ok_or_else(|| Error::Format("object rotation offset overflows".into()))?;
        let mut rotation_reader = Reader::at(data, rotation_offset, "object rotation")?;
        Some(f64::from(rotation_reader.read_f32("rotation degrees")?))
    } else {
        None
    };
    reader.set_position(frame.end, "object base end")?;

    Ok(ObjectBase {
        bbox,
        rotation_degrees,
    })
}

fn open_object_frame(
    reader: &mut Reader<'_>,
    expected_data_type: i16,
    field: &'static str,
) -> Result<ObjectFrame> {
    let start = reader.position();
    let size = usize::try_from(reader.read_u32(field)?)
        .map_err(|_| Error::Format(format!("{field} size does not fit in memory")))?;
    let end = start
        .checked_add(size)
        .ok_or_else(|| Error::Format(format!("{field} size overflows")))?;
    if size < 4 || end > reader.len() {
        return Err(Error::Format(format!(
            "{field} ends at 0x{end:x}, outside its {}-byte record",
            reader.len()
        )));
    }
    let data_type = reader.read_i16("object frame type")?;
    if data_type != expected_data_type {
        return Err(Error::Format(format!(
            "expected {field} type {expected_data_type}, found {data_type} at offset 0x{start:x}"
        )));
    }
    let flexible_offset = reader.read_u32("object flexible-data offset")?;
    read_bitfield(reader, "object property mask")?;
    let fields = read_bitfield(reader, "object field mask")?;
    Ok(ObjectFrame {
        start,
        end,
        flexible_offset,
        fields,
    })
}

fn read_bitfield(reader: &mut Reader<'_>, field: &'static str) -> Result<u32> {
    let byte_count = usize::from(reader.read_u8(field)?);
    if byte_count > 4 {
        return Err(Error::Format(format!(
            "{field} uses {byte_count} bytes; at most 4 are supported"
        )));
    }
    let bytes = reader.read_bytes(byte_count, field)?;
    let mut padded = [0_u8; 4];
    padded[..byte_count].copy_from_slice(bytes);
    Ok(u32::from_le_bytes(padded))
}

fn skip_frame(reader: &mut Reader<'_>, field: &'static str) -> Result<()> {
    let start = reader.position();
    let size = usize::try_from(reader.read_u32(field)?)
        .map_err(|_| Error::Format(format!("{field} size does not fit in memory")))?;
    if size < 4 {
        return Err(Error::Format(format!(
            "{field} size {size} is smaller than 4"
        )));
    }
    let end = start
        .checked_add(size)
        .ok_or_else(|| Error::Format(format!("{field} size overflows")))?;
    reader.set_position(end, field)
}

#[derive(Default)]
struct TextCommon {
    text: String,
    spans: Vec<RichTextSpan>,
    paragraphs: Vec<RichTextParagraph>,
    margins: Option<[f32; 4]>,
    gravity: Option<u8>,
}

impl TextCommon {
    fn into_rich_text_box(self, object_base: ObjectBase) -> RichTextBox {
        let mut runs = Vec::new();
        for span in &self.spans {
            let (bold, italic) = match span.kind {
                RichTextSpanType::Bold => (true, false),
                RichTextSpanType::Italic => (false, true),
                _ => continue,
            };
            if span.boolean_value() != Some(true) {
                continue;
            }
            let Some(start) = utf16_to_char_index(&self.text, span.start_utf16) else {
                continue;
            };
            let Some(end) = utf16_to_char_index(&self.text, span.end_utf16) else {
                continue;
            };
            if start < end {
                runs.push(RichTextRun {
                    start,
                    end,
                    bold,
                    italic,
                });
            }
        }

        let color = self
            .spans
            .iter()
            .find(|span| span.kind == RichTextSpanType::ForegroundColor)
            .and_then(RichTextSpan::color_value);
        let highlight_color = self
            .spans
            .iter()
            .find(|span| span.kind == RichTextSpanType::BackgroundColor)
            .and_then(RichTextSpan::color_value);
        let font_size = self
            .spans
            .iter()
            .find(|span| span.kind == RichTextSpanType::FontSize)
            .and_then(RichTextSpan::font_size_value)
            .filter(|size| size.is_finite() && *size > 0.0);
        let underline = self.spans.iter().any(|span| {
            span.kind == RichTextSpanType::Underline && span.boolean_value() == Some(true)
        });

        RichTextBox {
            bbox: object_base.bbox,
            rotation_degrees: object_base.rotation_degrees,
            text: self.text,
            color,
            highlight_color,
            underline,
            font_size,
            runs,
            spans: self.spans,
            paragraphs: self.paragraphs,
            margins: self.margins,
            gravity: self.gravity,
        }
    }
}

fn parse_text_common_at(
    data: &[u8],
    offset: usize,
    limits: &ParseLimits,
    context: &'static str,
) -> Result<TextCommon> {
    let mut reader = Reader::at(data, offset, context)?;
    let common_size = usize::try_from(reader.read_u32("text common size")?)
        .map_err(|_| Error::Format(format!("{context}: text common size is too large")))?;
    let common_bytes = reader.read_bytes(common_size, "text common payload")?;
    let mut common = Reader::new(common_bytes, context);

    let text = common.read_utf16_u32("text", limits.max_text_characters)?;
    let span_count = usize::try_from(common.read_u32("style span count")?)
        .map_err(|_| Error::Format(format!("{context}: style span count is too large")))?;
    check_limit("text spans", limits.max_text_spans, span_count)?;
    let mut spans = Vec::with_capacity(span_count);
    for _ in 0..span_count {
        let record_size = usize::from(common.read_u16("style span size")?);
        if record_size < 16 {
            return Err(Error::Format(format!(
                "{context}: style span size {record_size} is smaller than 16"
            )));
        }
        let record_bytes = common.read_bytes(record_size, "style span")?;
        let mut record = Reader::new(record_bytes, context);
        let kind = RichTextSpanType::from(record.read_u32("style span type")?);
        let start_utf16 = record.read_u32("style span start")?;
        let end_utf16 = record.read_u32("style span end")?;
        let expand = record.read_u32("style span expansion flag")? != 0;
        let payload = record
            .read_bytes(record.remaining(), "style span payload")?
            .to_vec();
        spans.push(RichTextSpan {
            kind,
            start_utf16,
            end_utf16,
            expand,
            payload,
        });
    }

    let paragraph_count = usize::try_from(common.read_u32("paragraph count")?)
        .map_err(|_| Error::Format(format!("{context}: paragraph count is too large")))?;
    check_limit(
        "text paragraphs",
        limits.max_text_paragraphs,
        paragraph_count,
    )?;
    let mut paragraphs = Vec::with_capacity(paragraph_count);
    for _ in 0..paragraph_count {
        let record_size = usize::from(common.read_u16("paragraph size")?);
        if record_size < 12 {
            return Err(Error::Format(format!(
                "{context}: paragraph size {record_size} is smaller than 12"
            )));
        }
        let record_bytes = common.read_bytes(record_size, "paragraph")?;
        let mut record = Reader::new(record_bytes, context);
        let kind = RichTextParagraphType::from(record.read_u32("paragraph type")?);
        let start_utf16 = record.read_u32("paragraph start")?;
        let end_utf16 = record.read_u32("paragraph end")?;
        let payload = record
            .read_bytes(record.remaining(), "paragraph payload")?
            .to_vec();
        paragraphs.push(RichTextParagraph {
            kind,
            start_utf16,
            end_utf16,
            payload,
        });
    }

    let margins = Some([
        common.read_f32("left text margin")?,
        common.read_f32("top text margin")?,
        common.read_f32("right text margin")?,
        common.read_f32("bottom text margin")?,
    ]);
    let gravity = Some(common.read_u8("text gravity")?);

    Ok(TextCommon {
        text,
        spans,
        paragraphs,
        margins,
        gravity,
    })
}

fn utf16_to_char_index(text: &str, target: u32) -> Option<usize> {
    let target = usize::try_from(target).ok()?;
    let mut utf16_offset = 0_usize;
    for (char_index, character) in text.chars().enumerate() {
        if utf16_offset == target {
            return Some(char_index);
        }
        utf16_offset = utf16_offset.checked_add(character.len_utf16())?;
        if utf16_offset > target {
            return None;
        }
    }
    (utf16_offset == target).then_some(text.chars().count())
}

fn check_limit(resource: &'static str, limit: usize, actual: usize) -> Result<()> {
    if actual > limit {
        Err(Error::LimitExceeded {
            resource,
            limit: u64::try_from(limit).unwrap_or(u64::MAX),
            actual: u64::try_from(actual).unwrap_or(u64::MAX),
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ObjectBase, parse_text_common_at};
    use crate::{BoundingBox, ParseLimits, RichTextParagraphType, RichTextSpanType};

    #[test]
    fn decodes_utf16_ranges_without_splitting_emoji() {
        let text = "A😀B";
        let mut payload = Vec::new();
        payload.extend_from_slice(&(text.encode_utf16().count() as u32).to_le_bytes());
        for unit in text.encode_utf16() {
            payload.extend_from_slice(&unit.to_le_bytes());
        }
        payload.extend_from_slice(&1_u32.to_le_bytes());
        payload.extend_from_slice(&20_u16.to_le_bytes());
        payload.extend_from_slice(&5_u32.to_le_bytes());
        payload.extend_from_slice(&1_u32.to_le_bytes());
        payload.extend_from_slice(&3_u32.to_le_bytes());
        payload.extend_from_slice(&0_u32.to_le_bytes());
        payload.extend_from_slice(&1_u32.to_le_bytes());
        payload.extend_from_slice(&1_u32.to_le_bytes());
        payload.extend_from_slice(&16_u16.to_le_bytes());
        payload.extend_from_slice(&5_u32.to_le_bytes());
        payload.extend_from_slice(&0_u32.to_le_bytes());
        payload.extend_from_slice(&4_u32.to_le_bytes());
        payload.extend_from_slice(&8_u32.to_le_bytes());
        for margin in [1.0_f32, 2.0, 3.0, 4.0] {
            payload.extend_from_slice(&margin.to_le_bytes());
        }
        payload.push(3);

        let mut data = (payload.len() as u32).to_le_bytes().to_vec();
        data.extend_from_slice(&payload);
        let common = parse_text_common_at(&data, 0, &ParseLimits::default(), "test text").unwrap();
        let box_ = common.into_rich_text_box(ObjectBase {
            bbox: BoundingBox::default(),
            rotation_degrees: None,
        });

        assert_eq!(box_.text, text);
        assert_eq!(box_.spans[0].kind, RichTextSpanType::Bold);
        assert_eq!((box_.runs[0].start, box_.runs[0].end), (1, 2));
        assert_eq!(box_.paragraphs[0].kind, RichTextParagraphType::Bullet);
        assert_eq!(box_.margins, Some([1.0, 2.0, 3.0, 4.0]));
        assert_eq!(box_.gravity, Some(3));
    }
}

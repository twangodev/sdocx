use crate::binary::Reader;
use crate::error::{Error, Result};
use crate::frame::Frame;
use crate::types::{
    BoundingBox, ObjectSpanLayoutConstraint, ObjectSpanLayoutOption, ObjectType, RichTextBox,
    RichTextCodeBlock, RichTextObjectContent, RichTextObjectSpan, RichTextParagraph,
    RichTextParagraphType, RichTextRun, RichTextSection, RichTextSpan, RichTextSpanType,
    RichTextTable, RichTextTableCell, RichTextTableRow,
};
use crate::{ObjectMetadata, ParseLimits};

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
        0,
    )?;
    let body_size = usize::try_from(reader.read_u32("body object size")?)
        .map_err(|_| Error::Format("body object size does not fit in memory".into()))?;
    let body = parse_text_object(
        reader.read_bytes(body_size, "body object")?,
        limits,
        "note body",
        0,
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

/// Structured page text plus optional features retained/skipped without full
/// semantic support. The page parser turns these into caller-visible findings.
pub(crate) struct DecodedTextBox {
    pub(crate) text_box: RichTextBox,
    pub(crate) unsupported: Vec<&'static str>,
}

pub(crate) fn parse_page_text_box(data: &[u8], limits: &ParseLimits) -> Result<DecodedTextBox> {
    let mut reader = Reader::new(data, "page text box");
    let mut decoded = parse_text_frames(&mut reader, limits, "page text box", 0)?;
    let tail = Frame::read(&mut reader)?;
    tail.expect_kind(2)?;
    // Native ComponentImage::TextboxGetOwnBinary writes border color, width
    // and type as field bits 1, 2 and 3. They are not text-formatting spans.
    let mut border = Reader::new(tail.flexible, "text box border");
    if !tail.fields.contains(0) {
        if tail.fields.contains(1) {
            border.read_u32("border color")?;
        }
        if tail.fields.contains(2) {
            let width = border.read_f32("border width")?;
            if !width.is_finite() || width < 0.0 {
                return Err(Error::Format("invalid text box border width".into()));
            }
        }
        if tail.fields.contains(3) {
            border.read_u16("border type")?;
        }
    }
    if tail.fields.has_other_bits(0)
        || tail.properties.has_other_bits(0)
        || !tail.fixed.is_empty()
        || border.remaining() != 0
    {
        decoded
            .unsupported
            .push("text box border or extension settings");
    }
    while reader.remaining() != 0 {
        Frame::read(&mut reader)?;
        if !decoded.unsupported.contains(&"additional text box frames") {
            decoded.unsupported.push("additional text box frames");
        }
    }
    Ok(decoded)
}

fn parse_text_object(
    data: &[u8],
    limits: &ParseLimits,
    context: &'static str,
    depth: usize,
) -> Result<RichTextBox> {
    let mut reader = Reader::new(data, context);
    Ok(parse_text_frames(&mut reader, limits, context, depth)?.text_box)
}

fn parse_text_frames(
    reader: &mut Reader<'_>,
    limits: &ParseLimits,
    context: &'static str,
    depth: usize,
) -> Result<DecodedTextBox> {
    check_limit(
        "rich-text object nesting depth",
        limits.max_object_nesting_depth,
        depth.saturating_add(1),
    )?;
    let object_base = ObjectMetadata::read(reader)?;
    let shape = Frame::read(reader)?;
    shape.expect_kind(6)?;
    let shape_text = Frame::read(reader)?;
    shape_text.expect_kind(7)?;
    let mut flexible = Reader::new(shape_text.flexible, context);
    let common = if shape_text.fields.contains(0) {
        parse_text_common(&mut flexible, limits, context, depth)?
    } else {
        TextCommon::default()
    };
    let mut unsupported = Vec::new();
    if shape.fields.has_other_bits(0)
        || shape.properties.has_other_bits(0)
        || !shape.fixed.is_empty()
        || !shape.flexible.is_empty()
    {
        unsupported.push("shape settings");
    }
    if shape_text.fields.has_other_bits(1)
        || shape_text.properties.has_other_bits(0)
        || !shape_text.fixed.is_empty()
        || flexible.remaining() != 0
    {
        unsupported.push("shape-text extension fields");
    }
    Ok(finish_shape_text(common, object_base, unsupported))
}

pub(crate) fn parse_shape_text(
    reader: &mut Reader<'_>,
    limits: &ParseLimits,
    metadata: ObjectMetadata,
) -> Result<DecodedTextBox> {
    check_limit(
        "rich-text object nesting depth",
        limits.max_object_nesting_depth,
        1,
    )?;
    let common = parse_text_common(reader, limits, "shape text", 0)?;
    Ok(finish_shape_text(common, metadata, Vec::new()))
}

fn finish_shape_text(
    common: TextCommon,
    object_base: ObjectMetadata,
    mut unsupported: Vec<&'static str>,
) -> DecodedTextBox {
    if common.has_extensions {
        unsupported.push("text-common extension data");
    }
    if common
        .spans
        .iter()
        .any(|span| matches!(span.kind, RichTextSpanType::Other(_)))
    {
        unsupported.push("unknown text style spans");
    }
    if common
        .paragraphs
        .iter()
        .any(|paragraph| matches!(paragraph.kind, RichTextParagraphType::Other(_)))
    {
        unsupported.push("unknown text paragraph records");
    }
    if common
        .object_spans
        .iter()
        .any(|span| span.content.is_none())
    {
        unsupported.push("unsupported embedded text objects");
    }
    DecodedTextBox {
        text_box: common.into_rich_text_box(object_base),
        unsupported,
    }
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

#[derive(Default)]
struct TextCommon {
    has_extensions: bool,
    text: String,
    spans: Vec<RichTextSpan>,
    paragraphs: Vec<RichTextParagraph>,
    object_spans: Vec<RichTextObjectSpan>,
    text_sections: Vec<RichTextSection>,
    margins: Option<[f32; 4]>,
    gravity: Option<u8>,
}

impl TextCommon {
    fn into_rich_text_box(self, object_base: ObjectMetadata) -> RichTextBox {
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
            object_spans: self.object_spans,
            text_sections: self.text_sections,
            margins: self.margins,
            gravity: self.gravity,
        }
    }
}

fn parse_text_common(
    reader: &mut Reader<'_>,
    limits: &ParseLimits,
    context: &'static str,
    depth: usize,
) -> Result<TextCommon> {
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
        let start_paragraph = record.read_u32("paragraph start")?;
        let end_paragraph = record.read_u32("paragraph end")?;
        let payload = record
            .read_bytes(record.remaining(), "paragraph payload")?
            .to_vec();
        paragraphs.push(RichTextParagraph {
            kind,
            start_paragraph,
            end_paragraph,
            payload,
        });
    }

    let margins = [
        common.read_f32("left text margin")?,
        common.read_f32("top text margin")?,
        common.read_f32("right text margin")?,
        common.read_f32("bottom text margin")?,
    ];
    if margins.iter().any(|margin| !margin.is_finite()) {
        return Err(Error::Format(format!("{context}: non-finite text margin")));
    }
    let gravity = Some(common.read_u8("text gravity")?);
    let text_sections = if common.remaining() >= 2 {
        let section_count = usize::from(common.read_u16("text section count")?);
        check_limit("text sections", limits.max_pages, section_count)?;
        let mut sections = Vec::with_capacity(section_count);
        for _ in 0..section_count {
            sections.push(RichTextSection {
                start_utf16: common.read_u32("text section start")? as i32,
                length_utf16: common.read_u32("text section length")? as i32,
            });
        }
        sections
    } else {
        Vec::new()
    };
    let mut has_extensions = false;
    let object_spans = if common.remaining() >= 8 {
        let object_span_flags = common.read_u32("object span flags")?;
        has_extensions |= object_span_flags & !1 != 0;
        common.read_u32("object span reserved field")?;
        if object_span_flags & 1 != 0 {
            parse_object_spans(&mut common, limits, context, depth)?
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    Ok(TextCommon {
        has_extensions: has_extensions || common.remaining() != 0,
        text,
        spans,
        paragraphs,
        object_spans,
        text_sections,
        margins: Some(margins),
        gravity,
    })
}

fn parse_object_spans(
    reader: &mut Reader<'_>,
    limits: &ParseLimits,
    context: &'static str,
    depth: usize,
) -> Result<Vec<RichTextObjectSpan>> {
    let count = usize::try_from(reader.read_u32("object span count")?)
        .map_err(|_| Error::Format(format!("{context}: object span count is too large")))?;
    check_limit("text object spans", limits.max_text_object_spans, count)?;
    let mut spans = Vec::with_capacity(count);
    for _ in 0..count {
        let span_size = usize::try_from(reader.read_u32("object span size")?)
            .map_err(|_| Error::Format(format!("{context}: object span size is too large")))?;
        if span_size < 20 {
            return Err(Error::Format(format!(
                "{context}: object span size {span_size} is smaller than 20"
            )));
        }
        let span_bytes = reader.read_bytes(span_size, "object span")?;
        let mut span = Reader::new(span_bytes, context);
        let object_size = usize::try_from(span.read_u32("object span object size")?)
            .map_err(|_| Error::Format(format!("{context}: embedded object size is too large")))?;
        if object_size > span_size - 20 {
            return Err(Error::Format(format!(
                "{context}: embedded object size {object_size} exceeds its {span_size}-byte span"
            )));
        }
        let object_type = ObjectType::from(span.read_u32("object span object type")?);
        let object_data = span
            .read_bytes(object_size, "object span object binary")?
            .to_vec();
        let content = parse_object_span_content(
            object_type,
            &object_data,
            limits,
            context,
            depth.saturating_add(1),
        )?;
        let text_index_utf16 = span.read_u32("object span text index")? as i32;
        let layout_option = ObjectSpanLayoutOption::from(span.read_u32("object span layout")?);
        let layout_constraint =
            ObjectSpanLayoutConstraint::from(span.read_u32("object span constraint")?);
        spans.push(RichTextObjectSpan {
            object_type,
            object_data,
            content,
            text_index_utf16,
            layout_option,
            layout_constraint,
        });
    }
    Ok(spans)
}

fn parse_object_span_content(
    object_type: ObjectType,
    data: &[u8],
    limits: &ParseLimits,
    context: &'static str,
    depth: usize,
) -> Result<Option<RichTextObjectContent>> {
    match object_type {
        ObjectType::Table => parse_table_object(data, limits, context, depth)
            .map(Box::new)
            .map(RichTextObjectContent::Table)
            .map(Some),
        ObjectType::CodeBlock => parse_code_block_object(data, limits, context, depth)
            .map(Box::new)
            .map(RichTextObjectContent::CodeBlock)
            .map(Some),
        _ => Ok(None),
    }
}

fn parse_code_block_object(
    data: &[u8],
    limits: &ParseLimits,
    context: &'static str,
    depth: usize,
) -> Result<RichTextCodeBlock> {
    check_limit(
        "rich-text object nesting depth",
        limits.max_object_nesting_depth,
        depth.saturating_add(1),
    )?;
    let mut reader = Reader::new(data, context);
    let object_base = ObjectMetadata::read(&mut reader)?;
    let frame = open_next_object_frame(&mut reader, 23, "code block")?;
    let mut flexible = Reader::new(frame.flexible, context);
    let title = if frame.fields.contains(0) {
        Some(parse_sized_text_object(
            &mut flexible,
            limits,
            "code block title",
            depth,
        )?)
    } else {
        None
    };
    let body = if frame.fields.contains(1) {
        Some(parse_sized_text_object(
            &mut flexible,
            limits,
            "code block body",
            depth,
        )?)
    } else {
        None
    };

    Ok(RichTextCodeBlock {
        bbox: object_base.bbox,
        rotation_degrees: object_base.rotation_degrees,
        title,
        body,
    })
}

fn parse_table_object(
    data: &[u8],
    limits: &ParseLimits,
    context: &'static str,
    depth: usize,
) -> Result<RichTextTable> {
    check_limit(
        "rich-text object nesting depth",
        limits.max_object_nesting_depth,
        depth.saturating_add(1),
    )?;
    let mut reader = Reader::new(data, context);
    let object_base = ObjectMetadata::read(&mut reader)?;
    let frame = open_next_object_frame(&mut reader, 22, "table")?;
    let mut flexible = Reader::new(frame.flexible, context);

    if frame.fields.contains(0) {
        flexible.read_f32("table vertical cell padding")?;
    }
    if frame.fields.contains(1) {
        flexible.read_f32("table horizontal cell padding")?;
    }

    let column_widths = if frame.fields.contains(2) {
        let count = usize::try_from(flexible.read_u32("table column count")?)
            .map_err(|_| Error::Format(format!("{context}: table column count is too large")))?;
        check_limit("table columns", limits.max_objects_per_page, count)?;
        let mut widths = Vec::with_capacity(count);
        for _ in 0..count {
            widths.push(flexible.read_f32("table column width")?);
        }
        widths
    } else {
        Vec::new()
    };

    let rows = if frame.fields.contains(3) {
        let count = usize::try_from(flexible.read_u32("table row count")?)
            .map_err(|_| Error::Format(format!("{context}: table row count is too large")))?;
        check_limit("table rows", limits.max_objects_per_page, count)?;
        let mut rows = Vec::with_capacity(count);
        let mut total_cells = 0_usize;
        for _ in 0..count {
            let row_size = usize::try_from(flexible.read_u32("table row size")?)
                .map_err(|_| Error::Format(format!("{context}: table row size is too large")))?;
            let row_data = flexible.read_bytes(row_size, "table row")?;
            let row = parse_table_row(row_data, limits, context, depth)?;
            total_cells = total_cells
                .checked_add(row.cells.len())
                .ok_or_else(|| Error::Format(format!("{context}: table cell count overflows")))?;
            check_limit("table cells", limits.max_objects_per_page, total_cells)?;
            rows.push(row);
        }
        rows
    } else {
        Vec::new()
    };

    Ok(RichTextTable {
        bbox: object_base.bbox,
        rotation_degrees: object_base.rotation_degrees,
        column_widths,
        rows,
    })
}

fn parse_table_row(
    data: &[u8],
    limits: &ParseLimits,
    context: &'static str,
    depth: usize,
) -> Result<RichTextTableRow> {
    let mut reader = Reader::new(data, context);
    let flexible_offset = reader.read_u32("table row flexible-data offset")?;
    read_bitfield(&mut reader, "table row property mask")?;
    read_bitfield(&mut reader, "table row field mask")?;
    let height = reader.read_f32("table row height")?;
    let index = reader.read_u32("table row index")?;
    let count = usize::try_from(reader.read_u32("table row cell count")?)
        .map_err(|_| Error::Format(format!("{context}: table row cell count is too large")))?;
    check_limit("table row cells", limits.max_objects_per_page, count)?;
    let mut cells = Vec::with_capacity(count);
    for _ in 0..count {
        let cell_size = usize::try_from(reader.read_u32("table cell size")?)
            .map_err(|_| Error::Format(format!("{context}: table cell size is too large")))?;
        cells.push(parse_table_cell(
            reader.read_bytes(cell_size, "table cell")?,
            limits,
            context,
            depth,
        )?);
    }
    if flexible_offset != 0 && flexible_offset as usize > data.len() {
        return Err(Error::Format(format!(
            "{context}: table row flexible-data offset is outside its record"
        )));
    }
    Ok(RichTextTableRow {
        index,
        height,
        cells,
    })
}

fn parse_table_cell(
    data: &[u8],
    limits: &ParseLimits,
    context: &'static str,
    depth: usize,
) -> Result<RichTextTableCell> {
    let mut reader = Reader::new(data, context);
    let flexible_offset = reader.read_u32("table cell flexible-data offset")?;
    let properties = read_bitfield(&mut reader, "table cell property mask")?;
    read_bitfield(&mut reader, "table cell field mask")?;
    let column_index = reader.read_u32("table cell column index")?;
    let row_span = reader.read_u32("table cell row span")?;
    let column_span = reader.read_u32("table cell column span")?;
    let background_color = reader.read_u32("table cell background color")?;
    let bbox = BoundingBox {
        x_min: reader.read_f64("table cell left")?,
        y_min: reader.read_f64("table cell top")?,
        x_max: reader.read_f64("table cell right")?,
        y_max: reader.read_f64("table cell bottom")?,
    };
    let vertical_alignment = reader.read_u8("table cell vertical alignment")?;
    let content = parse_sized_text_object(&mut reader, limits, "table cell text", depth)?;
    if flexible_offset != 0 && flexible_offset as usize > data.len() {
        return Err(Error::Format(format!(
            "{context}: table cell flexible-data offset is outside its record"
        )));
    }
    Ok(RichTextTableCell {
        column_index,
        row_span,
        column_span,
        background_color,
        has_own_background_color: properties & 1 != 0,
        bbox,
        vertical_alignment,
        content,
    })
}

fn parse_sized_text_object(
    reader: &mut Reader<'_>,
    limits: &ParseLimits,
    field: &'static str,
    depth: usize,
) -> Result<RichTextBox> {
    let size = usize::try_from(reader.read_u32(field)?)
        .map_err(|_| Error::Format(format!("{field} size is too large")))?;
    parse_text_object(
        reader.read_bytes(size, field)?,
        limits,
        field,
        depth.saturating_add(1),
    )
}

fn open_next_object_frame<'a>(
    reader: &mut Reader<'a>,
    expected_data_type: i16,
    field: &'static str,
) -> Result<Frame<'a>> {
    while reader.remaining() != 0 {
        let frame = Frame::read(reader)?;
        if frame.kind == expected_data_type {
            return Ok(frame);
        }
    }
    Err(Error::Format(format!(
        "{field} frame type {expected_data_type} was not found"
    )))
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
    use super::{ObjectMetadata, parse_code_block_object, parse_table_object, parse_text_common};
    use crate::{
        ObjectSpanLayoutConstraint, ObjectType, ParseLimits, RichTextParagraphType,
        RichTextSpanType,
    };

    fn object_base_frame() -> Vec<u8> {
        let mut data = 63_u32.to_le_bytes().to_vec();
        data.extend_from_slice(&0_i16.to_le_bytes());
        data.extend_from_slice(&0_u32.to_le_bytes());
        data.push(0);
        data.push(0);
        data.extend_from_slice(&5500_u32.to_le_bytes());
        data.extend_from_slice(&0_u16.to_le_bytes());
        data.extend_from_slice(&0_i64.to_le_bytes());
        for value in [0.0_f64, 0.0, 100.0, 100.0] {
            data.extend_from_slice(&value.to_le_bytes());
        }
        data.extend_from_slice(&0_u32.to_le_bytes());
        data.push(0);
        data
    }

    fn empty_text_object() -> Vec<u8> {
        let mut data = object_base_frame();
        data.extend_from_slice(&object_frame(6, 0, &[]));
        data.extend_from_slice(&12_u32.to_le_bytes());
        data.extend_from_slice(&7_i16.to_le_bytes());
        data.extend_from_slice(&0_u32.to_le_bytes());
        data.push(0);
        data.push(0);
        data
    }

    fn object_frame(kind: i16, fields: u8, flexible: &[u8]) -> Vec<u8> {
        let header_size = 13_u32;
        let mut data = (header_size + flexible.len() as u32).to_le_bytes().to_vec();
        data.extend_from_slice(&kind.to_le_bytes());
        data.extend_from_slice(&header_size.to_le_bytes());
        data.push(0);
        data.push(1);
        data.push(fields);
        data.extend_from_slice(flexible);
        data
    }

    #[test]
    fn decodes_code_block_text_objects() {
        let title = empty_text_object();
        let mut flexible = (title.len() as u32).to_le_bytes().to_vec();
        flexible.extend_from_slice(&title);
        let mut data = object_base_frame();
        data.extend_from_slice(&object_frame(23, 1, &flexible));

        let code = parse_code_block_object(&data, &ParseLimits::default(), "test code", 0).unwrap();

        assert_eq!(code.title.unwrap().text, "");
        assert!(code.body.is_none());
    }

    #[test]
    fn decodes_table_rows_cells_and_text() {
        let text = empty_text_object();
        let mut cell = 0_u32.to_le_bytes().to_vec();
        cell.push(1);
        cell.push(1);
        cell.push(0);
        for value in [0_u32, 1, 1, 0xffeeeeee] {
            cell.extend_from_slice(&value.to_le_bytes());
        }
        for value in [0.0_f64, 0.0, 50.0, 24.0] {
            cell.extend_from_slice(&value.to_le_bytes());
        }
        cell.push(0);
        cell.extend_from_slice(&(text.len() as u32).to_le_bytes());
        cell.extend_from_slice(&text);

        let mut row = 0_u32.to_le_bytes().to_vec();
        row.push(0);
        row.push(0);
        row.extend_from_slice(&24.0_f32.to_le_bytes());
        row.extend_from_slice(&7_u32.to_le_bytes());
        row.extend_from_slice(&1_u32.to_le_bytes());
        row.extend_from_slice(&(cell.len() as u32).to_le_bytes());
        row.extend_from_slice(&cell);

        let mut flexible = 1_u32.to_le_bytes().to_vec();
        flexible.extend_from_slice(&50.0_f32.to_le_bytes());
        flexible.extend_from_slice(&1_u32.to_le_bytes());
        flexible.extend_from_slice(&(row.len() as u32).to_le_bytes());
        flexible.extend_from_slice(&row);
        let mut data = object_base_frame();
        data.extend_from_slice(&object_frame(22, 0x0c, &flexible));

        let table = parse_table_object(&data, &ParseLimits::default(), "test table", 0).unwrap();

        assert_eq!(table.column_widths, [50.0]);
        assert_eq!(table.rows[0].index, 7);
        assert_eq!(table.rows[0].height, 24.0);
        let cell = &table.rows[0].cells[0];
        assert_eq!(cell.column_index, 0);
        assert_eq!(cell.row_span, 1);
        assert_eq!(cell.column_span, 1);
        assert!(cell.has_own_background_color);
        assert_eq!(cell.content.text, "");
    }

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
        payload.extend_from_slice(&2_u16.to_le_bytes());
        payload.extend_from_slice(&0_i32.to_le_bytes());
        payload.extend_from_slice(&3_i32.to_le_bytes());
        payload.extend_from_slice(&3_i32.to_le_bytes());
        payload.extend_from_slice(&1_i32.to_le_bytes());
        payload.extend_from_slice(&1_u32.to_le_bytes());
        payload.extend_from_slice(&0_u32.to_le_bytes());
        payload.extend_from_slice(&1_u32.to_le_bytes());
        payload.extend_from_slice(&20_u32.to_le_bytes());
        payload.extend_from_slice(&0_u32.to_le_bytes());
        payload.extend_from_slice(&24_u32.to_le_bytes());
        payload.extend_from_slice(&2_i32.to_le_bytes());
        payload.extend_from_slice(&0_u32.to_le_bytes());
        payload.extend_from_slice(&2_u32.to_le_bytes());

        let mut data = (payload.len() as u32).to_le_bytes().to_vec();
        data.extend_from_slice(&payload);
        let common = parse_text_common(
            &mut crate::binary::Reader::new(&data, "test text"),
            &ParseLimits::default(),
            "test text",
            0,
        )
        .unwrap();
        let box_ = common.into_rich_text_box(
            ObjectMetadata::read(&mut crate::binary::Reader::new(
                &object_base_frame(),
                "test base",
            ))
            .unwrap(),
        );

        assert_eq!(box_.text, text);
        assert_eq!(box_.spans[0].kind, RichTextSpanType::Bold);
        assert_eq!((box_.runs[0].start, box_.runs[0].end), (1, 2));
        assert_eq!(box_.paragraphs[0].kind, RichTextParagraphType::Bullet);
        assert_eq!(box_.text_sections[0].start_utf16, 0);
        assert_eq!(box_.text_sections[0].length_utf16, 3);
        assert_eq!(box_.text_sections[1].start_utf16, 3);
        assert_eq!(box_.text_sections[1].length_utf16, 1);
        assert_eq!(box_.object_spans[0].object_type, ObjectType::AttachedFile);
        assert!(box_.object_spans[0].content.is_none());
        assert_eq!(box_.object_spans[0].text_index_utf16, 2);
        assert_eq!(
            box_.object_spans[0].layout_constraint,
            ObjectSpanLayoutConstraint::OverPages
        );
        assert_eq!(box_.margins, Some([1.0, 2.0, 3.0, 4.0]));
        assert_eq!(box_.gravity, Some(3));
    }
}

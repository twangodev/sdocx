use crate::ParseLimits;
use crate::binary::Reader;
use crate::decode::decode_stroke;
use crate::error::{Error, Result};
use crate::object::read_bbox;
use crate::storage::{StoredObject, StoredPage};
use crate::types::{
    BoundingBox, Color, ObjectType, Page, PageElement, PageTemplate, PageTemplateSource,
    RichTextBox, RichTextRun,
};

/// Derive visible content from the same bounded object tree returned to SDK
/// callers. Stroke records are never located by offsets or byte scanning.
pub(crate) fn parse_page(data: &[u8], stored: &StoredPage, limits: &ParseLimits) -> Result<Page> {
    let header = &stored.header;
    let stroke_count = stored
        .layers
        .layers
        .iter()
        .map(|layer| count_strokes(&layer.objects))
        .sum();
    check_limit(
        "strokes per page",
        limits.max_strokes_per_page,
        stroke_count,
    )?;
    let mut page = Page {
        uuid: header.uuid.clone(),
        width: header.width,
        height: header.height,
        content_bbox: BoundingBox::default(),
        background_color: None,
        template: None,
        strokes: Vec::with_capacity(stroke_count),
        elements: Vec::new(),
    };
    parse_page_properties(data, stored, &mut page)?;
    let mut image_count = 0;
    for layer in &stored.layers.layers {
        decode_objects(data, &layer.objects, &mut page, &mut image_count, limits)?;
    }
    Ok(page)
}

fn count_strokes(objects: &[StoredObject]) -> usize {
    objects
        .iter()
        .map(|object| {
            usize::from(object.object_type == ObjectType::Stroke) + count_strokes(&object.children)
        })
        .sum()
}

fn decode_objects(
    data: &[u8],
    objects: &[StoredObject],
    page: &mut Page,
    image_count: &mut usize,
    limits: &ParseLimits,
) -> Result<()> {
    for object in objects {
        let payload = object
            .payload(data)
            .ok_or_else(|| Error::Format("object payload is outside its page".into()))?;
        if object.object_type == ObjectType::Stroke {
            let stroke = decode_stroke(payload, limits).map_err(|error| match error {
                Error::Format(message) => Error::Format(format!(
                    "page {}: stroke at 0x{:x}: {message}",
                    page.uuid, object.payload_offset
                )),
                error => error,
            })?;
            page.strokes.push(stroke);
        } else if matches!(
            object.object_type,
            ObjectType::TextBox | ObjectType::Image | ObjectType::Shape | ObjectType::Line
        ) {
            // Non-stroke interpretation remains best-effort, but scanning is
            // now confined to this object's payload, never its siblings.
            let elements = parse_page_elements(payload, 0, page.width, page.height, limits)?;
            check_limit(
                "objects per page",
                limits.max_objects_per_page,
                page.elements.len() + elements.len(),
            )?;
            for mut element in elements {
                if let PageElement::Image { media_index, .. } = &mut element {
                    *media_index = *image_count;
                    *image_count += 1;
                }
                page.elements.push(element);
            }
        }
        decode_objects(data, &object.children, page, image_count, limits)?;
    }
    Ok(())
}

fn parse_page_properties(data: &[u8], stored: &StoredPage, page: &mut Page) -> Result<()> {
    let header = &stored.header;
    if header.property_offset == 0 {
        return Ok(());
    }
    let bytes = data
        .get(header.property_offset as usize..header.raw_layer_offset as usize)
        .ok_or_else(|| Error::Format("page flexible fields are outside the header".into()))?;
    let mut fields = Reader::new(bytes, "page flexible fields");
    let mut pdf_page_index = None;
    for bit in 0..=9 {
        if header.property_mask & (1 << bit) == 0 {
            continue;
        }
        match bit {
            0 => page.content_bbox = read_bbox(&mut fields)?,
            1 => {
                let count = fields.read_u16("tag count")?;
                for _ in 0..count {
                    fields.read_utf16_u16("tag")?;
                }
            }
            2 => {
                fields.read_utf16_u16("template URI")?;
            }
            3 | 4 | 6 | 7 => {
                fields.read_u32("background property")?;
            }
            5 => {
                let argb = fields.read_u32("background color")?;
                page.background_color = Some(Color {
                    r: (argb >> 16) as u8,
                    g: (argb >> 8) as u8,
                    b: argb as u8,
                });
            }
            8 => {
                let count = fields.read_u16("PDF record count")?;
                for index in 0..count {
                    fields.read_u32("PDF media ID")?;
                    let page_index = fields.read_u32("PDF page index")?;
                    if index == 0 {
                        pdf_page_index = Some(page_index);
                    }
                    fields.skip(16, "PDF rectangle")?;
                }
            }
            9 => {
                let id = fields.read_u32("template type")?;
                if is_builtin_template_id(id) {
                    page.template = Some(PageTemplate {
                        id,
                        source: PageTemplateSource::BuiltIn,
                    });
                }
            }
            _ => unreachable!(),
        }
    }
    if let Some(page_index) = pdf_page_index {
        page.template = Some(PageTemplate {
            id: page_index,
            source: PageTemplateSource::CustomPdf { page_index },
        });
    }
    Ok(())
}

fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    Some(u32::from_le_bytes(data.get(offset..end)?.try_into().ok()?))
}

fn read_f64(data: &[u8], offset: usize) -> Option<f64> {
    let end = offset.checked_add(8)?;
    Some(f64::from_le_bytes(data.get(offset..end)?.try_into().ok()?))
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

fn is_builtin_template_id(id: u32) -> bool {
    id != 0 && id <= 0xFFFF
}

fn parse_page_elements(
    data: &[u8],
    start: usize,
    width: u32,
    height: u32,
    limits: &ParseLimits,
) -> Result<Vec<PageElement>> {
    let mut elements = Vec::new();
    let mut image_count = 0;

    let mut uuid_off = find_next_ascii_uuid(data, start);
    while let Some(current_uuid_off) = uuid_off {
        let next_uuid = current_uuid_off
            .checked_add(36)
            .and_then(|start| find_next_ascii_uuid(data, start));
        let record_end = next_uuid.unwrap_or(data.len());

        let Some(bbox) = find_object_bbox(data, current_uuid_off, width, height) else {
            uuid_off = next_uuid;
            continue;
        };
        let record = &data[current_uuid_off..record_end];

        if let Some(text_box) = parse_text_box_record(record, bbox) {
            elements.push(PageElement::TextBox(text_box));
        } else if looks_like_image_record(record) {
            elements.push(PageElement::Image {
                bbox,
                media_index: image_count,
            });
            image_count += 1;
        }
        check_limit(
            "objects per page",
            limits.max_objects_per_page,
            elements.len(),
        )?;
        uuid_off = next_uuid;
    }

    Ok(elements)
}

fn find_next_ascii_uuid(data: &[u8], start: usize) -> Option<usize> {
    let mut offset = start;
    while let Some(end) = offset.checked_add(36) {
        let Some(candidate) = data.get(offset..end) else {
            break;
        };
        if is_ascii_uuid(candidate) {
            return Some(offset);
        }
        offset = offset.checked_add(1)?;
    }
    None
}

fn is_ascii_uuid(bytes: &[u8]) -> bool {
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(i, &b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_hexdigit(),
        })
}

fn find_object_bbox(data: &[u8], uuid_off: usize, width: u32, height: u32) -> Option<BoundingBox> {
    let search_end = (uuid_off + 128).min(data.len().saturating_sub(32));
    for offset in uuid_off + 36..=search_end {
        let bbox = BoundingBox {
            x_min: read_f64(data, offset)?,
            y_min: read_f64(data, offset + 8)?,
            x_max: read_f64(data, offset + 16)?,
            y_max: read_f64(data, offset + 24)?,
        };
        if plausible_bbox(bbox, width, height) {
            return Some(bbox);
        }
    }

    None
}

fn plausible_bbox(bbox: BoundingBox, width: u32, height: u32) -> bool {
    bbox.x_min.is_finite()
        && bbox.y_min.is_finite()
        && bbox.x_max.is_finite()
        && bbox.y_max.is_finite()
        && bbox.x_min >= 1.0
        && bbox.y_min >= 1.0
        && bbox.x_max > bbox.x_min
        && bbox.y_max > bbox.y_min
        && bbox.x_max <= width as f64 * 1.25
        && bbox.y_max <= height as f64 * 1.25
        && bbox.x_max - bbox.x_min > 8.0
        && bbox.y_max - bbox.y_min > 8.0
}

fn looks_like_image_record(record: &[u8]) -> bool {
    record.windows(4).any(|window| window == b"Re")
        || record
            .windows(4)
            .any(|window| window == b"\x01\x00\x04\x20")
}

fn parse_text_box_record(record: &[u8], bbox: BoundingBox) -> Option<RichTextBox> {
    let (text, text_end) = first_utf16_text(record)?;
    let styles = &record[text_end..];
    let color = tlv_color(styles, 0x01);
    let highlight_color = tlv_color(styles, 0x11);
    let underline = tlv_u32(styles, 0x07).is_some_and(|value| value != 0)
        || tlv_u32(styles, 0x06).is_some_and(|value| value != 0);
    let font_size = tlv_f32(styles, 0x03);
    let rotation_degrees = infer_rotation_degrees(record, bbox);
    let runs = parse_rich_text_runs(styles, text.chars().count());

    Some(RichTextBox {
        bbox,
        rotation_degrees,
        text,
        color,
        highlight_color,
        underline,
        font_size,
        runs,
        spans: Vec::new(),
        paragraphs: Vec::new(),
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: None,
        gravity: None,
    })
}

fn first_utf16_text(data: &[u8]) -> Option<(String, usize)> {
    let mut offset = 0;
    while offset + 6 <= data.len() {
        let mut end = offset;
        let mut units = Vec::new();
        while end + 2 <= data.len() {
            let unit = u16::from_le_bytes(data[end..end + 2].try_into().ok()?);
            let printable = unit == 0x0A || (0x20..=0xD7FF).contains(&unit);
            if !printable {
                break;
            }
            units.push(unit);
            end += 2;
        }
        let text = String::from_utf16(&units).ok()?;
        let trimmed = text.trim();
        if trimmed.chars().filter(|c| !c.is_whitespace()).count() >= 3
            && looks_like_note_text(trimmed)
        {
            return Some((text, end));
        }
        offset += 2;
    }
    None
}

fn looks_like_note_text(text: &str) -> bool {
    let mut total = 0;
    let mut common = 0;
    for ch in text.chars() {
        if ch.is_whitespace() {
            continue;
        }
        total += 1;
        if ch.is_ascii_alphanumeric() || ch.is_ascii_punctuation() {
            common += 1;
        }
    }
    total >= 3 && common * 4 >= total * 3
}

fn tlv_color(data: &[u8], tag: u16) -> Option<Color> {
    let marker = [0x18, 0x00, tag as u8, (tag >> 8) as u8];
    for offset in 0..data.len().saturating_sub(22) {
        if data[offset..offset + 4] == marker && data[offset + 21] == 0xFF {
            return Some(Color {
                r: data[offset + 20],
                g: data[offset + 19],
                b: data[offset + 18],
            });
        }
    }
    None
}

fn tlv_u32(data: &[u8], tag: u16) -> Option<u32> {
    let marker = [0x18, 0x00, tag as u8, (tag >> 8) as u8];
    for offset in 0..data.len().saturating_sub(22) {
        if data[offset..offset + 4] == marker {
            return read_u32(data, offset + 18);
        }
    }
    None
}

fn tlv_f32(data: &[u8], tag: u16) -> Option<f32> {
    let marker = [0x18, 0x00, tag as u8, (tag >> 8) as u8];
    for offset in 0..data.len().saturating_sub(24) {
        if data[offset..offset + 4] == marker {
            for value_offset in [18, 20, 24] {
                let value = f32::from_le_bytes(
                    data[offset + value_offset..offset + value_offset + 4]
                        .try_into()
                        .ok()?,
                );
                if value.is_finite() && (4.0..=96.0).contains(&value) {
                    return Some(value);
                }
            }
        }
    }
    None
}

fn parse_rich_text_runs(data: &[u8], text_len: usize) -> Vec<RichTextRun> {
    let mut runs = Vec::new();
    collect_style_runs(data, text_len, 0x05, true, false, &mut runs);
    collect_style_runs(data, text_len, 0x06, false, true, &mut runs);
    runs
}

fn collect_style_runs(
    data: &[u8],
    text_len: usize,
    tag: u16,
    bold: bool,
    italic: bool,
    runs: &mut Vec<RichTextRun>,
) {
    let marker = [0x18, 0x00, tag as u8, (tag >> 8) as u8];
    for offset in 0..data.len().saturating_sub(18) {
        if data[offset..offset + 4] != marker {
            continue;
        }
        let Some(start) = read_u32(data, offset + 6).map(|value| value as usize) else {
            continue;
        };
        let Some(end) = read_u32(data, offset + 10).map(|value| value as usize) else {
            continue;
        };
        let enabled = read_u32(data, offset + 18).is_some_and(|value| value != 0);
        if enabled && start < end && end <= text_len {
            runs.push(RichTextRun {
                start,
                end,
                bold,
                italic,
            });
        }
    }
}

fn infer_rotation_degrees(record: &[u8], bbox: BoundingBox) -> Option<f64> {
    let mut points = Vec::new();
    for offset in 0..record.len().saturating_sub(16) {
        let x = read_f64(record, offset)?;
        let y = read_f64(record, offset + 8)?;
        if x.is_finite()
            && y.is_finite()
            && x >= bbox.x_min - bbox.x_max
            && x <= bbox.x_max + bbox.x_max
            && y >= bbox.y_min - bbox.y_max
            && y <= bbox.y_max + bbox.y_max
        {
            points.push((x, y));
        }
    }
    for pair in points.windows(2) {
        let dx = pair[1].0 - pair[0].0;
        let dy = pair[1].1 - pair[0].1;
        let distance = dx.hypot(dy);
        if distance > 40.0 {
            let degrees = dy.atan2(dx).to_degrees();
            if degrees.abs() > 5.0 && degrees.abs() < 85.0 {
                return Some(degrees);
            }
        }
    }
    None
}

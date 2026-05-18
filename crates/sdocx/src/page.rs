use crate::decode::{decode_coordinates, decode_trailing};
use crate::error::{Error, Result};
use crate::types::{
    BoundingBox, Color, Page, PageElement, PageTemplate, PageTemplateSource, RichTextBox,
    RichTextRun, Stroke,
};

const PRE_STROKE_RECORD_LEN: usize = 71;
const STROKE_HEADER_LEN: usize = 89; // bbox(32) + meta(41) + start(16)
const EXTRA_LEN_BIAS: u8 = 0x79; // byte value at record+3 when no extras are present

struct ParsedStroke {
    stroke: Stroke,
    next_record_off: usize,
}

/// Parse a `.page` binary file into a `Page`.
///
/// Layout: `base = u32 @ 0x00`; stroke count at `base + 0x66`; first stroke at `base + 0xB5`.
/// Each stroke is preceded by a 71-byte record; on v4.4.x+, byte 3 of that record encodes
/// an extra-attribute-block length (value − 0x79) injected inside the stroke's metadata.
pub fn parse_page(data: &[u8]) -> Result<Page> {
    if data.len() < 0xA0 {
        return Err(Error::Format("page file too short for header".into()));
    }

    // Base offset at 0x00 — shifts stroke fields for files with embedded media
    let base = u32::from_le_bytes(data[0x00..0x04].try_into().unwrap()) as usize;

    // Page dimensions at 0x16 and 0x1A
    let width = u32::from_le_bytes(data[0x16..0x1A].try_into().unwrap());
    let height = u32::from_le_bytes(data[0x1A..0x1E].try_into().unwrap());

    // Page UUID at 0x28, length at 0x26
    let uuid_char_len = u16::from_le_bytes(data[0x26..0x28].try_into().unwrap()) as usize;
    let uuid_bytes = &data[0x28..0x28 + uuid_char_len * 2];
    let uuid: String = uuid_bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .map(|c| char::from_u32(c as u32).unwrap_or('\u{FFFD}'))
        .collect();

    // Content bounding box at 0x80 (4 x f64)
    let content_bbox = BoundingBox {
        x_min: f64::from_le_bytes(data[0x80..0x88].try_into().unwrap()),
        y_min: f64::from_le_bytes(data[0x88..0x90].try_into().unwrap()),
        x_max: f64::from_le_bytes(data[0x90..0x98].try_into().unwrap()),
        y_max: f64::from_le_bytes(data[0x98..0xA0].try_into().unwrap()),
    };

    let background_color = page_background_color(data, base);
    let template = background_color.and_then(|_| page_template(data, base));

    // Stroke count at base + 0x66
    let sc_off = base + 0x66;
    if sc_off + 4 > data.len() {
        return Err(Error::Format("page file too short for stroke count".into()));
    }
    let stroke_count = u32::from_le_bytes(data[sc_off..sc_off + 4].try_into().unwrap()) as usize;

    let mut strokes = Vec::with_capacity(stroke_count);
    // base + 0xB5 - 71 = base + 0x6E; byte 3 of that record is base + 0x71.
    let mut record_off = base + 0xB5 - PRE_STROKE_RECORD_LEN;

    for _ in 0..stroke_count {
        let extra_len = data
            .get(record_off + 3)
            .copied()
            .unwrap_or(EXTRA_LEN_BIAS)
            .saturating_sub(EXTRA_LEN_BIAS) as usize;

        let off = record_off + PRE_STROKE_RECORD_LEN;
        if off + STROKE_HEADER_LEN + extra_len > data.len() {
            break;
        }

        let current = parse_stroke(data, off, extra_len, StrokeLayout::Current);
        let shifted = parse_stroke(data, off, extra_len, StrokeLayout::StartPointMinusThree);

        let parsed = match (current, shifted) {
            (Some(current), Some(shifted))
                if shifted.stroke.points.len() > current.stroke.points.len() =>
            {
                shifted
            }
            (Some(current), _) => current,
            (None, Some(shifted)) => shifted,
            (None, None) => break,
        };

        record_off = parsed.next_record_off;
        strokes.push(parsed.stroke);
    }

    let elements = parse_page_elements(data, record_off, width, height);

    Ok(Page {
        uuid,
        width,
        height,
        content_bbox,
        background_color,
        template,
        strokes,
        elements,
    })
}

#[derive(Clone, Copy)]
enum StrokeLayout {
    Current,
    StartPointMinusThree,
}

fn parse_stroke(
    data: &[u8],
    off: usize,
    extra_len: usize,
    layout: StrokeLayout,
) -> Option<ParsedStroke> {
    let bbox = BoundingBox {
        x_min: read_f64(data, off)?,
        y_min: read_f64(data, off + 8)?,
        x_max: read_f64(data, off + 16)?,
        y_max: read_f64(data, off + 24)?,
    };

    let (meta_off, n_points_off, sp_off, next_record_adjust) = match layout {
        StrokeLayout::Current => (off + 32 + extra_len, 39, off + 73 + extra_len, 0),
        StrokeLayout::StartPointMinusThree => (off + 32, 36, off + 70, 3),
    };

    let data_len = read_u32(data, meta_off + 21)? as usize;
    let n_points = read_u16(data, meta_off + n_points_off)? as usize;
    let start_x = read_f64(data, sp_off)?;
    let start_y = read_f64(data, sp_off + 8)?;
    if !start_x.is_finite() || !start_y.is_finite() || n_points == 0 {
        return None;
    }

    let data_off = sp_off + 16;
    let data_end = data_off.checked_add(data_len)?;
    if data_end > data.len() {
        return None;
    }
    let data_blob = &data[data_off..data_end];

    let (points, n_coord_bytes) =
        decode_coordinates(data_blob, start_x, start_y, n_points.saturating_sub(1));
    if points.is_empty()
        || points
            .iter()
            .any(|point| !point.x.is_finite() || !point.y.is_finite())
    {
        return None;
    }

    let trailing = decode_trailing(data_blob, n_coord_bytes, points.len().saturating_sub(1));

    Some(ParsedStroke {
        stroke: Stroke {
            bbox,
            points,
            pressures: trailing.pressures,
            timestamps: trailing.timestamps,
            tilt_x: trailing.tilt_x,
            tilt_y: trailing.tilt_y,
            color: trailing.color,
            pen_width: trailing.pen_width,
        },
        next_record_off: data_end + next_record_adjust,
    })
}

fn read_u16(data: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        data.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        data.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn read_f64(data: &[u8], offset: usize) -> Option<f64> {
    Some(f64::from_le_bytes(
        data.get(offset..offset + 8)?.try_into().ok()?,
    ))
}

fn page_background_color(data: &[u8], base: usize) -> Option<crate::types::Color> {
    let offset = match base {
        0x90 => 0x84,
        0xA6 => 0x80,
        _ => 0xA4,
    };
    if data.len() >= offset + 4 && data[offset + 3] == 0xFF {
        Some(crate::types::Color {
            r: data[offset + 2],
            g: data[offset + 1],
            b: data[offset],
        })
    } else {
        None
    }
}

fn page_template(data: &[u8], base: usize) -> Option<PageTemplate> {
    match base {
        // Short built-in template page records store the template id in the compact header.
        0x90 => {
            let id = read_u32(data, 0x8C)?;
            is_builtin_template_id(id).then_some(PageTemplate {
                id,
                source: PageTemplateSource::BuiltIn,
            })
        }
        // Custom downloaded templates are backed by media PDFs. The compact header stores the
        // zero-based PDF page index in the high 16 bits of this field.
        0xA6 => {
            let page_index = read_u32(data, 0x8C)? >> 16;
            Some(PageTemplate {
                id: page_index,
                source: PageTemplateSource::CustomPdf { page_index },
            })
        }
        _ => {
            let id = if base >= 0xE7 {
                read_u32(data, 0xAC)?
            } else {
                read_u32(data, 0xB4)?
            };
            is_builtin_template_id(id).then_some(PageTemplate {
                id,
                source: PageTemplateSource::BuiltIn,
            })
        }
    }
}

fn is_builtin_template_id(id: u32) -> bool {
    id != 0 && id <= 0xFFFF
}

fn parse_page_elements(data: &[u8], start: usize, width: u32, height: u32) -> Vec<PageElement> {
    let mut elements = Vec::new();
    let mut image_count = 0;

    for uuid_off in find_ascii_uuid_offsets(data, start) {
        let Some(bbox) = find_object_bbox(data, uuid_off, width, height) else {
            continue;
        };

        let next_uuid = find_ascii_uuid_offsets(data, uuid_off + 36)
            .into_iter()
            .next()
            .unwrap_or(data.len());
        let record = &data[uuid_off..next_uuid];

        if let Some(text_box) = parse_text_box_record(record, bbox) {
            elements.push(PageElement::TextBox(text_box));
        } else if looks_like_image_record(record) {
            elements.push(PageElement::Image {
                bbox,
                media_index: image_count,
            });
            image_count += 1;
        }
    }

    elements
}

fn find_ascii_uuid_offsets(data: &[u8], start: usize) -> Vec<usize> {
    let mut offsets = Vec::new();
    let mut offset = start;
    while offset + 36 <= data.len() {
        if is_ascii_uuid(&data[offset..offset + 36]) {
            offsets.push(offset);
            offset += 36;
        } else {
            offset += 1;
        }
    }
    offsets
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

#[cfg(test)]
mod tests {
    use super::parse_page;
    use crate::types::{Color, PageTemplate, PageTemplateSource};

    #[test]
    fn parses_page_header_background_color() {
        let mut data = vec![0; 0xA8];
        data[0x16..0x1A].copy_from_slice(&1080_u32.to_le_bytes());
        data[0x1A..0x1E].copy_from_slice(&1527_u32.to_le_bytes());
        data[0xA4..0xA8].copy_from_slice(&[0xDD, 0xDD, 0xF5, 0xFF]);

        let page = parse_page(&data).unwrap();

        assert_eq!(
            page.background_color,
            Some(Color {
                r: 0xF5,
                g: 0xDD,
                b: 0xDD,
            })
        );
    }

    #[test]
    fn parses_page_template_id() {
        let mut data = vec![0; 0x200];
        data[0x00..0x04].copy_from_slice(&0xE7_u32.to_le_bytes());
        data[0x16..0x1A].copy_from_slice(&1080_u32.to_le_bytes());
        data[0x1A..0x1E].copy_from_slice(&1527_u32.to_le_bytes());
        data[0xA4..0xA8].copy_from_slice(&[0xDD, 0xDA, 0xCB, 0xFF]);
        data[0xAC..0xB0].copy_from_slice(&1_u32.to_le_bytes());

        let page = parse_page(&data).unwrap();

        assert_eq!(
            page.template,
            Some(PageTemplate {
                id: 1,
                source: PageTemplateSource::BuiltIn,
            })
        );
    }

    #[test]
    fn parses_short_page_template_id() {
        let mut data = vec![0; 0x200];
        data[0x00..0x04].copy_from_slice(&0x90_u32.to_le_bytes());
        data[0x16..0x1A].copy_from_slice(&1080_u32.to_le_bytes());
        data[0x1A..0x1E].copy_from_slice(&1527_u32.to_le_bytes());
        data[0x84..0x88].copy_from_slice(&[0xDD, 0xDA, 0xCB, 0xFF]);
        data[0x8C..0x90].copy_from_slice(&10_u32.to_le_bytes());

        let page = parse_page(&data).unwrap();

        assert_eq!(
            page.template,
            Some(PageTemplate {
                id: 10,
                source: PageTemplateSource::BuiltIn,
            })
        );
    }

    #[test]
    fn parses_custom_pdf_page_template() {
        let mut data = vec![0; 0x200];
        data[0x00..0x04].copy_from_slice(&0xA6_u32.to_le_bytes());
        data[0x16..0x1A].copy_from_slice(&1080_u32.to_le_bytes());
        data[0x1A..0x1E].copy_from_slice(&1528_u32.to_le_bytes());
        data[0x80..0x84].copy_from_slice(&[0xDD, 0xDA, 0xCB, 0xFF]);
        data[0x8C..0x90].copy_from_slice(&(3_u32 << 16).to_le_bytes());

        let page = parse_page(&data).unwrap();

        assert_eq!(
            page.template,
            Some(PageTemplate {
                id: 3,
                source: PageTemplateSource::CustomPdf { page_index: 3 },
            })
        );
    }

    #[test]
    fn parses_older_page_template_id_offset_as_absent_when_zero() {
        let mut data = vec![0; 0x200];
        data[0x00..0x04].copy_from_slice(&0xE3_u32.to_le_bytes());
        data[0x16..0x1A].copy_from_slice(&1080_u32.to_le_bytes());
        data[0x1A..0x1E].copy_from_slice(&1527_u32.to_le_bytes());
        data[0xA4..0xA8].copy_from_slice(&[0xFC, 0xFC, 0xFC, 0xFF]);
        data[0xAC..0xB0].copy_from_slice(&1_u32.to_le_bytes());
        data[0xB4..0xB8].copy_from_slice(&0_u32.to_le_bytes());

        let page = parse_page(&data).unwrap();

        assert_eq!(page.template, None);
    }
}

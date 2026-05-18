use crate::decode::{decode_coordinates, decode_trailing};
use crate::error::{Error, Result};
use crate::types::{BoundingBox, Page, Stroke};

const PRE_STROKE_RECORD_LEN: usize = 71;
const STROKE_HEADER_LEN: usize = 89; // bbox(32) + meta(41) + start(16)
const EXTRA_LEN_BIAS: u8 = 0x79; // byte value at record+3 when no extras are present

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

        // 1) Bounding box (32 bytes, 4 x f64)
        let bbox = BoundingBox {
            x_min: f64::from_le_bytes(data[off..off + 8].try_into().unwrap()),
            y_min: f64::from_le_bytes(data[off + 8..off + 16].try_into().unwrap()),
            x_max: f64::from_le_bytes(data[off + 16..off + 24].try_into().unwrap()),
            y_max: f64::from_le_bytes(data[off + 24..off + 32].try_into().unwrap()),
        };

        // 2) Metadata (41 + extra_len bytes): data_len @ 21, n_points @ 39
        let meta_off = off + 32 + extra_len;
        let data_len =
            u32::from_le_bytes(data[meta_off + 21..meta_off + 25].try_into().unwrap()) as usize;
        let n_points =
            u16::from_le_bytes(data[meta_off + 39..meta_off + 41].try_into().unwrap()) as usize;

        // 3) Start point (16 bytes, 2 x f64)
        let sp_off = off + 73 + extra_len;
        let start_x = f64::from_le_bytes(data[sp_off..sp_off + 8].try_into().unwrap());
        let start_y = f64::from_le_bytes(data[sp_off + 8..sp_off + 16].try_into().unwrap());

        // 4) Delta data
        let data_off = sp_off + 16;
        if data_off + data_len > data.len() {
            break;
        }
        let data_blob = &data[data_off..data_off + data_len];

        let (points, n_coord_bytes) =
            decode_coordinates(data_blob, start_x, start_y, n_points.saturating_sub(1));
        let trailing = decode_trailing(data_blob, n_coord_bytes, points.len().saturating_sub(1));

        strokes.push(Stroke {
            bbox,
            points,
            pressures: trailing.pressures,
            timestamps: trailing.timestamps,
            tilt_x: trailing.tilt_x,
            tilt_y: trailing.tilt_y,
            color: trailing.color,
            pen_width: trailing.pen_width,
        });

        // Next pre-stroke record starts immediately after this stroke's delta data.
        record_off = data_off + data_len;
    }

    Ok(Page {
        uuid,
        width,
        height,
        content_bbox,
        strokes,
    })
}

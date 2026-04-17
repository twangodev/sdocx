use crate::decode::{decode_coordinates, decode_trailing};
use crate::error::{Error, Result};
use crate::types::{BoundingBox, Page, Stroke};

/// Parse a `.page` binary file into a `Page`.
///
/// Layout: `base = u32 @ 0x00`; stroke count at `base + 0x66`; records at `base + 0xB5`.
///
/// Newer Samsung Notes (v4.4.x+) may inject per-stroke extra attribute blocks.
/// The block length for the next stroke is `byte - 0x79` at `base + 0x71` (first stroke)
/// or byte 3 of the 71-byte inter-stroke record (subsequent strokes).
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

    // extra_len for the first stroke lives in the pre-stroke record at base + 0x71.
    let mut extra_len: usize = if base + 0x71 < data.len() {
        (data[base + 0x71] as usize).saturating_sub(0x79)
    } else {
        0
    };

    let mut strokes = Vec::with_capacity(stroke_count);
    let mut off = base + 0xB5;

    for _ in 0..stroke_count {
        if off + 89 + extra_len > data.len() {
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

        // 4) Delta data — n_points lets the decoder tolerate non-standard flag bits.
        let data_off = sp_off + 16;
        if data_off + data_len > data.len() {
            break;
        }
        let data_blob = &data[data_off..data_off + data_len];

        let n_deltas = n_points.saturating_sub(1);
        let (points, n_coord_bytes) = decode_coordinates(data_blob, start_x, start_y, Some(n_deltas));
        let n_delta_points = points.len().saturating_sub(1);

        let trailing = decode_trailing(data_blob, n_coord_bytes, n_delta_points);

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

        // 5) Advance past deltas + 71-byte inter-stroke record; byte 3 carries next extra_len.
        let inter_off = data_off + data_len;
        if inter_off + 71 <= data.len() {
            extra_len = (data[inter_off + 3] as usize).saturating_sub(0x79);
        } else {
            extra_len = 0;
        }
        off = inter_off + 71;
    }

    Ok(Page {
        uuid,
        width,
        height,
        content_bbox,
        strokes,
    })
}

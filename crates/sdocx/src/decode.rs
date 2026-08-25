const POINT_DELTA_SCALE: f64 = 1.0 / 32.0;
const FLOAT_DELTA_SCALE: f64 = 1.0 / 4096.0;
/// Color marker: leading byte is a format version (0x02 on older Samsung Notes,
/// 0x03 on v4.4.x+) followed by this fixed 5-byte tail.
const COLOR_MARKER_TAIL: &[u8] = &[0x00, 0x01, 0x00, 0x00, 0x00];
const COLOR_MARKER_LEN: usize = COLOR_MARKER_TAIL.len() + 1;

use crate::types::{Color, Point};

/// Decode Samsung's signed-magnitude Q10.5 point delta.
///
/// Bit 15 is the sign and bits 0..=14 hold the magnitude. This matches the
/// `ObjectStrokeBinaryHandler::sm_RestoreStroke` implementation in Samsung's
/// S Pen SDK. Treating the high byte as a sign flag loses seven magnitude bits.
fn decode_point_delta(raw: u16) -> f64 {
    let magnitude = f64::from(raw & 0x7fff) * POINT_DELTA_SCALE;
    if raw & 0x8000 == 0 {
        magnitude
    } else {
        -magnitude
    }
}

/// Decode Samsung's signed-magnitude Q3.12 floating-point delta.
///
/// This representation is used by pressure, tilt, and orientation channels.
fn decode_float_delta(raw: u16) -> f64 {
    let magnitude = f64::from(raw & 0x7fff) * FLOAT_DELTA_SCALE;
    if raw & 0x8000 == 0 {
        magnitude
    } else {
        -magnitude
    }
}

/// Decode delta-encoded coordinates. Each quartet contains little-endian Q10.5
/// signed-magnitude words for `dx` and `dy`.
///
/// Reads exactly `n_deltas` quartets, producing `n_deltas + 1` points (including the start).
/// Stops early if `data` runs out. Returns `(points, n_coord_bytes)`.
pub fn decode_coordinates(
    data: &[u8],
    start_x: f64,
    start_y: f64,
    n_deltas: usize,
) -> (Vec<Point>, usize) {
    let mut x = start_x;
    let mut y = start_y;
    let mut points = vec![Point { x, y }];
    let limit = n_deltas.saturating_mul(4);
    let mut i = 0;

    while i + 3 < data.len() && i < limit {
        let dx = decode_point_delta(u16::from_le_bytes([data[i], data[i + 1]]));
        let dy = decode_point_delta(u16::from_le_bytes([data[i + 2], data[i + 3]]));

        x += dx;
        y += dy;
        points.push(Point { x, y });
        i += 4;
    }

    (points, i)
}

/// Decoded trailing channel data from a stroke's data blob.
pub struct TrailingData {
    pub pressures: Vec<f64>,
    pub timestamps: Vec<i64>,
    pub tilts: Vec<f64>,
    pub orientations: Vec<f64>,
    pub color: Option<Color>,
    pub pen_width: f32,
}

/// Decode the compressed per-point channels following the coordinate deltas.
///
/// Pressure, timestamp, tilt, and orientation each store their first value in
/// full (four bytes), followed by `n_points - 1` two-byte deltas. Tilt and
/// orientation are optional as a pair and are decoded only when the containing
/// stroke property flags explicitly declare them. The returned vectors
/// therefore line up one-for-one with `Stroke::points`.
pub fn decode_trailing(
    data_blob: &[u8],
    n_coord_bytes: usize,
    n_points: usize,
    has_stylus_channels: bool,
) -> Option<TrailingData> {
    if n_points == 0 {
        return None;
    }

    let delta_count = n_points - 1;
    let mut cursor = n_coord_bytes;

    let pressures = decode_float_channel(data_blob, &mut cursor, delta_count)?;
    let timestamps = decode_timestamp_channel(data_blob, &mut cursor, delta_count)?;

    let (tilts, orientations) = if has_stylus_channels {
        let tilts = decode_float_channel(data_blob, &mut cursor, delta_count)?;
        let orientations = decode_float_channel(data_blob, &mut cursor, delta_count)?;
        (tilts, orientations)
    } else {
        (Vec::new(), Vec::new())
    };

    // Extract color and pen width from the color marker
    let (color, pen_width) = extract_color_and_width(data_blob);

    Some(TrailingData {
        pressures,
        timestamps,
        tilts,
        orientations,
        color,
        pen_width,
    })
}

fn decode_float_channel(data: &[u8], cursor: &mut usize, delta_count: usize) -> Option<Vec<f64>> {
    let first = read_f32(data, *cursor)? as f64;
    *cursor = cursor.checked_add(4)?;

    let byte_len = delta_count.checked_mul(2)?;
    let end = cursor.checked_add(byte_len)?;
    let deltas = data.get(*cursor..end)?;
    *cursor = end;

    let mut value = first;
    let mut values = Vec::with_capacity(delta_count + 1);
    values.push(value);
    for raw in deltas.as_chunks::<2>().0 {
        value += decode_float_delta(u16::from_le_bytes([raw[0], raw[1]]));
        values.push(value);
    }
    Some(values)
}

fn decode_timestamp_channel(
    data: &[u8],
    cursor: &mut usize,
    delta_count: usize,
) -> Option<Vec<i64>> {
    let mut value = i64::from(read_i32(data, *cursor)?);
    *cursor = cursor.checked_add(4)?;

    let byte_len = delta_count.checked_mul(2)?;
    let end = cursor.checked_add(byte_len)?;
    let deltas = data.get(*cursor..end)?;
    *cursor = end;

    let mut values = Vec::with_capacity(delta_count + 1);
    values.push(value);
    for raw in deltas.as_chunks::<2>().0 {
        // The native reader zero-extends the stored 16-bit timestamp delta.
        value += i64::from(u16::from_le_bytes([raw[0], raw[1]]));
        values.push(value);
    }
    Some(values)
}

fn read_f32(data: &[u8], offset: usize) -> Option<f32> {
    let end = offset.checked_add(4)?;
    Some(f32::from_le_bytes(data.get(offset..end)?.try_into().ok()?))
}

fn read_i32(data: &[u8], offset: usize) -> Option<i32> {
    let end = offset.checked_add(4)?;
    Some(i32::from_le_bytes(data.get(offset..end)?.try_into().ok()?))
}

fn extract_color_and_width(data_blob: &[u8]) -> (Option<Color>, f32) {
    let mut color = None;
    let mut width: f32 = 0.8;

    // Find the last color marker: [VV, 00, 01, 00, 00, 00] with VV in {0x02, 0x03}.
    let pos = data_blob
        .windows(COLOR_MARKER_LEN)
        .rposition(|w| matches!(w[0], 0x02 | 0x03) && &w[1..] == COLOR_MARKER_TAIL);

    if let Some(pos) = pos {
        let after = &data_blob[pos + COLOR_MARKER_LEN..];
        if after.len() >= 4 && after[3] == 0xFF {
            // BGRA color present
            color = Some(Color {
                r: after[2],
                g: after[1],
                b: after[0],
            });
            if after.len() >= 8 {
                width = f32::from_le_bytes([after[4], after[5], after[6], after[7]]);
            }
        } else if after.len() >= 4 {
            width = f32::from_le_bytes([after[0], after[1], after[2], after[3]]);
        }
    }

    (color, width)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_coordinates_simple() {
        // Two deltas: (+32/32, +64/32) then (+0, -32/32)
        let data = [
            32, 0x00, 64, 0x00, // dx=+1.0, dy=+2.0
            0, 0x00, 32, 0x80, // dx=+0.0, dy=-1.0
        ];
        let (points, n_bytes) = decode_coordinates(&data, 10.0, 20.0, 2);
        assert_eq!(points.len(), 3);
        assert!((points[0].x - 10.0).abs() < 1e-10);
        assert!((points[0].y - 20.0).abs() < 1e-10);
        assert!((points[1].x - 11.0).abs() < 1e-10);
        assert!((points[1].y - 22.0).abs() < 1e-10);
        assert!((points[2].x - 11.0).abs() < 1e-10);
        assert!((points[2].y - 21.0).abs() < 1e-10);
        assert_eq!(n_bytes, 8);
    }

    #[test]
    fn test_decode_coordinates_negative() {
        let data = [
            64, 0x80, 32, 0x80, // dx=-2.0, dy=-1.0
        ];
        let (points, n_bytes) = decode_coordinates(&data, 5.0, 5.0, 1);
        assert_eq!(points.len(), 2);
        assert!((points[1].x - 3.0).abs() < 1e-10);
        assert!((points[1].y - 4.0).abs() < 1e-10);
        assert_eq!(n_bytes, 4);
    }

    #[test]
    fn test_decode_coordinates_preserves_high_magnitude_bits() {
        // +300.5 = 0x2590 / 32; -2.25 = -(0x0048 / 32).
        let data = [0x90, 0x25, 0x48, 0x80];
        let (points, n_bytes) = decode_coordinates(&data, 0.0, 0.0, 1);
        assert_eq!(points.len(), 2);
        assert!((points[1].x - 300.5).abs() < 1e-10);
        assert!((points[1].y + 2.25).abs() < 1e-10);
        assert_eq!(n_bytes, 4);
    }

    #[test]
    fn test_extract_color_v4_4_marker() {
        // v4.4.x uses 0x03 as the color-marker version byte.
        let mut data = vec![0u8; 20];
        let marker_pos = 4;
        data[marker_pos..marker_pos + 6].copy_from_slice(&[0x03, 0x00, 0x01, 0x00, 0x00, 0x00]);
        data[marker_pos + 6] = 0x14; // B
        data[marker_pos + 7] = 0xA1; // G
        data[marker_pos + 8] = 0x47; // R
        data[marker_pos + 9] = 0xFF; // A
        let (color, _) = extract_color_and_width(&data);
        assert_eq!(
            color,
            Some(Color {
                r: 0x47,
                g: 0xA1,
                b: 0x14
            })
        );
    }

    #[test]
    fn test_extract_color_rejects_unknown_version() {
        // Leading byte must be 0x02 or 0x03; 0x01 is not a known version.
        let mut data = vec![0u8; 20];
        data[4..10].copy_from_slice(&[0x01, 0x00, 0x01, 0x00, 0x00, 0x00]);
        data[10] = 0x14;
        data[11] = 0xA1;
        data[12] = 0x47;
        data[13] = 0xFF;
        let (color, _) = extract_color_and_width(&data);
        assert_eq!(color, None);
    }

    #[test]
    fn test_extract_color_with_bgra() {
        // Marker + BGRA (B=0x14, G=0xA1, R=0x47, A=0xFF) + pen width
        let mut data = vec![0u8; 20];
        let marker_pos = 4;
        data[marker_pos..marker_pos + 6].copy_from_slice(&[0x02, 0x00, 0x01, 0x00, 0x00, 0x00]);
        data[marker_pos + 6] = 0x14; // B
        data[marker_pos + 7] = 0xA1; // G
        data[marker_pos + 8] = 0x47; // R
        data[marker_pos + 9] = 0xFF; // A
        let width_bytes = 5.54_f32.to_le_bytes();
        data[marker_pos + 10..marker_pos + 14].copy_from_slice(&width_bytes);

        let (color, width) = extract_color_and_width(&data);
        assert_eq!(
            color,
            Some(Color {
                r: 0x47,
                g: 0xA1,
                b: 0x14
            })
        );
        assert!((width - 5.54).abs() < 0.01);
    }

    #[test]
    fn test_extract_color_default() {
        // Marker + pen width only (no 0xFF at byte 3)
        let mut data = vec![0u8; 16];
        let marker_pos = 2;
        data[marker_pos..marker_pos + 6].copy_from_slice(&[0x02, 0x00, 0x01, 0x00, 0x00, 0x00]);
        let width_bytes = 9.12_f32.to_le_bytes();
        data[marker_pos + 6..marker_pos + 10].copy_from_slice(&width_bytes);

        let (color, width) = extract_color_and_width(&data);
        assert_eq!(color, None);
        assert!((width - 9.12).abs() < 0.01);
    }

    #[test]
    fn test_decode_trailing_channels() {
        let n_coord_bytes = 4;
        let n_points = 3;

        let mut blob = vec![0u8; n_coord_bytes];
        blob.extend_from_slice(&0.25_f32.to_le_bytes());
        blob.extend_from_slice(&0x0200_u16.to_le_bytes()); // +0.125
        blob.extend_from_slice(&0x8100_u16.to_le_bytes()); // -0.0625
        blob.extend_from_slice(&1000_i32.to_le_bytes());
        blob.extend_from_slice(&5_u16.to_le_bytes());
        blob.extend_from_slice(&7_u16.to_le_bytes());

        let result = decode_trailing(&blob, n_coord_bytes, n_points, false).unwrap();
        assert_eq!(result.pressures.len(), 3);
        assert_eq!(result.pressures, vec![0.25, 0.375, 0.3125]);
        assert_eq!(result.timestamps, vec![1000, 1005, 1012]);
        assert!(result.tilts.is_empty());
        assert!(result.orientations.is_empty());
    }

    #[test]
    fn test_decode_optional_tilt_and_orientation_channels() {
        let n_coord_bytes = 0;
        let n_points = 2;
        let mut blob = Vec::new();
        blob.extend_from_slice(&0.5_f32.to_le_bytes());
        blob.extend_from_slice(&0_u16.to_le_bytes());
        blob.extend_from_slice(&42_i32.to_le_bytes());
        blob.extend_from_slice(&3_u16.to_le_bytes());
        blob.extend_from_slice(&0.25_f32.to_le_bytes());
        blob.extend_from_slice(&0x0400_u16.to_le_bytes()); // +0.25
        blob.extend_from_slice(&(-1.0_f32).to_le_bytes());
        blob.extend_from_slice(&0x8200_u16.to_le_bytes()); // -0.125

        let result = decode_trailing(&blob, n_coord_bytes, n_points, true).unwrap();
        assert_eq!(result.tilts, vec![0.25, 0.5]);
        assert_eq!(result.orientations, vec![-1.0, -1.125]);
    }
}

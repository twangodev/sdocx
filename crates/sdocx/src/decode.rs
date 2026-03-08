const DELTA_SCALE: f64 = 1.0 / 32.0;
const MAX_PRESSURE: f64 = 1400.0;
const COLOR_MARKER: &[u8] = &[0x02, 0x00, 0x01, 0x00, 0x00, 0x00];

use crate::types::{Color, Point};

/// Decode sign-magnitude byte pairs: (magnitude, sign_flag).
/// `0x00` = positive, `0x80` = negative.
pub fn decode_sign_mag(data: &[u8], offset: usize, count: usize) -> Vec<i64> {
    let mut vals = Vec::with_capacity(count);
    for i in 0..count {
        let pos = offset + i * 2;
        if pos + 1 >= data.len() {
            break;
        }
        let mag = data[pos] as i64;
        let sign = data[pos + 1];
        vals.push(if sign == 0x00 { mag } else { -mag });
    }
    vals
}

/// Decode delta-encoded coordinates from a data blob.
/// Returns `(points, n_coord_bytes)` where `n_coord_bytes` is how many bytes
/// were consumed by coordinate data.
pub fn decode_coordinates(data: &[u8], start_x: f64, start_y: f64) -> (Vec<Point>, usize) {
    let mut x = start_x;
    let mut y = start_y;
    let mut points = vec![Point { x, y }];
    let mut i = 0;

    while i + 3 < data.len() {
        let dx_mag = data[i];
        let dx_sign = data[i + 1];
        let dy_mag = data[i + 2];
        let dy_sign = data[i + 3];

        if (dx_sign != 0x00 && dx_sign != 0x80) || (dy_sign != 0x00 && dy_sign != 0x80) {
            break;
        }

        let dx = if dx_sign == 0x00 {
            dx_mag as f64
        } else {
            -(dx_mag as f64)
        } * DELTA_SCALE;
        let dy = if dy_sign == 0x00 {
            dy_mag as f64
        } else {
            -(dy_mag as f64)
        } * DELTA_SCALE;

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
    pub tilt_x: Vec<i64>,
    pub tilt_y: Vec<i64>,
    pub color: Option<Color>,
    pub pen_width: f32,
}

/// Decode all trailing data: per-point channels (pressure, timestamp, tilt_x, tilt_y)
/// and per-stroke color + pen width.
pub fn decode_trailing(data_blob: &[u8], n_coord_bytes: usize, n_points: usize) -> TrailingData {
    let trail_start = n_coord_bytes + 4; // 4-byte gap after coordinates

    // Decode 4 per-point channels
    let mut channels: [Vec<i64>; 4] = Default::default();
    for (ch_idx, channel) in channels.iter_mut().enumerate() {
        let ch_offset = trail_start + ch_idx * n_points * 2;
        if ch_offset + n_points * 2 > data_blob.len() {
            continue;
        }
        let deltas = decode_sign_mag(data_blob, ch_offset, n_points);
        let mut cumsum: i64 = 0;
        let mut values = Vec::with_capacity(deltas.len());
        for d in deltas {
            cumsum += d;
            values.push(cumsum);
        }
        *channel = values;
    }

    // Normalize pressure to 0.0..1.0
    let pressures: Vec<f64> = channels[0]
        .iter()
        .map(|&v| (v as f64 / MAX_PRESSURE).clamp(0.0, 1.0))
        .collect();

    let timestamps = std::mem::take(&mut channels[1]);
    let tilt_x = std::mem::take(&mut channels[2]);
    let tilt_y = std::mem::take(&mut channels[3]);

    // Extract color and pen width from the color marker
    let (color, pen_width) = extract_color_and_width(data_blob);

    TrailingData {
        pressures,
        timestamps,
        tilt_x,
        tilt_y,
        color,
        pen_width,
    }
}

fn extract_color_and_width(data_blob: &[u8]) -> (Option<Color>, f32) {
    let mut color = None;
    let mut width: f32 = 0.8;

    // Find the last occurrence of the color marker
    let pos = data_blob
        .windows(COLOR_MARKER.len())
        .rposition(|w| w == COLOR_MARKER);

    if let Some(pos) = pos {
        let after = &data_blob[pos + COLOR_MARKER.len()..];
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
    fn test_decode_sign_mag_positive() {
        let data = [42, 0x00, 10, 0x00];
        let vals = decode_sign_mag(&data, 0, 2);
        assert_eq!(vals, vec![42, 10]);
    }

    #[test]
    fn test_decode_sign_mag_negative() {
        let data = [42, 0x80, 10, 0x80];
        let vals = decode_sign_mag(&data, 0, 2);
        assert_eq!(vals, vec![-42, -10]);
    }

    #[test]
    fn test_decode_sign_mag_mixed() {
        let data = [5, 0x00, 3, 0x80, 7, 0x00];
        let vals = decode_sign_mag(&data, 0, 3);
        assert_eq!(vals, vec![5, -3, 7]);
    }

    #[test]
    fn test_decode_sign_mag_with_offset() {
        let data = [0xFF, 0xFF, 5, 0x00, 3, 0x80];
        let vals = decode_sign_mag(&data, 2, 2);
        assert_eq!(vals, vec![5, -3]);
    }

    #[test]
    fn test_decode_coordinates_simple() {
        // Two deltas: (+32/32, +64/32) then (+0, -32/32)
        let data = [
            32, 0x00, 64, 0x00, // dx=+1.0, dy=+2.0
            0, 0x00, 32, 0x80, // dx=+0.0, dy=-1.0
            0xFF, 0xFF, // terminator (invalid sign)
        ];
        let (points, n_bytes) = decode_coordinates(&data, 10.0, 20.0);
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
        let (points, n_bytes) = decode_coordinates(&data, 5.0, 5.0);
        assert_eq!(points.len(), 2);
        assert!((points[1].x - 3.0).abs() < 1e-10);
        assert!((points[1].y - 4.0).abs() < 1e-10);
        assert_eq!(n_bytes, 4);
    }

    #[test]
    fn test_extract_color_with_bgra() {
        // Marker + BGRA (B=0x14, G=0xA1, R=0x47, A=0xFF) + pen width
        let mut data = vec![0u8; 20];
        let marker_pos = 4;
        data[marker_pos..marker_pos + 6].copy_from_slice(COLOR_MARKER);
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
        data[marker_pos..marker_pos + 6].copy_from_slice(COLOR_MARKER);
        let width_bytes = 9.12_f32.to_le_bytes();
        data[marker_pos + 6..marker_pos + 10].copy_from_slice(&width_bytes);

        let (color, width) = extract_color_and_width(&data);
        assert_eq!(color, None);
        assert!((width - 9.12).abs() < 0.01);
    }

    #[test]
    fn test_decode_trailing_pressure() {
        // Build a minimal data blob: 4 bytes of coord data + 4-byte gap + pressure deltas
        let n_coord_bytes = 4;
        let n_points = 3;

        let mut blob = vec![0u8; 100];
        // Pressure deltas at offset 8 (n_coord_bytes + 4): [100, +], [50, +], [20, -]
        let trail_start = n_coord_bytes + 4;
        blob[trail_start] = 100;
        blob[trail_start + 1] = 0x00; // +100
        blob[trail_start + 2] = 50;
        blob[trail_start + 3] = 0x00; // +50
        blob[trail_start + 4] = 20;
        blob[trail_start + 5] = 0x80; // -20

        let result = decode_trailing(&blob, n_coord_bytes, n_points);
        assert_eq!(result.pressures.len(), 3);
        // cumsum: 100, 150, 130 -> normalized: 100/1400, 150/1400, 130/1400
        assert!((result.pressures[0] - 100.0 / 1400.0).abs() < 1e-10);
        assert!((result.pressures[1] - 150.0 / 1400.0).abs() < 1e-10);
        assert!((result.pressures[2] - 130.0 / 1400.0).abs() < 1e-10);
    }
}

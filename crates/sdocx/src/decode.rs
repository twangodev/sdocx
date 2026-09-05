use crate::binary::Reader;
use crate::frame::Frame;
use crate::{Color, Error, ObjectMetadata, ParseLimits, Point, Result, Stroke, StrokeStyle};

/// Decode a type-1 outer object's base/stroke frame chain. The caller supplies
/// only its payload, excluding the object hash and recursive child records.
pub(crate) fn decode_stroke(data: &[u8], limits: &ParseLimits) -> Result<Stroke> {
    let mut frames = Reader::new(data, "stroke object");
    let base = ObjectMetadata::read(&mut frames)?;

    let frame = Frame::read(&mut frames)?;
    frame.expect_kind(1)?;
    let channels = StrokeChannels::read(&frame, limits)?;
    let count = usize::from(channels.point_count);
    let compressed = channels.compressed;
    let stylus = channels.stylus;
    let mut fixed = Reader::new(channels.data, "stroke channels");

    let mut points = Vec::with_capacity(count);
    if count != 0 && compressed {
        let mut x = fixed.read_f64("first x")?;
        let mut y = fixed.read_f64("first y")?;
        points.push(Point { x, y });
        for _ in 1..count {
            x += signed_delta(fixed.read_u16("x delta")?, 32.0);
            y += signed_delta(fixed.read_u16("y delta")?, 32.0);
            points.push(Point { x, y });
        }
    } else {
        for _ in 0..count {
            points.push(Point {
                x: fixed.read_f64("x")?,
                y: fixed.read_f64("y")?,
            });
        }
    }
    if points
        .iter()
        .any(|point| !point.x.is_finite() || !point.y.is_finite())
    {
        return Err(Error::Format(
            "stroke contains non-finite coordinates".into(),
        ));
    }
    // Both native representations store complete arrays by channel, including
    // the uncompressed form (they are not interleaved point structs).
    let pressures = float_channel(&mut fixed, count, compressed)?;
    let mut timestamps = Vec::with_capacity(count);
    if count != 0 {
        let mut timestamp = i64::from(fixed.read_i32("first timestamp")?);
        timestamps.push(timestamp);
        for _ in 1..count {
            timestamp = if compressed {
                timestamp + i64::from(fixed.read_u16("timestamp delta")?)
            } else {
                i64::from(fixed.read_i32("timestamp")?)
            };
            timestamps.push(timestamp);
        }
    }
    let (tilts, orientations) = if stylus {
        (
            float_channel(&mut fixed, count, compressed)?,
            float_channel(&mut fixed, count, compressed)?,
        )
    } else {
        (Vec::new(), Vec::new())
    };
    let (style, _) = StrokeStyle::read_prefix(&frame)?;
    // Future frame extensions are bounded too; no frame may borrow its bytes
    // from the object hash or the next sibling.
    while frames.remaining() != 0 {
        Frame::read(&mut frames)?;
    }
    Ok(Stroke {
        bbox: base.bbox,
        points,
        pressures,
        timestamps,
        tilts,
        orientations,
        color: style.color_argb.map(|argb| Color {
            r: (argb >> 16) as u8,
            g: (argb >> 8) as u8,
            b: argb as u8,
        }),
        pen_width: style.pen_size.unwrap_or(0.8),
    })
}

fn signed_delta(raw: u16, divisor: f64) -> f64 {
    let magnitude = f64::from(raw & 0x7fff) / divisor;
    if raw & 0x8000 == 0 {
        magnitude
    } else {
        -magnitude
    }
}

fn float_channel(reader: &mut Reader<'_>, count: usize, compressed: bool) -> Result<Vec<f64>> {
    let mut values = Vec::with_capacity(count);
    if count != 0 {
        let mut value = f64::from(reader.read_f32("first channel value")?);
        values.push(value);
        for _ in 1..count {
            value = if compressed {
                value + signed_delta(reader.read_u16("channel delta")?, 4096.0)
            } else {
                f64::from(reader.read_f32("channel value")?)
            };
            values.push(value);
        }
    }
    if values.iter().any(|value| !value.is_finite()) {
        return Err(Error::Format(
            "stroke contains non-finite channel values".into(),
        ));
    }
    Ok(values)
}

pub(crate) struct StrokeChannels<'a> {
    pub(crate) point_count: u16,
    pub(crate) compressed: bool,
    pub(crate) stylus: bool,
    pub(crate) tool_type_raw: u16,
    pub(crate) data: &'a [u8],
}

impl<'a> StrokeChannels<'a> {
    pub(crate) fn read(frame: &Frame<'a>, limits: &ParseLimits) -> Result<Self> {
        let mut fixed = Reader::new(frame.fixed, "stroke channels");
        let point_count = fixed.read_u16("point count")?;
        let count = usize::from(point_count);
        if count > limits.max_points_per_stroke {
            return Err(Error::LimitExceeded {
                resource: "points per stroke",
                limit: limits.max_points_per_stroke as u64,
                actual: count as u64,
            });
        }
        let compressed = frame.properties.contains(0);
        let stylus = frame.properties.contains(2);
        let channel_bytes = if count == 0 {
            0
        } else if compressed {
            16 + (count - 1) * 4 + (2 + usize::from(stylus) * 2) * (4 + (count - 1) * 2)
        } else {
            count * (24 + usize::from(stylus) * 8)
        };
        if fixed.remaining() != channel_bytes + 2 {
            return Err(Error::Format(format!(
                "stroke channels: {count} points require {} fixed bytes, found {}",
                channel_bytes + 2,
                fixed.remaining()
            )));
        }
        let data = fixed.read_bytes(channel_bytes, "point channels")?;
        let tool_type_raw = fixed.read_u16("tool/input type")?;
        Ok(Self {
            point_count,
            compressed,
            stylus,
            tool_type_raw,
            data,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::signed_delta;

    #[test]
    fn packed_deltas_preserve_sign_and_high_magnitude_bits() {
        assert_eq!(signed_delta(0x2590, 32.0), 300.5);
        assert_eq!(signed_delta(0x8048, 32.0), -2.25);
        assert_eq!(signed_delta(0x8200, 4096.0), -0.125);
    }
}

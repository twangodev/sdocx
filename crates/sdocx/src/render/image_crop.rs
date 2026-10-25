//! Geometry for ordinary image source-rectangle crops.
//!
//! Native image drawing sends the crop rectangle as a bitmap source rectangle,
//! then stretches that selected rectangle over the image's existing bounds.
//! This helper computes the equivalent full-source placement for a viewport
//! clipped to those bounds. A separate narrow PNG path can copy exact RGB8 or
//! RGBA8 crop pixels when the image has no metadata that re-encoding could
//! discard.

use crate::BoundingBox;
use std::io::Cursor;

const MAX_CROP_DECODE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
struct CropPngMetadata {
    significant_bits: Option<Vec<u8>>,
    srgb_intent: Option<png::SrgbRenderingIntent>,
}

/// Return an exact RGB8 or RGBA8 PNG source crop when the only ancillary
/// metadata is supported sBIT/sRGB data and stored pixels need no conversion.
///
/// The cropped image is intended to be placed directly in the existing
/// destination rectangle. Unsupported encodings or metadata return `None` so
/// callers can retain the original source image and crop geometry.
pub(crate) fn crop_rgb8_or_rgba8_png(source_bytes: &[u8], crop: [i32; 4]) -> Option<Vec<u8>> {
    let metadata = supported_crop_png_metadata(source_bytes)?;

    let mut decoder = png::Decoder::new(Cursor::new(source_bytes));
    decoder.set_limits(png::Limits {
        bytes: MAX_CROP_DECODE_BYTES,
    });
    let mut reader = decoder.read_info().ok()?;
    let info = reader.info();
    if info.sbit.as_deref() != metadata.significant_bits.as_deref()
        || info.srgb != metadata.srgb_intent
    {
        return None;
    }
    let (color_type, bytes_per_pixel) = match info.color_type {
        png::ColorType::Rgb => (png::ColorType::Rgb, 3),
        png::ColorType::Rgba => (png::ColorType::Rgba, 4),
        _ => return None,
    };
    if info.bit_depth != png::BitDepth::Eight {
        return None;
    }

    let width = usize::try_from(info.width).ok()?;
    let height = usize::try_from(info.height).ok()?;
    let source_len = width.checked_mul(height)?.checked_mul(bytes_per_pixel)?;
    if width == 0 || height == 0 || source_len > MAX_CROP_DECODE_BYTES {
        return None;
    }

    let [left, top, right, bottom] = crop;
    if left < 0 || top < 0 || right <= left || bottom <= top {
        return None;
    }
    let (left, top, right, bottom) = (
        usize::try_from(left).ok()?,
        usize::try_from(top).ok()?,
        usize::try_from(right).ok()?,
        usize::try_from(bottom).ok()?,
    );
    if right > width
        || bottom > height
        || (left == 0 && top == 0 && right == width && bottom == height)
    {
        return None;
    }

    let output_len = reader.output_buffer_size()?;
    if output_len != source_len || output_len > MAX_CROP_DECODE_BYTES {
        return None;
    }
    let mut decoded = vec![0; output_len];
    let frame = reader.next_frame(&mut decoded).ok()?;
    if frame.width as usize != width
        || frame.height as usize != height
        || frame.color_type != color_type
        || frame.bit_depth != png::BitDepth::Eight
        || frame.buffer_size() != source_len
    {
        return None;
    }
    reader.finish().ok()?;

    let crop_width = right.checked_sub(left)?;
    let crop_height = bottom.checked_sub(top)?;
    let crop_len = crop_width
        .checked_mul(crop_height)?
        .checked_mul(bytes_per_pixel)?;
    if crop_len > MAX_CROP_DECODE_BYTES {
        return None;
    }
    let mut pixels = Vec::with_capacity(crop_len);
    for y in top..bottom {
        let row_start = y
            .checked_mul(width)?
            .checked_add(left)?
            .checked_mul(bytes_per_pixel)?;
        let row_end = row_start.checked_add(crop_width.checked_mul(bytes_per_pixel)?)?;
        pixels.extend_from_slice(decoded.get(row_start..row_end)?);
    }
    if pixels.len() != crop_len {
        return None;
    }

    let crop_width = u32::try_from(crop_width).ok()?;
    let crop_height = u32::try_from(crop_height).ok()?;
    let mut encoded = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut encoded, crop_width, crop_height);
        encoder.set_color(color_type);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        if let Some(significant_bits) = &metadata.significant_bits {
            writer
                .write_chunk(png::chunk::sBIT, significant_bits)
                .ok()?;
        }
        if let Some(intent) = metadata.srgb_intent {
            writer.write_chunk(png::chunk::sRGB, &[intent as u8]).ok()?;
        }
        writer.write_image_data(&pixels).ok()?;
    }
    Some(encoded)
}

/// Preserve only the bit-depth and standard sRGB declarations supported by
/// the crop encoder. Other ancillary chunks stay on the existing source-image
/// path so re-encoding cannot discard unmodeled metadata.
fn supported_crop_png_metadata(bytes: &[u8]) -> Option<CropPngMetadata> {
    const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if !bytes.starts_with(SIGNATURE) {
        return None;
    }

    let mut offset = SIGNATURE.len();
    let mut saw_header = false;
    let mut sample_count = 0;
    let mut saw_sbit = false;
    let mut significant_bits = None;
    let mut saw_srgb = false;
    let mut srgb_intent = None;
    let mut saw_data = false;
    while offset.checked_add(12).is_some_and(|end| end <= bytes.len()) {
        let length = usize::try_from(u32::from_be_bytes(
            bytes[offset..offset + 4].try_into().ok()?,
        ))
        .ok()?;
        let kind: [u8; 4] = bytes[offset + 4..offset + 8].try_into().ok()?;
        let end = offset.checked_add(12)?.checked_add(length)?;
        if end > bytes.len() {
            return None;
        }
        let data = bytes.get(offset + 8..offset + 8 + length)?;

        match &kind {
            b"IHDR" if !saw_header && !saw_data && offset == SIGNATURE.len() && length == 13 => {
                if data[8] != 8 {
                    return None;
                }
                sample_count = match data[9] {
                    2 => 3,
                    6 => 4,
                    _ => return None,
                };
                saw_header = true;
            }
            b"sBIT" if saw_header && !saw_sbit && !saw_srgb && !saw_data => {
                if data.len() != sample_count || data.iter().any(|bits| !(1..=8).contains(bits)) {
                    return None;
                }
                significant_bits = Some(data.to_vec());
                saw_sbit = true;
            }
            b"sRGB" if saw_header && !saw_srgb && !saw_data && length == 1 => {
                srgb_intent = Some(match data[0] {
                    0 => png::SrgbRenderingIntent::Perceptual,
                    1 => png::SrgbRenderingIntent::RelativeColorimetric,
                    2 => png::SrgbRenderingIntent::Saturation,
                    3 => png::SrgbRenderingIntent::AbsoluteColorimetric,
                    _ => return None,
                });
                saw_srgb = true;
            }
            b"IDAT" if saw_header => saw_data = true,
            b"IEND" if saw_header && saw_data && length == 0 => {
                return (end == bytes.len()).then_some(CropPngMetadata {
                    significant_bits,
                    srgb_intent,
                });
            }
            _ => return None,
        }
        offset = end;
    }
    None
}

/// Return the full bitmap placement that maps an in-bounds source crop onto
/// `displayed_bbox`.
///
/// `None` means the encoded image dimensions could not be read, the crop is
/// empty/inverted/outside the source, or the displayed geometry is invalid.
pub(crate) fn crop_only_placement(
    source_bytes: &[u8],
    crop_rect: [i32; 4],
    displayed_bbox: BoundingBox,
) -> Option<BoundingBox> {
    let source = imagesize::blob_size(source_bytes).ok()?;
    source_crop_placement(source.width, source.height, crop_rect, displayed_bbox)
}

fn source_crop_placement(
    source_width: usize,
    source_height: usize,
    [left, top, right, bottom]: [i32; 4],
    displayed: BoundingBox,
) -> Option<BoundingBox> {
    if source_width == 0
        || source_height == 0
        || left < 0
        || top < 0
        || right <= left
        || bottom <= top
        || right as usize > source_width
        || bottom as usize > source_height
        || !valid_bbox(displayed)
    {
        return None;
    }

    let crop_width = f64::from(right - left);
    let crop_height = f64::from(bottom - top);
    let scale_x = (displayed.x_max - displayed.x_min) / crop_width;
    let scale_y = (displayed.y_max - displayed.y_min) / crop_height;
    let source_width = source_width as f64;
    let source_height = source_height as f64;
    let x_min = displayed.x_min - f64::from(left) * scale_x;
    let y_min = displayed.y_min - f64::from(top) * scale_y;
    let placed = BoundingBox {
        x_min,
        y_min,
        x_max: x_min + source_width * scale_x,
        y_max: y_min + source_height * scale_y,
    };
    valid_bbox(placed).then_some(placed)
}

fn valid_bbox(bbox: BoundingBox) -> bool {
    bbox.x_min.is_finite()
        && bbox.y_min.is_finite()
        && bbox.x_max.is_finite()
        && bbox.y_max.is_finite()
        && bbox.x_max > bbox.x_min
        && bbox.y_max > bbox.y_min
}

#[cfg(test)]
mod tests {
    use super::*;

    // 4×3 RGBA PNG with red/green top quadrants and blue/yellow bottom
    // quadrants. This asymmetric source makes the source-to-destination map
    // independently checkable without relying on production render helpers.
    const QUADRANTS: &[u8] = include_bytes!("../../tests/assets/crop-quadrants.png");
    const NATIVE_ALPHA_SAMPLE: &[u8] =
        include_bytes!("../../tests/assets/crop-native-alpha-4x4.png");

    fn decode_rgba8(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
        let mut reader = png::Decoder::new(Cursor::new(bytes)).read_info().unwrap();
        assert_eq!(reader.info().color_type, png::ColorType::Rgba);
        assert_eq!(reader.info().bit_depth, png::BitDepth::Eight);
        let mut decoded = vec![0; reader.output_buffer_size().unwrap()];
        let frame = reader.next_frame(&mut decoded).unwrap();
        decoded.truncate(frame.buffer_size());
        (frame.width, frame.height, decoded)
    }

    fn rgb_png_with_chunks(
        significant_bits_chunks: &[&[u8]],
        srgb_chunks: &[&[u8]],
        source_gamma: Option<png::ScaledFloat>,
    ) -> Vec<u8> {
        let pixels = [
            11, 12, 13, 21, 22, 23, 31, 32, 33, 41, 42, 43, // y=0
            51, 52, 53, 61, 62, 63, 71, 72, 73, 81, 82, 83, // y=1
            91, 92, 93, 101, 102, 103, 111, 112, 113, 121, 122, 123, // y=2
        ];
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, 4, 3);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        if let Some(gamma) = source_gamma {
            encoder.set_source_gamma(gamma);
        }
        let mut writer = encoder.write_header().unwrap();
        for significant_bits in significant_bits_chunks {
            writer
                .write_chunk(png::chunk::sBIT, significant_bits)
                .unwrap();
        }
        for intent in srgb_chunks {
            writer.write_chunk(png::chunk::sRGB, intent).unwrap();
        }
        writer.write_image_data(&pixels).unwrap();
        writer.finish().unwrap();
        bytes
    }

    #[test]
    fn crops_exact_rgba_source_pixels_without_resampling() {
        let (source_width, source_height, source) = decode_rgba8(NATIVE_ALPHA_SAMPLE);
        assert_eq!((source_width, source_height), (4, 4));

        let cropped = crop_rgb8_or_rgba8_png(NATIVE_ALPHA_SAMPLE, [1, 1, 3, 3]).unwrap();
        let (width, height, pixels) = decode_rgba8(&cropped);
        assert_eq!((width, height), (2, 2));

        let mut expected = Vec::new();
        for y in 1..3_usize {
            expected.extend_from_slice(&source[(y * 4 + 1) * 4..(y * 4 + 3) * 4]);
        }
        assert_eq!(pixels, expected);
    }

    #[test]
    fn preserves_srgb_and_rejects_other_color_profiles() {
        let srgb = rgb_png_with_chunks(&[], &[&[0]], None);
        let cropped = crop_rgb8_or_rgba8_png(&srgb, [1, 1, 3, 3]).unwrap();
        let reader = png::Decoder::new(Cursor::new(cropped)).read_info().unwrap();
        assert_eq!(
            reader.info().srgb,
            Some(png::SrgbRenderingIntent::Perceptual)
        );

        let gamma = rgb_png_with_chunks(&[], &[], Some(png::ScaledFloat::new(0.45455)));
        assert_eq!(crop_rgb8_or_rgba8_png(&gamma, [1, 1, 3, 3]), None);
        assert_eq!(crop_rgb8_or_rgba8_png(b"not a PNG", [0, 0, 1, 1]), None);

        let mut malformed = NATIVE_ALPHA_SAMPLE.to_vec();
        *malformed.last_mut().unwrap() ^= 1;
        assert_eq!(crop_rgb8_or_rgba8_png(&malformed, [1, 1, 3, 3]), None);
    }

    #[test]
    fn preserves_real_sbit_srgb_metadata_and_exact_rgb_crop_pixels() {
        let source = rgb_png_with_chunks(&[&[8, 8, 8]], &[&[0]], None);
        let cropped = crop_rgb8_or_rgba8_png(&source, [1, 1, 3, 3]).unwrap();
        let mut reader = png::Decoder::new(Cursor::new(cropped)).read_info().unwrap();
        assert_eq!(reader.info().color_type, png::ColorType::Rgb);
        assert_eq!(reader.info().bit_depth, png::BitDepth::Eight);
        assert_eq!(reader.info().sbit.as_deref(), Some(&[8, 8, 8][..]));
        assert_eq!(
            reader.info().srgb,
            Some(png::SrgbRenderingIntent::Perceptual)
        );
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        let frame = reader.next_frame(&mut pixels).unwrap();
        assert_eq!((frame.width, frame.height), (2, 2));
        pixels.truncate(frame.buffer_size());
        assert_eq!(
            pixels,
            [61, 62, 63, 71, 72, 73, 101, 102, 103, 111, 112, 113]
        );
    }

    #[test]
    fn rejects_duplicate_or_malformed_preserved_metadata() {
        for source in [
            rgb_png_with_chunks(&[&[8, 8, 8], &[8, 8, 8]], &[&[0]], None),
            rgb_png_with_chunks(&[&[0, 8, 8]], &[&[0]], None),
            rgb_png_with_chunks(&[&[8, 8, 8]], &[&[4]], None),
            rgb_png_with_chunks(&[&[8, 8, 8]], &[&[0], &[0]], None),
        ] {
            assert_eq!(crop_rgb8_or_rgba8_png(&source, [1, 1, 3, 3]), None);
        }
    }

    #[test]
    fn crops_exact_rgb_source_pixels_with_asymmetric_source_coordinates() {
        let source_pixels = [
            11, 12, 13, 21, 22, 23, 31, 32, 33, 41, 42, 43, // y=0
            51, 52, 53, 61, 62, 63, 71, 72, 73, 81, 82, 83, // y=1
            91, 92, 93, 101, 102, 103, 111, 112, 113, 121, 122, 123, // y=2
        ];
        let mut source_png = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut source_png, 4, 3);
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&source_pixels)
                .unwrap();
        }

        let cropped = crop_rgb8_or_rgba8_png(&source_png, [1, 1, 3, 3]).unwrap();
        let mut reader = png::Decoder::new(Cursor::new(cropped)).read_info().unwrap();
        assert_eq!(reader.info().color_type, png::ColorType::Rgb);
        assert_eq!(reader.info().bit_depth, png::BitDepth::Eight);
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        let frame = reader.next_frame(&mut pixels).unwrap();
        assert_eq!((frame.width, frame.height), (2, 2));
        pixels.truncate(frame.buffer_size());
        assert_eq!(
            pixels,
            [61, 62, 63, 71, 72, 73, 101, 102, 103, 111, 112, 113]
        );
    }

    #[test]
    fn maps_colored_source_crop_over_unchanged_display_bounds() {
        assert_eq!(imagesize::blob_size(QUADRANTS).unwrap().width, 4);
        assert_eq!(imagesize::blob_size(QUADRANTS).unwrap().height, 3);

        let displayed = BoundingBox {
            x_min: 10.0,
            y_min: 20.0,
            x_max: 50.0,
            y_max: 50.0,
        };
        let original = crop_only_placement(QUADRANTS, [1, 1, 3, 2], displayed).unwrap();

        // Source edge (1,1) maps to displayed top-left (10,20), and (3,2)
        // maps to bottom-right (50,50). Red and green source pixel centers
        // consequently land at (20,35) and (40,35), respectively.
        assert_eq!(
            original,
            BoundingBox {
                x_min: -10.0,
                y_min: -10.0,
                x_max: 70.0,
                y_max: 80.0,
            }
        );
        let map_x =
            |source_x: f64| original.x_min + source_x * (displayed.x_max - displayed.x_min) / 2.0;
        let map_y = |source_y: f64| original.y_min + source_y * (displayed.y_max - displayed.y_min);
        assert_eq!((map_x(1.0), map_y(1.0)), (10.0, 20.0));
        assert_eq!((map_x(1.5), map_y(1.5)), (20.0, 35.0));
        assert_eq!((map_x(2.5), map_y(1.5)), (40.0, 35.0));
        assert_eq!((map_x(3.0), map_y(2.0)), (50.0, 50.0));
        assert_eq!(imagesize::blob_size(QUADRANTS).unwrap().width, 4);
    }

    #[test]
    fn rejects_empty_inverted_negative_and_out_of_bounds_crops() {
        let bbox = BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 10.0,
            y_max: 10.0,
        };
        for crop in [
            [1, 1, 1, 2],
            [1, 2, 3, 2],
            [2, 1, 1, 2],
            [1, 2, 3, 1],
            [-1, 0, 2, 2],
            [0, -1, 2, 2],
            [0, 0, 5, 2],
            [0, 0, 2, 4],
        ] {
            assert_eq!(
                crop_only_placement(QUADRANTS, crop, bbox),
                None,
                "crop {crop:?}"
            );
        }
    }

    #[test]
    fn rejects_invalid_destination_and_unreadable_image_dimensions() {
        for bbox in [
            BoundingBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: 0.0,
                y_max: 1.0,
            },
            BoundingBox {
                x_min: 1.0,
                y_min: 0.0,
                x_max: 0.0,
                y_max: 1.0,
            },
            BoundingBox {
                x_min: f64::NAN,
                y_min: 0.0,
                x_max: 1.0,
                y_max: 1.0,
            },
            BoundingBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: f64::INFINITY,
                y_max: 1.0,
            },
        ] {
            assert_eq!(crop_only_placement(QUADRANTS, [0, 0, 1, 1], bbox), None);
        }
        assert_eq!(
            crop_only_placement(
                b"not an image",
                [0, 0, 1, 1],
                BoundingBox {
                    x_min: 0.0,
                    y_min: 0.0,
                    x_max: 1.0,
                    y_max: 1.0,
                }
            ),
            None
        );
    }
}

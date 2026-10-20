use std::io::Cursor;

const MAX_PNG_DECODE_BYTES: usize = 64 * 1024 * 1024;

/// Validate the first complete frame only for the raster format currently
/// covered by this bounded probe. `None` means the MIME type or resource size
/// is unsupported; `Some(false)` means decoding was attempted and failed.
pub(super) fn decode_saved_png(mime_type: &str, data: &[u8]) -> Option<bool> {
    decode_saved_png_with_limit(mime_type, data, MAX_PNG_DECODE_BYTES)
}

fn decode_saved_png_with_limit(mime_type: &str, data: &[u8], decode_limit: usize) -> Option<bool> {
    if mime_type != "image/png" {
        return None;
    }

    let mut decoder = png::Decoder::new(Cursor::new(data));
    decoder.set_limits(png::Limits {
        bytes: decode_limit,
    });
    let mut reader = match decoder.read_info() {
        Ok(reader) => reader,
        Err(error) => return classify_decode_error(error),
    };
    let buffer_size = reader.output_buffer_size()?;
    if buffer_size > MAX_PNG_DECODE_BYTES {
        return None;
    }
    let mut buffer = vec![0; buffer_size];
    match reader.next_frame(&mut buffer) {
        Ok(_) => Some(true),
        Err(error) => classify_decode_error(error),
    }
}

fn classify_decode_error(error: png::DecodingError) -> Option<bool> {
    match error {
        png::DecodingError::LimitsExceeded | png::DecodingError::Parameter(_) => None,
        png::DecodingError::IoError(_) | png::DecodingError::Format(_) => Some(false),
    }
}

#[cfg(test)]
mod tests {
    use super::{decode_saved_png, decode_saved_png_with_limit};
    use png::{BitDepth, ColorType, Encoder};

    fn one_pixel_png() -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut encoder = Encoder::new(&mut bytes, 1, 1);
        encoder.set_color(ColorType::Rgba);
        encoder.set_depth(BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[0, 0, 0, 255]).unwrap();
        drop(writer);
        bytes
    }

    fn png_with_dimensions(width: u32, height: u32) -> Vec<u8> {
        fn append_chunk(bytes: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
            bytes.extend((data.len() as u32).to_be_bytes());
            bytes.extend(kind);
            bytes.extend(data);
            let mut crc = !0_u32;
            for byte in kind.iter().chain(data) {
                crc ^= u32::from(*byte);
                for _ in 0..8 {
                    crc = if crc & 1 == 0 {
                        crc >> 1
                    } else {
                        (crc >> 1) ^ 0xedb8_8320
                    };
                }
            }
            bytes.extend((!crc).to_be_bytes());
        }

        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut header = Vec::new();
        header.extend(width.to_be_bytes());
        header.extend(height.to_be_bytes());
        header.extend([8, 6, 0, 0, 0]);
        append_chunk(&mut bytes, b"IHDR", &header);
        append_chunk(&mut bytes, b"IDAT", &[0]);
        append_chunk(&mut bytes, b"IEND", &[]);
        bytes
    }

    #[test]
    fn saved_png_probe_decodes_pixels_and_rejects_truncation() {
        let png = one_pixel_png();
        assert_eq!(decode_saved_png("image/png", &png), Some(true));
        assert_eq!(
            decode_saved_png("image/png", &png[..png.len() - 12]),
            Some(false)
        );
        assert_eq!(decode_saved_png("image/jpeg", &png), None);
        assert_eq!(decode_saved_png("image/png", b"not a png"), Some(false));
        assert_eq!(decode_saved_png_with_limit("image/png", &png, 0), None);
        assert_eq!(
            decode_saved_png_with_limit("image/png", b"not a png", 64),
            Some(false)
        );
        assert_eq!(
            decode_saved_png("image/png", &png_with_dimensions(8193, 8193)),
            None
        );
    }
}

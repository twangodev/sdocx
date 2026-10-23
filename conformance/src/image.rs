use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

use crate::Result;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RgbImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<[u8; 3]>,
}

impl RgbImage {
    pub fn blank(width: u32, height: u32, color: [u8; 3]) -> Self {
        Self {
            width,
            height,
            pixels: vec![color; width as usize * height as usize],
        }
    }

    pub fn from_png(path: impl AsRef<Path>) -> Result<Self> {
        let mut decoder = png::Decoder::new(BufReader::new(File::open(path)?));
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder.read_info()?;
        let mut data = vec![
            0;
            reader
                .output_buffer_size()
                .ok_or("PNG output buffer size overflow")?
        ];
        let info = reader.next_frame(&mut data)?;
        let bytes = &data[..info.buffer_size()];
        let mut image = Self::blank(info.width, info.height, [255; 3]);
        let channels = match info.color_type {
            png::ColorType::Grayscale => 1,
            png::ColorType::GrayscaleAlpha => 2,
            png::ColorType::Rgb => 3,
            png::ColorType::Rgba => 4,
            png::ColorType::Indexed => return Err("indexed PNG was not expanded by decoder".into()),
        };
        for (index, chunk) in bytes.chunks_exact(channels).enumerate() {
            let (rgb, alpha) = match channels {
                1 => ([chunk[0]; 3], 255),
                2 => ([chunk[0]; 3], chunk[1]),
                3 => ([chunk[0], chunk[1], chunk[2]], 255),
                4 => ([chunk[0], chunk[1], chunk[2]], chunk[3]),
                _ => unreachable!(),
            };
            image.pixels[index] = composite_white(rgb, alpha);
        }
        Ok(image)
    }

    pub fn to_png(&self, path: impl AsRef<Path>) -> Result<()> {
        let file = BufWriter::new(File::create(path)?);
        let mut encoder = png::Encoder::new(file, self.width, self.height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        let bytes: Vec<u8> = self
            .pixels
            .iter()
            .flat_map(|pixel| pixel.iter().copied())
            .collect();
        writer.write_image_data(&bytes)?;
        Ok(())
    }

    pub fn pixel(&self, x: u32, y: u32) -> [u8; 3] {
        self.pixels[(y * self.width + x) as usize]
    }
    pub fn set_pixel(&mut self, x: u32, y: u32, rgb: [u8; 3]) {
        self.pixels[(y * self.width + x) as usize] = rgb;
    }
}

pub fn composite_white(rgb: [u8; 3], alpha: u8) -> [u8; 3] {
    std::array::from_fn(|i| {
        ((u16::from(rgb[i]) * u16::from(alpha) + 255 * (255 - u16::from(alpha)) + 127) / 255) as u8
    })
}

#[derive(Clone, Copy, Debug)]
pub struct MetricsOptions {
    pub channel_threshold: u8,
    pub ink_threshold: u8,
    pub tolerance: u8,
}

impl Default for MetricsOptions {
    fn default() -> Self {
        Self {
            channel_threshold: 16,
            ink_threshold: 32,
            tolerance: 1,
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, PartialEq)]
pub struct PixelMetrics {
    pub mean_absolute_rgb_error: f64,
    pub changed_pixel_fraction: f64,
    pub reference_ink_pixels: usize,
    pub sdk_ink_pixels: usize,
    pub missing_ink_fraction: f64,
    pub extra_ink_fraction: f64,
}

pub fn measure(
    reference: &RgbImage,
    candidate: &RgbImage,
    options: MetricsOptions,
) -> Result<(PixelMetrics, RgbImage)> {
    if (reference.width, reference.height) != (candidate.width, candidate.height) {
        return Err("pixel comparison requires matching dimensions".into());
    }
    let mut sum = 0u64;
    let mut changed = 0usize;
    let mut ref_ink = vec![false; reference.pixels.len()];
    let mut sdk_ink = vec![false; candidate.pixels.len()];
    let mut diff = RgbImage::blank(reference.width, reference.height, [0; 3]);
    for i in 0..reference.pixels.len() {
        let mut pixel_changed = false;
        for channel in 0..3 {
            let d = reference.pixels[i][channel].abs_diff(candidate.pixels[i][channel]);
            sum += u64::from(d);
            pixel_changed |= d > options.channel_threshold;
            diff.pixels[i][channel] = d.saturating_mul(4);
        }
        changed += usize::from(pixel_changed);
        ref_ink[i] = reference.pixels[i].into_iter().min().unwrap() < 255 - options.ink_threshold;
        sdk_ink[i] = candidate.pixels[i].into_iter().min().unwrap() < 255 - options.ink_threshold;
    }
    let ref_count = ref_ink.iter().filter(|&&v| v).count();
    let sdk_count = sdk_ink.iter().filter(|&&v| v).count();
    let mut missing = 0;
    let mut extra = 0;
    for y in 0..reference.height as i32 {
        for x in 0..reference.width as i32 {
            let i = (y as u32 * reference.width + x as u32) as usize;
            if ref_ink[i]
                && !dilated_contains(
                    &sdk_ink,
                    reference.width as i32,
                    reference.height as i32,
                    x,
                    y,
                    options.tolerance as i32,
                )
            {
                missing += 1;
            }
            if sdk_ink[i]
                && !dilated_contains(
                    &ref_ink,
                    reference.width as i32,
                    reference.height as i32,
                    x,
                    y,
                    options.tolerance as i32,
                )
            {
                extra += 1;
            }
        }
    }
    let n = reference.pixels.len().max(1) as f64;
    Ok((
        PixelMetrics {
            mean_absolute_rgb_error: sum as f64 / (3.0 * n) / 255.0,
            changed_pixel_fraction: changed as f64 / n,
            reference_ink_pixels: ref_count,
            sdk_ink_pixels: sdk_count,
            missing_ink_fraction: if ref_count == 0 {
                0.0
            } else {
                missing as f64 / ref_count as f64
            },
            extra_ink_fraction: if sdk_count == 0 {
                0.0
            } else {
                extra as f64 / sdk_count as f64
            },
        },
        diff,
    ))
}

fn dilated_contains(mask: &[bool], width: i32, height: i32, x: i32, y: i32, radius: i32) -> bool {
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let xx = x + dx;
            let yy = y + dy;
            if xx >= 0 && yy >= 0 && xx < width && yy < height && mask[(yy * width + xx) as usize] {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    #[test]
    fn mean_rgb_error_preserves_reference_workflow_rounding() {
        let reference = RgbImage::blank(3, 1, [0; 3]);
        let mut candidate = reference.clone();
        candidate.set_pixel(0, 0, [17, 0, 0]);
        let (metrics, _) = measure(&reference, &candidate, MetricsOptions::default()).unwrap();
        // The previous NumPy runner averages RGB before normalizing by 255.
        assert_eq!(metrics.mean_absolute_rgb_error, 0.007407407407407407);
    }

    #[test]
    fn blank_alpha_composites_over_white() {
        let image = RgbImage::blank(1, 1, composite_white([0, 0, 0], 0));
        assert_eq!(image.pixel(0, 0), [255; 3]);
    }
    #[test]
    fn png_rgba_input_composites_over_white_like_the_reference_workflow() {
        let path = std::env::temp_dir().join(format!(
            "sdocx-rgba-{}-{}.png",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let file = File::create(&path).unwrap();
        let mut encoder = png::Encoder::new(file, 2, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer
            .write_image_data(&[0, 0, 0, 0, 0, 0, 0, 128])
            .unwrap();
        drop(writer);
        let image = RgbImage::from_png(&path).unwrap();
        assert_eq!(image.pixel(0, 0), [255; 3]);
        assert_eq!(image.pixel(1, 0), [127; 3]);
        let _ = std::fs::remove_file(path);
    }
    #[test]
    fn tolerance_changes_ink_only_not_rgb_difference() {
        let mut a = RgbImage::blank(5, 5, [255; 3]);
        a.set_pixel(1, 2, [0; 3]);
        let mut b = RgbImage::blank(5, 5, [255; 3]);
        b.set_pixel(2, 2, [0; 3]);
        let (exact, _) = measure(
            &a,
            &b,
            MetricsOptions {
                tolerance: 0,
                ..Default::default()
            },
        )
        .unwrap();
        let (near, _) = measure(
            &a,
            &b,
            MetricsOptions {
                tolerance: 1,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(exact.missing_ink_fraction > 0.0);
        assert_eq!(near.missing_ink_fraction, 0.0);
        assert_eq!(exact.changed_pixel_fraction, near.changed_pixel_fraction);
    }
}

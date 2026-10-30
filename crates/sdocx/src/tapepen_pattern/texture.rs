/*
 * Copyright (c) 2013-14 Mikko Mononen memon@inside.org
 *
 * This software is provided 'as-is', without any express or implied
 * warranty.  In no event will the authors be held liable for any damages
 * arising from the use of this software.
 *
 * Permission is granted to anyone to use this software for any purpose,
 * including commercial applications, and to alter it and redistribute it
 * freely, subject to the following restrictions:
 *
 * 1. The origin of this software must not be misrepresented; you must not
 * claim that you wrote the original software. If you use this software
 * in a product, an acknowledgment in the product documentation would be
 * appreciated but is not required.
 * 2. Altered source versions must be plainly marked as such, and must not be
 * misrepresented as being the original software.
 * 3. This notice may not be removed or altered from any source distribution.
 *
 * The polygon rasterization is heavily based on stb_truetype rasterizer
 * by Sean Barrett - http://nothings.org/
 *
 */
//! Altered Rust translation of NanoSVG solid blending and defringing.

fn div255(x: u32) -> u32 {
    ((x + 1) * 257) >> 16
}

pub fn blend(dst: &mut [u8], coverage: &[u8], color: u32, opacity: f32) {
    assert_eq!(dst.len(), coverage.len() * 4);
    assert!(opacity.is_finite());
    let alpha = ((color >> 24) * (opacity.clamp(0.0, 1.0) * 256.0) as u32) >> 8;
    for (pixel, &cover) in dst.as_chunks_mut::<4>().0.iter_mut().zip(coverage) {
        let a = div255(u32::from(cover) * alpha);
        let inverse = 255 - a;
        for (i, channel) in pixel[..3].iter_mut().enumerate() {
            let c = (color >> (8 * i)) & 255;
            *channel = (div255(c * a) + div255(inverse * u32::from(*channel))) as u8;
        }
        pixel[3] = (a + div255(inverse * u32::from(pixel[3]))) as u8;
    }
}

pub fn finish(image: &mut [u8], width: usize, height: usize) {
    assert_eq!(image.len(), width * height * 4);
    for p in image.as_chunks_mut::<4>().0.iter_mut() {
        let a = u32::from(p[3]);
        for c in &mut p[..3] {
            if let Some(value) = (u32::from(*c) * 255).checked_div(a) {
                *c = value as u8;
            }
        }
    }
    for y in 0..height {
        for x in 0..width {
            let i = (y * width + x) * 4;
            if image[i + 3] != 0 {
                continue;
            }
            let mut sum = [0u32; 3];
            let mut count = 0;
            let mut add = |j: usize| {
                if image[j + 3] != 0 {
                    for c in 0..3 {
                        sum[c] += u32::from(image[j + c]);
                    }
                    count += 1;
                }
            };
            if x > 1 {
                add(i - 4);
            }
            if x + 1 < width {
                add(i + 4);
            }
            if y > 1 {
                add(i - width * 4);
            }
            if y + 1 < height {
                add(i + width * 4);
            }
            for c in 0..3 {
                if let Some(value) = sum[c].checked_div(count) {
                    image[i + c] = value as u8;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transparent_solid_overlap_and_defringe() {
        let mut image = vec![0; 12];
        blend(&mut image, &[0, 255, 0], 0x800000ff, 1.0);
        blend(&mut image, &[0, 255, 0], 0x8000ff00, 1.0);
        assert_eq!(&image[4..8], &[63, 128, 0, 191]);
        finish(&mut image, 3, 1);
        assert_eq!(image, [84, 170, 0, 0, 84, 170, 0, 191, 84, 170, 0, 0]);
    }
}

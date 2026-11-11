//! Experimental host-profile material transport; not renderer dispatch.
#![allow(non_snake_case)]
use base64::{Engine as _, engine::general_purpose::STANDARD};

#[derive(Clone, Copy)]
pub(super) struct Projection {
    pub width: u32,
    pub height: u32,
    pub matrix: [f32; 16],
    pub bg_scale: [f32; 2],
}

impl Projection {
    fn screen(self, x: f32, y: f32) -> Result<[f32; 2], String> {
        // GLES matrix payloads are column-major. Transform world coordinates
        // to clip coordinates, divide by w, then convert to top-left pixels.
        let m = self.matrix;
        let clip_x = m[0] * x + m[4] * y + m[12];
        let clip_y = m[1] * x + m[5] * y + m[13];
        let clip_w = m[3] * x + m[7] * y + m[15];
        if !clip_w.is_finite() || clip_w == 0.0 {
            return Err("captured projection produced invalid w".into());
        }
        let point = [
            (clip_x / clip_w + 1.0) * 0.5 * self.width as f32,
            self.height as f32 - (clip_y / clip_w + 1.0) * 0.5 * self.height as f32,
        ];
        if point.iter().any(|v| !v.is_finite()) {
            return Err("invalid projected coordinate".into());
        }
        Ok(point)
    }

    fn periods(self) -> Result<[f32; 2], String> {
        let p = [
            self.width as f32 / self.bg_scale[0],
            self.height as f32 / self.bg_scale[1],
        ];
        if p.iter().any(|v| !v.is_finite() || *v <= 0.0) {
            return Err("observed background scale gives invalid texture period".into());
        }
        Ok(p)
    }
}

fn encode_ga_png(width: u32, height: u32, gray: &[u8]) -> Result<Vec<u8>, String> {
    if gray.len() != width as usize * height as usize {
        return Err(format!(
            "bad grayscale plane: {} bytes for {width}x{height}",
            gray.len()
        ));
    }
    // PNG gray+alpha stores the same native byte in both channels. SVG's alpha
    // mask therefore receives the source's exact 8-bit value, not luminance.
    let mut ga = Vec::with_capacity(gray.len() * 2);
    for value in gray {
        ga.extend_from_slice(&[*value, *value]);
    }
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::GrayscaleAlpha);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|e| format!("PNG header: {e}"))?;
        writer
            .write_image_data(&ga)
            .map_err(|e| format!("PNG body: {e}"))?;
    }
    Ok(out)
}

fn num(value: f32) -> String {
    // Nine decimal places exceed f32's round-trip precision over this canvas.
    format!("{value:.9}")
}

fn png_data_uri(plane: &[u8], width: u32, height: u32) -> Result<String, String> {
    let png = encode_ga_png(width, height, plane)?;
    Ok(format!("data:image/png;base64,{}", STANDARD.encode(png)))
}

const MAX_SVG_BYTES: usize = 32 * 1024 * 1024;
// Every stamp has a full-viewport filter. Bound the aggregate filter area so a
// small SVG cannot request thousands of page-sized reader intermediates.
const MAX_FILTER_PIXELS: u64 = 32 * 1024 * 1024;

fn check_filter_work(width: u32, height: u32, stamps: usize) -> Result<(), String> {
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|area| area.checked_mul(u64::try_from(stamps).ok()?))
        .ok_or("material filter work limit")?;
    if pixels > MAX_FILTER_PIXELS {
        return Err("material filter work limit".into());
    }
    Ok(())
}

fn point_padding(texel_size: f32) -> Result<usize, String> {
    let needed = (1.0 / texel_size).ceil();
    // A 10-texel tile and padding on both sides must fit the 512 limit.
    if !needed.is_finite() || needed < 0.0 || needed > 250.0 {
        return Err("point padding exceeds limit".into());
    }
    Ok(needed as usize + 1)
}

fn check_svg_size(size: usize) -> Result<(), String> {
    if size > MAX_SVG_BYTES {
        Err("material output limit".into())
    } else {
        Ok(())
    }
}

fn padded_point_tile(
    point: &[u8],
    tile: usize,
    pad_x: usize,
    pad_y: usize,
) -> Result<(Vec<u8>, u32, u32), String> {
    if point.len() != 300 || tile > 2 {
        return Err("point atlas source dimensions or tile selector changed".into());
    }
    let width = 10usize
        .checked_add(pad_x.checked_mul(2).ok_or("point pad X overflow")?)
        .ok_or("point image width overflow")?;
    let height = 10usize
        .checked_add(pad_y.checked_mul(2).ok_or("point pad Y overflow")?)
        .ok_or("point image height overflow")?;
    // Keep bounded allocation independent of malformed/unsupported geometry.
    if width > 512 || height > 512 {
        return Err(format!(
            "derived padded point tile is unreasonably large: {width}x{height}"
        ));
    }
    let mut padded = Vec::with_capacity(width * height);
    for y in 0..height {
        let atlas_y = (tile as isize * 10 + y as isize - pad_y as isize).clamp(0, 29) as usize;
        let row = &point[atlas_y * 10..(atlas_y + 1) * 10];
        for x in 0..width {
            let atlas_x = if x < pad_x {
                0
            } else if x >= pad_x + 10 {
                9
            } else {
                x - pad_x
            };
            padded.push(row[atlas_x]);
        }
    }
    Ok((padded, width as u32, height as u32))
}

pub(super) fn emit_svg(
    records: &[super::Record],
    point_atlas: &[u8],
    background_texture: &[u8],
    projection: Projection,
    rgba: [f32; 4],
    background: Option<[u8; 3]>,
    k4: f32,
) -> Result<String, String> {
    if projection.matrix.iter().any(|v| !v.is_finite())
        || [1, 3, 4, 7].iter().any(|i| projection.matrix[*i] != 0.0)
        || projection.matrix[15] == 0.0
        || point_atlas.len() != 300
        || projection.width == 0
        || projection.height == 0
        || projection.width > 4096
        || projection.height > 4096
        || background_texture.len() != 150 * 150
        || records.len() > 8_192
        || rgba
            .iter()
            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        || k4 != 0.0
    {
        return Err("invalid material profile".into());
    }
    check_filter_work(projection.width, projection.height, records.len())?;
    let bg_uri = png_data_uri(background_texture, 150, 150)?;
    let WIDTH = projection.width;
    let HEIGHT = projection.height;
    let periods = projection.periods()?;
    let mut svg = String::with_capacity(210_000);
    svg.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    svg.push_str(&format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{WIDTH}\" height=\"{HEIGHT}\" viewBox=\"0 0 {WIDTH} {HEIGHT}\" color-interpolation=\"sRGB\" shape-rendering=\"crispEdges\">\n<defs>\n"));
    svg.push_str(&format!("<image id=\"native-bg-mask\" x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" image-rendering=\"optimizeSpeed\" style=\"image-rendering:pixelated\" transform=\"matrix(1 0 0 -1 0 {})\" href=\"{bg_uri}\"/>\n", num(periods[0]), num(periods[1]), num(periods[1])));
    let mut total_repetitions = 0_i64;
    for (i, record) in records.iter().enumerate() {
        let [x, y, radius, selector, bgx, bgy] = record.lanes;
        if record.lanes.iter().any(|v| !v.is_finite()) {
            return Err(format!("nonfinite stamp {i}"));
        }
        if radius <= 0.0 || selector.fract() != 0.0 || !(0.0..=2.0).contains(&selector) {
            return Err(format!(
                "bad native stamp {i}: radius={radius}, tile={selector}"
            ));
        }
        let tile = selector as usize;
        let center = projection.screen(x, y)?;
        let p0 = projection.screen(x - radius, y - radius)?;
        let p1 = projection.screen(x + radius, y + radius)?;
        let x0 = p0[0].min(p1[0]);
        let y0 = p0[1].min(p1[1]);
        let width = (p1[0] - p0[0]).abs();
        let height = (p1[1] - p0[1]).abs();
        // Vertex shader forms bgPosition from point.xy + aBGVertex. Project
        // that real lane offset and derive the repeated-image origin from the
        // interpolated normalized canvas UV (the PNG rows were GL-origin flipped).
        let bg_origin = projection.screen(x + bgx, y + bgy)?;
        let pattern_x = -(bg_origin[0] - center[0]);
        let pattern_y = HEIGHT as f32 - (bg_origin[1] - center[1]);
        let bg_mask_id = format!("background-mask-{i}");
        svg.push_str(&format!("<mask id=\"{bg_mask_id}\" maskUnits=\"userSpaceOnUse\" maskContentUnits=\"userSpaceOnUse\" x=\"0\" y=\"0\" width=\"{WIDTH}\" height=\"{HEIGHT}\" style=\"mask-type:alpha\"><g shape-rendering=\"crispEdges\">"));
        let min_x = ((x0 - pattern_x) / periods[0]).floor() as i32;
        let max_x = ((x0 + width - pattern_x) / periods[0]).floor() as i32;
        let min_y = ((y0 - pattern_y) / periods[1]).floor() as i32;
        let max_y = ((y0 + height - pattern_y) / periods[1]).floor() as i32;
        if i64::from(max_x) - i64::from(min_x) > 512 || i64::from(max_y) - i64::from(min_y) > 512 {
            return Err("background repetition limit".into());
        }
        let repetitions =
            (i64::from(max_x) - i64::from(min_x) + 1) * (i64::from(max_y) - i64::from(min_y) + 1);
        total_repetitions += repetitions;
        if repetitions > 4096 || total_repetitions > 100_000 || svg.len() > MAX_SVG_BYTES {
            return Err("material output limit".into());
        }
        for ky in min_y..=max_y {
            for kx in min_x..=max_x {
                let image_x = pattern_x + kx as f32 * periods[0];
                let image_y = pattern_y + ky as f32 * periods[1];
                svg.push_str(&format!(
                    "<use href=\"#native-bg-mask\" x=\"{}\" y=\"{}\"/>",
                    num(image_x),
                    num(image_y)
                ));
            }
        }
        svg.push_str("</g></mask>\n");
        // Each sampled native mask is rendered independently as an feImage
        // input over the full canvas. feComposite then evaluates the native
        // alpha product and factor five in one operation, avoiding the
        // intermediate 8-bit mask multiplication identified by the stage probe.
        let point_texel_px_x = width / 10.0;
        let point_texel_px_y = height / 10.0;
        if !point_texel_px_x.is_finite()
            || !point_texel_px_y.is_finite()
            || point_texel_px_x <= 0.0
            || point_texel_px_y <= 0.0
        {
            return Err(format!("stamp {i} has invalid projected point texel size"));
        }
        // resvg image drawing uses nearest texel filtering but antialiased
        // fill_rect coverage at the image's outer edge. Pad beyond a whole
        // framebuffer pixel, derived from the projected texel footprint.
        let pad_x = point_padding(point_texel_px_x)?;
        let pad_y = point_padding(point_texel_px_y)?;
        let (padded_tile, padded_width, padded_height) =
            padded_point_tile(point_atlas, tile, pad_x, pad_y)?;
        let padded_uri = png_data_uri(&padded_tile, padded_width, padded_height)?;
        let pad_px_x = pad_x as f32 * point_texel_px_x;
        let pad_px_y = pad_y as f32 * point_texel_px_y;
        let point_image_x = x0 - pad_px_x;
        let point_image_y = y0 - pad_px_y;
        let point_image_width = width + 2.0 * pad_px_x;
        let point_image_height = height + 2.0 * pad_px_y;
        let point_mask_id = format!("point-mask-{i}");
        svg.push_str(&format!("<mask id=\"{point_mask_id}\" maskUnits=\"userSpaceOnUse\" maskContentUnits=\"userSpaceOnUse\" x=\"0\" y=\"0\" width=\"{WIDTH}\" height=\"{HEIGHT}\" style=\"mask-type:alpha\"><image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" image-rendering=\"optimizeSpeed\" style=\"image-rendering:pixelated\" href=\"{}\"/></mask>\n", num(point_image_x), num(point_image_y), num(point_image_width), num(point_image_height), padded_uri));
        let point_input_id = format!("point-alpha-input-{i}");
        let bg_input_id = format!("background-alpha-input-{i}");
        let alpha_filter_id = format!("native-alpha-product-{i}");
        let point_rect_id = format!("point-alpha-rect-{i}");
        let bg_rect_id = format!("background-alpha-rect-{i}");
        svg.push_str(&format!("<g id=\"{point_input_id}\"><rect id=\"{point_rect_id}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#ffffff\" shape-rendering=\"crispEdges\" mask=\"url(#{point_mask_id})\"/></g>\n", num(x0), num(y0), num(width), num(height)));
        svg.push_str(&format!("<g id=\"{bg_input_id}\"><rect id=\"{bg_rect_id}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#ffffff\" mask=\"url(#{bg_mask_id})\"/></g>\n", num(x0), num(y0), num(width), num(height)));
        let k1 = 5.0f32 * rgba[3];
        let color_matrix = format!(
            "0 0 0 0 {} 0 0 0 0 {} 0 0 0 0 {} 0 0 0 1 0",
            num(rgba[0]),
            num(rgba[1]),
            num(rgba[2])
        );
        svg.push_str(&format!("<filter id=\"{alpha_filter_id}\" x=\"0\" y=\"0\" width=\"{WIDTH}\" height=\"{HEIGHT}\" filterUnits=\"userSpaceOnUse\" primitiveUnits=\"userSpaceOnUse\" color-interpolation-filters=\"sRGB\"><feImage href=\"#{point_input_id}\" x=\"0\" y=\"0\" width=\"{WIDTH}\" height=\"{HEIGHT}\" preserveAspectRatio=\"none\" result=\"point\"/><feImage href=\"#{bg_input_id}\" x=\"0\" y=\"0\" width=\"{WIDTH}\" height=\"{HEIGHT}\" preserveAspectRatio=\"none\" result=\"background\"/><feComposite in=\"point\" in2=\"background\" operator=\"arithmetic\" k1=\"{}\" k2=\"0\" k3=\"0\" k4=\"{}\" result=\"native-alpha\"/><feColorMatrix in=\"native-alpha\" type=\"matrix\" values=\"{}\" result=\"native-premult-rgba\"/></filter>\n", num(k1), num(k4), color_matrix));
    }
    svg.push_str("</defs>\n");
    if let Some(color) = background {
        svg.push_str(&format!("<rect x=\"0\" y=\"0\" width=\"{WIDTH}\" height=\"{HEIGHT}\" fill=\"#{:02x}{:02x}{:02x}\"/>\n", color[0], color[1], color[2]));
    }
    svg.push_str("<g color-interpolation=\"sRGB\" shape-rendering=\"crispEdges\">\n");
    for i in 0..records.len() {
        // Filter output is already premultiplied RGBA. Drawing it directly
        // avoids a second SVG mask pass (MaskU8) over the source RGB.
        svg.push_str(&format!("<rect x=\"0\" y=\"0\" width=\"{WIDTH}\" height=\"{HEIGHT}\" fill=\"#ffffff\" filter=\"url(#native-alpha-product-{i})\"/>\n"));
    }
    svg.push_str("</g>\n</svg>\n");
    check_svg_size(svg.len())?;
    Ok(svg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padding_and_final_size_limits_do_not_overflow() {
        assert!(point_padding(f32::MIN_POSITIVE).is_err());
        assert!(point_padding(0.0).is_err());
        assert_eq!(point_padding(1.0).unwrap(), 2);
        assert!(check_svg_size(MAX_SVG_BYTES).is_ok());
        assert!(check_svg_size(MAX_SVG_BYTES + 1).is_err());
    }

    #[test]
    fn full_canvas_filter_work_is_bounded_before_encoding() {
        assert!(check_filter_work(256, 256, 303).is_ok());
        assert!(check_filter_work(256, 256, 512).is_ok());
        assert!(check_filter_work(256, 256, 513).is_err());
        assert!(check_filter_work(4096, 4096, 3).is_err());
        assert!(check_filter_work(u32::MAX, u32::MAX, usize::MAX).is_err());
        let projection = Projection {
            width: 4096,
            height: 4096,
            matrix: [
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ],
            bg_scale: [1.0, 1.0],
        };
        let record = super::super::Record {
            lanes: [1.0, 1.0, 1.0, 0.0, 0.0, 0.0],
        };
        assert_eq!(
            emit_svg(
                &[record; 3],
                &[0; 300],
                &[0; 22500],
                projection,
                [0.0; 4],
                None,
                0.0
            )
            .unwrap_err(),
            "material filter work limit"
        );
    }

    #[test]
    fn rejects_unsupported_profiles_before_paint() {
        let mut projection = Projection {
            width: 256,
            height: 256,
            matrix: [
                0.0078125, 0.0, 0.0, 0.0, 0.0, 0.0078125, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -1.0, -1.0,
                0.0, 1.0,
            ],
            bg_scale: [2.2755556, 2.2755556],
        };
        let point = [0; 300];
        let background = [0; 22500];
        projection.matrix[1] = 0.1;
        assert!(emit_svg(&[], &point, &background, projection, [0.0; 4], None, 0.0).is_err());
        projection.matrix[1] = 0.0;
        assert!(
            emit_svg(
                &[],
                &point[..299],
                &background,
                projection,
                [0.0; 4],
                None,
                0.0
            )
            .is_err()
        );
        let bad_record = super::super::Record {
            lanes: [f32::NAN, 0.0, 1.0, 0.0, 0.0, 0.0],
        };
        assert!(
            emit_svg(
                &[bad_record],
                &point,
                &background,
                projection,
                [0.0; 4],
                None,
                0.0
            )
            .is_err()
        );
    }
}

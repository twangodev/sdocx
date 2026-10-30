//! Bounded ColoredPencil RTV1 paint with an optional explicit depth bitmap.
use crate::ink::host_raster::AxisRect;
use crate::ink::host_rgba8::{
    over, premultiply, quantize, sample_linear_clamp_red, sample_unorm8_half_rtz,
};
use crate::{InkRasterContext, InkRasterError, InkRasterOrigin};

const POINT: &[u8] = include_bytes!("assets/point.r8");
const MAX_SAMPLES: usize = 16 * 1024 * 1024;

struct Stamp {
    rect: AxisRect<4>,
    alpha: f32,
}

#[derive(Clone, Copy)]
pub(crate) struct DepthMap<'a> {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) pixels: &'a [[u8; 4]],
    pub(crate) background_scale: f32,
}

pub(crate) struct Plan<'a> {
    stamps: Vec<Stamp>,
    frame: [usize; 2],
    argb: u32,
    work: usize,
    depth: Option<DepthMap<'a>>,
}

pub(crate) fn prepare(
    rows: &[[f32; 5]],
    context: InkRasterContext,
    argb: u32,
    budget: usize,
) -> Result<Plan<'static>, InkRasterError> {
    prepare_depth(rows, context, argb, budget, None)
}

pub(crate) fn prepare_depth<'a>(
    rows: &[[f32; 5]],
    context: InkRasterContext,
    argb: u32,
    budget: usize,
    depth: Option<DepthMap<'a>>,
) -> Result<Plan<'a>, InkRasterError> {
    let count = context.pixel_count()?;
    if rows.len() > 16_384 {
        return Err(InkRasterError::WorkLimit);
    }
    let mut work = count + rows.len() * 32;
    if let Some(depth) = depth {
        let context = crate::InkRasterDepthContext {
            width: u32::try_from(depth.width).map_err(|_| InkRasterError::InvalidDepthMap)?,
            height: u32::try_from(depth.height).map_err(|_| InkRasterError::InvalidDepthMap)?,
            background_scale: depth.background_scale,
        };
        let depth_count = context.pixel_count()?;
        if depth_count != depth.pixels.len() {
            return Err(InkRasterError::InvalidDepthMap);
        }
        work += depth_count;
    }
    if work > budget {
        return Err(InkRasterError::WorkLimit);
    }
    let mut stamps = Vec::new();
    stamps
        .try_reserve_exact(rows.len())
        .map_err(|_| InkRasterError::Allocation)?;
    let mut samples = 0usize;
    let [width, height] = [context.width, context.height].map(|n| n as f32);
    let y_up = context.origin == InkRasterOrigin::BottomLeft;
    let scale = context.pixels_per_unit as f32;
    let project = |value: f32, extent: f32, sign: f32| {
        let clip = value * (2. * scale / extent * sign) - sign;
        (clip, clip.mul_add(extent / 2., extent / 2.))
    };
    for &[x, y, half, row, alpha] in rows {
        if [x, y, half, row, alpha].iter().any(|v| !v.is_finite())
            || half <= 0.
            || half > 768.
            || ![0., 1., 2.].contains(&row)
            || !(0. ..=1.).contains(&alpha)
        {
            return Err(InkRasterError::InvalidStroke("ColoredPencil derived stamp"));
        }
        let x_projected = [x - half, x + half].map(|p| project(p, width, 1.));
        let y_projected =
            [y - half, y + half].map(|p| project(p, height, if y_up { 1. } else { -1. }));
        let xx = x_projected.map(|(_, pixel)| pixel);
        let yy = y_projected.map(|(_, pixel)| pixel);
        let pu = x_projected.map(|(clip, _)| (clip + 1.) * 0.5);
        let pv = y_projected.map(|(clip, _)| (clip + 1.) * 0.5);
        let values = [
            [0., 0., 1., 1.],
            [
                (1. + row) * (1. / 3.),
                row * (1. / 3.),
                (1. + row) * (1. / 3.),
                row * (1. / 3.),
            ],
            [pu[0], pu[0], pu[1], pu[1]],
            [pv[0], pv[1], pv[0], pv[1]],
        ];
        let rect = AxisRect::new(xx, yy, [context.width, context.height], values, y_up)
            .ok_or(InkRasterError::InvalidStroke("ColoredPencil varying plane"))?;
        let [xa, ya, xb, yb] = rect.bounds();
        let area = (xb - xa) * (yb - ya);
        samples += area;
        work += area * if depth.is_some() { 5 } else { 1 };
        if samples > MAX_SAMPLES || work > budget {
            return Err(InkRasterError::WorkLimit);
        }
        stamps.push(Stamp { rect, alpha });
    }
    Ok(Plan {
        stamps,
        frame: [context.width as usize, context.height as usize],
        argb,
        work,
        depth,
    })
}

impl Plan<'_> {
    pub(crate) fn work(&self) -> usize {
        self.work
    }

    pub(crate) fn render(&self, prior: &[[u8; 4]]) -> Result<Vec<[u8; 4]>, InkRasterError> {
        if prior.len() != self.frame[0] * self.frame[1] {
            return Err(InkRasterError::InvalidBackdrop);
        }
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(prior.len())
            .map_err(|_| InkRasterError::Allocation)?;
        pixels.extend_from_slice(prior);
        let color = [
            (self.argb >> 16) as u8,
            (self.argb >> 8) as u8,
            self.argb as u8,
        ];
        let alpha = (self.argb >> 24) as u8 as f32 / 255.;
        let depth = self.depth.map(|depth| {
            (
                depth,
                [
                    (self.frame[0] as f32 / depth.width as f32) / depth.background_scale,
                    (self.frame[1] as f32 / depth.height as f32) / depth.background_scale,
                ],
            )
        });
        for stamp in &self.stamps {
            let [xa, ya, xb, yb] = stamp.rect.bounds();
            for y in ya..yb {
                for x in xa..xb {
                    let Some(uv) = stamp.rect.sample(x, y) else {
                        continue;
                    };
                    let index = |q: f32, n: usize| {
                        (q * n as f32).floor().clamp(0., (n - 1) as f32) as usize
                    };
                    let sample =
                        sample_unorm8_half_rtz(POINT[index(uv[1], 30) * 10 + index(uv[0], 10)]);
                    let coverage = alpha * sample;
                    let coverage = if let Some((depth, scale)) = depth {
                        coverage
                            * sample_linear_clamp_red(
                                depth.pixels,
                                depth.width,
                                depth.height,
                                [uv[2] * scale[0], uv[3] * scale[1]],
                            )
                    } else {
                        coverage
                    };
                    let source_alpha = quantize(coverage * stamp.alpha);
                    let source = premultiply([color[0], color[1], color[2], source_alpha]);
                    let pixel = &mut pixels[y * self.frame[0] + x];
                    *pixel = over(*pixel, source);
                }
            }
        }
        Ok(pixels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use flate2::read::ZlibDecoder;
    use serde::Deserialize;
    use sha2::{Digest, Sha256};
    use std::collections::BTreeMap;
    use std::io::Read;

    #[derive(Deserialize)]
    struct NativeCase {
        source: String,
        argb: u32,
        origin: String,
        scale: u32,
        frame: [u32; 2],
        background: [u8; 4],
        records: usize,
        stream_sha256: String,
        frame_sha256: String,
        sparse_zlib_base64: String,
        changed_pixels: usize,
        prior_verified: bool,
    }
    #[derive(Deserialize)]
    struct Native {
        cases: BTreeMap<String, NativeCase>,
    }

    #[test]
    fn native_depth_branch_matches_bitmap_sampling_and_ordered_rgba() {
        use serde_json::Value;
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../../conformance/colored-pencil-v4-depth-native.json"
        ))
        .unwrap();
        let geometry: Value = serde_json::from_str(include_str!(
            "../../../../../conformance/colored-pencil-v4-source-context-native.json"
        ))
        .unwrap();
        let no_depth: Value = serde_json::from_str(include_str!(
            "../../../../../conformance/colored-pencil-v4-context-paint-native.json"
        ))
        .unwrap();
        let unhex = |s: &str| {
            (0..s.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
                .collect::<Vec<_>>()
        };
        assert_eq!(fixture["cases"].as_object().unwrap().len(), 360);
        let mut failures = Vec::new();
        for (name, case) in fixture["cases"].as_object().unwrap() {
            assert_eq!(case["prior_verified"], true);
            assert_eq!(case["native_depth_pointer_verified"], true);
            assert_eq!(case["depth_texture_unit"], 1);
            let state = &case["depth_sampler_state"];
            for (pname, value) in [
                ("10240", 9729),
                ("10241", 9729),
                ("10242", 33071),
                ("10243", 33071),
            ] {
                assert_eq!(state[pname], value);
            }
            let source = geometry["cases"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["id"] == case["source"])
                .unwrap();
            let bytes = unhex(source["stream_hex"].as_str().unwrap());
            assert_eq!(
                format!("{:x}", Sha256::digest(&bytes)),
                case["stream_sha256"].as_str().unwrap()
            );
            let rows: Vec<[f32; 5]> = bytes
                .as_chunks::<20>()
                .0
                .iter()
                .map(|row| {
                    std::array::from_fn(|i| {
                        f32::from_le_bytes(row[i * 4..i * 4 + 4].try_into().unwrap())
                    })
                })
                .collect();
            let map = &fixture["depth_inputs"][case["depth"].as_str().unwrap()];
            let raw = unhex(map["rgba_hex"].as_str().unwrap());
            assert_eq!(
                format!("{:x}", Sha256::digest(&raw)),
                map["proof"]["rgba_sha256"].as_str().unwrap()
            );
            let rgba = raw.as_chunks::<4>().0;
            let depth = DepthMap {
                width: map["width"].as_u64().unwrap() as usize,
                height: map["height"].as_u64().unwrap() as usize,
                pixels: rgba,
                background_scale: case["background_scale"].as_f64().unwrap() as f32,
            };
            let frame =
                std::array::from_fn::<_, 2, _>(|i| case["frame"][i].as_u64().unwrap() as u32);
            for (i, size) in [depth.width, depth.height].into_iter().enumerate() {
                let expected = (frame[i] as f32 / size as f32) / depth.background_scale;
                assert_eq!(
                    expected.to_bits(),
                    (case["paper_scale_upload"][i].as_f64().unwrap() as f32).to_bits()
                );
            }
            let context = InkRasterContext {
                width: frame[0],
                height: frame[1],
                pixels_per_unit: case["scale"].as_u64().unwrap() as u32,
                origin: if case["origin"] == "top" {
                    InkRasterOrigin::TopLeft
                } else {
                    InkRasterOrigin::BottomLeft
                },
            };
            let background = std::array::from_fn(|i| case["background"][i].as_u64().unwrap() as u8);
            let prior = vec![background; context.pixel_count().unwrap()];
            let plan = prepare_depth(
                &rows,
                context,
                case["argb"].as_u64().unwrap() as u32,
                64 * 1024 * 1024,
                Some(depth),
            )
            .unwrap();
            let actual = plan.render(&prior).unwrap();
            let compressed = base64::engine::general_purpose::STANDARD
                .decode(case["rgba_zlib_base64"].as_str().unwrap())
                .unwrap();
            let mut bottom = Vec::new();
            ZlibDecoder::new(compressed.as_slice())
                .take((prior.len() * 4 + 1) as u64)
                .read_to_end(&mut bottom)
                .unwrap();
            assert_eq!(bottom.len(), prior.len() * 4);
            assert_eq!(
                format!("{:x}", Sha256::digest(&bottom)),
                case["frame_sha256"].as_str().unwrap()
            );
            let expected: Vec<_> = bottom
                .as_chunks::<4>()
                .0
                .chunks_exact(frame[0] as usize)
                .rev()
                .flatten()
                .copied()
                .collect();
            if case["depth"] == "black" {
                assert_eq!(expected, prior);
            }
            if case["depth"] == "white" {
                let background_name = if background == [0; 4] {
                    "clear"
                } else if background == [255; 4] {
                    "white"
                } else {
                    "purple"
                };
                let counterpart = format!(
                    "{}_{}_{}_{}",
                    case["source"].as_str().unwrap(),
                    case["origin"].as_str().unwrap(),
                    context.pixels_per_unit,
                    background_name
                );
                assert_eq!(
                    case["frame_sha256"],
                    no_depth["cases"][counterpart]["frame_sha256"]
                );
            }
            if case["depth"] == "grid_alpha0" {
                assert_eq!(
                    case["frame_sha256"],
                    fixture["cases"][name.replace("grid_alpha0", "grid")]["frame_sha256"]
                );
            }
            let errors: Vec<_> = actual
                .iter()
                .zip(&expected)
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .map(|(i, (a, b))| (i % frame[0] as usize, i / frame[0] as usize, *a, *b))
                .collect();
            if !errors.is_empty() {
                failures.push((
                    name,
                    errors.len(),
                    errors.into_iter().take(2).collect::<Vec<_>>(),
                ));
            }
        }
        assert!(failures.is_empty(), "{failures:?}");
    }

    #[test]
    fn varying_records_match_native_rgba_over_verified_priors() {
        assert_eq!(
            POINT,
            include_bytes!("../../../../../conformance/colored-pencil-point.r8").as_slice()
        );
        let native: Native = serde_json::from_str(include_str!(
            "../../../../../conformance/colored-pencil-v4-canvas-native.json"
        ))
        .unwrap();
        assert_eq!(native.cases.len(), 100);
        let geometry: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../conformance/colored-pencil-native.json"
        ))
        .unwrap();
        let binary = include_bytes!("../../../../../conformance/colored-pencil-native.bin");
        let mut failures = Vec::new();
        for (name, case) in native.cases {
            assert!(case.prior_verified);
            let source = geometry["cases"]
                .as_array()
                .unwrap()
                .iter()
                .find(|s| s["id"] == case.source)
                .unwrap();
            let meta = &source["native_stream"];
            let offset = meta["offset"].as_u64().unwrap() as usize;
            let size = meta["length"].as_u64().unwrap() as usize;
            let bytes = &binary[offset..offset + size];
            assert_eq!(format!("{:x}", Sha256::digest(bytes)), case.stream_sha256);
            let rows: Vec<[f32; 5]> = bytes
                .as_chunks::<20>()
                .0
                .iter()
                .map(|r| {
                    std::array::from_fn(|i| {
                        f32::from_le_bytes(r[i * 4..i * 4 + 4].try_into().unwrap())
                    })
                })
                .collect();
            assert_eq!(rows.len(), case.records);
            let [width, height] = case.frame.map(|n| n as usize);
            let context = InkRasterContext {
                width: case.frame[0],
                height: case.frame[1],
                pixels_per_unit: case.scale,
                origin: if case.origin == "top" {
                    InkRasterOrigin::TopLeft
                } else {
                    InkRasterOrigin::BottomLeft
                },
            };
            let prior = vec![case.background; width * height];
            let plan = prepare(&rows, context, case.argb, 64 * 1024 * 1024).unwrap();
            assert!(plan.work() >= width * height);
            let image = plan.render(&prior).unwrap();
            let mut expected = prior;
            let compressed = base64::engine::general_purpose::STANDARD
                .decode(case.sparse_zlib_base64)
                .unwrap();
            let mut sparse = Vec::new();
            ZlibDecoder::new(compressed.as_slice())
                .take((width * height * 8 + 1) as u64)
                .read_to_end(&mut sparse)
                .unwrap();
            assert_eq!(sparse.len(), case.changed_pixels * 8);
            for pixel in sparse.as_chunks::<8>().0 {
                let index = u32::from_le_bytes(pixel[..4].try_into().unwrap()) as usize;
                assert!(index < width * height);
                let (x, y) = (index % width, index / width);
                expected[(height - 1 - y) * width + x] = pixel[4..8].try_into().unwrap();
            }
            let bottom: Vec<_> = expected
                .chunks_exact(width)
                .rev()
                .flatten()
                .flatten()
                .copied()
                .collect();
            assert_eq!(format!("{:x}", Sha256::digest(bottom)), case.frame_sha256);
            let differences: Vec<_> = image
                .iter()
                .zip(&expected)
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .collect();
            if !differences.is_empty() {
                let sum: u64 = image
                    .iter()
                    .flatten()
                    .zip(expected.iter().flatten())
                    .map(|(&a, &b)| u64::from(a.abs_diff(b)))
                    .sum();
                failures.push((
                    name,
                    differences.len(),
                    sum,
                    differences
                        .iter()
                        .take(2)
                        .map(|(i, (a, b))| (i % width, i / width, **a, **b))
                        .collect::<Vec<_>>(),
                ));
            }
        }
        assert!(
            failures.is_empty(),
            "native raster mismatches: {failures:?}"
        );
    }
}

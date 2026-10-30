use super::*;

fn mask(rows: &[[f32; 6]], projection: Option<[f32; 16]>) -> Vec<[u8; 4]> {
    mask_in_frame(rows, projection, [SIZE, SIZE])
}

fn mask_in_frame(
    rows: &[[f32; 6]],
    projection: Option<[f32; 16]>,
    frame: [usize; 2],
) -> Vec<[u8; 4]> {
    mask_and_trace(rows, projection, frame, 0).0
}

fn mask_and_trace(
    rows: &[[f32; 6]],
    projection: Option<[f32; 16]>,
    frame: [usize; 2],
    trace_index: usize,
) -> (Vec<[u8; 4]>, Vec<[u8; 4]>) {
    let matrix = projection.unwrap_or([
        1. / 128.,
        0.,
        0.,
        0.,
        0.,
        1. / 128.,
        0.,
        0.,
        0.,
        0.,
        1.,
        0.,
        0.,
        0.,
        0.,
        1.,
    ]);
    super::super::raster::prepare(
        rows,
        frame.map(|n| n as u32),
        matrix,
        0x8044aaee,
        usize::MAX,
    )
    .unwrap()
    .mask_with_trace(trace_index)
}

fn composite(mask: &[[u8; 4]], argb: u32) -> Vec<u8> {
    let color = input_color(argb);
    mask.iter()
        .flat_map(|&pixel| {
            if pixel[0] == 0 {
                return [0; 4];
            }
            source_shader_rgba(pixel[1], color, pixel[0]).1
        })
        .collect()
}

#[test]
fn native_composite_kernel_matches_every_mask_red_green_pair() {
    use std::io::Read;
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../../conformance/watercolor-composite-rg-native.json"
    ))
    .unwrap();
    assert_eq!(fixture["cases"].as_object().unwrap().len(), 6);
    for (name, case) in fixture["cases"].as_object().unwrap() {
        assert_eq!(case["pre_composite_prior"]["all_zero"], true);
        assert_eq!(case["composite_input_prior"]["verified_readback"], true);
        let background: [u8; 4] =
            std::array::from_fn(|c| case["background"][c].as_u64().unwrap() as u8);
        let packed = STANDARD
            .decode(case["rgba_zlib_base64"].as_str().unwrap())
            .unwrap();
        let mut native = Vec::new();
        flate2::read::ZlibDecoder::new(packed.as_slice())
            .read_to_end(&mut native)
            .unwrap();
        assert_eq!(native.len(), SIZE * SIZE * 4);
        assert_eq!(sha256(&native), case["sha256"].as_str().unwrap());
        let color = input_color(case["argb"].as_u64().unwrap() as u32);
        let mut actual = Vec::new();
        for g in 0..=255u8 {
            for r in 0..=255u8 {
                let source = if r == 0 {
                    [0; 4]
                } else {
                    source_shader_rgba(g, color, r).1
                };
                actual.extend_from_slice(&crate::ink::host_rgba8::over(background, source));
            }
        }
        let count = actual.iter().zip(&native).filter(|(a, b)| a != b).count();
        let max = actual
            .iter()
            .zip(&native)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap();
        eprintln!("{name}: {count} differing channels, max {max}");
        assert_eq!(count, 0, "{name}");
    }
}

fn apply_sparse(value: &Value, index_bytes: usize, pixels: &mut [u8]) {
    let hex = value.as_str().unwrap().as_bytes();
    let length = (index_bytes + 4) * 2;
    assert!(hex.len().is_multiple_of(length));
    let byte = |pair: &[u8]| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap();
    for record in hex.chunks_exact(length) {
        let index = (0..index_bytes).fold(0usize, |index, c| {
            index | (usize::from(byte(&record[c * 2..c * 2 + 2])) << (c * 8))
        });
        for c in 0..4 {
            pixels[index * 4 + c] = byte(&record[(index_bytes + c) * 2..(index_bytes + c + 1) * 2]);
        }
    }
}

fn expected_sparse(value: &Value) -> Vec<u8> {
    let mut pixels = vec![0; SIZE * SIZE * 4];
    apply_sparse(value, 2, &mut pixels);
    pixels
}

fn bottom_bytes(top: &[[u8; 4]]) -> Vec<u8> {
    top.as_chunks::<SIZE>()
        .0
        .iter()
        .rev()
        .flatten()
        .flatten()
        .copied()
        .collect()
}

#[test]
fn native_rt_layers_match_density_projection_and_verified_clear_controls() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../../conformance/watercolor-host-raster-native.json"
    ))
    .unwrap();
    assert_eq!(fixture["cases"].as_object().unwrap().len(), 23);
    let (base, _, _, argb) = inputs();
    let dense = dense_repeat32_rows(&base);
    for (name, case) in fixture["cases"].as_object().unwrap() {
        assert_eq!(
            case["verified_output_prior"]["verified_readback"], true,
            "{name}"
        );
        assert_eq!(case["pre_composite_prior"]["all_zero"], true, "{name}");
        assert_eq!(
            case["verified_output_prior"]["attachment"], case["pre_composite_prior"]["attachment"],
            "{name}"
        );
        let repeat = case["repeat"].as_u64().unwrap() as usize;
        let projection = match case["origin"].as_str().unwrap() {
            "center" => None,
            origin => {
                let scale = if origin == "top2" {
                    1.0 / 64.0
                } else {
                    1.0 / 128.0
                };
                let top = origin != "bottom";
                let mut matrix = [0.; 16];
                matrix[0] = scale;
                matrix[5] = if top { -scale } else { scale };
                matrix[10] = 1.;
                matrix[12] = -1.;
                matrix[13] = if top { 1. } else { -1. };
                matrix[15] = 1.;
                Some(matrix)
            }
        };
        let actual_mask = mask(&dense[..base.len() * repeat], projection);
        let bottom_mask = bottom_bytes(&actual_mask);
        assert_eq!(
            sha256(&bottom_mask),
            case["mask_sha256"].as_str().unwrap(),
            "{name}: mask hash"
        );
        assert_eq!(
            bottom_mask,
            expected_sparse(&case["mask_sparse_hex"]),
            "{name}: mask pixels"
        );
        let final_top = composite(&actual_mask, argb);
        let final_pixels: Vec<[u8; 4]> = final_top.as_chunks::<4>().0.to_vec();
        let final_bottom = bottom_bytes(&final_pixels);
        assert_eq!(
            sha256(&final_bottom),
            case["final_sha256"].as_str().unwrap(),
            "{name}: final hash"
        );
        assert_eq!(
            final_bottom,
            expected_sparse(&case["final_sparse_hex"]),
            "{name}: final pixels"
        );
    }
}

#[test]
fn redraw_state_controls_clear_and_reused_mask_accumulation() {
    use std::io::Read;
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../../conformance/watercolor-redraw-state-native.json"
    ))
    .unwrap();
    assert_eq!(fixture["cases"].as_object().unwrap().len(), 8);
    let (base, _, _, argb) = inputs();
    let dense = dense_repeat32_rows(&base);
    for (name, case) in fixture["cases"].as_object().unwrap() {
        let source = &fixture["priors"][case["prior"].as_str().unwrap()];
        let packed = STANDARD
            .decode(source["rgba_zlib_base64"].as_str().unwrap())
            .unwrap();
        let mut prior = Vec::new();
        flate2::read::ZlibDecoder::new(packed.as_slice())
            .read_to_end(&mut prior)
            .unwrap();
        assert_eq!(prior.len(), SIZE * SIZE * 4);
        assert_eq!(sha256(&prior), source["sha256"].as_str().unwrap());
        let repeat = case["repeat"].as_u64().unwrap() as usize;
        let rows = &dense[..base.len() * repeat];
        let stream: Vec<u8> = rows
            .iter()
            .flatten()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        assert_eq!(
            sha256(&stream),
            case["stream_sha256"].as_str().unwrap(),
            "{name}"
        );
        let one_mask = bottom_bytes(&mask(rows, None));
        let mut accumulated = vec![0u8; one_mask.len()];
        for (index, frame) in case["frames"].as_array().unwrap().iter().enumerate() {
            let redraw = frame["redraw"].as_bool().unwrap();
            assert_eq!(
                frame["native_state_byte"].as_u64().unwrap(),
                u64::from(redraw)
            );
            assert_eq!(
                frame["incoming_composite"]["matches_supplied_prior"], redraw,
                "{name}:{index}"
            );
            assert_eq!(
                frame["incoming_composite"]["all_zero"], !redraw,
                "{name}:{index}"
            );
            assert_eq!(
                frame["verified_prior"]["attachment"],
                frame["incoming_composite"]["attachment"]
            );
            for (dst, src) in accumulated.iter_mut().zip(&one_mask) {
                *dst = dst.saturating_add(*src);
            }
            assert_eq!(
                sha256(&accumulated),
                frame["mask_sha256"].as_str().unwrap(),
                "{name}:{index}: mask"
            );
            assert_eq!(accumulated, expected_sparse(&frame["mask_sparse_hex"]));
            let background = if redraw {
                prior.clone()
            } else {
                vec![0; prior.len()]
            };
            let color = input_color(argb);
            let actual: Vec<u8> = accumulated
                .as_chunks::<4>()
                .0
                .iter()
                .zip(background.as_chunks::<4>().0)
                .flat_map(|(mask, &dst)| {
                    let src = if mask[0] == 0 {
                        [0; 4]
                    } else {
                        source_shader_rgba(mask[1], color, mask[0]).1
                    };
                    crate::ink::host_rgba8::over(dst, src)
                })
                .collect();
            let mut expected = background;
            apply_sparse(&frame["final_sparse_hex"], 2, &mut expected);
            assert_eq!(
                sha256(&actual),
                frame["final_sha256"].as_str().unwrap(),
                "{name}:{index}: final"
            );
            assert_eq!(actual, expected, "{name}:{index}: pixels");
        }
    }
}

#[test]
fn redraw_masks_and_final_pixels_match_non_square_destination_frames() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../../conformance/watercolor-redraw-frames-native.json"
    ))
    .unwrap();
    assert_eq!(fixture["cases"].as_object().unwrap().len(), 6);
    let (base, _, _, argb) = inputs();
    let dense = dense_repeat32_rows(&base);
    for (name, case) in fixture["cases"].as_object().unwrap() {
        let width = case["frame"][0].as_u64().unwrap() as usize;
        let height = case["frame"][1].as_u64().unwrap() as usize;
        let matrix =
            std::array::from_fn(|i| case["projection_matrices"][0][i].as_f64().unwrap() as f32);
        let repeat = case["repeat"].as_u64().unwrap() as usize;
        let mask = mask_in_frame(&dense[..base.len() * repeat], Some(matrix), [width, height]);
        let bottom: Vec<u8> = mask
            .chunks_exact(width)
            .rev()
            .flatten()
            .flatten()
            .copied()
            .collect();
        let mut native_mask = vec![0; bottom.len()];
        apply_sparse(&case["mask_sparse_hex"], 4, &mut native_mask);
        assert_eq!(sha256(&native_mask), case["mask_sha256"].as_str().unwrap());
        let differences: Vec<_> = bottom
            .iter()
            .zip(&native_mask)
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, (&a, &b))| (i / 4 % width, i / 4 / width, i % 4, a, b))
            .collect();
        assert!(
            differences.is_empty(),
            "{name}: mask differences {differences:?}"
        );
        let prior: [u8; 4] = std::array::from_fn(|i| case["background"][i].as_u64().unwrap() as u8);
        let color = input_color(argb);
        let actual: Vec<u8> = bottom
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|mask| {
                let src = if mask[0] == 0 {
                    [0; 4]
                } else {
                    source_shader_rgba(mask[1], color, mask[0]).1
                };
                crate::ink::host_rgba8::over(prior, src)
            })
            .collect();
        let mut expected = prior.repeat(width * height);
        apply_sparse(&case["final_sparse_hex"], 4, &mut expected);
        assert_eq!(
            sha256(&actual),
            case["final_sha256"].as_str().unwrap(),
            "{name}: final"
        );
        assert_eq!(actual, expected, "{name}: final pixels");
    }
}

#[test]
fn native_dense_prefix_pixel_trace() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../../conformance/watercolor-prefix-pixel-native.json"
    ))
    .unwrap();
    let frames: Value = serde_json::from_str(include_str!(
        "../../../../../../conformance/watercolor-redraw-frames-native.json"
    ))
    .unwrap();
    let matrix = std::array::from_fn(|i| {
        frames["cases"]["1920x1080_1_32"]["projection_matrices"][0][i]
            .as_f64()
            .unwrap() as f32
    });
    let (base, _, _, _) = inputs();
    let (_, trace) = mask_and_trace(&dense_repeat32_rows(&base), Some(matrix), [1920, 1080], 10);
    let native: Vec<[u8; 4]> = fixture["prefix_rgba"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| std::array::from_fn(|i| p[i].as_u64().unwrap() as u8))
        .collect();
    let mismatches: Vec<_> = trace
        .iter()
        .zip(&native)
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .collect();
    assert!(mismatches.is_empty(), "prefix mismatches {mismatches:?}");
    assert_eq!(trace, native);
}

#[test]
fn opaque_saved_alpha_matches_native_clipped_rgb() {
    let case: Value = serde_json::from_str(include_str!(
        "../../../../../../conformance/watercolor-redraw-opacity-native.json"
    ))
    .unwrap();
    let matrix =
        std::array::from_fn(|i| case["projection_matrices"][0][i].as_f64().unwrap() as f32);
    let (base, _, _, _) = inputs();
    let mask = mask_in_frame(&dense_repeat32_rows(&base), Some(matrix), [768, 640]);
    let color = input_color(case["argb"].as_u64().unwrap() as u32);
    let prior: [u8; 4] = std::array::from_fn(|c| case["background"][c].as_u64().unwrap() as u8);
    let actual: Vec<u8> = mask
        .as_chunks::<768>()
        .0
        .iter()
        .rev()
        .flatten()
        .flat_map(|mask| {
            let src = if mask[0] == 0 {
                [0; 4]
            } else {
                source_shader_rgba(mask[1], color, mask[0]).1
            };
            crate::ink::host_rgba8::over(prior, src)
        })
        .collect();
    let mut native = prior.repeat(768 * 640);
    apply_sparse(&case["final_sparse_hex"], 4, &mut native);
    assert_eq!(sha256(&native), case["final_sha256"].as_str().unwrap());
    let differences: Vec<_> = actual
        .iter()
        .zip(&native)
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, (&model, &native))| (i / 4 % 768, i / 4 / 768, i % 4, model, native))
        .collect();
    assert!(
        differences.is_empty(),
        "opaque RGBA differences {differences:?}"
    );
}

#[test]
fn clipped_pixel_matches_every_native_precision_prefix() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../../conformance/watercolor-precision-pixel-native.json"
    ))
    .unwrap();
    let frames: Value = serde_json::from_str(include_str!(
        "../../../../../../conformance/watercolor-redraw-frames-native.json"
    ))
    .unwrap();
    let matrix = std::array::from_fn(|i| {
        frames["cases"]["768x640_2_32"]["projection_matrices"][0][i]
            .as_f64()
            .unwrap() as f32
    });
    let (base, _, _, _) = inputs();
    let rows = dense_repeat32_rows(&base);
    let (_, trace) = mask_and_trace(&rows, Some(matrix), [768, 640], 8 * 768 + 44);
    let native: Vec<[u8; 4]> = fixture["prefix_rgba"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| std::array::from_fn(|i| p[i].as_u64().unwrap() as u8))
        .collect();
    let mismatches: Vec<_> = trace
        .iter()
        .zip(&native)
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .collect();
    assert!(
        mismatches.is_empty(),
        "precision prefix mismatches {mismatches:?}"
    );
    assert_eq!(trace, native);
}

#[test]
fn half_vertex_model_matches_all_native_feedback_lanes() {
    use std::io::Read;
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../../conformance/watercolor-vertex-native.json"
    ))
    .unwrap();
    let frames: Value = serde_json::from_str(include_str!(
        "../../../../../../conformance/watercolor-redraw-frames-native.json"
    ))
    .unwrap();
    assert_eq!(fixture["verified_prefix_count"], 480);
    assert_eq!(fixture["native"]["primitive_count"], 960);
    let packed = STANDARD
        .decode(fixture["native"]["rgba_f32_zlib_base64"].as_str().unwrap())
        .unwrap();
    let mut native = Vec::new();
    flate2::read::ZlibDecoder::new(packed.as_slice())
        .read_to_end(&mut native)
        .unwrap();
    assert_eq!(
        sha256(&native),
        fixture["native"]["raw_sha256"].as_str().unwrap()
    );
    assert_eq!(native.len(), 480 * 6 * 7 * 4);
    let matrix = std::array::from_fn(|i| {
        frames["cases"]["768x640_2_32"]["projection_matrices"][0][i]
            .as_f64()
            .unwrap() as f32
    });
    let (base, _, _, _) = inputs();
    let rows = dense_repeat32_rows(&base);
    for (record, row) in rows.iter().enumerate() {
        let vertices = vertices_with_projection(*row, matrix);
        for (vertex, index) in [0, 1, 2, 2, 1, 3].into_iter().enumerate() {
            let v = vertices[index];
            let expected = [
                (v.x as f32 - 128.) / 128.,
                (128. - v.y as f32) / 128.,
                0.,
                1.,
                v.u as f32,
                v.v as f32,
                row[5],
            ];
            for (lane, expected) in expected.into_iter().enumerate() {
                let offset = ((record * 6 + vertex) * 7 + lane) * 4;
                let captured = f32::from_le_bytes(native[offset..offset + 4].try_into().unwrap());
                assert_eq!(
                    captured.to_bits(),
                    expected.to_bits(),
                    "vertex feedback {record}:{vertex}:{lane}"
                );
            }
        }
    }
}

#[test]
fn packaged_pattern_matches_native_rgba_channels() {
    let native = resvg::tiny_skia::Pixmap::decode_png(PATTERN_PNG).unwrap();
    let packaged = include_bytes!("../assets/pattern.rg8");
    assert_eq!(packaged.len(), 256 * 256 * 2);
    for (native, packaged) in native
        .data()
        .as_chunks::<4>()
        .0
        .iter()
        .zip(packaged.as_chunks::<2>().0)
    {
        assert_eq!(&native[..2], packaged);
    }
}
